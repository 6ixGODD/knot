"""Run real server processes and the explicitly supplied network corpus probe."""
import argparse
import json
import os
from pathlib import Path
import shutil
import signal
import socket
import subprocess
import sys
import time

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "verify"))
from common import Runner, sha


def main():
    parser = argparse.ArgumentParser()
    for key in ("server", "probe", "contracts", "out"):
        parser.add_argument("--" + key, required=True, type=Path)
    parser.add_argument("--go-client", type=Path)
    parser.add_argument("--model-manifest", type=Path)
    args = parser.parse_args()
    if bool(args.go_client) != bool(args.model_manifest):
        parser.error("go-client and model-manifest must be supplied together")
    args.server, args.probe, args.contracts = (p.resolve() for p in (args.server, args.probe, args.contracts))
    run = Runner(args.out)
    (run.out / "evidence.json").write_text('{"passed":false}\n')
    models = run.out / "models"
    models.mkdir(exist_ok=False)
    index = json.loads((args.contracts / "golden/index.json").read_text())
    for case in index["cases"]:
        if case["modelValidation"] == "valid":
            shutil.copyfile(args.contracts / "golden" / case["id"] / "model.json", models / (case["id"] + ".json"))
    certs = Path(__file__).resolve().parents[2] / "tests/server/certs"
    records = []
    if args.go_client:
        shutil.copyfile(args.contracts / "examples/speaker.json", models / "speaker.json")
        identities = json.loads(subprocess.check_output([str(args.model_manifest.resolve()), str(models / "speaker.json")]))
        speaker_digest = identities[0]["modelDigest"]
    for tls in (False, True):
        mode = "mtls" if tls else "plaintext"
        with socket.socket() as port:
            port.bind(("127.0.0.1", 0))
            address = f"127.0.0.1:{port.getsockname()[1]}"
        endpoint = ("https://" if tls else "http://") + address
        command = [str(args.server), "serve", "--listen", address, "--model-dir", str(models), "--transport", mode, "--max-compute", "16"]
        health = [str(args.server), "healthcheck", "--endpoint", endpoint]
        probe = [str(args.probe), "--endpoint", endpoint, "--contracts", str(args.contracts), "--out", str(run.out / (mode + "-probe"))]
        if tls:
            command += ["--tls-cert", str(certs / "server.pem"), "--tls-key", str(certs / "server-key.pem"), "--client-ca", str(certs / "ca.pem")]
            for target, identity in ((health, "health"), (probe, "client")):
                target += ["--ca", str(certs / "ca.pem"), "--cert", str(certs / (identity + ".pem")), "--key", str(certs / (identity + "-key.pem")), "--server-name", "localhost"]
        started = time.monotonic()
        with (run.out / (mode + "-server.log")).open("wb") as log:
            process = subprocess.Popen(command, stdout=log, stderr=log, env=run.env, creationflags=subprocess.CREATE_NEW_PROCESS_GROUP if os.name == "nt" else 0)
            try:
                deadline = time.monotonic() + 60
                while True:
                    if process.poll() is not None:
                        raise RuntimeError("server exited before readiness")
                    status = subprocess.run(health, stdout=subprocess.PIPE, stderr=subprocess.PIPE, env=run.env, timeout=5)
                    if status.returncode == 0:
                        break
                    if time.monotonic() >= deadline:
                        raise RuntimeError("readiness timeout")
                    time.sleep(0.05)
                run.run(mode + "-health", health, timeout=5)
                run.run(mode + "-golden", probe, timeout=180)
                if args.go_client:
                    client = [str(args.go_client.resolve()), "--endpoint", address, "--digest", speaker_digest, "--events", str(args.contracts / "examples/speaker-events.json")]
                    if tls:
                        client += ["--ca", str(certs / "ca.pem"), "--cert", str(certs / "client.pem"), "--key", str(certs / "client-key.pem"), "--server-name", "localhost"]
                    else:
                        client += ["--plaintext"]
                    run.run(mode + "-go-client", client, timeout=40)
                process.send_signal(signal.CTRL_BREAK_EVENT if os.name == "nt" else signal.SIGTERM)
                code = process.wait(timeout=15)
                records.append(dict(mode=mode, argv=command, exitCode=code, seconds=time.monotonic()-started))
                if code != 0:
                    raise RuntimeError("server failed graceful shutdown")
            finally:
                if process.poll() is None:
                    process.kill()
                    process.wait(timeout=10)
    (run.out / "evidence.json").write_text(json.dumps(dict(passed=True, serverSha256=sha(args.server), probeSha256=sha(args.probe), processes=records), indent=2) + "\n")


if __name__ == "__main__":
    main()
