"""Verify the actual Linux image configuration, filesystem and gRPC behavior."""
import argparse
import json
import os
from pathlib import Path
import shutil
import socket
import subprocess
import sys
import tarfile
import time
import uuid

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "verify"))
from common import Runner


def inspect(reference):
    return json.loads(subprocess.check_output(["docker", "image", "inspect", reference], encoding="utf-8"))[0]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--image", required=True)
    for name in ("probe", "contracts", "out"):
        parser.add_argument("--" + name, required=True, type=Path)
    parser.add_argument("--go-client", required=True, type=Path)
    parser.add_argument("--model-manifest", required=True, type=Path)
    args = parser.parse_args()
    run = Runner(args.out)
    (run.out / "evidence.json").write_text('{"passed":false}\n')
    image = inspect(args.image)
    if image["Os"] != "linux" or image["Architecture"] != "amd64":
        raise RuntimeError("image must be linux/amd64")
    config = image["Config"]
    if config["User"] != "10001:10001" or config["Entrypoint"] != ["/usr/local/bin/knot-server"] or config["StopSignal"] != "SIGTERM":
        raise RuntimeError("invalid runtime configuration")
    if config.get("Volumes"):
        raise RuntimeError("implicit writable volume")
    if config.get("Healthcheck", {}).get("Test") != ["CMD", "/usr/local/bin/knot-server", "healthcheck"]:
        raise RuntimeError("missing native healthcheck")
    models = run.out / "models"
    models.mkdir(exist_ok=False)
    contracts = args.contracts.resolve()
    for case in json.loads((contracts / "golden/index.json").read_text())["cases"]:
        if case["modelValidation"] == "valid":
            shutil.copyfile(contracts / "golden" / case["id"] / "model.json", models / (case["id"] + ".json"))
    certs = Path(__file__).resolve().parents[2] / "tests/server/certs"
    shutil.copyfile(contracts / "examples/speaker.json", models / "speaker.json")
    identities = json.loads(subprocess.check_output([str(args.model_manifest.resolve()), str(models / "speaker.json")]))
    speaker_digest = identities[0]["modelDigest"]
    events = json.loads((contracts / "examples/speaker-events.json").read_text())
    prefix = run.out / "speaker-selected-events.json"
    prefix.write_text(json.dumps(events[:-1]) + "\n")
    records = []
    for tls in (False, True):
        mode = "mtls" if tls else "plaintext"
        name = "knot-verification-" + uuid.uuid4().hex
        with socket.socket() as port:
            port.bind(("127.0.0.1", 0))
            host_port = port.getsockname()[1]
        scheme = "https" if tls else "http"
        command = ["docker", "run", "--detach", "--name", name, "--read-only", "--cap-drop", "ALL", "--security-opt", "no-new-privileges", "--memory", "512m", "--cpus", "2", "--publish", f"127.0.0.1:{host_port}:50051", "--mount", f"type=bind,src={models},dst=/models,readonly", "--mount", f"type=bind,src={certs},dst=/certs,readonly", "--env", f"KNOT_HEALTH_ENDPOINT={scheme}://127.0.0.1:50051"]
        if tls:
            for key, value in {"CA":"/certs/ca.pem", "CERT":"/certs/health.pem", "KEY":"/certs/health-key.pem", "SERVER_NAME":"localhost"}.items():
                command += ["--env", "KNOT_HEALTH_" + key + "=" + value]
        command += [image["Id"], "serve", "--transport", mode, "--max-compute", "16"]
        if tls:
            command += ["--tls-cert", "/certs/server.pem", "--tls-key", "/certs/server-key.pem", "--client-ca", "/certs/ca.pem"]
        try:
            run.run(mode + "-start", command, timeout=30)
            deadline = time.monotonic() + 120
            while True:
                state = json.loads(subprocess.check_output(["docker", "inspect", name], encoding="utf-8"))[0]["State"]
                if not state["Running"]:
                    raise RuntimeError("container exited during startup")
                if state.get("Health", {}).get("Status") == "healthy":
                    break
                if time.monotonic() > deadline:
                    raise RuntimeError("container health timeout")
                time.sleep(1)
            probe = [str(args.probe.resolve()), "--endpoint", f"{scheme}://127.0.0.1:{host_port}", "--contracts", str(contracts), "--out", str(run.out / (mode + "-probe"))]
            if tls:
                probe += ["--ca", str(certs / "ca.pem"), "--cert", str(certs / "client.pem"), "--key", str(certs / "client-key.pem"), "--server-name", "localhost"]
            run.run(mode + "-golden", probe, timeout=180)
            client = [str(args.go_client.resolve()), "--endpoint", f"127.0.0.1:{host_port}", "--digest", speaker_digest]
            if tls:
                client += ["--ca", str(certs / "ca.pem"), "--cert", str(certs / "client.pem"), "--key", str(certs / "client-key.pem"), "--server-name", "localhost"]
            else:
                client += ["--plaintext"]
            for label, path, expected in (("selected", prefix, 282800), ("reverted", contracts / "examples/speaker-events.json", 159900)):
                result = json.loads(run.run(mode + "-go-" + label, client + ["--events", str(path)], timeout=40))
                if result["snapshot"]["price"]["totalMinor"] != expected:
                    raise RuntimeError("Go Speaker price mismatch")
            run.run(mode + "-uid", ["docker", "exec", name, "id", "-u"], timeout=10)
            if (run.out / (mode + "-uid.log")).read_text().strip() != "10001":
                raise RuntimeError("process is not nonroot")
            run.run(mode + "-stop", ["docker", "stop", "--time", "15", name], timeout=20)
            state = json.loads(subprocess.check_output(["docker", "inspect", name], encoding="utf-8"))[0]["State"]
            if state["ExitCode"] != 0 or state["OOMKilled"]:
                raise RuntimeError("container did not drain cleanly")
            run.run(mode + "-logs", ["docker", "logs", name], timeout=10)
            records.append(dict(mode=mode, state=state))
        finally:
            subprocess.run(["docker", "rm", "--force", name], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=30)
    # Scan an unstarted image filesystem, without mounted test certificates/models.
    name = "knot-filesystem-" + uuid.uuid4().hex
    try:
        run.run("filesystem-create", ["docker", "create", "--name", name, image["Id"]], timeout=30)
        archive = run.out / "filesystem.tar"
        run.run("filesystem-export", ["docker", "export", "--output", str(archive), name], timeout=120)
        with tarfile.open(archive) as bundle:
            for item in bundle:
                parts = Path(item.name).parts
                if any(part in (".git", ".mine", "target", "rustup", ".cargo", "node_modules") for part in parts) or item.name.endswith(("AGENTS.md", "REQUIREMENTS.md", "-key.pem")) or "/docs/design/" in item.name or "/docs/plan/" in item.name:
                    raise RuntimeError("forbidden image content: " + item.name)
    finally:
        subprocess.run(["docker", "rm", "--force", name], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=30)
    (run.out / "evidence.json").write_text(json.dumps(dict(passed=True, imageId=image["Id"], rootfs=image["RootFS"], runtimeConfig=config, processes=records), indent=2) + "\n")


if __name__ == "__main__":
    main()
