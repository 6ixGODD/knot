# KNOT

KNOT is an in-process product configuration kernel. It accepts a versioned JSON product and returns complete state, price, flow and visual descriptions through a C ABI and C++17, Python and Go clients. It does not render, store sessions, authorize payments or run a service.

Use the internally supplied SDK. It includes precompiled shared/static libraries, CMake targets, schemas, examples, dependency notices and the CLI.

Available internal packages target Windows x86_64 MSVC and Linux x86_64 GNU. C/C++ consumers need a matching compiler and CMake, but no Rust toolchain or dependency downloads. Read `manifest.json` for the exact tested host, compiler, target and CRT; compatibility with older operating systems or other ABI combinations is not established. macOS is outside the validated package scope.

Start with [Quickstart](docs/quickstart.md). See [integration and ownership](docs/integration.md), [C API](docs/c-api.md), [C++ API](docs/cpp-api.md), [DSL](docs/dsl-reference.md), [state machine](docs/state-machine.md), [pricing](docs/pricing.md), [visual IR](docs/visual-ir.md), [limits](docs/limits.md), and [troubleshooting](docs/troubleshooting.md).

Third-party notices, including those for the private JSON dependency, are included in the package.
