# Get or build the service

The Linux amd64 service image is published to [GHCR](https://github.com/6ixGODD/knot/pkgs/container/knot). Download the protocol, Go client and speaker frontend from [Release v0.1.0](https://github.com/6ixGODD/knot/releases/tag/v0.1.0). The native C/C++ SDK is a separate delivery.

## Run the released example

Install Docker with Linux containers, Python 3.9+ and Go 1.26.4. Download `knot-grpc-0.1.0.tar.gz` from the Release and run from that download directory:

```powershell
docker login ghcr.io
docker pull ghcr.io/6ixgodd/knot:0.1.0
tar -xzf ./knot-grpc-0.1.0.tar.gz
cd knot-grpc-0.1.0
$out = Join-Path $env:TEMP ('knot-speaker-' + [guid]::NewGuid().ToString('N'))
python examples/grpc/speaker_web.py --image ghcr.io/6ixgodd/knot:0.1.0 --out $out --port 8765
```

Open <http://127.0.0.1:8765>. Ctrl+C stops the demo. First launch downloads Go modules and builds the bridge; no Rust compiler or protoc is needed. See [example interactions](../examples/grpc/README.md).

The repository and GHCR package are private. At `docker login`, supply an authorized GitHub username and a classic PAT with `read:packages` as the password. Never commit the token. Repository access alone does not authenticate the Docker client. The Release's `release.json` records the immutable image digest, release source and original tested binary source; `SHA256SUMS` covers attachments.

## Build from source

From a clean committed checkout, install Docker with Linux containers and Python 3.11+. The script downloads the pinned Rust builder and protoc, compiles the server and model-manifest helper once in the release profile, generates the protocol package, assembles the runtime image, and verifies that the saved image reloads. No host Rust installation is needed.

```powershell
$out = Join-Path $env:TEMP ('knot-package-' + [guid]::NewGuid().ToString('N'))
$cache = Join-Path $env:TEMP 'knot-package-cache'
python tools/server/ci_package.py --out $out --cache $cache
```

On Linux, use `python3 tools/server/ci_package.py --out /tmp/knot-package-001 --cache /tmp/knot-package-cache`. The output directory must be new; keep the cache for subsequent builds. The ready-to-use files are in `delivery/` under the output directory. Follow its `README.md`.

## CI cost boundary

`Service package` runs only on manual dispatch: one Ubuntu x64 job, a 40-minute hard limit, a 30-minute build limit and a five-minute smoke-check limit. It reuses the Cargo registry/build cache and Go cache. No automatic retries, platform matrix, full workspace test suite, fuzzing or Miri are included. A successful run establishes package construction and the recorded short checks, not full cross-platform release certification. Artifacts expire after seven days. The older broad verification workflows are manual-only and are not prerequisites for this job.

`Publish GHCR release` promotes an already successful build in one Ubuntu job capped at ten minutes. It verifies source equivalence, archive checksums and smoke evidence, pushes GHCR tags, pulls back the exact digest, verifies unchanged runtime layers/configuration, then publishes the Release. It does not compile Rust or run the full suite. Actions artifacts are intermediate build inputs; GHCR and Releases are the distribution entry points.
