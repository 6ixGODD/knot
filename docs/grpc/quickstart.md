# Start a local gRPC service

[简体中文](quickstart.zh-CN.md) · [Guide index](README.md)

Prerequisites: Docker with Linux amd64 container support and `curl` (Windows: `curl.exe`). The commands below use one line each so shell continuation syntax is unnecessary. Run them in a new working directory.

## 1. Pull the image and obtain one product

```sh
docker pull ghcr.io/6ixgodd/knot:0.1.0
mkdir models
curl -fL https://raw.githubusercontent.com/6ixGODD/knot/v0.1.0/contracts/examples/speaker.json -o models/speaker.json
```

In PowerShell use `curl.exe` in the download command. This downloads only the sample product; no SDK, frontend or protocol archive is needed to start the server. Use the Release's `release.json` image digest instead of the version tag when pinning a deployment.

Mount only product JSON files. Do **not** mount the archive's entire `models/` directory: its manifest and event files are not products. The server compiles all top-level `.json` files before becoming ready; one invalid file makes startup fail.

## 2. Start the server

Linux/macOS shell (the supplied image is Linux amd64; Docker must support that platform):

```sh
docker run --detach --name knot-grpc --read-only --cap-drop ALL --security-opt no-new-privileges --memory 512m --cpus 2 --publish 127.0.0.1:50051:50051 --mount "type=bind,src=$(pwd)/models,dst=/models,readonly" --env KNOT_HEALTH_ENDPOINT=http://127.0.0.1:50051 ghcr.io/6ixgodd/knot:0.1.0 serve --transport plaintext
```

PowerShell:

```powershell
$models = (Resolve-Path ./models).Path
docker run --detach --name knot-grpc --read-only --cap-drop ALL --security-opt no-new-privileges --memory 512m --cpus 2 --publish 127.0.0.1:50051:50051 --mount "type=bind,src=$models,dst=/models,readonly" --env KNOT_HEALTH_ENDPOINT=http://127.0.0.1:50051 ghcr.io/6ixgodd/knot:0.1.0 serve --transport plaintext
```

Plaintext is explicitly enabled for this loopback-only tutorial. Remote deployments use [mTLS](../deployment.md). The server runs as UID 10001; mounted model files must be readable by it.

## 3. Check readiness

```sh
docker exec knot-grpc /usr/local/bin/knot-server healthcheck --endpoint http://127.0.0.1:50051
docker logs knot-grpc
```

The health command must exit 0 and print `SERVING`. Startup compilation can take time; if it is not ready, inspect logs before calling the service. The image has a healthcheck configured for the local HTTP endpoint above. A TCP connection alone does not prove model initialization succeeded.

## 4. Make the first request

The unchanged sample's compiled digest is:

```text
sha256:73e5998863973011c590d9883bb0a593f62d77fca5d10480b4f0029153ed1541
```

It is **not** the SHA-256 of the JSON file. Keep the model unchanged for this tutorial. Custom models require an operator-provided compiled digest; see [model identity](clients.md#model-identity).

Follow the setup for your language in [clients](clients.md), then call `Initial`. For example, from the Go example directory:

```sh
go run ./cmd/call Initial --plaintext --endpoint 127.0.0.1:50051 --digest sha256:73e5998863973011c590d9883bb0a593f62d77fca5d10480b4f0029153ed1541
```

The returned JSON contains `state`, `snapshot`, `changes` and `diagnostics`. Save only the complete `state` as the input for later calls. `Initial` has revision 0; `snapshot` describes the current flow and visual properties. Call `Quote` to obtain the full-product price range; the unfinished `snapshot.price` is not that range.

Continue with the [Speaker walkthrough](speaker.md). To stop and remove this tutorial container:

```sh
docker stop --time 15 knot-grpc
docker rm knot-grpc
```

If startup fails: check Docker uses Linux containers, port 50051 is free, the mount is an absolute existing directory, and it contains only product files. `NOT_FOUND` on an RPC means the requested digest is not loaded; `UNAVAILABLE` usually requires checking the address, readiness, TLS and logs.
