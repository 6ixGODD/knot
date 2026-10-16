"""Run the local Linux server and native regression gates inside a toolchain image."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import sys

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "verify"))
from common import Runner


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
    run.run("server-process", [sys.executable, "tools/server/verify.py", "--server", target / "debug/knot-server", "--probe", target / "debug/examples/grpc_probe", "--contracts", "contracts", "--out", run.out / "server"], timeout=300)
    (run.out / "evidence.json").write_text(json.dumps(dict(passed=True, platform="linux/amd64", native=True, server=True), indent=2) + "\n")


if __name__ == "__main__":
    main()
