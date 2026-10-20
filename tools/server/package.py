"""Build a local protocol package; never publish or select a product license."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tarfile
import tomllib

ROOT = Path(__file__).resolve().parents[2]


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--out", required=True, type=Path)
    parser.add_argument("--release", action="store_true", help="reuse the release build profile")
    args = parser.parse_args()
    output = args.out.resolve()
    output.mkdir(parents=True, exist_ok=True)
    version = tomllib.loads((ROOT / "Cargo.toml").read_text())["workspace"]["package"]["version"]
    package = output / ("knot-grpc-" + version)
    package.mkdir(exist_ok=False)
    lock = json.loads((ROOT / "tools/server/toolchain-lock.json").read_text())
    protoc = os.environ["PROTOC"]
    actual = subprocess.check_output([protoc, "--version"], encoding="utf-8").strip()
    if actual != "libprotoc " + lock["protoc"]["version"]:
        raise RuntimeError("protoc version mismatch")
    for source, destination in [("contracts/grpc", "contracts/grpc"), ("contracts/schemas", "contracts/schemas"), ("contracts/examples", "contracts/examples"), ("examples/grpc/go", "examples/grpc/go")]:
        shutil.copytree(ROOT / source, package / destination, ignore=shutil.ignore_patterns("*.exe", "__pycache__"))
    shutil.copyfile(ROOT / "examples/grpc/speaker_demo.py", package / "examples/grpc/speaker_demo.py")
    shutil.copyfile(ROOT / "examples/grpc/speaker_web.py", package / "examples/grpc/speaker_web.py")
    for name in ("walkthrough.py", "python/client.py", "node/client.cjs"):
        destination = package / "examples/grpc" / name
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(ROOT / "examples/grpc" / name, destination)
    shutil.copytree(ROOT / "docs/grpc", package / "docs/grpc")
    for name in ("README.md", "README.zh-CN.md"):
        shutil.copyfile(ROOT / "examples/grpc" / name, package / "examples/grpc" / name)
    from notices import collect
    collect(package / "licenses/third-party")
    for name in ("grpc-api.md", "grpc-api.zh-CN.md", "deployment.md", "service-package.md",
                 "dsl-reference.zh-CN.md", "integration-overview.zh-CN.md"):
        (package / "docs").mkdir(exist_ok=True)
        shutil.copyfile(ROOT / "docs" / name, package / "docs" / name)
    shutil.copyfile(ROOT / "tools/server/toolchain-lock.json", package / "toolchain-lock.json")
    shutil.copyfile(ROOT / "tools/server/license-status.json", package / "licenses/license-status.json")
    (package / "models").mkdir()
    for name in ("speaker.json", "speaker-events.json"):
        shutil.copyfile(ROOT / "contracts/examples" / name, package / "models" / name)
    subprocess.run([protoc, "--proto_path=" + str(ROOT / "contracts/grpc"), "--include_imports", "--include_source_info", "--descriptor_set_out=" + str(package / "contracts/grpc/knot.bin"), str(ROOT / "contracts/grpc/knot/v1/configuration.proto")], check=True)
    # No implicit lookup of another workspace's prebuilt executable.
    profile = ["--release"] if args.release else []
    models = subprocess.check_output(["cargo", "run", *profile, "-q", "-p", "knot-server", "--example", "model_manifest", "--locked", "--", str(package / "models/speaker.json")], cwd=ROOT)
    json.loads(models)
    (package / "models/manifest.json").write_bytes(models)
    config = subprocess.check_output(["cargo", "run", *profile, "-q", "-p", "knot-server", "--locked", "--", "describe-config"], cwd=ROOT)
    if config != (ROOT / "contracts/grpc/server-config.json").read_bytes():
        raise RuntimeError("generated configuration drift")
    commit = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, encoding="utf-8").strip()
    (package / "manifest.json").write_text(json.dumps(dict(workspaceVersion=version, distribution="internal-candidate", sourceCommit=commit, toolchainLockSha256=digest(package / "toolchain-lock.json"), cargoLockSha256=digest(ROOT / "Cargo.lock")), indent=2) + "\n")
    files = sorted(p for p in package.rglob("*") if p.is_file())
    (package / "checksums.json").write_text(json.dumps({p.relative_to(package).as_posix(): digest(p) for p in files}, indent=2) + "\n")
    archive = output / (package.name + ".tar.gz")
    with tarfile.open(archive, "w:gz") as bundle:
        bundle.add(package, arcname=package.name)
    (output / "package.json").write_text(json.dumps(dict(archive=archive.name, sha256=digest(archive), internalCandidate=True), indent=2) + "\n")


if __name__ == "__main__":
    main()
