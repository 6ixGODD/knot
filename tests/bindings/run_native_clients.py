"""Execute native Python/cgo component tests using CMake's actual artifacts."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import sys

def main():
    p = argparse.ArgumentParser()
    p.add_argument('--build-dir', type=Path, required=True)
    p.add_argument('--config', default='Release')
    a = p.parse_args()
    build = a.build_dir.resolve()
    fixture = json.loads((build / f'fixtures/{a.config}.json').read_text())
    root = Path(__file__).resolve().parents[2]
    for key in ('library', 'linkLibrary', 'include'):
        if not Path(fixture[key]).is_absolute() or not Path(fixture[key]).exists():
            raise ValueError(f'Missing absolute fixture artifact: {key}')
    env = os.environ.copy()
    env['KNOT_TEST_NATIVE_LIB'] = fixture['library']
    env['PYTHONPATH'] = str(root / 'bindings/python')
    env['PYTHONDONTWRITEBYTECODE'] = '1'
    env['CGO_ENABLED'] = '1'
    env['CGO_CFLAGS'] = '-I "' + fixture['include'] + '"'
    env['CGO_LDFLAGS'] = '"' + fixture['linkLibrary'] + '"'
    if sys.platform != 'win32':
        env['CGO_LDFLAGS'] += ' "-Wl,-rpath,' + str(Path(fixture['library']).parent) + '"'
    env['PATH'] = str(Path(fixture['library']).parent) + os.pathsep + env.get('PATH', '')
    env['GOCACHE'] = str(build.parent / 'go-cache')
    env['GOTMPDIR'] = str(build.parent / 'go-tmp')
    Path(env['GOTMPDIR']).mkdir(exist_ok=True)
    commands = [([sys.executable,'-m','unittest','discover','-s',str(root/'tests/bindings'),'-p','test_python.py','-v'],root),
                (['go','test','-count=1','-v','./...'],root/'bindings/go')]
    evidence = []
    for i, (cmd, cwd) in enumerate(commands):
        r = subprocess.run(cmd, cwd=cwd, env=env, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
        output = r.stdout.decode(errors='replace')
        print(output)
        (build / f'native-client-{i}.log').write_text(output, encoding='utf-8')
        evidence.append({'command':cmd,'cwd':str(cwd),'exitCode':r.returncode})
        (build/'native-clients.json').write_text(json.dumps(evidence,indent=2))
        if r.returncode: return r.returncode
    return 0

if __name__ == '__main__':
    raise SystemExit(main())
