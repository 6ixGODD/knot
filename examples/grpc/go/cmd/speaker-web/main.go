// speaker-web is a loopback-only browser demo backed by the real gRPC service.
package main

import (
	"context"
	"embed"
	"encoding/json"
	"errors"
	"flag"
	"fmt"
	"io"
	"io/fs"
	"knot.local/grpc-example/internal/validation"
	"mime"
	"net"
	"net/http"
	"os"
	"time"

	"google.golang.org/grpc"
	"google.golang.org/grpc/credentials/insecure"
	"google.golang.org/grpc/status"
	knotv1 "knot.local/grpc-example/gen/knot/v1"
)

//go:embed web
var assets embed.FS

type bridge struct {
	client knotv1.ConfigurationServiceClient
	digest string
}

func send(w http.ResponseWriter, code int, value any) {
	w.Header().Set("Content-Type", "application/json; charset=utf-8")
	w.WriteHeader(code)
	_ = json.NewEncoder(w).Encode(value)
}

func (b bridge) call(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		w.Header().Set("Allow", "POST")
		send(w, 405, map[string]string{"transportError": "POST required"})
		return
	}
	mediaType, _, mediaErr := mime.ParseMediaType(r.Header.Get("Content-Type"))
	if mediaErr != nil || mediaType != "application/json" {
		send(w, 415, map[string]string{"transportError": "JSON required"})
		return
	}
	var input struct {
		State json.RawMessage `json:"state"`
		Event json.RawMessage `json:"event"`
		Quote json.RawMessage `json:"quote"`
	}
	r.Body = http.MaxBytesReader(w, r.Body, 768*1024)
	decoder := json.NewDecoder(r.Body)
	decoder.DisallowUnknownFields()
	if err := decoder.Decode(&input); err != nil {
		send(w, 400, map[string]string{"transportError": "Invalid request JSON"})
		return
	}
	if decoder.Decode(new(any)) != io.EOF {
		send(w, 400, map[string]string{"transportError": "Trailing JSON"})
		return
	}
	ctx, cancel := context.WithTimeout(r.Context(), 5*time.Second)
	defer cancel()
	var reply *knotv1.ConfigurationResponse
	var err error
	if r.URL.Path == "/api/quote" {
		reply, err := b.client.Quote(ctx, &knotv1.QuoteRequest{ModelDigest: &b.digest, QuoteJson: input.Quote})
		if err != nil {
			send(w, 502, map[string]string{"transportError": status.Code(err).String()})
			return
		}
		if reply != nil {
			switch outcome := reply.Outcome.(type) {
			case *knotv1.QuoteResponse_QuoteJson:
				if validation.Validate("quote", outcome.QuoteJson) == nil {
					send(w, 200, map[string]any{"quote": json.RawMessage(outcome.QuoteJson)})
					return
				}
			case *knotv1.QuoteResponse_ErrorJson:
				if validation.Validate("error", outcome.ErrorJson) == nil {
					send(w, 200, map[string]any{"error": json.RawMessage(outcome.ErrorJson)})
					return
				}
			}
		}
		send(w, 502, map[string]string{"transportError": "Invalid Quote outcome"})
		return
	}
	switch r.URL.Path {
	case "/api/initial":
		reply, err = b.client.Initial(ctx, &knotv1.InitialRequest{ModelDigest: &b.digest})
	case "/api/apply":
		reply, err = b.client.Apply(ctx, &knotv1.ApplyRequest{ModelDigest: &b.digest, StateJson: input.State, EventJson: input.Event})
	case "/api/evaluate":
		reply, err = b.client.Evaluate(ctx, &knotv1.EvaluateRequest{ModelDigest: &b.digest, StateJson: input.State})
	default:
		http.NotFound(w, r)
		return
	}
	if err != nil {
		send(w, 502, map[string]string{"transportError": status.Code(err).String()})
		return
	}
	if reply != nil {
		switch outcome := reply.Outcome.(type) {
		case *knotv1.ConfigurationResponse_ResultJson:
			if validation.Validate("result", outcome.ResultJson) == nil {
				send(w, 200, map[string]any{"result": json.RawMessage(outcome.ResultJson)})
				return
			}
		case *knotv1.ConfigurationResponse_ErrorJson:
			if validation.Validate("error", outcome.ErrorJson) == nil {
				send(w, 200, map[string]any{"error": json.RawMessage(outcome.ErrorJson)})
				return
			}
		}
	}
	send(w, 502, map[string]string{"transportError": "Invalid gRPC outcome"})
}

func run() error {
	listen := flag.String("listen", "127.0.0.1:8765", "loopback HTTP address")
	endpoint := flag.String("grpc", "127.0.0.1:50051", "loopback plaintext gRPC address")
	digest := flag.String("digest", "sha256:73e5998863973011c590d9883bb0a593f62d77fca5d10480b4f0029153ed1541", "deployed Speaker digest")
	eventsPath := flag.String("events", "", "Speaker event fixture")
	flag.Parse()
	for _, address := range []string{*listen, *endpoint} {
		host, _, err := net.SplitHostPort(address)
		if err != nil || host != "127.0.0.1" {
			return errors.New("this plaintext example requires 127.0.0.1 addresses")
		}
	}
	events, err := os.ReadFile(*eventsPath)
	if err != nil || !json.Valid(events) {
		return errors.New("valid --events file required")
	}
	conn, err := grpc.NewClient(*endpoint, grpc.WithTransportCredentials(insecure.NewCredentials()),
		grpc.WithDefaultCallOptions(grpc.MaxCallRecvMsgSize(4195328)))
	if err != nil {
		return err
	}
	defer conn.Close()
	b := bridge{client: knotv1.NewConfigurationServiceClient(conn), digest: *digest}
	mux := http.NewServeMux()
	mux.HandleFunc("/api/", b.call)
	mux.HandleFunc("GET /api/example", func(w http.ResponseWriter, r *http.Request) {
		send(w, 200, map[string]any{"events": json.RawMessage(events), "digest": *digest})
	})
	web, err := fs.Sub(assets, "web")
	if err != nil {
		return err
	}
	mux.Handle("/", http.FileServer(http.FS(web)))
	handler := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Cache-Control", "no-store")
		w.Header().Set("X-Content-Type-Options", "nosniff")
		w.Header().Set("Content-Security-Policy", "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self'; connect-src 'self'; frame-ancestors 'none'")
		if r.Host != *listen || (r.Header.Get("Origin") != "" && r.Header.Get("Origin") != "http://"+*listen) {
			http.Error(w, "Local same-origin requests only", http.StatusForbidden)
			return
		}
		mux.ServeHTTP(w, r)
	})
	server := &http.Server{Addr: *listen, Handler: handler, ReadHeaderTimeout: 5 * time.Second,
		ReadTimeout: 10 * time.Second, WriteTimeout: 10 * time.Second, IdleTimeout: 30 * time.Second}
	fmt.Println("Speaker configurator: http://" + *listen)
	return server.ListenAndServe()
}

func main() {
	if err := run(); err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
}
