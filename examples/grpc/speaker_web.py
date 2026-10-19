"""Start the local SVG configurator and its real gRPC container; Ctrl+C stops both."""
import argparse
import json
import os
from pathlib import Path
import shutil
import socket
import subprocess
import time
import urllib.request
import uuid

ROOT = Path(__file__).resolve().parents[2]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--image', required=True, help='local image built with the Quote RPC')
    parser.add_argument('--out', required=True, type=Path, help='new directory outside the checkout')
    parser.add_argument('--port', type=int, default=8765, help='loopback browser port')
    args = parser.parse_args()
    if not 1024 <= args.port <= 65535:
        parser.error('port must be between 1024 and 65535')
    out = args.out.resolve()
    if out.is_relative_to(ROOT):
        parser.error('output must be outside the source checkout')
    out.mkdir(parents=True, exist_ok=False)
    # Refuse a busy browser port before building or starting any container.
    with socket.socket() as check:
        check.bind(('127.0.0.1', args.port))
    image = json.loads(subprocess.check_output(['docker', 'image', 'inspect', args.image]))[0]
    binary = out / ('speaker-web.exe' if os.name == 'nt' else 'speaker-web')
    env = dict(os.environ, CGO_ENABLED='0', GOTOOLCHAIN='local', GOMAXPROCS='1')
    with (out / 'build.log').open('wb') as log:
        subprocess.run(['go', 'build', '-p=1', '-mod=readonly', '-trimpath', '-o', str(binary), './cmd/speaker-web'],
                       cwd=ROOT / 'examples/grpc/go', env=env, stdout=log, stderr=subprocess.STDOUT, check=True, timeout=600)
    models = out / 'models'
    models.mkdir()
    shutil.copyfile(ROOT / 'contracts/examples/speaker.json', models / 'speaker.json')
    with socket.socket() as sock:
        sock.bind(('127.0.0.1', 0))
        grpc_port = sock.getsockname()[1]
    name = 'knot-speaker-web-' + uuid.uuid4().hex
    web = None
    url = f'http://127.0.0.1:{args.port}'
    try:
        subprocess.run(['docker', 'run', '--detach', '--name', name, '--read-only', '--cap-drop', 'ALL',
            '--security-opt', 'no-new-privileges', '--memory', '512m', '--cpus', '2',
            '--publish', f'127.0.0.1:{grpc_port}:50051', '--mount', f'type=bind,src={models},dst=/models,readonly',
            image['Id'], 'serve', '--transport', 'plaintext'], check=True, capture_output=True, timeout=60)
        deadline = time.monotonic() + 60
        while subprocess.run(['docker', 'exec', name, '/usr/local/bin/knot-server', 'healthcheck',
                '--endpoint', 'http://127.0.0.1:50051'], capture_output=True, timeout=10).returncode:
            if time.monotonic() > deadline:
                raise RuntimeError('gRPC service did not become healthy')
            time.sleep(0.25)
        with (out / 'web.log').open('wb') as log:
            web = subprocess.Popen([str(binary), '--listen', f'127.0.0.1:{args.port}',
                '--grpc', f'127.0.0.1:{grpc_port}', '--events', str(ROOT / 'contracts/examples/speaker-events.json')],
                stdout=log, stderr=subprocess.STDOUT)
            deadline = time.monotonic() + 30
            while True:
                if web.poll() is not None:
                    raise RuntimeError('web server exited; see web.log')
                try:
                    with urllib.request.urlopen(url + '/api/example', timeout=2) as response:
                        json.load(response)
                    break
                except OSError:
                    if time.monotonic() > deadline:
                        raise RuntimeError('web server did not become ready')
                    time.sleep(0.25)
            (out / 'running.json').write_text(json.dumps(dict(url=url, launcherPid=os.getpid(),
                webPid=web.pid, container=name, imageId=image['Id']), indent=2) + '\n', encoding='utf-8')
            print('Open ' + url + ' — Ctrl+C stops this demo.', flush=True)
            if web.wait() != 0:
                raise RuntimeError('web server exited with an error')
    except KeyboardInterrupt:
        print('\nStopping the demo.', flush=True)
    finally:
        if web is not None and web.poll() is None:
            web.terminate()
            web.wait(timeout=10)
        subprocess.run(['docker', 'stop', '--time', '15', name], capture_output=True, timeout=30)
        subprocess.run(['docker', 'rm', '--force', name], capture_output=True, timeout=30)


if __name__ == '__main__':
    main()
