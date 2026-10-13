# KNOT

KNOT is an in-process product configuration kernel. It accepts a versioned JSON product and returns complete state, price, flow and visual descriptions through a C ABI and C++17, Python and Go clients. It does not render, store sessions, authorize payments or run a service.

The client components are available in this source tree. A production SDK package has not yet passed integration acceptance. There is no published download URL; use a locally supplied candidate package when integration produces one. Component test libraries are not SDK packages.

Target packages: Windows x86_64 MSVC, Linux x86_64 GNU, macOS Apple Silicon. Tested native baseline systems are Windows Server 2022, Ubuntu 24.04/glibc 2.39 and macOS 15.7.9; earlier systems are not certified. These are native-probe results, not completed SDK package certification.

Start with [Quickstart](docs/quickstart.md). See [integration and ownership](docs/integration.md), [C API](docs/c-api.md), [C++ API](docs/cpp-api.md), [DSL](docs/dsl-reference.md), [state machine](docs/state-machine.md), [pricing](docs/pricing.md), [visual IR](docs/visual-ir.md), [limits](docs/limits.md), and [troubleshooting](docs/troubleshooting.md).

The package must include third-party notices, including the private JSON dependency. KNOT's own distribution license is a release prerequisite; dependency licenses do not grant a KNOT license.
