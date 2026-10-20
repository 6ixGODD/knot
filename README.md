# KNOT

KNOT is a product configuration engine available as a gRPC service or an in-process library. It accepts a versioned JSON product and returns complete state, price, flow and visual descriptions through a C ABI and C++17, Python and Go clients. It does not render, store sessions or authorize payments.

Choose [gRPC service integration](docs/grpc/README.md) ([简体中文](docs/grpc/README.zh-CN.md)) or the [native SDK quickstart](docs/quickstart.md).

For the complete Chinese integration path, read the [system overview](docs/integration-overview.zh-CN.md), [Product DSL guide](docs/dsl-reference.zh-CN.md), then [service setup and calls](docs/grpc/README.zh-CN.md).

The internally supplied native SDK includes precompiled shared/static libraries, CMake targets, schemas, examples, dependency notices and the CLI.

Available internal packages target Windows x86_64 MSVC and Linux x86_64 GNU. C/C++ consumers need a matching compiler and CMake, but no Rust toolchain or dependency downloads. Read `manifest.json` for the exact tested host, compiler, target and CRT; compatibility with older operating systems or other ABI combinations is not established. macOS is outside the validated package scope.

Start with [Quickstart](docs/quickstart.md). See [integration and ownership](docs/integration.md), [C API](docs/c-api.md), [C++ API](docs/cpp-api.md), [DSL](docs/dsl-reference.md), [state machine](docs/state-machine.md), [pricing](docs/pricing.md), [visual IR](docs/visual-ir.md), [limits](docs/limits.md), and [troubleshooting](docs/troubleshooting.md).

Third-party notices, including those for the private JSON dependency, are included in the package.

## gRPC service

Start with the [service integration guide](docs/grpc/README.md) ([简体中文](docs/grpc/README.zh-CN.md)): pull Docker, call the four RPCs from Go/Python/Node.js, then follow a complete Speaker configuration workflow. Service consumers need no native KNOT library or Rust compiler.

- [Docker quickstart](docs/grpc/quickstart.md)
- [API semantics and errors](docs/grpc-api.md)
- [Language clients](docs/grpc/clients.md)
- [Real configuration walkthrough](docs/grpc/speaker.md)
- [Optional SVG frontend](examples/grpc/README.md)

Distribution: [GHCR image](https://github.com/6ixGODD/knot/pkgs/container/knot) and [Release](https://github.com/6ixGODD/knot/releases/tag/v0.1.0). The runtime image and protocol/examples archive serve different purposes; see [delivery contents](docs/service-package.md).
