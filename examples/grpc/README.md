# Speaker examples

[简体中文](README.zh-CN.md) · [gRPC API](../../docs/grpc-api.md)

Run a speaker configurator against the real KNOT gRPC service. The browser example is a frontend integration reference; the scripted example records reproducible purchase and edit scenarios.

## Run the browser example

Use Docker with Linux containers, Python 3.9+ and Go 1.26.4. Download `knot-grpc-0.1.0.tar.gz` from the [v0.1.0 Release](https://github.com/6ixGODD/knot/releases/tag/v0.1.0) and run these commands in the download directory. The GHCR package is private: authenticate using an authorized GitHub username and a classic PAT with `read:packages`. The first launch downloads Go modules; no Rust or protoc is required.

```powershell
docker login ghcr.io
docker pull ghcr.io/6ixgodd/knot:0.1.0
tar -xzf ./knot-grpc-0.1.0.tar.gz
$image = 'ghcr.io/6ixgodd/knot:0.1.0'
cd knot-grpc-0.1.0
$out = Join-Path $env:TEMP ('knot-speaker-web-' + [guid]::NewGuid().ToString('N'))
python examples/grpc/speaker_web.py --image $image --out $out --port 8765
```

Open <http://127.0.0.1:8765>. Use the matching image with Quote support. The launcher builds the Go bridge, starts a read-only container and binds both services to loopback. Press Ctrl+C in the launching terminal to stop them and remove the container. Logs remain in the new output directory; do not reuse that directory for another launch.

## Try these interactions

1. Choose active → aluminum → a panel color → 200W → LED. Finish the last step to see the confirmed complete price.
2. Return to panel color using the step buttons or Previous. Change the color and observe the panel; existing choices remain saved.
3. Return to material and select walnut. The server clears incompatible power/extras answers and hides the LED. Continue with the available choices.
4. Reload the page to restore the saved configuration through Evaluate. Use Restart to begin again.

Before completion, the price covers legal complete configurations, including unanswered required choices. Each option shows the whole-product price after choosing it, not a surcharge. Equal range endpoints may display as one amount; the heading still identifies an unfinished range.

The price region always identifies the last confirmed quote. It stays visible while a request is pending, so it may temporarily refer to the previous selection. Errors appear separately; do not use the retained quote to authorize checkout. Price digits are never animated through invented intermediate amounts.

During continuous color input, one Apply runs at a time and only the latest unsent color is retained. Visual follows successful server responses; Quote follows after pending changes settle or input pauses for 120 ms. Confirmed panel and LED colors have an 80 ms transition, disabled by reduced-motion preferences. Network latency can still delay the preview; the browser's native color picker varies by platform.

## Source map

| File | Responsibility |
|---|---|
| [speaker_web.py](speaker_web.py) | Build, startup, health check and cleanup |
| [main.go](go/cmd/speaker-web/main.go) | Local HTTP-to-gRPC bridge and embedded assets |
| [session.mjs](go/cmd/speaker-web/web/session.mjs) | Serialized Apply, color coalescing, stale Quote rejection |
| [app.js](go/cmd/speaker-web/web/app.js) | Controls, saved State and confirmed SVG properties |
| [style.css](go/cmd/speaker-web/web/style.css) | Layout and motion |
| [web/](go/cmd/speaker-web/web/) | Four SVG layers and HTML |
| [speaker.json](../../contracts/examples/speaker.json) | Product choices, rules and visual targets |

Restart the launcher after editing embedded assets. Keep prices and reconciliation on the server; map returned Visual targets to your own assets. The local bridge is example code, not a public REST API. localStorage stores this browser origin's configuration; a different port has separate storage. There are no accounts, shared sessions or payment handling. Expand Request/response to inspect the latest exchange; it includes full demo answers.

## Troubleshooting and checks

- Port in use: stop the previous launcher or choose another `--port`.
- Output directory exists: supply a new path outside the extracted package/source tree.
- Missing Quote or stale page: use the matching image, restart after source changes, then refresh.
- Restore fails: the saved State is retained. Check the error and deployed model; Restart explicitly discards that saved configuration after successful initialization.
- Build/start failure: inspect `build.log`, `web.log` and terminal output in the output directory.

Run frontend queue regressions from the package root:

```text
node --test examples/grpc/go/cmd/speaker-web/web/app.test.mjs
```

Node is needed only for this test. To run the scripted scenarios, use `speaker_demo.py` with the same `--image` and a different fresh `--out`. See the [scenario table and response files](../../docs/grpc-api.md#scripted-scenarios). Its legacy `snapshot.price` subtotal is distinct from the browser's completion Quote.
