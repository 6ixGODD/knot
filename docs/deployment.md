# Deploy the gRPC service

Pull the public Linux amd64 image with `docker pull ghcr.io/6ixgodd/knot:0.1.0`; no login is required. Download matching protocol/examples from the Release. See [download and source-build instructions](service-package.md). The image contains no models or credentials.

Mount a complete model directory read-only. Startup compiles every top-level `.json` model before serving; invalid or empty directories fail without partial readiness. Keep all digests still referenced by client sessions. Change models or certificates by replacing the deployment and restarting.

The server defaults to mTLS. Supply a server certificate/key, a client CA, and a separate clientAuth identity for health probes. Server name verification is required; do not reuse the server private key as a probe identity. Configuration flags, environment names, defaults and limits are generated in [server-config.json](../contracts/grpc/server-config.json). Explicit CLI values override environment values. Empty values and plaintext/TLS option combinations are rejected.

A container deployment needs:

- Read-only mounts at `/models` and `/certs`.
- `--read-only --cap-drop ALL --security-opt no-new-privileges`.
- A loopback host mapping such as `--publish 127.0.0.1:50051:50051`.
- `KNOT_TLS_CERT`, `KNOT_TLS_KEY` and `KNOT_CLIENT_CA` paths under `/certs`.
- `KNOT_HEALTH_ENDPOINT=https://127.0.0.1:50051`, `KNOT_HEALTH_CA`, `KNOT_HEALTH_CERT`, `KNOT_HEALTH_KEY` and `KNOT_HEALTH_SERVER_NAME` for the independent probe identity.

The process runs as UID/GID 10001. Mounted files must be readable by that identity; grants should be limited to the deployment. For isolated local use, set `KNOT_TRANSPORT=plaintext` explicitly, use an HTTP health endpoint and omit all TLS options. Do not expose an unauthenticated listener publicly.

`knot-server healthcheck` prints SERVING and exits 0 only on a healthy authenticated connection. Failure prints NOT_SERVING and exits 1. `knot-server describe-config` prints configuration metadata without reading environment secrets or starting the service.

Use `docker stop --time 15 <container>` with the default 10-second drain budget. SIGTERM stops admission and publishes NOT_SERVING before draining. Exit codes are 0 for normal shutdown, 2 for invalid configuration, 3 for model initialization failure, 4 for internal failure and 5 for drain timeout. A fault takes precedence over a simultaneous timeout. Forced OOM/SIGKILL is not graceful shutdown.

The standard test allocation is 512 MiB and two CPUs for the supplied fixtures; it is not a promise that every maximum-sized model fits in that memory. Logs contain fixed result categories, digests and counts, not raw user answers or certificate material.

Build from source with the pinned Rust toolchain and the verified protoc compiler installed by `tools/server/bootstrap.py --out <tools>`. Set `PROTOC` to that compiler, then use `cargo build -p knot-server --release --locked`. The runtime image does not contain a compiler or download dependencies during startup.
