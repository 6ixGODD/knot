# Binding component evidence

These tests exercise the real C++ codec/RAII bridge and Python/cgo boundary
against a controlled test-only C ABI. `knot_test_boundary_*` is not a kernel,
does not calculate price/flow/visual behavior, and must never enter a SDK package.
All 43 Golden expected JSON documents are decoded as frozen input, not evaluated
as business logic here; error status/code/path/ruleIds are also compared.

Configure this directory with CMake 3.23+ and `HOST_JSON_DIR` pointing to the
digest-checked nlohmann 3.11.3 header fixture used by native P0. The bridge uses
only the single pinned 3.12.0 vendor source. CTest executes two variants of
codec/ownership/error/allocation tests, native layout, eight host/header/configuration
cases, archive linkage/namespace inspection, five intentional pollution build
failures, and CMake public interface inspection. Negative builds must fail
with the exact pollution diagnostic, not an arbitrary compiler failure.

Build Release, then run `ctest --test-dir <build> -C Release --output-on-failure`.
C/C++ product examples are object-compiled only. No test pretends to link them
against an available production kernel. Allocation tests inject into the initial
response copy and an identified owning DTO label copy, then verify exact native
release. They do not promise recovery from global process OOM: the pinned JSON
library's noexcept destructor itself allocates a temporary destruction stack.
Native OOM/abort and unwinding termination remain outside that guarantee.

Run `python tests/bindings/run_native_clients.py --build-dir <build> --config
Release` from the source root. It reads CMake's `fixtures/Release.json` and fails
for missing artifacts. Python tests check actual ctypes layout, copied UTF-8,
close and error-path release counters. Go tests execute actual cgo calls from
the binding module, including copied handles, close and no-JSON failures.
Set `CC` to an available cgo-compatible compiler; the runner enables cgo and
uses explicit native include/link locations. On Windows an LLVM-MinGW compiler
can consume the test DLL's MSVC import library. This is a C ABI check, not C++
ABI compatibility evidence. The runner keeps mutable Go outputs beside build.

Run `python tests/bindings/check_docs.py --contracts contracts --out <output>`
with explicit isolated CARGO_TARGET_DIR and CARGO_BUILD_JOBS=1. This checks
public links, exact source snippets, generated contract tables, and invokes
the real locked Rust Schema validator for each labelled JSON example. An
invalid example is accepted only when the validator returns its JSON error
and exit 1; build failures are failures. The check requires Python 3.11+.

Tested binding language baselines: C++17, Python 3.11, Go 1.26.4. CMake generation
may use any detected Python 3.11+. Final package compiler/ABI/CRT combinations
must be recorded and independently tested. Minimum historical OS support is
not inferred from runner labels.

The mandatory real-kernel handoff in [handoff.json](handoff.json) remains
unverified here. Integration must run it using production libraries, full
Golden cases, three platforms and relocated offline packages. There is no
permission to package mocks or report component tests as SDK release readiness.
