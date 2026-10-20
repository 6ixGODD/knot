# Build a speaker configuration flow

[简体中文](speaker.zh-CN.md) · [Guide index](README.md)

Scenario: a customer configures an active speaker, chooses its cabinet/color/power and LED option, then changes the cabinet. The backend must quote a complete product before all answers exist, remove incompatible choices after edits and restore saved configurations correctly.

Use the unchanged [Speaker model](../../contracts/examples/speaker.json), the [local service](quickstart.md), and one [language client](clients.md). The frontend is optional. The same flow works in a shopping application, a sales tool or a backend integration.

## Run the complete workflow

From the repository root after client setup, choose one command. Use a new output directory each time. Commands work in Bash and PowerShell (`python3` may replace `python` on Unix).

```sh
python examples/grpc/walkthrough.py --out ../speaker-python-run -- python examples/grpc/python/client.py
python examples/grpc/walkthrough.py --out ../speaker-node-run -- node examples/grpc/node/client.cjs
```

The driver uses localhost:50051 by default; add `--endpoint host:port` before `--` to change it. It is explicitly a local plaintext tutorial. It invokes the chosen client as a subprocess and retains each input file, full stdout/stderr and exit code. It checks 20 calls covering the four methods, successful pricing, domain errors and an unknown model transport error.

For Go, build from the repository root, then use the executable instead of `go run` so domain exit code 2 is preserved:

```sh
go -C examples/grpc/go build -o ../../../knot-call ./cmd/call
python examples/grpc/walkthrough.py --out ../speaker-go-binary-run -- ./knot-call
```

On Windows use `-o ../../../knot-call.exe` and `-- ./knot-call.exe`.

## 1. Start a configuration and show a price range

Call Initial with the sample model digest. Persist its complete State at revision 0. Call Quote with `schemaVersion="1.0"` and that State. The initial price is CNY 1,199–3,027, represented by `minMinor=119900`, `maxMinor=302700`, `currency="CNY"`, `status="range"`.

Amounts are integer minor units, not floating-point currency. This range includes remaining required choices. Do not display the legacy `snapshot.price.totalMinor` as a complete-product quote while configuration is unfinished.

## 2. Submit choices and render confirmed state

Submit `speaker_type=active`, `material=aluminum`, `panel_color="#315A79"`, `power=200w`, then `extras=["led"]`. Each is a `set_answer` Event with the revision from the latest successful State. The driver constructs these requests; inspect its numbered `.event.json` files.

After each Apply, replace the whole State, render the returned `snapshot.visual`, and request Quote for that new State. The completed configuration is CNY 2,828 (`282800` minor units), with Quote status `final` and equal bounds. The LED visual target is visible.

An option's Quote candidate is the **whole-product price after that choice**, not a fixed surcharge. For a multi-select question, a candidate toggles one option in the current set; Apply still receives the entire desired set.

## 3. Go back and edit

To inspect material again, call Quote with `focusQuestionId="material"` and the current State. Navigation itself does not change answers or revision. Submit a new Apply only after the customer changes the answer.

Changing material to walnut clears incompatible power/extras answers. Save the returned State as a whole, update the cabinet asset and hide the LED using Visual. Quote becomes CNY 1,599–2,327 with status `range`; do not keep showing the earlier final price. Trying `power=200w` for this material yields `INVALID_OPTION`; preserve the last successful State.

## 4. Restore, handle concurrency and save

Evaluate the saved State to rebuild the view. It returns unchanged State and revision; then Quote provides current complete-product pricing. A failed Evaluate must not overwrite the saved configuration.

KNOT is stateless: two Apply calls using the same old State can both succeed. Your application must compare-and-swap the database record using the old revision (and its own session/version key). Only acknowledge the edit after that save succeeds. An Event with an old `expectedRevision` against newer State yields `REVISION_CONFLICT`; the driver verifies this separately.

For a UI, serialize Apply requests and reject stale Quote responses by model digest, revision, focus and local request sequence. Keep the last confirmed price visibly labeled while a new request is pending. Do not let an older color response overwrite a newer selection. Quote is not a payment authorization; order creation must verify the intended model/configuration and apply your own checkout policy.

## Add a browser later

The [SVG demo](../../examples/grpc/README.md) adds a Go HTTP bridge because the runtime exposes ordinary gRPC, not REST or gRPC-Web. Its HTTP endpoints belong to that example, not the KNOT service API. Your own backend can use any of the three clients and expose your chosen browser API.
