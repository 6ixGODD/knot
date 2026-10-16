"""Use a dedicated native Docker volume and export only run evidence to the host."""
import argparse
import json
from pathlib import Path
import subprocess
import sys
import uuid

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "verify"))
from common import Runner
ROOT = Path(__file__).resolve().parents[2]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--image", required=True)
    parser.add_argument("--volume", required=True)
    parser.add_argument("--stage", choices=("native", "security"), default="native")
    parser.add_argument("--out", required=True, type=Path)
    args = parser.parse_args()
    run = Runner(args.out)
    (run.out / "evidence.json").write_text('{"passed":false}\n')
    image = json.loads(subprocess.check_output(["docker", "image", "inspect", args.image], encoding="utf-8"))[0]
    if image["Os"] != "linux" or image["Architecture"] != "amd64":
        raise RuntimeError("Linux amd64 toolchain image required")
    run.run("volume", ["docker", "volume", "create", args.volume], timeout=30)
    run_id = uuid.uuid4().hex
    run.run("volume-prepare", ["docker", "run", "--rm", "--network", "none", "--mount", "type=volume,src=" + args.volume + ",dst=/cache", image["Id"], "python3", "-c", "from pathlib import Path; import sys; [Path('/cache', p).mkdir(parents=True, exist_ok=True) for p in ('registry','git','runs/'+sys.argv[1])]", run_id] , timeout=30)
    name = "knot-server-verification-" + uuid.uuid4().hex
    gitdir = subprocess.check_output(["git", "rev-parse", "--path-format=absolute", "--git-common-dir"], cwd=ROOT, encoding="utf-8").strip()
    worktree_git = Path(subprocess.check_output(["git", "rev-parse", "--absolute-git-dir"], cwd=ROOT, encoding="utf-8").strip())
    relative_git = worktree_git.relative_to(Path(gitdir)).as_posix()
    container_git = "/git" if relative_git == "." else "/git/" + relative_git
    command = ["docker", "run", "--name", name, "--init", "--memory", "6g", "--cpus", "2", "--mount", "type=bind,src=" + str(ROOT) + ",dst=/workspace,readonly", "--mount", "type=bind,src=" + gitdir + ",dst=/git,readonly", "--mount", "type=volume,src=" + args.volume + ",dst=/cache", "--mount", "type=volume,src=" + args.volume + ",dst=/usr/local/cargo/registry,volume-subpath=registry", "--mount", "type=volume,src=" + args.volume + ",dst=/usr/local/cargo/git,volume-subpath=git", "--mount", "type=volume,src=" + args.volume + ",dst=/evidence,volume-subpath=runs/" + run_id, "--workdir", "/workspace"]
    for key, value in {"CARGO_TARGET_DIR":"/cache/cargo-target", "CARGO_HOME":"/usr/local/cargo", "CARGO_BUILD_JOBS":"1", "GIT_DIR":container_git, "GIT_WORK_TREE":"/workspace", "PYTHONDONTWRITEBYTECODE":"1", "GIT_CONFIG_COUNT":"1", "GIT_CONFIG_KEY_0":"safe.directory", "GIT_CONFIG_VALUE_0":"/workspace"}.items():
        command += ["--env", key + "=" + value]
    command += [image["Id"], "python3", "tools/server/linux_entry.py", "--out", "/evidence", "--stage", args.stage]
    failure = None
    try:
        run.run("linux", command, timeout=14400)
    except Exception as error:
        failure = error
    try:
        exported = run.out / "linux"
        exported.mkdir(exist_ok=True)
        run.run("export", ["docker", "run", "--rm", "--network", "none", "--mount", "type=volume,src=" + args.volume + ",dst=/result,readonly,volume-subpath=runs/" + run_id, "--mount", "type=bind,src=" + str(exported) + ",dst=/export", image["Id"], "python3", "-c", "from pathlib import Path; import shutil; [(shutil.copytree(p,Path('/export')/p.name,ignore=shutil.ignore_patterns('cargo-target','asan-target','miri-target','go-cache','go-tmp')) if p.is_dir() else shutil.copy2(p,Path('/export')/p.name)) for p in Path('/result').iterdir()]"], timeout=600)
    except Exception:
        if failure is None:
            raise
    finally:
        subprocess.run(["docker", "rm", name], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=30)
    if failure is not None:
        raise failure
    result = json.loads((run.out / "linux/evidence.json").read_text())
    if not result["passed"]:
        raise RuntimeError("Linux verification failed")
    (run.out / "evidence.json").write_text(json.dumps(dict(passed=True, stage=args.stage, imageId=image["Id"], volume=args.volume, runId=run_id, outputs="linux"), indent=2) + "\n")


if __name__ == "__main__":
    main()
