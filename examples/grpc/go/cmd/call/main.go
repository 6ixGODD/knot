// Command call invokes one RPC against an independently deployed KNOT server.
package main

import (
	"context"
	"crypto/tls"
	"crypto/x509"
	"errors"
	"flag"
	"fmt"
	"os"
	"time"

	"google.golang.org/grpc"
	"google.golang.org/grpc/credentials"
	"google.golang.org/grpc/credentials/insecure"
	pb "knot.local/grpc-example/gen/knot/v1"
	"knot.local/grpc-example/internal/validation"
)

func run() (int, error) {
	if len(os.Args) < 2 {
		return 1, errors.New("method: Initial|Apply|Evaluate|Quote")
	}
	method := os.Args[1]
	f := flag.NewFlagSet("call", flag.ContinueOnError)
	endpoint := f.String("endpoint", "127.0.0.1:50051", "server address (TLS hostname must match certificate)")
	digest := f.String("digest", "", "model digest")
	input := f.String("input", "", "State or QuoteRequest JSON file")
	event := f.String("event", "", "Event JSON file")
	plain := f.Bool("plaintext", false, "isolated local connection")
	ca := f.String("ca", "", "server CA PEM")
	cert := f.String("cert", "", "client certificate PEM")
	key := f.String("key", "", "client private key PEM")
	if err := f.Parse(os.Args[2:]); err != nil {
		return 1, err
	}
	if *digest == "" || f.NArg() != 0 {
		return 1, errors.New("digest required; unexpected positional arguments rejected")
	}
	var transport credentials.TransportCredentials
	if *plain {
		if *ca != "" || *cert != "" || *key != "" {
			return 1, errors.New("plaintext cannot use TLS options")
		}
		transport = insecure.NewCredentials()
	} else {
		pem, err := os.ReadFile(*ca)
		if err != nil {
			return 1, err
		}
		roots := x509.NewCertPool()
		if !roots.AppendCertsFromPEM(pem) {
			return 1, errors.New("invalid CA")
		}
		identity, err := tls.LoadX509KeyPair(*cert, *key)
		if err != nil {
			return 1, err
		}
		transport = credentials.NewTLS(&tls.Config{MinVersion: tls.VersionTLS12, RootCAs: roots, Certificates: []tls.Certificate{identity}})
	}
	conn, err := grpc.NewClient(*endpoint, grpc.WithTransportCredentials(transport), grpc.WithDisableRetry(), grpc.WithDefaultCallOptions(grpc.MaxCallRecvMsgSize(4195328)))
	if err != nil {
		return 1, err
	}
	defer conn.Close()
	client := pb.NewConfigurationServiceClient(conn)
	ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
	defer cancel()
	var raw []byte
	if method != "Initial" {
		raw, err = os.ReadFile(*input)
		if err != nil {
			return 1, err
		}
	}
	var response *pb.ConfigurationResponse
	var payload []byte
	kind := "result"
	domain := false
	switch method {
	case "Initial":
		response, err = client.Initial(ctx, &pb.InitialRequest{ModelDigest: digest})
	case "Apply":
		var action []byte
		action, err = os.ReadFile(*event)
		if err != nil {
			return 1, err
		}
		response, err = client.Apply(ctx, &pb.ApplyRequest{ModelDigest: digest, StateJson: raw, EventJson: action})
	case "Evaluate":
		response, err = client.Evaluate(ctx, &pb.EvaluateRequest{ModelDigest: digest, StateJson: raw})
	case "Quote":
		var quote *pb.QuoteResponse
		quote, err = client.Quote(ctx, &pb.QuoteRequest{ModelDigest: digest, QuoteJson: raw})
		if err == nil {
			switch outcome := quote.Outcome.(type) {
			case *pb.QuoteResponse_QuoteJson:
				payload = outcome.QuoteJson
				kind = "quote"
			case *pb.QuoteResponse_ErrorJson:
				payload = outcome.ErrorJson
				domain = true
			default:
				return 1, errors.New("missing outcome")
			}
		}
	default:
		return 1, errors.New("unknown method")
	}
	if err != nil {
		return 1, err
	}
	if response != nil {
		switch outcome := response.Outcome.(type) {
		case *pb.ConfigurationResponse_ResultJson:
			payload = outcome.ResultJson
		case *pb.ConfigurationResponse_ErrorJson:
			payload = outcome.ErrorJson
			domain = true
		default:
			return 1, errors.New("missing outcome")
		}
	}
	if domain {
		kind = "error"
	}
	if err = validation.Validate(kind, payload); err != nil {
		return 1, err
	}
	if domain {
		fmt.Fprintln(os.Stderr, string(payload))
		return 2, nil
	}
	fmt.Println(string(payload))
	return 0, nil
}

func main() {
	code, err := run()
	if err != nil {
		fmt.Fprintln(os.Stderr, err)
	}
	os.Exit(code)
}
