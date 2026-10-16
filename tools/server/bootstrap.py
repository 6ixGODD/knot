"""Install the pinned protobuf compiler after archive and version verification."""
import argparse
import hashlib
import json
import platform
import subprocess
import sysconfig
import urllib.request
import zipfile
from pathlib import Path


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--out", required=True, type=Path)
    args = parser.parse_args()
    lock = json.loads(Path(__file__).with_name("toolchain-lock.json").read_text())
    config = lock["protoc"]
    target = "win64" if platform.system() == "Windows" else "linux-x86_64"
    if platform.machine().lower() not in ("amd64", "x86_64") and sysconfig.get_platform() != "win-amd64":
        raise SystemExit("unsupported compiler platform")
    args.out.mkdir(parents=True, exist_ok=True)
    archive = args.out / f"protoc-{config['version']}-{target}.zip"
    if not archive.exists():
        temporary = archive.with_suffix(".download")
        with urllib.request.urlopen(config["baseUrl"] + archive.name, timeout=120) as source, temporary.open("wb") as dest:
            while chunk := source.read(1024 * 1024):
                dest.write(chunk)
        temporary.replace(archive)
    with archive.open("rb") as stream:
        digest = hashlib.file_digest(stream, "sha256").hexdigest()
    if digest != config["archives"][target]:
        raise SystemExit("protoc archive checksum mismatch")
    install = args.out / f"protoc-{config['version']}-{target}"
    with zipfile.ZipFile(archive) as bundle:
        for item in bundle.infolist():
            if not (install / item.filename).resolve().is_relative_to(install.resolve()):
                raise SystemExit("unsafe archive member")
        bundle.extractall(install)
    binary = install / "bin" / ("protoc.exe" if target == "win64" else "protoc")
    if target != "win64":
        binary.chmod(0o755)
    version = subprocess.check_output([str(binary), "--version"], encoding="utf-8").strip()
    if version != "libprotoc " + config["version"]:
        raise SystemExit("protoc version mismatch")
    print(json.dumps({"protoc": str(binary.resolve()), "version": version, "archiveSha256": digest}))


if __name__ == "__main__":
    main()
