"""Run the local Linux server and native regression gates inside a toolchain image."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import sys

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "verify"))
from common import ROOT, Runner


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--out", required=True, type=Path)
    args = parser.parse_args()
    run = Runner(args.out)
    (run.out / "evidence.json").write_text('{"passed":false}\n')
    tools = Path(os.environ["CARGO_TARGET_DIR"]).parent / "tools"
    run.run("bootstrap", [sys.executable, "tools/server/bootstrap.py", "--out", tools], timeout=300)
    receipt = json.loads((run.out / "bootstrap.log").read_text())
    run.env["PROTOC"] = receipt["protoc"]
    run.env["CARGO_PROFILE_DEV_DEBUG"] = "0"
    run.env["CARGO_PROFILE_TEST_DEBUG"] = "0"
    run.env["CARGO_INCREMENTAL"] = "0"
    run.env["RUSTDOCFLAGS"] = "-D warnings"
    run.run("native-regression", [sys.executable, "tools/verify/all.py", "--stage", "native", "--out", run.out / "native"], timeout=10800)
    run.run("server-build", ["cargo", "build", "-p", "knot-server", "--bins", "--examples", "--locked"])
    target = Path(run.env["CARGO_TARGET_DIR"])
    go_env = dict(run.env, CGO_ENABLED="0", GOTOOLCHAIN="local", GOMAXPROCS="1", GOFLAGS="-p=1", GOCACHE=str(target.parent / "go-cache"), GOTMPDIR=str(target.parent / "go-tmp"))
    Path(go_env["GOTMPDIR"]).mkdir(exist_ok=True)
    go_client = target.parent / "go-client"
    for label, command in (("go-test", ["go", "test", "./..."]), ("go-vet", ["go", "vet", "./..."]), ("go-build", ["go", "build", "-trimpath", "-o", str(go_client), "."])):
        run.run(label, command, cwd=ROOT / "examples/grpc/go", env=go_env)
    run.run("server-process", [sys.executable, "tools/server/verify.py", "--server", target / "debug/knot-server", "--probe", target / "debug/examples/grpc_probe", "--contracts", "contracts", "--go-client", go_client, "--model-manifest", target / "debug/examples/model_manifest", "--out", run.out / "server"], timeout=300)
    run.run("connections", [sys.executable, "tools/server/verify_connections.py", "--server", target / "debug/knot-server", "--models", run.out / "server/models", "--out", run.out / "connections"], timeout=90)
    (run.out / "evidence.json").write_text(json.dumps(dict(passed=True, platform="linux/amd64", native=True, server=True), indent=2) + "\n")


if __name__ == "__main__":
    main()
