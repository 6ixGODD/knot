"""Promote a verified CI image to public GHCR without compiling Rust again."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tarfile
import tomllib
import time

ROOT = Path(__file__).resolve().parents[2]


def command(*args, **kwargs):
    return subprocess.check_output([str(a) for a in args], cwd=ROOT, timeout=300, **kwargs)


def api(endpoint):
    return json.loads(command("gh", "api", endpoint))


def sha(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", required=True, type=Path)
    args = parser.parse_args()
    out = args.out.resolve()
    out.mkdir(parents=True, exist_ok=False)
    repo = os.environ["GITHUB_REPOSITORY"]
    commit = os.environ["GITHUB_SHA"]
    run_id = os.environ["BUILD_RUN"]
    if not run_id.isdigit():
        raise RuntimeError("build_run must be a numeric run ID")
    version = tomllib.loads((ROOT / "Cargo.toml").read_text())["workspace"]["package"]["version"]
    tag = "v" + version
    if command("git", "rev-parse", tag + "^{commit}").decode().strip() != commit:
        raise RuntimeError("release tag must identify this exact master commit")
    if api("repos/" + repo)["private"]:
        raise RuntimeError("public delivery requires the owner's public repository")
    build = api(f"repos/{repo}/actions/runs/{run_id}")
    if build["conclusion"] != "success" or build["path"] != ".github/workflows/package.yml":
        raise RuntimeError("source must be a successful Service package run")
    source = build["head_sha"]
    # The release may change documentation/publication tooling, never the tested product.
    command("git", "diff", "--exit-code", source, commit, "--", "Cargo.toml", "Cargo.lock",
            "rust-toolchain.toml", "Dockerfile", "crates", "contracts", "examples/grpc/go",
            "examples/grpc/speaker_demo.py", "examples/grpc/speaker_web.py", "tools/server/toolchain-lock.json")
    candidate = out / "candidate"
    command("gh", "run", "download", run_id, "--repo", repo, "--name",
            "knot-service-linux-amd64-" + source, "--dir", candidate)
    for line in (candidate / "SHA256SUMS").read_text().splitlines():
        expected, name = line.split("  ", 1)
        if Path(name).name != name or sha(candidate / name) != expected:
            raise RuntimeError("candidate checksum mismatch")
    receipt = json.loads((candidate / "release.json").read_text())
    smoke = json.loads((candidate / "browser-smoke/result.json").read_text())
    if receipt["sourceCommit"] != source or smoke["sourceCommit"] != source or not smoke["passed"] or smoke["imageId"] != receipt["imageId"]:
        raise RuntimeError("candidate smoke identity mismatch")
    command("docker", "load", "--input", candidate / "knot-runtime.tar.gz")
    original = json.loads(command("docker", "image", "inspect", receipt["image"]))[0]
    if original["Id"] != receipt["imageId"]:
        raise RuntimeError("candidate image ID mismatch")
    image = "ghcr.io/" + repo.lower() + ":" + version
    context = out / "labels"
    context.mkdir()
    (context / "Dockerfile").write_text(
        f"FROM {receipt['image']}\n"
        f"LABEL org.opencontainers.image.source=https://github.com/{repo}\n"
        'LABEL org.opencontainers.image.description="KNOT gRPC configuration service"\n'
        f"LABEL io.knot.release.revision={commit}\n")
    command("docker", "build", "--tag", image, context)
    promoted = json.loads(command("docker", "image", "inspect", image))[0]
    if original["RootFS"] != promoted["RootFS"]:
        raise RuntimeError("promotion changed runtime layers")
    old_config, new_config = dict(original["Config"]), dict(promoted["Config"])
    old_config.pop("Labels", None)
    new_config.pop("Labels", None)
    if old_config != new_config:
        raise RuntimeError("promotion changed runtime configuration")
    # Token travels only over stdin; it is never printed or persisted in a receipt.
    command("docker", "login", "ghcr.io", "--username", repo.split("/")[0], "--password-stdin",
            input=os.environ["GH_TOKEN"].encode())
    print(command("docker", "push", image).decode(), flush=True)
    local = json.loads(command("docker", "image", "inspect", image))[0]
    digest = next(d for d in local["RepoDigests"] if d.startswith("ghcr.io/" + repo.lower() + "@"))
    command("docker", "image", "rm", image)
    command("docker", "image", "rm", receipt["image"])
    print(command("docker", "pull", digest).decode(), flush=True)
    pulled = json.loads(command("docker", "image", "inspect", digest))[0]
    if pulled["Id"] != promoted["Id"] or pulled["RootFS"] != original["RootFS"]:
        raise RuntimeError("registry pull identity mismatch")
    config = command("docker", "run", "--rm", digest, "describe-config")
    if config != (ROOT / "contracts/grpc/server-config.json").read_bytes():
        raise RuntimeError("registry image configuration contract mismatch")
    package = api(f"users/{repo.split('/')[0]}/packages/container/{repo.split('/')[1].lower()}")
    print("GHCR uploaded: " + package["html_url"], flush=True)
    deadline = time.monotonic() + 240
    while package["visibility"] != "public":
        if time.monotonic() > deadline:
            raise RuntimeError("package visibility must be set to public before Release publication")
        print("Waiting for package visibility to become public", flush=True)
        time.sleep(5)
        package = api(f"users/{repo.split('/')[0]}/packages/container/{repo.split('/')[1].lower()}")
    if package.get("repository", {}).get("full_name", "").lower() != repo.lower():
        raise RuntimeError("package must be linked to this repository")
    anonymous = out / "anonymous-docker"
    anonymous.mkdir()
    print(command("docker", "--config", anonymous, "pull", digest).decode(), flush=True)
    release = out / "release"
    release.mkdir()
    with tarfile.open(candidate / f"knot-grpc-{version}.tar.gz") as archive:
        archive.extractall(out / "protocol", filter="data")
    protocol = out / "protocol" / f"knot-grpc-{version}"
    for name, expected in json.loads((protocol / "checksums.json").read_text()).items():
        if sha(protocol / name) != expected:
            raise RuntimeError("protocol checksum mismatch")
    for path in list((ROOT / "docs").glob("*.md")):
        if (protocol / "docs" / path.name).exists():
            shutil.copyfile(path, protocol / "docs" / path.name)
    shutil.copytree(ROOT / "docs/grpc", protocol / "docs/grpc", dirs_exist_ok=True)
    for name in ("README.md", "README.zh-CN.md"):
        shutil.copyfile(ROOT / "examples/grpc" / name, protocol / "examples/grpc" / name)
    manifest = json.loads((protocol / "manifest.json").read_text())
    manifest.update(sourceCommit=commit, binarySourceCommit=source)
    (protocol / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    checks = {p.relative_to(protocol).as_posix(): sha(p) for p in sorted(protocol.rglob("*"))
              if p.is_file() and p.name != "checksums.json"}
    (protocol / "checksums.json").write_text(json.dumps(checks, indent=2) + "\n")
    with tarfile.open(release / f"knot-grpc-{version}.tar.gz", "w:gz") as archive:
        archive.add(protocol, arcname=protocol.name)
    identity = dict(version=version, tag=tag, sourceCommit=commit, binarySourceCommit=source,
                    buildRun=build["html_url"], image=image, imageDigest=digest, imageId=pulled["Id"],
                    packageUrl=package["html_url"], visibility=package["visibility"],
                    runtimeLayersUnchanged=True)
    (release / "release.json").write_text(json.dumps(identity, indent=2) + "\n")
    (release / "validation.json").write_text(json.dumps(dict(buildConclusion=build["conclusion"],
        originalBrowserSmoke=smoke, registryPullVerified=True, anonymousPullVerified=True, runtimeConfigVerified=True), indent=2) + "\n")
    shutil.copyfile(ROOT / "docs/service-package.md", release / "README.md")
    sums = [sha(p) + "  " + p.name for p in sorted(release.iterdir())]
    (release / "SHA256SUMS").write_text("\n".join(sums) + "\n")
    notes = out / "release-notes.md"
    notes.write_text(f"KNOT {version}: gRPC service and speaker configurator.\n\n"
        f"Container package: {package['html_url']}\n\n```sh\ndocker pull {image}\n```\n\n"
        "The container is public; no GitHub account, token or repository membership is required to pull it. "
        "Download the protocol/example archive below and follow examples/grpc/README.md "
        "or README.zh-CN.md. Docker Linux containers, Python 3.9+ and Go 1.26.4 are required.\n\n"
        f"Registry digest: `{digest}`.\n\nRelease source: `{commit}`. "
        f"Runtime compiled and smoke-tested in [CI]({build['html_url']}) from `{source}`; "
        "product source and runtime layers are unchanged. Promotion only adds registry metadata and current docs. "
        "Linux amd64; native SDK remains separate; macOS is not claimed.\n")
    existing = json.loads(command("gh", "release", "view", tag, "--repo", repo, "--json", "isDraft,assets"))
    if not existing["isDraft"]:
        raise RuntimeError("refuse to replace assets of an already published release")
    for asset in existing["assets"]:
        command("gh", "release", "delete-asset", tag, asset["name"], "--repo", repo, "--yes")
    command("gh", "release", "upload", tag, *sorted(release.iterdir()), "--repo", repo)
    uploaded = json.loads(command("gh", "release", "view", tag, "--repo", repo, "--json", "assets"))["assets"]
    if len(uploaded) != len(list(release.iterdir())):
        raise RuntimeError("release asset count mismatch")
    for asset in uploaded:
        if asset["digest"] != "sha256:" + sha(release / asset["name"]):
            raise RuntimeError("release upload checksum mismatch")
    command("gh", "release", "edit", tag, "--repo", repo, "--notes-file", notes,
            "--title", "KNOT " + tag, "--target", commit, "--draft=false", "--latest", "--verify-tag")
    print(json.dumps(identity), flush=True)


if __name__ == "__main__":
    main()
