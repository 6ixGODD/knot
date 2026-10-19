"""Export only tracked build inputs; Git exclusions are not a Docker boundary."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[2]
ROOT_FILES = {"Cargo.toml", "Cargo.lock", "rust-toolchain.toml", "Dockerfile", ".dockerignore", "LICENSE"}
PREFIXES = ("crates/", "contracts/schemas/", "contracts/currency/", "contracts/grpc/", "tools/server/", "tools/package/notices/")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--out", required=True, type=Path)
    args = parser.parse_args()
    output = args.out.resolve()
    if output.exists():
        raise SystemExit("context destination must be new")
    paths = subprocess.check_output(["git", "ls-files", "-z"], cwd=ROOT).decode("utf-8").split("\0")
    selected = []
    for name in paths:
        if not name or not (name in ROOT_FILES or name.startswith(PREFIXES)):
            continue
        parts = Path(name).parts
        if any(part in ("tests", "examples", "target", "__pycache__") for part in parts):
            continue
        if Path(name).suffix in (".pem", ".key", ".zip", ".exe"):
            raise SystemExit("forbidden build input")
        selected.append(name)
    required = ["Dockerfile", "Cargo.lock", "crates/knot-server/Cargo.toml", "contracts/grpc/knot/v1/configuration.proto", "tools/server/toolchain-lock.json"]
    if any(name not in selected for name in required):
        raise SystemExit("required build input is not tracked")
    output.mkdir(parents=True)
    hashes = {}
    for name in selected:
        source = ROOT / name
        if source.is_symlink() or not source.resolve().is_relative_to(ROOT):
            raise SystemExit("redirected input")
        destination = output / name
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, destination)
        hashes[name] = hashlib.sha256(destination.read_bytes()).hexdigest()
    (output / "context-manifest.json").write_text(json.dumps(hashes, indent=2) + "\n")
    print(json.dumps({"files": len(hashes), "context": str(output)}))


if __name__ == "__main__":
    main()
