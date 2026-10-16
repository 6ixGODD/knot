"""Exercise connection admission and initial-header deadlines against a real process."""
import argparse
import json
import os
from pathlib import Path
import signal
import socket
import subprocess
import time


def closed(stream):
    try:
        return stream.recv(1) == b""
    except ConnectionResetError:
        return True


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--server", required=True, type=Path)
    parser.add_argument("--models", required=True, type=Path)
    parser.add_argument("--out", required=True, type=Path)
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=False)
    (args.out / "evidence.json").write_text('{"passed":false}\n')
    certs = Path(__file__).resolve().parents[2] / "tests/server/certs"
    records = []
    for tls in (False, True):
        with socket.socket() as reservation:
            reservation.bind(("127.0.0.1", 0))
            port = reservation.getsockname()[1]
        address = ("127.0.0.1", port)
        mode = "mtls" if tls else "plaintext"
        command = [str(args.server.resolve()), "serve", "--listen", f"127.0.0.1:{port}", "--model-dir", str(args.models.resolve()), "--max-connections", "2", "--transport", mode]
        health = [str(args.server.resolve()), "healthcheck", "--endpoint", ("https" if tls else "http") + f"://127.0.0.1:{port}"]
        if tls:
            command += ["--tls-cert", str(certs / "server.pem"), "--tls-key", str(certs / "server-key.pem"), "--client-ca", str(certs / "ca.pem")]
            health += ["--ca", str(certs / "ca.pem"), "--cert", str(certs / "health.pem"), "--key", str(certs / "health-key.pem"), "--server-name", "localhost"]
        with (args.out / (mode + ".log")).open("wb") as log:
            process = subprocess.Popen(command, stdout=log, stderr=log, creationflags=subprocess.CREATE_NEW_PROCESS_GROUP if os.name == "nt" else 0)
            held = []
            try:
                until = time.monotonic() + 30
                while subprocess.run(health, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=5).returncode:
                    if process.poll() is not None or time.monotonic() > until:
                        raise RuntimeError("service never became healthy")
                    time.sleep(0.05)
                # Let the completed health connection close before occupying both slots.
                time.sleep(0.1)
                started = time.monotonic()
                held = [socket.create_connection(address, timeout=8) for _ in range(2)]
                time.sleep(0.1)
                with socket.create_connection(address, timeout=2) as excess:
                    if not closed(excess):
                        raise RuntimeError("excess connection was not rejected")
                for stream in held:
                    # HTTP/2 may send SETTINGS before the client sends its preface.
                    received = 0
                    while True:
                        try:
                            data = stream.recv(4096)
                        except ConnectionResetError:
                            break
                        if not data:
                            break
                        received += len(data)
                        if received > 65536:
                            raise RuntimeError("unexpected pre-header response size")
                seconds = time.monotonic() - started
                if not 4 <= seconds <= 8:
                    raise RuntimeError("unexpected initial-header deadline")
                subprocess.run(health, check=True, stdout=log, stderr=log, timeout=5)
                process.send_signal(signal.CTRL_BREAK_EVENT if os.name == "nt" else signal.SIGTERM)
                code = process.wait(timeout=15)
                if code != 0:
                    raise RuntimeError("shutdown failed")
                records.append(dict(mode=mode, initialDeadlineSeconds=seconds, exitCode=code))
            finally:
                for stream in held:
                    stream.close()
                if process.poll() is None:
                    process.kill()
                    process.wait(timeout=5)
    (args.out / "evidence.json").write_text(json.dumps(dict(passed=True, cases=records), indent=2) + "\n")


if __name__ == "__main__":
    main()
