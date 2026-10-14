# KNOT

KNOT is an in-process product configuration kernel. It accepts a versioned JSON product and returns complete state, price, flow and visual descriptions through a C ABI and C++17, Python and Go clients. It does not render, store sessions, authorize payments or run a service.

Use a locally supplied candidate SDK; there is no published download URL. The Windows candidate runs the production kernel through shared/static C and C++ clients, Python, Go and the CLI. It includes precompiled libraries, CMake targets, schemas, examples and dependency notices.

Target packages: Windows x86_64 MSVC, Linux x86_64 GNU, macOS Apple Silicon. Windows candidate checks include relocation and both C/C++ linkages without Rust or dependency downloads. Production package certification on Linux/macOS remains pending. Read each candidate's `manifest.json` for its exact build host, compiler, target and CRT; minimum supported OS versions are not yet established.

Start with [Quickstart](docs/quickstart.md). See [integration and ownership](docs/integration.md), [C API](docs/c-api.md), [C++ API](docs/cpp-api.md), [DSL](docs/dsl-reference.md), [state machine](docs/state-machine.md), [pricing](docs/pricing.md), [visual IR](docs/visual-ir.md), [limits](docs/limits.md), and [troubleshooting](docs/troubleshooting.md).

The package must include third-party notices, including the private JSON dependency. KNOT's own distribution license is a release prerequisite; dependency licenses do not grant a KNOT license.
