"""Collect dependency notices from the locked Cargo packages for internal delivery."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[2]


def collect(output):
    output = Path(output)
    output.mkdir(parents=True, exist_ok=False)
    metadata = json.loads(subprocess.check_output(["cargo", "metadata", "--locked", "--format-version", "1"], cwd=ROOT))
    index = json.loads((ROOT / "tools/package/notices/index.json").read_text())
    index.update(json.loads((ROOT / "tools/server/notices/index.json").read_text()))
    records = []
    for package in metadata["packages"]:
        if package["source"] is None:
            continue
        key = package["name"] + "-" + package["version"]
        source = Path(package["manifest_path"]).parent
        notices = []
        for path in sorted(source.rglob("*")):
            if path.is_file() and path.name.lower().startswith(("license", "licence", "copying", "notice", "copyright")):
                relative = Path(key) / path.relative_to(source)
                destination = output / relative
                destination.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(path, destination)
                notices.append(relative.as_posix())
        upstream = None
        if not notices:
            upstream = index.get(key)
            if upstream:
                for notice in upstream["notices"]:
                    path = ROOT / notice["path"]
                    if hashlib.sha256(path.read_bytes()).hexdigest() != notice["sha256"]:
                        raise RuntimeError("changed notice snapshot: " + key)
                    relative = Path(key) / path.name
                    (output / key).mkdir(exist_ok=True)
                    shutil.copyfile(path, output / relative)
                    notices.append(relative.as_posix())
        if not package.get("license") or not notices:
            raise RuntimeError("missing dependency notice: " + key)
        if any(marker in package["license"] for marker in ("MPL-", "LGPL-", "GPL-")):
            shutil.copytree(source, output / key / "source", ignore=shutil.ignore_patterns(".cargo-ok", ".cargo-checksum.json"))
        records.append(dict(name=package["name"], version=package["version"], license=package["license"], source=package["source"], notices=notices, upstreamNotice=upstream))
    (output / "dependencies.json").write_text(json.dumps(records, indent=2) + "\n")


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--out", required=True)
    collect(parser.parse_args().out)
