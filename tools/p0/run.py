"""Run the real ZEN and native P0 probes; outputs and complete logs stay outside source."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import shlex
import shutil
import subprocess
import struct
import sys
import urllib.request
import xml.etree.ElementTree as ET

sys.dont_write_bytecode = True
from check_json_coexistence import HOST_FILES


def digest(path):
    with Path(path).open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def pe_export_name(path):
    """Read the linker's actual DLL export name, rather than guessing Cargo's hash suffix."""
    with path.open('rb') as stream:
        def read(offset, count):
            stream.seek(offset)
            data = stream.read(count)
            if len(data) != count:
                raise ValueError('truncated PE image')
            return data
        nt = struct.unpack('<I', read(0x3c, 4))[0]
        if read(nt, 4) != b'PE\0\0':
            raise ValueError('invalid PE signature')
        sections = struct.unpack('<H', read(nt+6, 2))[0]
        optional_size = struct.unpack('<H', read(nt+20, 2))[0]
        optional = nt+24
        magic = struct.unpack('<H', read(optional, 2))[0]
        if magic not in (0x10b, 0x20b):
            raise ValueError('unsupported PE format')
        export_rva = struct.unpack('<I', read(optional+(112 if magic == 0x20b else 96), 4))[0]
        table = [struct.unpack('<IIII', read(optional+optional_size+i*40+8, 16)) for i in range(sections)]
        def offset(rva):
            for virtual_size, address, raw_size, raw_offset in table:
                if address <= rva < address+max(virtual_size, raw_size):
                    return raw_offset+rva-address
            raise ValueError('PE RVA outside sections')
        name_rva = struct.unpack('<I', read(offset(export_rva)+12, 4))[0]
        name = read(offset(name_rva), 256).split(b'\0', 1)[0].decode('ascii')
        if Path(name).name != name or not name.endswith('.dll'):
            raise ValueError('invalid DLL export name')
        return name


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--out', type=Path, required=True)
    parser.add_argument('--cmake', default='cmake')
    parser.add_argument('--host-json-dir', type=Path)
    args = parser.parse_args()
    source = Path(__file__).resolve().parents[2]
    out = args.out.resolve()
    if out == source or source in out.parents:
        parser.error('output must be outside the implementation source worktree')
    out.mkdir(parents=True, exist_ok=True)
    evidence = dict(passed=False, platform=platform.platform(), machine=platform.machine(),
                    commands=[], lockSha256=digest(source / 'Cargo.lock'))
    env = dict(os.environ)
    env.setdefault('CARGO_TARGET_DIR', str(out.parent / 'cargo-target'))
    evidence['cargoTargetDir'] = env['CARGO_TARGET_DIR']

    def save():
        (out / 'evidence.json').write_text(json.dumps(evidence, ensure_ascii=False, indent=2)+'\n', encoding='utf-8')

    def run(name, argv):
        log = out / (name + '.log')
        code = None
        timed_out = False
        try:
            with log.open('wb') as stream:
                code = subprocess.run(argv, cwd=source, env=env, stdout=stream,
                                      stderr=subprocess.STDOUT, timeout=1200).returncode
        except subprocess.TimeoutExpired:
            timed_out = True
        evidence['commands'].append(dict(name=name, argv=argv, exitCode=code, timedOut=timed_out, log=log.name))
        save()
        print(f'{name}: exit={code}, timeout={timed_out}', flush=True)
        if code != 0 or timed_out:
            raise RuntimeError(f'{name} did not pass; see {log.name}')
        return log.read_text(encoding='utf-8', errors='replace')

    try:
        cmake = shutil.which(args.cmake)
        if not cmake and sys.platform == 'win32':
            vswhere = Path(os.environ.get('ProgramFiles(x86)', 'C:/Program Files (x86)')) / 'Microsoft Visual Studio/Installer/vswhere.exe'
            if vswhere.is_file():
                location = subprocess.check_output([str(vswhere), '-latest', '-property', 'installationPath'], text=True).strip()
                candidate = Path(location) / 'Common7/IDE/CommonExtensions/Microsoft/CMake/CMake/bin/cmake.exe'
                if candidate.is_file():
                    cmake = str(candidate)
        if not cmake:
            raise RuntimeError('CMake unavailable')
        ctest = str(Path(cmake).with_name('ctest.exe' if sys.platform == 'win32' else 'ctest'))
        host = args.host_json_dir.resolve() if args.host_json_dir else out / 'host-json'
        host.mkdir(parents=True, exist_ok=True)
        for name, (url, expected) in HOST_FILES.items():
            path = host / name
            if not path.is_file():
                with urllib.request.urlopen(url, timeout=60) as response:
                    path.write_bytes(response.read(2*1024*1024))
            if digest(path) != expected:
                raise RuntimeError(f'host fixture digest mismatch: {name}')
        evidence['rustc'] = run('rustc-version', ['rustc', '-vV'])
        evidence['cmake'] = run('cmake-version', [cmake, '--version'])
        run('backend', ['cargo', 'test', '-p', 'knot-rules-zen', '--locked'])
        artifacts = run('cargo-build', ['cargo', 'build', '-p', 'knot-rules-zen', '--example', 'p0_native', '--locked', '--message-format=json'])
        paths = []
        for line in artifacts.splitlines():
            if not line.startswith('{'):
                continue
            message = json.loads(line)
            if message.get('reason') == 'compiler-artifact' and message['target']['name'] == 'p0_native':
                paths.extend(Path(path) for path in message['filenames'])
        shared = next(p for p in paths if p.suffix in ('.dll', '.so', '.dylib'))
        static = next(p for p in paths if p.suffix in ('.lib', '.a') and not p.name.endswith('.dll.lib'))
        libraries = run('native-libraries', ['cargo', 'rustc', '-p', 'knot-rules-zen', '--example', 'p0_native', '--locked', '--', '--print', 'native-static-libs'])
        native_line = next(line.split('native-static-libs:', 1)[1].strip() for line in libraries.splitlines() if 'native-static-libs:' in line)
        native_libs = shlex.split(native_line, posix=True)
        configure = [cmake, '-S', str(source / 'tests/p0/native'), '-B', str(out / 'native'),
                     f'-DP0_SHARED={shared.as_posix()}', f'-DP0_STATIC={static.as_posix()}',
                     '-DP0_SYSTEM_LIBS='+';'.join(native_libs), f'-DHOST_JSON_DIR={host.as_posix()}',
                     '-DCMAKE_BUILD_TYPE=Release']
        if sys.platform == 'win32':
            implib = next(p for p in paths if p.name.endswith('.dll.lib'))
            runtime_name = pe_export_name(shared)
            evidence['dllExportName'] = runtime_name
            configure += ['-G', 'Visual Studio 17 2022', '-A', 'x64', f'-DP0_IMPLIB={implib.as_posix()}', f'-DP0_RUNTIME_NAME={runtime_name}']
        run('native-configure', configure)
        run('native-build', [cmake, '--build', str(out / 'native'), '--config', 'Release', '--parallel', '2'])
        run('native-test', [ctest, '--test-dir', str(out / 'native'), '-C', 'Release', '--verbose', '--output-junit', str(out / 'native-results.xml')])
        cases = ET.parse(out / 'native-results.xml').getroot().findall('.//testcase')
        if len(cases) != 13 or any(case.find(tag) is not None for case in cases for tag in ('failure', 'error', 'skipped')):
            raise RuntimeError('expected 4 native and 9 JSON runtime tests, all executed and passing')
        run('json-isolation', [sys.executable, str(source / 'tools/p0/check_json_isolation.py'), '--out', str(out / 'json-isolation'), '--host-json-dir', str(host), '--cmake', cmake])
        evidence['artifacts'] = {str(p): digest(p) for p in paths if p.is_file()}
        evidence['runtimeTests'] = 13
        evidence['nativeSystemLibraries'] = native_libs
        evidence['sourceSha256'] = {p.relative_to(source).as_posix(): digest(p) for folder in ('crates', 'contracts', 'tests', 'tools/p0', 'vendor') for p in sorted((source / folder).rglob('*')) if p.is_file()}
        for name in ('Cargo.toml', 'rust-toolchain.toml', '.gitattributes', '.github/workflows/p0.yml'):
            evidence['sourceSha256'][name] = digest(source / name)
        if evidence['lockSha256'] != digest(source / 'Cargo.lock'):
            raise RuntimeError('candidate lock changed during P0')
        evidence['passed'] = True
        save()
        return 0
    except (OSError, ValueError, RuntimeError, StopIteration, ET.ParseError, subprocess.SubprocessError) as error:
        evidence['error'] = str(error)
        save()
        print(f'FAIL: {error}', file=sys.stderr)
        return 1


if __name__ == '__main__':
    raise SystemExit(main())
