# Native verification

Set `CARGO_TARGET_DIR` to an absolute, isolated output directory and use one Cargo
build job. Run from the source checkout:

```text
python tools/verify/all.py --stage native --out <absolute-output>/native
```

This runs Cargo gates, native probes, binding and documentation checks, builds an
internal candidate, compares the frozen Golden cases through real consumers,
relocates the package, and measures the standard benchmark. `commands.json`
records command lines, exit codes, durations and timeouts. Existing command logs
are retained. Use a fresh output directory for another complete candidate run;
packaging and relocation refuse to overwrite earlier candidates.

On Windows, `KNOT_GO_CC` can select the shared cgo-compatible C compiler without
changing the Rust/native compiler. MSVC path mapping covers both slash spellings
and both cc host/target flag namespaces. Package checks scan binaries as well as
text; the librarian normalizes absolute archive member names. The pinned Rust
compiler's bundled `rust-objcopy` removes any remaining host-bearing debug
sections from extracted assembly objects; code and data remain intact.

Linux security CI uses separate `--stage fuzz` and `--stage memory` jobs with the
pinned nightly, Miri and cargo-fuzz. Separate runners keep their build outputs
within the hosted disk budget and let both gates report failures independently.
They run five native fuzz targets for at least 300 seconds each, actual ABI
buffer code under Miri, and production FFI tests under ASan. `--stage security`
runs both gates sequentially on hosts with enough disk space.
`--stage all` requires both stages; unsupported security tooling is a failure.
A Windows native pass is not a security or cross-platform pass.

## Local Linux containers

Use Docker with Linux amd64 containers. The verification image provides Ubuntu
24.04, Rust 1.97.1, Go 1.26.4 and native build tools. The separate security image
also contains the pinned nightly, Miri and cargo-fuzz. Images contain tools only;
source and Git metadata are mounted read-only. Keep outputs and persistent caches
outside the checkout, and choose a fresh output directory for each invocation.

```text
python tools/verify/linux.py --build --stage native --cache <absolute-cache> --out <absolute-output>/native
python tools/verify/linux.py --build --stage memory --cache <absolute-cache> --out <absolute-output>/memory
python tools/verify/linux.py --stage fuzz --cache <absolute-cache> --out <absolute-output>/fuzz
python tools/verify/linux.py --build --stage consumer --cache <absolute-cache> --package-dir <absolute-output>/native/verify/package --out <absolute-output>/consumer
```

Omit `--build` to reuse an installed image. Run stages serially when sharing a
cache. The consumer container has no Rust toolchain, no source mount and no
network. Image identity, Docker commands, exit codes and test logs remain in the
output directory. `--base-image` can reuse an existing Ubuntu image with Rust;
the default build uses the pinned Ubuntu digest in `linux/Dockerfile`.

On Docker Desktop, add `--cache-volume <dedicated-volume-name>` to keep caches
and build outputs on the Linux filesystem instead of a Windows bind mount.
Use the same volume for serial stages. The runner creates it if necessary and
records the volume name and unique run subdirectory in `docker-evidence.json`.
Logs and deliverables are exported to `--out` even when verification fails;
Cargo, ASan and Miri target directories remain in the volume for diagnosis.
The original host cache is not moved or deleted. `--cache` still identifies the
host cache location for runs without a volume. Source and Git remain read-only.
Do not remove the volume while a run is active or before exporting its evidence.

Native CI disables development/test debug information and incremental output to
fit hosted runners. Debug assertions, overflow checks and Release settings remain
unchanged. Memory CI keeps line tables for sanitizer diagnostics.

Candidates are internal until the product license and dependency notice evidence
satisfy the public-package checks. No script publishes an artifact. Automated
relocation does not replace an independent first-time developer's experiment.
