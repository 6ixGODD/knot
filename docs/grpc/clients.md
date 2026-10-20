# Language clients

[简体中文](clients.zh-CN.md) · [Guide index](README.md)

These clients connect to an already running gRPC server. They do not start Docker or a frontend. Use the current source checkout: the new clients are not in the older v0.1.0 protocol archive. Only the chosen language runtime and gRPC dependencies are required. First [start the server](quickstart.md).

All three accept the same arguments. Success writes complete JSON to stdout (exit 0); a domain error writes its full envelope to stderr (exit 2); transport/protocol errors exit 1. Only a successful result may replace saved State. Clients use ten-second deadlines, a 4 MiB + 1 KiB receive limit and no explicit retries.

## Go

Requires Go 1.26.4. Generated clients are included; no protoc or cgo is needed. From the repository root:

```sh
cd examples/grpc/go
go run ./cmd/call Initial --plaintext --digest sha256:73e5998863973011c590d9883bb0a593f62d77fca5d10480b4f0029153ed1541
```

[Client source](../../examples/grpc/go/cmd/call/main.go) uses `NewConfigurationServiceClient`, calls each generated RPC, switches on the oneof type and validates the JSON Schema.

## Python

Requires Python 3.10+. From the repository root, create a virtual environment:

```sh
python -m venv .venv
```

Linux/macOS: `. .venv/bin/activate`. PowerShell: `.venv/Scripts/Activate.ps1`. Then:

```sh
python -m pip install grpcio==1.84.0 grpcio-tools==1.84.0
python -m grpc_tools.protoc -I contracts/grpc --python_out=examples/grpc/python --grpc_python_out=examples/grpc/python contracts/grpc/knot/v1/configuration.proto
python examples/grpc/python/client.py Initial --plaintext --digest sha256:73e5998863973011c590d9883bb0a593f62d77fca5d10480b4f0029153ed1541
```

[Client source](../../examples/grpc/python/client.py) uses `ConfigurationServiceStub`, passes raw UTF-8 bytes and checks `WhichOneof("outcome")`. Generated Python code is network serialization code, not a KNOT native build.

## Node.js

Requires Node.js 20+. From the repository root:

```sh
npm install --prefix examples/grpc/node @grpc/grpc-js@1.14.6 @grpc/proto-loader@0.8.1
node examples/grpc/node/client.cjs Initial --plaintext --digest sha256:73e5998863973011c590d9883bb0a593f62d77fca5d10480b4f0029153ed1541
```

[Client source](../../examples/grpc/node/client.cjs) loads the proto dynamically, sends JSON as `Buffer`, enables `oneofs: true` and checks `outcome`. This is server-side Node.js, not browser gRPC-Web. Decimal business values remain strings; do not convert them to floating point.

## Call each method

Replace PROGRAM with `go run ./cmd/call` in the Go directory, or `python examples/grpc/python/client.py` / `node examples/grpc/node/client.cjs` from the root. Replace DIGEST with the full digest above. File paths are relative to your current directory.

```text
PROGRAM Initial --plaintext --digest DIGEST
PROGRAM Apply --plaintext --digest DIGEST --input state.json --event event.json
PROGRAM Evaluate --plaintext --digest DIGEST --input state.json
PROGRAM Quote --plaintext --digest DIGEST --input quote.json
```

`state.json` contains only the complete `state` from Initial/Apply, not the entire response. `event.json` contains one Event. `quote.json` contains the version and full state object, plus optional focus/preview fields. The [Speaker walkthrough](speaker.md) and [runnable driver](../../examples/grpc/walkthrough.py) prepare these files and exercise all four methods for any of the clients.

## Remote connections and validation

Remove `--plaintext`; supply `--ca server-ca.pem --cert client.pem --key client-key.pem --endpoint knot.example.com:50051`. The endpoint hostname must match the server certificate SAN. No example disables hostname verification. Obtain a client identity from the operator and keep its private key outside source control.

The compact Python/Node examples check oneof, UTF-8, JSON and version; they are not complete hostile-response validators. Production code must also validate the [JSON Schemas](../../contracts/schemas/README.md) and match response identity/revision to its request before persistence. The Go example uses full Schema validation. gRPC OK alone never establishes domain success.

## Model identity

No Compile, upload or list-model RPC exists. The operator supplies the compiled digest for each deployed product. The fixed tutorial digest applies only to unchanged `speaker.json`. A custom model's publisher can generate its manifest with matching source tooling:

```sh
cargo run --release --locked -p knot-server --example model_manifest -- /absolute/path/product.json
```

This build step belongs to the model publisher, not network clients. The runtime image currently has no standalone manifest command. A JSON file hash is not a compiled digest. Preserve old model versions while saved sessions reference them; do not replace an old State's digest to force a migration.

The generation/loading approach follows the official [Python](https://grpc.io/docs/languages/python/basics/) and [Node](https://grpc.io/docs/languages/node/basics/) tutorials. Other languages, including Java and C#, can generate clients from the same proto and follow the same bytes/oneof/deadline contract. Runnable examples here cover Go, Python and Node.js.
