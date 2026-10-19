"""Build an internal native candidate with explicit contents and verified checksums."""
import argparse
import json
import os
from pathlib import Path
import platform
import re
import shlex
import shutil
import sys
import zipfile
sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'verify'))
from common import ROOT, Runner, cmake, sha
from archive import contains_workspace_path, normalize_coff

EXPORTS = {'knot_abi_version', 'knot_model_create', 'knot_initial', 'knot_apply', 'knot_evaluate', 'knot_result_free', 'knot_model_free'}
# The locked AWS-LC C objects use dllexport on Windows. They are not Rust/Zen
# types or part of the KNOT API; enumerate them rather than allowing arbitrary exports.
WINDOWS_TRANSITIVE_EXPORTS = {'aws_lc_0_46_0_'+name for name in ['jent_entropy_collector_alloc','jent_entropy_collector_free','jent_entropy_init','jent_entropy_init_ex','jent_entropy_switch_notime_impl','jent_read_entropy','jent_read_entropy_safe','jent_set_fips_failure_callback','jent_version']}

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--out', type=Path, required=True)
    parser.add_argument('--public', action='store_true', help='Require an owner-selected product LICENSE')
    args = parser.parse_args()
    runner = Runner(args.out)
    path_roots = (ROOT, runner.out, Path(runner.env['CARGO_TARGET_DIR']))
    object_tool = None
    if args.public and not (ROOT / 'LICENSE').is_file():
        raise RuntimeError('Product LICENSE is not selected; public package prohibited')
    version = runner.run('rustc-version', ['rustc', '-vV'])
    target = re.search(r'^host: (.+)$', version, re.M).group(1)
    if target not in ('x86_64-pc-windows-msvc', 'x86_64-unknown-linux-gnu', 'aarch64-apple-darwin'):
        raise RuntimeError('Unsupported target ' + target)
    runner.cargo('release', 'build', '--workspace', '--release', '--locked')
    link = runner.cargo('static-libs', 'rustc', '-p', 'knot-ffi', '--lib', '--release', '--locked', '--', '--print=native-static-libs')
    found = re.findall(r'native-static-libs:\s*(.+)', link)
    if not found:
        raise RuntimeError('rustc did not report native static dependencies')
    native = shlex.split(found[-1], posix=True)
    cmake_native = []
    cmake_options = []
    index = 0
    while index < len(native):
        flag = native[index]
        if target == 'x86_64-pc-windows-msvc' and flag.startswith('/'):
            # CMake treats slash-prefixed MSVC options in LINK_LIBRARIES as
            # file paths. DEFAULTLIB belongs in LINK_OPTIONS instead.
            cmake_options.append(flag)
            index += 1
        elif flag == '-framework':
            if index + 1 >= len(native): raise RuntimeError('Truncated framework argument')
            cmake_native.append(flag + ' ' + native[index+1])
            index += 2
        else:
            cmake_native.append(flag)
            index += 1
    bridge = runner.out / 'bridges'
    command = [cmake(), '-S', ROOT / 'bindings/cpp', '-B', bridge, '-DCMAKE_BUILD_TYPE=Release']
    mappings = [(ROOT, 'knot-source'), (runner.out, 'knot-package')]
    if sys.platform == 'win32':
        cpp_flags = '/experimental:deterministic ' + ' '.join('"/pathmap:'+spelling+'='+label+'"'
            for path, label in mappings for spelling in dict.fromkeys((path.as_posix(), str(path))))
    else:
        cpp_flags = ' '.join('"-ffile-prefix-map='+path.as_posix()+'='+label+'"' for path, label in mappings)
    command.append('-DCMAKE_CXX_FLAGS='+cpp_flags)
    if sys.platform == 'win32': command.extend(['-A', 'x64'])
    runner.run('bridge-configure', command)
    runner.run('bridge-build', [cmake(), '--build', bridge, '--config', 'Release', '--parallel', '1'])
    cmake_version = runner.run('cmake-version', [cmake(), '--version']).splitlines()[0]
    compiler_files = list((bridge/'CMakeFiles').glob('*/CMakeCXXCompiler.cmake'))
    if len(compiler_files) != 1: raise RuntimeError('Ambiguous C++ compiler metadata')
    compiler_info = compiler_files[0].read_text(encoding='utf-8')
    cpp_tools = {}
    for field in ['ID', 'VERSION']:
        match = re.search(r'set\(CMAKE_CXX_COMPILER_'+field+r' "([^"\n]+)"\)', compiler_info)
        if not match: raise RuntimeError('Missing C++ compiler '+field)
        cpp_tools[field.lower()] = match.group(1)
    sdk = runner.out / ('knot-0.1.0-' + target)
    if sdk.exists():
        raise RuntimeError('Candidate directory already exists; choose a new output to retain prior evidence')
    sdk.mkdir()
    def copy(source, destination):
        source = Path(source)
        dest = sdk / destination
        dest.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source, dest)
    for source, dest in [(ROOT/'contracts/abi/knot.h', 'include/knot.h'), (ROOT/'bindings/cpp/include/knot/knot.hpp','include/knot/knot.hpp'), (ROOT/'cmake/KnotConfig.cmake','lib/cmake/Knot/KnotConfig.cmake'), (ROOT/'cmake/KnotTargets.cmake','lib/cmake/Knot/KnotTargets.cmake'), (ROOT/'README.md','README.md'), (ROOT/'tests/package/consume.py','tools/consume.py')]: copy(source,dest)
    for filename in ['bindings/python/knot/__init__.py', 'bindings/go/knot.go', 'bindings/go/go.mod']:
        copy(ROOT/filename,filename)
    for directory in ['contracts', 'examples']:
        for source in sorted((ROOT/directory).rglob('*')):
            if source.is_file() and source.suffix not in ('.pyc',) and '__pycache__' not in source.parts and not source.name.endswith('_test.go'):
                copy(source, source.relative_to(ROOT))
    public_docs = ['quickstart','integration','c-api','cpp-api','dsl-reference','state-machine','pricing','visual-ir','limits','troubleshooting','grpc-api','grpc-api.zh-CN','deployment']
    for name in public_docs: copy(ROOT/f'docs/{name}.md', f'docs/{name}.md')
    output = Path(runner.env['CARGO_TARGET_DIR'])/'release'
    if sys.platform == 'win32':
        dynamic = output/'knot_ffi.dll'
        for source, dest in [(dynamic,'bin/knot.dll'),(output/'knot_ffi.lib','lib/static/knot.lib'),(output/'knot.exe','bin/knot.exe')]: copy(source,dest)
        for mode in ['shared','static']: copy(bridge/f'Release/knot_cpp_{mode}.lib',f'lib/cpp/knot_cpp_{mode}.lib')
        # Resolve dumpbin from the selected Visual Studio compiler installation.
        selected_compiler = re.search(r'set\(CMAKE_CXX_COMPILER "([^"\n]+)"\)', compiler_info)
        if not selected_compiler: raise RuntimeError('Missing selected MSVC compiler path')
        dumpbin_path = Path(selected_compiler.group(1)).with_name('dumpbin.exe')
        if not dumpbin_path.is_absolute() or not dumpbin_path.is_file():
            raise RuntimeError('dumpbin required beside the selected MSVC compiler')
        dumpbin = str(dumpbin_path)
        exports_text = runner.run('exports',[dumpbin,'/exports',dynamic])
        # dumpbin may append an ICF/debug-symbol alias after the exported name.
        exports = set(re.findall(r'^\s*\d+\s+[0-9A-F]+\s+[0-9A-F]+\s+(\S+)(?:\s+=.*)?\s*$', exports_text,re.M))
        if not EXPORTS.issubset(exports) or exports - (EXPORTS | WINDOWS_TRANSITIVE_EXPORTS):
            raise RuntimeError('Unexpected Windows exported symbols: '+str(sorted(exports)))
        definition = runner.out/'knot.def'
        definition.write_text('LIBRARY knot.dll\nEXPORTS\n'+'\n'.join(sorted(EXPORTS))+'\n',encoding='ascii')
        (sdk/'lib/shared').mkdir(parents=True,exist_ok=True)
        librarian = Path(dumpbin).with_name('lib.exe')
        sysroot = Path(runner.run('sysroot', ['rustc', '--print', 'sysroot']).strip())
        objcopy = sysroot/'lib/rustlib'/target/'bin/rust-objcopy.exe'
        if not objcopy.is_file(): raise RuntimeError('The pinned Rust compiler must provide rust-objcopy')
        object_tool = runner.run('object-tool', [objcopy, '--version']).strip()
        normalize_coff(runner, librarian, output/'knot_ffi.lib', sdk/'lib/static/knot.lib', objcopy, path_roots)
        # Bind the public DLL name without colliding with Cargo's knot CLI artifacts.
        # Only KNOT functions enter the consumer import library.
        runner.run('import-library',[librarian,'/nologo','/machine:x64','/def:'+str(definition),'/out:'+str(sdk/'lib/shared/knot.lib')])
        (sdk/'lib/shared/knot.exp').unlink(missing_ok=True)
    else:
        name = 'libknot.dylib' if sys.platform == 'darwin' else 'libknot.so'
        dynamic = output/('libknot_ffi.dylib' if sys.platform=='darwin' else 'libknot_ffi.so')
        copy(dynamic,'lib/'+name); copy(output/'libknot_ffi.a','lib/static/libknot.a'); copy(output/'knot','bin/knot')
        for mode in ['shared','static']: copy(bridge/f'libknot_cpp_{mode}.a',f'lib/cpp/libknot_cpp_{mode}.a')
        argv = ['nm','-gU',dynamic] if sys.platform == 'darwin' else ['nm','-D','--defined-only',dynamic]
        exports_text = runner.run('exports',argv)
        exports = {line.split()[-1].removeprefix('_') for line in exports_text.splitlines() if line.split()}
    allowed = EXPORTS | (WINDOWS_TRANSITIVE_EXPORTS if sys.platform=='win32' else set())
    if not EXPORTS.issubset(exports) or exports - allowed: raise RuntimeError(f'Unexpected exported symbols: missing={sorted(EXPORTS-exports)} extra={sorted(exports-allowed)}')
    # Replace static dependencies with this exact rustc result, preserving flag order/pairs.
    targets = sdk/'lib/cmake/Knot/KnotTargets.cmake'
    content = targets.read_text(encoding='utf-8')
    content += '\n# Native dependencies reported by the compiler for this candidate.\n'
    content += 'set_property(TARGET Knot::Static PROPERTY INTERFACE_LINK_LIBRARIES "' + ';'.join(cmake_native).replace('"','\\"') + '")\n'
    content += 'set_property(TARGET Knot::Static PROPERTY INTERFACE_LINK_OPTIONS "' + ';'.join(cmake_options).replace('"','\\"') + '")\n'
    targets.write_text(content,encoding='utf-8')
    metadata = json.loads(runner.run('metadata',['cargo','metadata','--format-version','1','--locked'],separate_stderr=True))
    notice_index_path = ROOT/'tools/package/notices/index.json'
    notice_index = json.loads(notice_index_path.read_text(encoding='utf-8')) if notice_index_path.is_file() else {}
    service_notices = ROOT / "tools/server/notices/index.json"
    if service_notices.is_file():
        notice_index.update(json.loads(service_notices.read_text(encoding="utf-8")))
    packages = []
    notice_revision_gaps = []
    for package in metadata['packages']:
        if package['source'] is None: continue
        source = Path(package['manifest_path']).parent
        notices = []
        for path in sorted(source.rglob('*')):
            if path.is_file() and path.name.lower().startswith(('license','licence','copying','notice','copyright')):
                relative = Path('licenses')/(package['name']+'-'+package['version'])/path.relative_to(source)
                copy(path, relative); notices.append(relative.as_posix())
        upstream = None
        if not notices:
            key = package['name']+'-'+package['version']
            upstream = notice_index.get(key)
            if upstream:
                if upstream.get('missingSeparateNotice'): notice_revision_gaps.append(key)
                for notice in upstream['notices']:
                    original = ROOT/notice['path']
                    if sha(original) != notice['sha256']: raise RuntimeError('Changed notice snapshot '+notice['path'])
                    relative = Path('licenses')/key/original.name
                    copy(original, relative); notices.append(relative.as_posix())
                    if not notice['publishedRevisionMatched']: notice_revision_gaps.append(key)
        if not package.get('license') or not notices: raise RuntimeError(f'Missing dependency license evidence: {package["name"]}')
        source_files = []
        if any(name in package['license'] for name in ['MPL-', 'LGPL-', 'GPL-']):
            for path in sorted(source.rglob('*')):
                if path.is_file() and path.name not in ('.cargo-ok', '.cargo-checksum.json'):
                    relative = Path('licenses')/(package['name']+'-'+package['version'])/'source'/path.relative_to(source)
                    copy(path, relative); source_files.append(relative.as_posix())
        packages.append(dict(name=package['name'],version=package['version'],source=package['source'],license=package['license'],notices=notices,upstreamNotice=upstream,sourceFiles=source_files))
    for path in (ROOT/'vendor/nlohmann').iterdir():
        if path.is_file() and path.name != 'json.hpp': copy(path,Path('licenses/nlohmann')/path.name)
    if (ROOT/'LICENSE').is_file(): copy(ROOT/'LICENSE','LICENSE')
    if args.public and notice_revision_gaps: raise RuntimeError('Some published revisions are unavailable for exact notice verification')
    manifest = dict(schemaVersion='1.0', version='0.1.0', target=target, productLicense='selected' if (ROOT/'LICENSE').is_file() else 'unselected-internal-only', publicReleaseAuthorized=False, rustc=version, cmake=cmake_version, cppCompiler=cpp_tools, buildHost=platform.platform(), configuration='Release', cppAbi='MSVC dynamic CRT' if sys.platform=='win32' else ('libc++' if sys.platform=='darwin' else 'libstdc++ default ABI'), minimumOs='not-established; tested host only', lockSha256=sha(ROOT/'Cargo.lock'), privateJsonSha256=sha(ROOT/'vendor/nlohmann/json.hpp'), nativeStaticLibraries=native, exports=sorted(exports), dependencies=packages)
    manifest['noticeRevisionGaps'] = sorted(set(notice_revision_gaps))
    manifest['archiveObjectTool'] = object_tool
    (sdk/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n',encoding='utf-8')
    forbidden = {'.mine','AGENTS.md','REQUIREMENTS.md','__pycache__'}
    files = sorted(p for p in sdk.rglob('*') if p.is_file())
    for path in files:
        relative = path.relative_to(sdk)
        if forbidden.intersection(relative.parts) or path.name.endswith('_test.go') or relative.as_posix().startswith(('docs/design/','docs/plan/','tests/')): raise RuntimeError('Forbidden package file '+str(relative))
        if contains_workspace_path(path, path_roots):
            raise RuntimeError('Workspace path leaked '+str(relative))
    (sdk/'checksums.json').write_text(json.dumps({p.relative_to(sdk).as_posix():sha(p) for p in files},indent=2)+'\n',encoding='utf-8')
    archive = Path(str(sdk)+'.zip')
    # Published crate archives may use epoch timestamps, outside ZIP's range.
    with zipfile.ZipFile(archive,'w',zipfile.ZIP_DEFLATED,strict_timestamps=False) as output_zip:
        for path in sorted(sdk.rglob('*')):
            if path.is_file(): output_zip.write(path,path.relative_to(sdk))
    receipt = dict(packageDirectory=sdk.name,archive=archive.name,sha256=sha(archive),target=target,status='internal-candidate')
    (runner.out/'package.json').write_text(json.dumps(receipt,indent=2)+'\n',encoding='utf-8')
    print(json.dumps(receipt))

if __name__ == '__main__': main()
