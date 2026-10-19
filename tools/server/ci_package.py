"""Build a Linux amd64 service/example bundle with one cached Rust compilation."""
import argparse
import gzip
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tarfile
import tomllib

ROOT = Path(__file__).resolve().parents[2]


def run(argv, **kwargs):
    print("+", " ".join(map(str, argv)), flush=True)
    subprocess.run(list(map(str, argv)), check=True, timeout=1800, **kwargs)


def compile_bundle(out):
    run(["git", "config", "--global", "--add", "safe.directory", str(ROOT)])
    run([sys.executable, "tools/server/bootstrap.py", "--out", "/cache/tools"], cwd=ROOT)
    lock = json.loads((ROOT / "tools/server/toolchain-lock.json").read_text())
    os.environ["PROTOC"] = f"/cache/tools/protoc-{lock['protoc']['version']}-linux-x86_64/bin/protoc"
    run(["cargo", "build", "--release", "--locked", "-p", "knot-server",
         "--bin", "knot-server", "--example", "model_manifest"], cwd=ROOT)
    run([sys.executable, "tools/server/package.py", "--release", "--out", out / "protocol"], cwd=ROOT)
    shutil.copyfile("/cache/target/release/knot-server", out / "knot-server")
    (out / "knot-server").chmod(0o755)
    shutil.copyfile("/etc/ssl/certs/ca-certificates.crt", out / "ca-certificates.crt")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", required=True, type=Path, help="new directory outside the checkout")
    parser.add_argument("--cache", type=Path, help="persistent build cache outside the checkout")
    parser.add_argument("--compile", action="store_true", help=argparse.SUPPRESS)
    args = parser.parse_args()
    out = args.out.resolve()
    if args.compile:
        compile_bundle(out)
        return
    if args.cache is None:
        parser.error("--cache is required")
    cache = args.cache.resolve()
    if out.is_relative_to(ROOT) or cache.is_relative_to(ROOT):
        parser.error("output/cache must be outside the checkout")
    if subprocess.check_output(["git", "status", "--porcelain"], cwd=ROOT):
        raise RuntimeError("commit source changes before producing an identified package")
    out.mkdir(parents=True, exist_ok=False)
    cache.mkdir(parents=True, exist_ok=True)
    lock = json.loads((ROOT / "tools/server/toolchain-lock.json").read_text())
    version = tomllib.loads((ROOT / "Cargo.toml").read_text())["workspace"]["package"]["version"]
    commit = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
    work = out / "work"
    work.mkdir()
    run(["docker", "run", "--rm", "--platform", "linux/amd64", "--cpus", "2",
         "--mount", f"type=bind,src={ROOT},dst=/src,readonly",
         "--mount", f"type=bind,src={work},dst=/out",
         "--mount", f"type=bind,src={cache},dst=/cache",
         "--workdir", "/src", "--env", "CARGO_HOME=/cache/cargo",
         "--env", "CARGO_TARGET_DIR=/cache/target", "--env", "CARGO_BUILD_JOBS=2",
         "--env", "CARGO_INCREMENTAL=0", "--env", "PYTHONDONTWRITEBYTECODE=1",
         lock["images"]["builder"], "python3", "tools/server/ci_package.py", "--compile", "--out", "/out"])
    protocol = work / "protocol" / f"knot-grpc-{version}"
    context = out / "runtime-context"
    context.mkdir()
    shutil.copy2(work / "knot-server", context / "knot-server")
    shutil.copy2(work / "ca-certificates.crt", context / "ca-certificates.crt")
    shutil.copytree(protocol / "licenses/third-party", context / "third-party")
    shutil.copytree(ROOT / "contracts/grpc", context / "contracts/grpc")
    (context / "tools/server").mkdir(parents=True)
    shutil.copyfile(ROOT / "tools/server/license-status.json", context / "tools/server/license-status.json")
    # Reuse the actual runtime definition; do not maintain a second container contract.
    dockerfile = (ROOT / "Dockerfile").read_text()
    marker = "FROM --platform=linux/amd64 " + lock["images"]["runtime"]
    if dockerfile.count(marker) != 1:
        raise RuntimeError("runtime Dockerfile boundary changed")
    runtime = marker + dockerfile.split(marker)[1]
    for source, local in [("/knot-server", "knot-server"),
                          ("/etc/ssl/certs/ca-certificates.crt", "ca-certificates.crt"),
                          ("/third-party/", "third-party/")]:
        old = "COPY --from=build " + source + " "
        if runtime.count(old) != 1:
            raise RuntimeError("runtime COPY contract changed")
        runtime = runtime.replace(old, "COPY " + local + " ")
    (context / "Dockerfile").write_text(runtime)
    image = f"knot-internal:{version}-{commit[:12]}"
    run(["docker", "build", "--platform", "linux/amd64", "--build-arg", f"SOURCE_COMMIT={commit}",
         "--build-arg", f"WORKSPACE_VERSION={version}", "--tag", image, context])
    delivery = out / "delivery"
    delivery.mkdir()
    archive = delivery / "knot-runtime.tar.gz"
    raw = work / "knot-runtime.tar"
    run(["docker", "save", "--output", raw, image])
    with raw.open("rb") as source, gzip.open(archive, "wb", compresslevel=1) as target:
        shutil.copyfileobj(source, target)
    run(["docker", "image", "rm", image])
    run(["docker", "load", "--input", archive])
    identity = json.loads(subprocess.check_output(["docker", "image", "inspect", image]))[0]
    if identity["Config"]["Labels"]["org.opencontainers.image.revision"] != commit:
        raise RuntimeError("runtime source identity mismatch")
    shutil.copyfile(work / "protocol" / f"knot-grpc-{version}.tar.gz", delivery / f"knot-grpc-{version}.tar.gz")
    manifest = dict(version=version, sourceCommit=commit, image=image, imageId=identity["Id"],
                    platform="linux/amd64", distribution="internal-candidate",
                    verification="see smoke/ and workflow result; not full release certification")
    (delivery / "release.json").write_text(json.dumps(manifest, indent=2) + "\n")
    (delivery / "README.md").write_text(
        "# KNOT service/example package\n\nDocker Linux containers, Python 3.9+ and Go " + lock["go"] + " are required.\n"
        "No Rust compiler is required to run this package.\n\nFrom this directory, in PowerShell:\n\n```powershell\n"
        "docker load --input ./knot-runtime.tar.gz\n"
        f"tar -xzf ./knot-grpc-{version}.tar.gz\n"
        f"$image = (Get-Content ./release.json | ConvertFrom-Json).image\ncd knot-grpc-{version}\n"
        "$out = Join-Path $env:TEMP ('knot-speaker-' + [guid]::NewGuid().ToString('N'))\n"
        "python examples/grpc/speaker_web.py --image $image --out $out --port 8765\n```\n\n"
        "Open http://127.0.0.1:8765. Press Ctrl+C to stop. See examples/grpc/README.zh-CN.md for Chinese instructions.\n")
    sums = []
    for path in sorted(delivery.iterdir()):
        with path.open("rb") as stream:
            sums.append(hashlib.file_digest(stream, "sha256").hexdigest() + "  " + path.name)
    (delivery / "SHA256SUMS").write_text("\n".join(sums) + "\n")
    print(json.dumps(manifest), flush=True)


if __name__ == "__main__":
    main()
