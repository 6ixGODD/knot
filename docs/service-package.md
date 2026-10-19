# Get or build the service package

The service package contains both `knot-runtime.tar.gz` (a Linux amd64 Docker image) and `knot-grpc-0.1.0.tar.gz` (protocol, Go client and speaker frontend sources). It is separate from the native C/C++ SDK. Repository access is required.

## Download a completed build

Open [Service package runs](https://github.com/6ixGODD/knot/actions/workflows/package.yml), select a **successful** run, and download its `knot-service-linux-amd64-<commit>` artifact. Extract the ZIP into a new directory. `release.json` records the exact source commit and image tag; `SHA256SUMS` covers the delivered files. Do not use artifacts from failed or cancelled runs.

With GitHub CLI, first select the successful run ID from `gh run list --repo 6ixGODD/knot --workflow package.yml --status success`, then run:

```powershell
$run = Read-Host 'Successful Service package run ID'
gh run download $run --repo 6ixGODD/knot --dir ./knot-download
```

Enter the downloaded artifact directory containing `release.json`. With Docker running in Linux-container mode, Python 3.9+ and Go 1.26.4 installed:

```powershell
docker load --input ./knot-runtime.tar.gz
tar -xzf ./knot-grpc-0.1.0.tar.gz
$image = (Get-Content ./release.json | ConvertFrom-Json).image
cd knot-grpc-0.1.0
$out = Join-Path $env:TEMP ('knot-speaker-' + [guid]::NewGuid().ToString('N'))
python examples/grpc/speaker_web.py --image $image --out $out --port 8765
```

Open <http://127.0.0.1:8765>. The first launch downloads Go modules and builds the bridge. No Rust compiler or protoc is needed. Ctrl+C stops the demo. See [example interactions](../examples/grpc/README.md).

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
