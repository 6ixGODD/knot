"""Exercise the extracted frontend package against its reloaded image on Linux."""
import argparse
import json
from pathlib import Path
import signal
import socket
import subprocess
import sys
import tarfile
import time
import urllib.request


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--delivery", required=True, type=Path)
    args = parser.parse_args()
    delivery = args.delivery.resolve()
    manifest = json.loads((delivery / "release.json").read_text())
    out = delivery / "browser-smoke"
    out.mkdir()
    with tarfile.open(delivery / f"knot-grpc-{manifest['version']}.tar.gz") as bundle:
        bundle.extractall(out, filter="data")
    package = out / f"knot-grpc-{manifest['version']}"
    with socket.socket() as sock:
        sock.bind(("127.0.0.1", 0))
        port = sock.getsockname()[1]
    runtime = out / "runtime"
    with (out / "launcher.log").open("wb") as log:
        process = subprocess.Popen([sys.executable, str(package / "examples/grpc/speaker_web.py"),
                                    "--image", manifest["image"], "--out", str(runtime), "--port", str(port)],
                                   stdout=log, stderr=subprocess.STDOUT)
        try:
            deadline = time.monotonic() + 180
            while not (runtime / "running.json").exists():
                if process.poll() is not None or time.monotonic() > deadline:
                    raise RuntimeError("packaged browser launcher failed; see launcher.log")
                time.sleep(0.25)
            base = f"http://127.0.0.1:{port}"

            def post(method, data):
                request = urllib.request.Request(base + "/api/" + method,
                    data=json.dumps(data).encode(), headers={"Content-Type": "application/json"})
                with urllib.request.urlopen(request, timeout=10) as response:
                    return json.load(response)

            initial = post("initial", {})["result"]
            quote = post("quote", {"quote": {"schemaVersion": "1.0", "state": initial["state"]}})["quote"]
            if quote["price"]["minMinor"] != 119900 or quote["price"]["maxMinor"] != 302700:
                raise RuntimeError("unexpected complete-product quote range")
            with urllib.request.urlopen(base + "/", timeout=5) as response:
                if response.status != 200:
                    raise RuntimeError("frontend not served")
            (out / "result.json").write_text(json.dumps(dict(passed=True, quote=quote,
                sourceCommit=manifest["sourceCommit"], imageId=manifest["imageId"]), indent=2) + "\n")
        finally:
            if process.poll() is None:
                process.send_signal(signal.SIGINT)
                process.wait(timeout=45)


if __name__ == "__main__":
    main()
