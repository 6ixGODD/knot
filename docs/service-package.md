# Service delivery contents

For service integration, start with the [gRPC guide](grpc/README.md) ([简体中文](grpc/README.zh-CN.md)).

| Delivery | Purpose | Required by |
|---|---|---|
| `ghcr.io/6ixgodd/knot:0.1.0` | Linux amd64 gRPC runtime | Service operator |
| `knot-grpc-0.1.0.tar.gz` in the Release | Protocol, schemas, model manifest, Go/browser examples | Optional convenience download |
| Your product JSON and its compiled digest | Defines the choices, rules, prices and visuals | Operator deploys JSON; callers use digest |
| Native SDK | In-process C ABI integration | Native consumers only |

The public image requires no GitHub login. It contains no product models or certificates. Follow the [Docker quickstart](grpc/quickstart.md) to run it independently of any frontend. An existing service's users need only endpoint, credentials, proto and model digest from its operator.

The [v0.1.0 Release](https://github.com/6ixGODD/knot/releases/tag/v0.1.0) records the exact image digest and binary source in `release.json`; `SHA256SUMS` covers its attachments. The old archive contains the original Go/browser example, not clients added later in the current source tree. Use the current checkout for the new language tutorials.

To run the optional frontend from that archive, see the [browser guide](../examples/grpc/README.md). Downloading or starting that frontend is not part of ordinary gRPC integration.

## Build from source

From a clean committed checkout, install Docker with Linux containers and Python 3.11+. The script downloads the pinned Rust builder and protoc, compiles the server and model-manifest helper once in the release profile, generates the protocol package, assembles the runtime image, and verifies that the saved image reloads. No host Rust installation is needed.

```powershell
$out = Join-Path $env:TEMP ('knot-package-' + [guid]::NewGuid().ToString('N'))
$cache = Join-Path $env:TEMP 'knot-package-cache'
python tools/server/ci_package.py --out $out --cache $cache
```

On Linux, use `python3 tools/server/ci_package.py --out /tmp/knot-package-001 --cache /tmp/knot-package-cache`. The output directory must be new; keep the cache for subsequent builds. The ready-to-use files are in `delivery/` under the output directory. Follow its `README.md`.
