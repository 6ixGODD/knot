# Integrate and persist

The installed CMake configuration is `lib/cmake/Knot/KnotConfig.cmake`. It resolves paths relative to the package, provides `Knot::Shared`, `Knot::Static`, `Knot::CppShared`, `Knot::CppStatic`, and downloads nothing. C++ consumers link one C++ target only; it supplies the matching C kernel and precompiled static bridge. Never link shared and static kernels into one consumer. No JSON headers or macros are public dependencies.

For static consumption change `KNOT_LINKAGE` to `static` in the [example build](../examples/CMakeLists.txt). MSVC import libraries reside in `lib/shared`, static kernel libraries in `lib/static`, bridges in `lib/cpp`, and DLLs in `bin`. The static target transmits `KNOT_STATIC` and native system libraries. The two bridge archives are distinct: a DLL-import bridge cannot replace the static-kernel bridge. Do not rename an import library to select linkage.

Persist only successful full State outputs. Replace full Snapshot outputs for display; do not apply Changes as a database patch or reimplement price calculations. A failed call leaves the caller's State unchanged. `expectedRevision` checks the supplied State; two clients can both have revision 5, so your storage write still needs a compare-and-swap or transaction against revision 5. On conflict, reload storage before deciding whether to retry.

On restart, use `evaluate` with the same compiled model and complete saved State. Do not reconstruct State from a cached Snapshot. A changed modelDigest causes MODEL_MISMATCH; keep the old model or explicitly begin a new configuration. The digest is an identity check, not a signature. Upgrade packages atomically and release every model before unloading its library. A model may serve concurrent independent states, but close/move/destruction must not overlap calls. Python users must synchronize close with calls; Go serializes its handle calls and Close.

Python requires 3.11 or newer and only the standard library. Put the package's `bindings/python` on PYTHONPATH. Pass an absolute native library path explicitly:

```python
from knot import Model, Error
with Model.from_file("speaker.json", library="C:/sdk/knot/bin/knot.dll") as model:
    result = model.initial()
    try:
        result = model.apply(result.state, {
            "schemaVersion": "1.0", "type": "set_answer",
            "questionId": "speaker_type", "value": "active",
            "expectedRevision": result.state["revision"]})
    except Error as error:
        print(error.code, error.path)
```

The [Python smoke](../examples/python/smoke.py) takes library, model and events paths. Close is explicit and idempotent; use a context manager, not garbage collection. Never manually free its buffers.

The [Go binding](../bindings/go/knot.go) exposes `Compile`, `Initial`, `Apply`, `Evaluate`, `Close` and raw owning JSON bytes. Use Go 1.26.4 as frozen in go.mod, enable cgo and provide CGO_CFLAGS for the package include and CGO_LDFLAGS for exactly one native library. On Windows use a cgo-compatible C compiler and the shared C ABI, not the MSVC C++ bridge. The [Go example](../examples/go/smoke.go) runs from the binding module with `go run ../../examples/go/smoke.go <speaker.json> <speaker-events.json>`. Always defer Close after successful Compile. A copied Model shares the same closed state; it is not a new native model.

Internal Windows x64 and Linux x64 packages provide shared/static C/C++ libraries, Python and Go bindings, and the CLI. Packages carry contract schemas, examples, toolchain/target/configuration/CRT metadata, checksums and dependency notices. Select the matching package and ABI configuration from `manifest.json`; macOS is outside the validated package scope.


## Command-line inspection and simulation

The package also includes `bin/knot` (`bin/knot.exe` on Windows). Add that directory to PATH or use the executable's absolute path.

```text
knot validate product.json
knot inspect product.json
knot simulate product.json --events events.json
knot explain product.json --state state.json
```

`validate` compiles the product and returns model identity and diagnostics. `inspect` also returns questions, rule count, flow, pricing and visual definitions. `explain` strictly restores the supplied full State and returns its complete evaluation result. All commands use the same compiler and configuration core as the native API.

`simulate` accepts a JSON array of at most 10,000 events and a file of at most 16 MiB. It writes one JSON object per line with `schemaVersion`, `step` and `result`: step 0 is the initial configuration; later steps contain successful results or the first error. Admission failures before the initial configuration use a step 0 error envelope. Processing stops at that error. Earlier output lines remain valid; the whole event file is not a transaction.

Exit status is 0 for success, 2 for usage or file/output I/O failure, 3 for model/state/event rejection, and 4 for internal failure. Business errors are JSON on stdout; usage and I/O diagnostics use stderr. A broken output pipe is a failure. No command downloads models or dependencies.
