"""Run real Speaker scenarios against a local container; retain every RPC outcome."""
import argparse
import json
import os
from pathlib import Path
import shutil
import socket
import subprocess
import time
import uuid

ROOT = Path(__file__).resolve().parents[2]
DIGEST = "sha256:73e5998863973011c590d9883bb0a593f62d77fca5d10480b4f0029153ed1541"


def require(condition, message):
    if not condition:
        raise RuntimeError(message)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--image", required=True, help="locally supplied KNOT image tag or ID")
    parser.add_argument("--out", required=True, type=Path, help="new directory for events, client and results")
    args = parser.parse_args()
    out = args.out.resolve()
    require(not out.is_relative_to(ROOT), "output must be outside the source checkout")
    out.mkdir(parents=True, exist_ok=False)
    records = []

    def run(name, argv, *, cwd=ROOT, env=None, expected=0, timeout=180):
        argv = [str(a) for a in argv]
        result = subprocess.run(argv, cwd=cwd, env=env, capture_output=True, timeout=timeout)
        (out / (name + ".stdout")).write_bytes(result.stdout)
        (out / (name + ".stderr")).write_bytes(result.stderr)
        records.append(dict(name=name, argv=argv, exitCode=result.returncode))
        (out / "commands.json").write_text(json.dumps(records, indent=2) + "\n", encoding="utf-8")
        require(result.returncode == expected, f"{name}: exit {result.returncode}; see {out}")
        return result

    image = json.loads(run("image", ["docker", "image", "inspect", args.image]).stdout)[0]
    client = out / ("grpc-example.exe" if os.name == "nt" else "grpc-example")
    env = dict(os.environ, CGO_ENABLED="0", GOTOOLCHAIN="local", GOMAXPROCS="1")
    run("build", ["go", "build", "-p=1", "-mod=readonly", "-trimpath", "-o", client, "."],
        cwd=ROOT / "examples/grpc/go", env=env, timeout=600)
    models = out / "models"
    models.mkdir()
    shutil.copyfile(ROOT / "contracts/examples/speaker.json", models / "speaker.json")
    events = json.loads((ROOT / "contracts/examples/speaker-events.json").read_text(encoding="utf-8"))
    passive = [dict(schemaVersion="1.0", type="set_answer", questionId=q, value=v, expectedRevision=i)
               for i, (q, v) in enumerate([
                   ("speaker_type", "passive"), ("material", "walnut"),
                   ("panel_color", "#315A79"), ("cable", "banana")])]
    disabled = json.loads(json.dumps(events[:4]))
    disabled[1]["value"] = "walnut"
    scenarios = {"premium": events[:5], "change-material": events, "passive": passive,
                 "disabled-power": disabled}
    for name, sequence in scenarios.items():
        (out / (name + ".events.json")).write_text(json.dumps(sequence, indent=2) + "\n", encoding="utf-8")
    with socket.socket() as sock:
        sock.bind(("127.0.0.1", 0))
        port = sock.getsockname()[1]
    container = "knot-speaker-demo-" + uuid.uuid4().hex
    summaries = []
    try:
        run("start", ["docker", "run", "--detach", "--name", container, "--read-only",
            "--cap-drop", "ALL", "--security-opt", "no-new-privileges", "--memory", "512m",
            "--cpus", "2", "--publish", f"127.0.0.1:{port}:50051",
            "--mount", f"type=bind,src={models},dst=/models,readonly", image["Id"],
            "serve", "--transport", "plaintext"])
        deadline = time.monotonic() + 60
        while True:
            health = subprocess.run(["docker", "exec", container, "/usr/local/bin/knot-server",
                "healthcheck", "--endpoint", "http://127.0.0.1:50051"], capture_output=True, timeout=10)
            if health.returncode == 0:
                break
            require(time.monotonic() < deadline, "server did not become healthy")
            time.sleep(0.25)
        for name in scenarios:
            result = run(name, [client, "--plaintext", "--trace", "--endpoint", f"127.0.0.1:{port}",
                "--digest", DIGEST, "--events", out / (name + ".events.json")],
                expected=1 if name == "disabled-power" else 0)
            trace = [json.loads(line) for line in result.stderr.splitlines() if line.startswith(b"{")]
            (out / (name + ".trace.jsonl")).write_text(
                "".join(json.dumps(t, ensure_ascii=False) + "\n" for t in trace), encoding="utf-8")
            require(trace[0]["rpc"] == "Initial", "missing initial response")
            if name == "disabled-power":
                require(trace[-1]["error"]["error"]["code"] == "INVALID_OPTION", "disabled power accepted")
                summaries.append(dict(scenario=name, error="INVALID_OPTION"))
                continue
            reply = json.loads(result.stdout)
            (out / (name + ".result.json")).write_text(json.dumps(reply, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
            require(trace[-1]["error"]["error"]["code"] == "REVISION_CONFLICT", "old revision accepted")
            require(trace[-2]["rpc"] == "Evaluate", "missing Evaluate")
            require(trace[-2]["result"]["state"] == trace[-3]["result"]["state"], "Evaluate changed State")
            price, flow = reply["snapshot"]["price"], reply["snapshot"]["flow"]
            expected = {"premium": (282800, "final", "complete", 5),
                        "change-material": (159900, "estimated", "in_progress", 6),
                        "passive": (119900, "final", "complete", 4)}[name]
            require((price["totalMinor"], price["status"], flow["status"], reply["state"]["revision"]) == expected,
                    name + ": unexpected price, flow or revision")
            answers = reply["state"]["answers"]
            visual = {t["id"]: t["properties"] for t in reply["snapshot"]["visual"]["targets"]}
            require(visual["speaker.led_ring"]["visible"] == (name == "premium"), "wrong LED visibility")
            require(visual["speaker.front_panel"]["color"] == "#315A79", "panel color lost")
            require(visual["speaker.cabinet"]["assetRef"] ==
                    ("cabinet-aluminum" if name == "premium" else "cabinet-walnut"), "wrong cabinet asset")
            if name != "premium":
                require("power" not in answers and "extras" not in answers, "inactive answers retained")
            if name == "change-material":
                require(flow["nextQuestionId"] == "power", "wrong resumed question")
                require(trace[-3]["result"]["changes"]["answerChanges"], "missing Apply changes")
            summaries.append(dict(scenario=name, totalMinor=price["totalMinor"], currency=price["currency"],
                                  priceStatus=price["status"], flow=flow["status"], nextQuestion=flow["nextQuestionId"]))
        run("stop", ["docker", "stop", "--time", "15", container])
        stopped = json.loads(run("stopped", ["docker", "inspect", container]).stdout)[0]["State"]
        require(stopped["ExitCode"] == 0 and not stopped["OOMKilled"], "server shutdown failed")
    finally:
        subprocess.run(["docker", "rm", "--force", container], capture_output=True, timeout=30)
    (out / "summary.json").write_text(json.dumps(dict(imageId=image["Id"], modelDigest=DIGEST,
        passed=True, scenarios=summaries), indent=2) + "\n", encoding="utf-8")
    for row in summaries:
        print(json.dumps(row, ensure_ascii=False))
    print("Full responses and RPC traces: " + str(out))


if __name__ == "__main__":
    main()
