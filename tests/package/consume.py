"""Verify and relocate a candidate, then consume both variants without Cargo or source."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import time
import zipfile

def sha(path):
    with path.open('rb') as stream: return hashlib.file_digest(stream,'sha256').hexdigest()

def verify(root):
    hashes = json.loads((root/'checksums.json').read_text(encoding='utf-8'))
    actual = {p.relative_to(root).as_posix() for p in root.rglob('*') if p.is_file()} - {'checksums.json'}
    if actual != set(hashes): raise RuntimeError('Package file inventory mismatch')
    for name, digest in hashes.items():
        path = (root/name).resolve()
        if root not in path.parents or sha(path) != digest: raise RuntimeError('Checksum mismatch: '+name)

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--package-dir',type=Path,required=True)
    parser.add_argument('--out',type=Path,required=True)
    args = parser.parse_args()
    source, out = args.package_dir.resolve(), args.out.resolve()
    out.mkdir(parents=True,exist_ok=True)
    evidence = dict(passed=False,commands=[],networkRequired=False,rustRequired=False)
    def save(): (out/'evidence.json').write_text(json.dumps(evidence,indent=2)+'\n',encoding='utf-8')
    save()
    receipt = json.loads((source/'package.json').read_text(encoding='utf-8'))
    archive = source/receipt['archive']
    if sha(archive) != receipt['sha256']: raise RuntimeError('Archive checksum mismatch')
    moved = out/'relocated sdk'
    if moved.exists(): raise RuntimeError('Consumer relocation must be a fresh directory')
    moved.mkdir()
    with zipfile.ZipFile(archive) as package:
        for entry in package.infolist():
            dest = (moved/entry.filename).resolve()
            if moved not in dest.parents or entry.file_size > 2*1024**3: raise RuntimeError('Unsafe archive member')
        package.extractall(moved)
        if sys.platform!='win32':
            for entry in package.infolist():
                if not entry.is_dir(): os.chmod(moved/entry.filename,(entry.external_attr >> 16) & 0o777)
    verify(moved)
    # Prove that checksum verification really rejects modified contents.
    probe = moved/'README.md'; original = probe.read_bytes(); probe.write_bytes(original+b'\ncorrupt\n')
    try:
        try: verify(moved)
        except RuntimeError: evidence['corruptionRejected'] = True
        else: raise RuntimeError('Corruption was accepted')
    finally: probe.write_bytes(original)
    verify(moved)
    cmake = shutil.which(os.environ.get('CMAKE','cmake'))
    if not cmake: raise RuntimeError('CMake must be explicitly available to the consumer')
    # No tool in these commands invokes Rust, fetches sources, or configures a download.
    env = dict(os.environ)
    env['PATH'] = os.pathsep.join(p for p in env.get('PATH','').split(os.pathsep) if '.cargo' not in p.lower() and '.rustup' not in p.lower())
    env['PATH'] = str(moved/'bin')+os.pathsep+env['PATH']
    env['LD_LIBRARY_PATH'] = str(moved/'lib')
    env['DYLD_LIBRARY_PATH'] = str(moved/'lib')
    def run(name, argv):
        start=time.monotonic()
        with (out/(name+'.log')).open('wb') as log:
            result=subprocess.run([str(a) for a in argv],cwd=out,env=env,stdout=log,stderr=subprocess.STDOUT,timeout=600)
        evidence['commands'].append(dict(name=name,argv=[str(a) for a in argv],exitCode=result.returncode,seconds=time.monotonic()-start));save()
        if result.returncode: raise RuntimeError(name+' failed')
        return (out/(name+'.log')).read_text(encoding='utf-8')
    for mode in ['shared','static']:
        build=out/(mode+'-build')
        command=[cmake,'-S',moved/'examples','-B',build,'-DCMAKE_PREFIX_PATH='+str(moved),'-DCMAKE_BUILD_TYPE=Release','-DKNOT_LINKAGE='+mode]
        if sys.platform=='win32': command+=['-A','x64']
        run(mode+'-configure',command)
        run(mode+'-build',[cmake,'--build',build,'--config','Release','--parallel','1'])
        binary=build/'Release' if sys.platform=='win32' else build
        suffix='.exe' if sys.platform=='win32' else ''
        c=run(mode+'-c',[binary/('c_smoke'+suffix),moved/'contracts/examples/speaker.json'])
        if json.loads(c)['state']['revision'] != 0: raise RuntimeError('C initial output mismatch')
        cpp=run(mode+'-cpp',[binary/('speaker'+suffix),moved/'contracts/examples/speaker.json'])
        if [line.strip() for line in cpp.splitlines()] != ['282800','159900']: raise RuntimeError('Speaker output mismatch: '+cpp)
    cli=moved/'bin'/('knot.exe' if sys.platform=='win32' else 'knot')
    info=json.loads(run('cli-validate',[cli,'validate',moved/'contracts/examples/speaker.json']))
    if info['productId']!='speaker-pro': raise RuntimeError('Packaged CLI model identity mismatch')
    sequence=run('cli-simulate',[cli,'simulate',moved/'contracts/examples/speaker.json','--events',moved/'contracts/examples/speaker-events.json'])
    steps=[json.loads(line) for line in sequence.splitlines()]
    if len(steps)!=7 or [v['result']['snapshot']['price']['totalMinor'] for v in steps[-2:]] != [282800,159900]: raise RuntimeError('Packaged CLI Speaker mismatch')
    evidence['passed']=True;save()
    print('Relocated shared/static C and C++ consumers passed')

if __name__=='__main__': main()
