# Use KNOT as a gRPC service

[简体中文](README.zh-CN.md)

Your application sends answers to KNOT and receives a consistent configuration, available choices, visual properties and a complete-product quote. The server runs in Docker; clients use ordinary gRPC over the network. No KNOT native library, C ABI, cgo or Rust compiler is needed by service consumers.

| Start here | What you will learn |
|---|---|
| [Quickstart](quickstart.md) | Pull the image, mount a product, check readiness and make the first call |
| [API reference](../grpc-api.md) | Inputs, outputs, errors and semantics of all four RPCs |
| [Go, Python and Node.js clients](clients.md) | Install dependencies and call each RPC directly |
| [Speaker configuration walkthrough](speaker.md) | Build a real purchase/edit flow with prices and saved state |
| [Deployment](../deployment.md) | mTLS, resource limits, model lifecycle and shutdown |
| [Optional browser demo](../../examples/grpc/README.md) | Render the configuration using SVG and a local HTTP bridge |

The runtime image contains the server, not your product catalog. A product is a JSON model deployed by the service operator. Each request selects its exact compiled `model_digest`; the application owns user authentication, session storage and checkout. KNOT does not store sessions or orders.

The Release protocol archive is a convenience bundle of `.proto`, JSON schemas, model fixtures and examples. You do not need to download it to pull/run the image or to call a service whose operator has already provided the endpoint, certificates, proto and model digest. The frontend and its Go bridge are optional consumers of the same service.
