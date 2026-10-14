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

Native CI disables development/test debug information and incremental output to
fit hosted runners. Debug assertions, overflow checks and Release settings remain
unchanged. Memory CI keeps line tables for sanitizer diagnostics.

Candidates are internal until the product license and dependency notice evidence
satisfy the public-package checks. No script publishes an artifact. Automated
relocation does not replace an independent first-time developer's experiment.
