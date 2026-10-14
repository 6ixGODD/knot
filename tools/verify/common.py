"""Bounded subprocess execution and evidence shared by verification entry points."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import shlex
import signal
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[2]

def sha(path):
    with Path(path).open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()

def cmake():
    found = shutil.which(os.environ.get('CMAKE', 'cmake'))
    if found:
        return found
    if sys.platform == 'win32':
        vswhere = Path(os.environ.get('ProgramFiles(x86)', 'C:/Program Files (x86)')) / 'Microsoft Visual Studio/Installer/vswhere.exe'
        location = subprocess.check_output([str(vswhere), '-latest', '-property', 'installationPath'], text=True).strip()
        path = Path(location) / 'Common7/IDE/CommonExtensions/Microsoft/CMake/CMake/bin/cmake.exe'
        if path.is_file():
            return str(path)
    raise RuntimeError('CMake is required')

class Runner:
    def __init__(self, out):
        self.out = Path(out).resolve()
        self.out.mkdir(parents=True, exist_ok=True)
        history = self.out / "commands.json"
        self.records = json.loads(history.read_text(encoding="utf-8")) if history.exists() else []
        self.env = dict(os.environ, PYTHONDONTWRITEBYTECODE='1', CARGO_BUILD_JOBS='1')
        if sys.platform == 'win32': self.env.setdefault('VSLANG', '1033')
        if not self.env.get('CARGO_TARGET_DIR'):
            raise RuntimeError('Set CARGO_TARGET_DIR to an isolated absolute output directory')
        if not Path(self.env['CARGO_TARGET_DIR']).is_absolute():
            raise RuntimeError('CARGO_TARGET_DIR must be absolute')
        # Native dependencies also embed __FILE__. Keep their diagnostics useful
        # without distributing the build host's absolute source/output paths.
        mappings = [(ROOT, 'knot-source'), (Path(self.env['CARGO_TARGET_DIR']), 'knot-build'),
                    (Path(self.env.get('CARGO_HOME', str(Path.home()/'.cargo'))), 'cargo-source')]
        flags = ['/experimental:deterministic', '/utf-8'] if sys.platform == 'win32' else []
        for source, destination in mappings:
            prefix = '/pathmap:' if sys.platform == 'win32' else '-ffile-prefix-map='
            # MSVC does not normalize separators in every CodeView record.
            for spelling in dict.fromkeys((source.as_posix(), str(source))):
                flags.append(prefix + spelling + '=' + destination)
        self.env['CC_SHELL_ESCAPED_FLAGS'] = '1'
        # cc uses HOST_* when HOST == TARGET, even for normal target libraries.
        for key in ('HOST_CFLAGS', 'HOST_CXXFLAGS', 'TARGET_CFLAGS', 'TARGET_CXXFLAGS'):
            existing = shlex.split(self.env.get(key, ''))
            self.env[key] = shlex.join(existing + [flag for flag in flags if flag not in existing])

    def run(self, name, argv, *, cwd=ROOT, env=None, timeout=7200):
        log = self.out / (name + '.log')
        if log.exists():
            index = 2
            while (self.out / f'{name}-{index}.log').exists():
                index += 1
            log = self.out / f'{name}-{index}.log'
        start = time.monotonic()
        record = dict(name=name, argv=[str(a) for a in argv], cwd=str(cwd), exitCode=None, timedOut=False, log=log.name)
        try:
            with log.open('wb') as stream:
                process = subprocess.Popen(record['argv'], cwd=cwd, env=env or self.env, stdout=stream, stderr=subprocess.STDOUT, start_new_session=sys.platform != 'win32')
                try:
                    record['exitCode'] = process.wait(timeout=timeout)
                except subprocess.TimeoutExpired:
                    # Stop only this command's process tree; Cargo may have active compiler children.
                    if sys.platform == 'win32':
                        subprocess.run(['taskkill', '/PID', str(process.pid), '/T', '/F'], stdout=stream, stderr=subprocess.STDOUT, timeout=30)
                    else:
                        os.killpg(process.pid, signal.SIGKILL)
                    process.wait(timeout=30)
                    raise
        except subprocess.TimeoutExpired:
            record['timedOut'] = True
        except OSError as error:
            record['error'] = str(error)
        finally:
            record['seconds'] = time.monotonic() - start
            self.records.append(record)
            (self.out / 'commands.json').write_text(json.dumps(self.records, indent=2) + '\n', encoding='utf-8')
        print(f'{name}: exit={record["exitCode"]} timeout={record["timedOut"]}', flush=True)
        if record['exitCode'] != 0 or record['timedOut']:
            raise RuntimeError(f'{name} failed: {log}')
        return log.read_text(encoding='utf-8', errors='replace')

    def cargo(self, name, *args):
        return self.run(name, ['cargo', *args])
