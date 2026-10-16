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
    parser.add_argument("--stage", choices=("native", "security"), default="native")
    args = parser.parse_args()
    run = Runner(args.out)
    modules = Path(run.env["CARGO_TARGET_DIR"]).parent / "go-mod-cache"
    modules.mkdir(parents=True, exist_ok=True)
    run.env["GOMODCACHE"] = str(modules)
    (run.out / "evidence.json").write_text('{"passed":false}\n')
    tools = Path(os.environ["CARGO_TARGET_DIR"]).parent / "tools"
    run.run("bootstrap", [sys.executable, "tools/server/bootstrap.py", "--out", tools], timeout=300)
    receipt = json.loads((run.out / "bootstrap.log").read_text())
    run.env["PROTOC"] = receipt["protoc"]
    debug = "line-tables-only" if args.stage == "security" else "0"
    run.env["CARGO_PROFILE_DEV_DEBUG"] = debug
    run.env["CARGO_PROFILE_TEST_DEBUG"] = debug
    run.env["CARGO_INCREMENTAL"] = "0"
    run.env["RUSTDOCFLAGS"] = "-D warnings"
    if args.stage == "security":
        for relative, cache in (("fuzz/cargo-target", "fuzz-target"), ("memory/miri-target", "miri-target"), ("memory/asan-target", "asan-target")):
            target = tools.parent / "security" / cache
            target.mkdir(parents=True, exist_ok=True)
            alias = run.out / "verify" / relative
            alias.parent.mkdir(parents=True, exist_ok=True)
            alias.symlink_to(target, target_is_directory=True)
        run.run("security", [sys.executable, "tools/verify/all.py", "--stage", "security", "--out", run.out / "verify"], timeout=14400)
        (run.out / "evidence.json").write_text(json.dumps(dict(passed=True, platform="linux/amd64", security=True)) + "\n")
        return
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
