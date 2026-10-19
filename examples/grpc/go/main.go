// Command grpc-example calls the stateless service without cgo or native libraries.
package main

import (
	"context"
	"crypto/tls"
	"crypto/x509"
	"encoding/json"
	"errors"
	"flag"
	"fmt"
	"knot.local/grpc-example/internal/validation"
	"os"
	"time"

	"google.golang.org/grpc"
	"google.golang.org/grpc/credentials"
	"google.golang.org/grpc/credentials/insecure"
	knotv1 "knot.local/grpc-example/gen/knot/v1"
)

func result(reply *knotv1.ConfigurationResponse) (json.RawMessage, error) {
	if reply == nil {
		return nil, errors.New("missing response")
	}
	switch outcome := reply.Outcome.(type) {
	case *knotv1.ConfigurationResponse_ResultJson:
		if err := validation.Validate("result", outcome.ResultJson); err != nil {
			return nil, err
		}
		if len(outcome.ResultJson) == 0 || !json.Valid(outcome.ResultJson) {
			return nil, errors.New("invalid result JSON")
		}
		var envelope map[string]json.RawMessage
		if json.Unmarshal(outcome.ResultJson, &envelope) != nil || len(envelope) != 5 || string(envelope["schemaVersion"]) != `"1.0"` {
			return nil, errors.New("invalid result envelope")
		}
		for _, name := range []string{"state", "snapshot", "changes"} {
			var object map[string]json.RawMessage
			if json.Unmarshal(envelope[name], &object) != nil || object == nil {
				return nil, errors.New("invalid result object")
			}
		}
		var diagnostics []json.RawMessage
		if json.Unmarshal(envelope["diagnostics"], &diagnostics) != nil || diagnostics == nil {
			return nil, errors.New("invalid diagnostics")
		}
		return outcome.ResultJson, nil
	case *knotv1.ConfigurationResponse_ErrorJson:
		code, err := errorCode(outcome.ErrorJson)
		if err != nil {
			return nil, err
		}
		return nil, fmt.Errorf("domain error: %s", code)
	default:
		return nil, errors.New("missing outcome")
	}
}
func errorCode(raw []byte) (string, error) {
	if err := validation.Validate("error", raw); err != nil {
		return "", err
	}
	invalid := errors.New("invalid error envelope")
	var envelope map[string]json.RawMessage
	if json.Unmarshal(raw, &envelope) != nil || len(envelope) != 2 || string(envelope["schemaVersion"]) != `"1.0"` {
		return "", invalid
	}
	var detail map[string]json.RawMessage
	if json.Unmarshal(envelope["error"], &detail) != nil || len(detail) != 5 {
		return "", invalid
	}
	var code string
	if json.Unmarshal(detail["code"], &code) != nil || code == "" {
		return "", invalid
	}
	for _, name := range []string{"message", "path"} {
		var value *string
		if json.Unmarshal(detail[name], &value) != nil || value == nil || name == "message" && *value == "" {
			return "", invalid
		}
	}
	var rules []string
	var fields map[string]json.RawMessage
	if json.Unmarshal(detail["ruleIds"], &rules) != nil || rules == nil || json.Unmarshal(detail["details"], &fields) != nil || fields == nil {
		return "", invalid
	}
	return code, nil
}

func state(reply *knotv1.ConfigurationResponse) ([]byte, error) {
	raw, err := result(reply)
	if err != nil {
		return nil, err
	}
	var envelope struct {
		State json.RawMessage `json:"state"`
	}
	if err = json.Unmarshal(raw, &envelope); err != nil {
		return nil, err
	}
	if len(envelope.State) == 0 {
		return nil, errors.New("missing State")
	}
	return envelope.State, nil
}
func run() error {
	endpoint := flag.String("endpoint", "127.0.0.1:50051", "gRPC address")
	digest := flag.String("digest", "", "model digest from the protocol package")
	eventsPath := flag.String("events", "", "complete event array")
	trace := flag.Bool("trace", false, "write complete RPC outcomes as JSON Lines to stderr")
	plaintext := flag.Bool("plaintext", false, "explicitly use an isolated plaintext connection")
	caPath := flag.String("ca", "", "server CA file")
	certPath := flag.String("cert", "", "client certificate file")
	keyPath := flag.String("key", "", "client key file")
	serverName := flag.String("server-name", "", "verified server SAN")
	flag.Parse()
	if *digest == "" || *eventsPath == "" || flag.NArg() != 0 {
		return errors.New("digest and events are required")
	}
	var transport credentials.TransportCredentials
	if *plaintext {
		if *caPath != "" || *certPath != "" || *keyPath != "" || *serverName != "" {
			return errors.New("TLS options with plaintext")
		}
		transport = insecure.NewCredentials()
	} else {
		if *serverName == "" {
			return errors.New("server-name required")
		}
		ca, err := os.ReadFile(*caPath)
		if err != nil {
			return errors.New("cannot read CA")
		}
		pool := x509.NewCertPool()
		if !pool.AppendCertsFromPEM(ca) {
			return errors.New("invalid CA")
		}
		identity, err := tls.LoadX509KeyPair(*certPath, *keyPath)
		if err != nil {
			return errors.New("invalid client identity")
		}
		transport = credentials.NewTLS(&tls.Config{MinVersion: tls.VersionTLS12, RootCAs: pool, Certificates: []tls.Certificate{identity}, ServerName: *serverName})
	}
	conn, err := grpc.NewClient(*endpoint, grpc.WithTransportCredentials(transport), grpc.WithDefaultCallOptions(grpc.MaxCallRecvMsgSize(4195328)))
	if err != nil {
		return err
	}
	defer conn.Close()
	client := knotv1.NewConfigurationServiceClient(conn)
	record := func(method string, reply *knotv1.ConfigurationResponse) error {
		if !*trace {
			return nil
		}
		if reply == nil {
			return errors.New("missing response")
		}
		entry := map[string]any{"rpc": method}
		switch outcome := reply.Outcome.(type) {
		case *knotv1.ConfigurationResponse_ResultJson:
			entry["result"] = json.RawMessage(outcome.ResultJson)
		case *knotv1.ConfigurationResponse_ErrorJson:
			entry["error"] = json.RawMessage(outcome.ErrorJson)
		default:
			return errors.New("missing outcome")
		}
		return json.NewEncoder(os.Stderr).Encode(entry)
	}
	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
	defer cancel()
	reply, err := client.Initial(ctx, &knotv1.InitialRequest{ModelDigest: digest})
	if err != nil {
		return err
	}
	if err = record("Initial", reply); err != nil {
		return err
	}
	current, err := state(reply)
	if err != nil {
		return err
	}
	encoded, err := os.ReadFile(*eventsPath)
	if err != nil {
		return err
	}
	var events []json.RawMessage
	if err = json.Unmarshal(encoded, &events); err != nil {
		return err
	}
	for _, event := range events {
		reply, err = client.Apply(ctx, &knotv1.ApplyRequest{ModelDigest: digest, StateJson: current, EventJson: event})
		if err != nil {
			return err
		}
		if err = record("Apply", reply); err != nil {
			return err
		}
		current, err = state(reply)
		if err != nil {
			return err
		}
	}
	reply, err = client.Evaluate(ctx, &knotv1.EvaluateRequest{ModelDigest: digest, StateJson: current})
	if err != nil {
		return err
	}
	if err = record("Evaluate", reply); err != nil {
		return err
	}
	raw, err := result(reply)
	if err != nil {
		return err
	}
	if len(events) > 0 {
		rejected, err := client.Apply(ctx, &knotv1.ApplyRequest{ModelDigest: digest, StateJson: current, EventJson: events[0]})
		if err != nil {
			return err
		}
		failure, ok := rejected.Outcome.(*knotv1.ConfigurationResponse_ErrorJson)
		if !ok {
			return errors.New("expected revision conflict branch")
		}
		code, err := errorCode(failure.ErrorJson)
		if err != nil || code != "REVISION_CONFLICT" {
			return errors.New("unexpected domain error")
		}
		if err = record("Apply(replayed event)", rejected); err != nil {
			return err
		}
	}
	_, err = fmt.Fprintln(os.Stdout, string(raw))
	return err
}
func main() {
	if err := run(); err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
}
