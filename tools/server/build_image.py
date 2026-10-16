"""Build one OCI layout, validate it, then import that exact runtime into Docker."""
import argparse
import json
from pathlib import Path
import subprocess
import sys
import tomllib

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "verify"))
from common import Runner
from oci import inspect_layout
ROOT = Path(__file__).resolve().parents[2]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--out", required=True, type=Path)
    parser.add_argument("--buildx", required=True, type=Path)
    parser.add_argument("--builder", required=True)
    args = parser.parse_args()
    run = Runner(args.out)
    (run.out / "evidence.json").write_text('{"passed":false}\n')
    if subprocess.check_output(["git", "status", "--porcelain"], cwd=ROOT):
        raise RuntimeError("commit the reviewed source before building an image")
    commit = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, encoding="utf-8").strip()
    version = tomllib.loads((ROOT / "Cargo.toml").read_text())["workspace"]["package"]["version"]
    lock = json.loads((ROOT / "tools/server/toolchain-lock.json").read_text())
    buildx = str(args.buildx.resolve())
    actual = subprocess.check_output([buildx, "version"], encoding="utf-8")
    if " v" + lock["buildx"]["version"] + " " not in actual:
        raise RuntimeError("Buildx version mismatch")
    if subprocess.run([buildx, "inspect", args.builder], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL).returncode:
        config = run.out / "buildkit.toml"
        config.write_text('[worker.oci]\n  max-parallelism = 1\n')
        run.run("builder-create", [buildx, "create", "--name", args.builder, "--driver", "docker-container", "--driver-opt", "image=" + lock["images"]["buildkit"], "--buildkitd-config", str(config)], timeout=120)
    context = run.out / "context"
    run.run("context", [sys.executable, "tools/server/build_context.py", "--out", str(context)], timeout=120)
    layout = run.out / "layout"
    tag = "knot-internal:" + version + "-" + commit[:12]
    run.run("oci-build", [buildx, "build", "--builder", args.builder, "--platform", "linux/amd64", "--provenance=mode=max", "--attest", "type=sbom,generator=" + lock["images"]["sbom"], "--build-arg", "SOURCE_COMMIT=" + commit, "--build-arg", "WORKSPACE_VERSION=" + version, "--metadata-file", str(run.out / "build-metadata.json"), "--output", "type=oci,dest=" + str(layout) + ",tar=false,name=" + tag, str(context)], timeout=10800)
    evidence = inspect_layout(layout)
    skopeo_version = run.run("skopeo-version", ["docker", "run", "--rm", lock["images"]["skopeo"], "--version"], timeout=300).strip()
    expected_skopeo = "skopeo version " + lock["skopeo"]["version"] + " commit: " + lock["skopeo"]["commit"]
    if skopeo_version != expected_skopeo:
        raise RuntimeError("Skopeo version mismatch")
    run.run("runtime-import", ["docker", "run", "--rm", "--mount", "type=bind,src=" + str(layout) + ",dst=/layout,readonly", "--volume", "/var/run/docker.sock:/var/run/docker.sock", lock["images"]["skopeo"], "--override-os", "linux", "--override-arch", "amd64", "copy", "oci:/layout", "docker-daemon:" + tag], timeout=300)
    image = json.loads(subprocess.check_output(["docker", "image", "inspect", tag], encoding="utf-8"))[0]
    if image["Id"] != evidence["configDigest"] or image["RootFS"]["Layers"] != evidence["runtimeConfig"]["rootfs"]["diff_ids"]:
        raise RuntimeError("imported runtime identity differs from OCI")
    for key in ("User", "Env", "Entrypoint", "Cmd", "WorkingDir", "StopSignal", "Healthcheck", "Labels"):
        if image["Config"].get(key) != evidence["runtimeConfig"]["config"].get(key):
            raise RuntimeError("imported runtime configuration changed")
    archive = run.out / "knot-runtime.tar"
    run.run("runtime-save", ["docker", "save", "--output", str(archive), tag], timeout=300)
    # Remove only our unique candidate tag. Reload the supplied archive without rebuilding.
    run.run("runtime-untag", ["docker", "image", "rm", tag], timeout=60)
    run.run("runtime-reload", ["docker", "load", "--input", str(archive)], timeout=300)
    reloaded = json.loads(subprocess.check_output(["docker", "image", "inspect", tag], encoding="utf-8"))[0]
    if reloaded["Id"] != image["Id"] or reloaded["RootFS"] != image["RootFS"] or reloaded["Config"] != image["Config"]:
        raise RuntimeError("reloaded archive identity changed")
    from common import sha
    evidence.update(archive=archive.name, archiveSha256=sha(archive), reloadedImageId=reloaded["Id"])
    evidence.update(passed=True, sourceCommit=commit, workspaceVersion=version, image=tag, distribution="internal-candidate", buildxVersion=actual.strip(), skopeoVersion=skopeo_version)
    (run.out / "evidence.json").write_text(json.dumps(evidence, indent=2) + "\n")


if __name__ == "__main__":
    main()
