"""Run Linux verification in reusable Docker images, with explicit host outputs."""
import argparse
import json
from pathlib import Path
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[2]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--out', type=Path, required=True)
    parser.add_argument('--cache', type=Path, required=True)
    parser.add_argument('--stage', choices=['native', 'fuzz', 'memory', 'consumer'], default='native')
    parser.add_argument('--build', action='store_true')
    parser.add_argument('--base-image', help='Optional existing Ubuntu/Rust image to reuse while building')
    parser.add_argument('--package-dir', type=Path, help='Required for the offline consumer stage')
    args = parser.parse_args()
    out, cache = args.out.resolve(), args.cache.resolve()
    if out.exists():
        parser.error('Choose a fresh output directory; previous evidence is retained')
    out.mkdir(parents=True)
    cache.mkdir(parents=True, exist_ok=True)
    records = []

    def run(name, command):
        start = time.monotonic()
        with (out / (name + '.log')).open('wb') as log:
            result = subprocess.run([str(x) for x in command], stdout=log, stderr=subprocess.STDOUT)
        records.append(dict(name=name, command=[str(x) for x in command], exitCode=result.returncode,
                            seconds=time.monotonic() - start, log=name + '.log'))
        (out / 'docker-commands.json').write_text(json.dumps(records, indent=2) + '\n', encoding='utf-8')
        print(f'{name}: exit={result.returncode}', flush=True)
        if result.returncode:
            raise RuntimeError(f'{name} failed; see {out / (name + ".log")}')
        return (out / (name + '.log')).read_text(encoding='utf-8')

    target = 'security' if args.stage in ('fuzz', 'memory') else args.stage
    image = 'knot-verify-' + target + ':rust1.97.1-go1.26.4'
    if args.build:
        command = ['docker', 'build', '--progress=plain', '--target', target, '-t', image]
        if args.base_image:
            command += ['--build-arg', 'BASE_IMAGE=' + args.base_image]
        run('image-build', command + [ROOT / 'tools/verify/linux'])
    inspection = json.loads(run('image-inspect', ['docker', 'image', 'inspect', image]))[0]
    if inspection['Os'] != 'linux' or inspection['Architecture'] != 'amd64':
        raise RuntimeError('Linux amd64 image required')
    image = inspection['Id']
    evidence = dict(passed=False, stage=args.stage, imageId=image,
                    architecture=inspection['Architecture'], os=inspection['Os'])
    (out / 'docker-evidence.json').write_text(json.dumps(evidence, indent=2) + '\n', encoding='utf-8')
    command = ['docker', 'run', '--rm', '--init', '--cpus', '2', '--memory', '6g',
               '--mount', f'type=bind,source={out},target=/evidence']
    if args.stage == 'consumer':
        if not args.package_dir:
            parser.error('--package-dir is required for consumer')
        package = args.package_dir.resolve()
        receipt = json.loads((package / 'package.json').read_text(encoding='utf-8'))
        script = '/candidate/' + receipt['packageDirectory'] + '/tools/consume.py'
        command += ['--network', 'none', '--mount', f'type=bind,source={package},target=/candidate,readonly',
                    image, 'python3', script, '--package-dir', '/candidate', '--out', '/evidence/consumer']
    else:
        git_common = Path(subprocess.check_output(['git', 'rev-parse', '--path-format=absolute', '--git-common-dir'], cwd=ROOT, text=True).strip())
        git_dir = Path(subprocess.check_output(['git', 'rev-parse', '--absolute-git-dir'], cwd=ROOT, text=True).strip())
        relative = git_dir.relative_to(git_common).as_posix()
        command += ['--mount', f'type=bind,source={ROOT},target=/workspace,readonly',
                    '--mount', f'type=bind,source={git_common},target=/git,readonly',
                    '--mount', f'type=bind,source={cache},target=/cache']
        for folder, destination in [('registry', '/usr/local/cargo/registry'), ('git', '/usr/local/cargo/git')]:
            (cache / folder).mkdir(exist_ok=True)
            command += ['--mount', f'type=bind,source={cache / folder},target={destination}']
        for key, value in dict(CARGO_TARGET_DIR='/cache/cargo-target', GIT_DIR='/git/' + relative,
                               GIT_WORK_TREE='/workspace', GIT_OPTIONAL_LOCKS='0', GIT_CONFIG_COUNT='1',
                               GIT_CONFIG_KEY_0='safe.directory', GIT_CONFIG_VALUE_0='/workspace').items():
            command += ['--env', key + '=' + value]
        command += [image, 'python3', 'tools/verify/all.py', '--stage', args.stage, '--out', '/evidence/verify']
    run('verify', command)
    evidence['passed'] = True
    (out / 'docker-evidence.json').write_text(json.dumps(evidence, indent=2) + '\n', encoding='utf-8')


if __name__ == '__main__':
    main()
