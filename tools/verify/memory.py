"""Run actual buffer ownership under Miri and the production FFI under ASan on Linux CI."""
import argparse
import json
import os
from pathlib import Path
import sys
sys.dont_write_bytecode=True
from common import ROOT,Runner

def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--out',type=Path,required=True);args=parser.parse_args();runner=Runner(args.out)
    evidence=dict(passed=False,coverage=['actual ABI boxed-buffer allocate/release under Miri','production FFI boundary and real ZEN/Core under ASan'],exclusions=['Miri cannot execute ZEN native QuickJS/AWS-LC dependencies; covered through compiled ASan tests instead'])
    (runner.out/'evidence.json').write_text(json.dumps(evidence,indent=2),encoding='utf-8')
    if sys.platform!='linux': raise RuntimeError('Memory gate requires the configured Linux CI job; not passed on this host')
    nightly=os.environ.get('KNOT_NIGHTLY','nightly-2026-08-29')
    version=runner.run('nightly',['rustc','+'+nightly,'-vV'])
    if 'commit-hash: 17fd5b8a37b6667b6cc137f3cc35f09759768a3b' not in version: raise RuntimeError('Unexpected nightly compiler')
    env=dict(runner.env,CARGO_TARGET_DIR=str(runner.out/'miri-target'))
    runner.run('miri',['cargo','+'+nightly,'miri','test','--manifest-path',ROOT/'tests/ffi/memory/Cargo.toml','--locked'],env=env)
    # Cargo clears target rustflags for host build dependencies even when the
    # triples coincide. Global CFLAGS would still instrument QuickJS linked
    # into a proc macro, which the unsanitized rustc cannot load.
    for name in ('asan-cc','asan-cxx'):
        wrapper=runner.out/name
        wrapper.write_text((ROOT/'tools/verify/asan_compiler.py').read_text(encoding='utf-8'),encoding='utf-8',newline='\n')
        wrapper.chmod(0o755)
    env=dict(runner.env,CARGO_TARGET_DIR=str(runner.out/'asan-target'),RUSTFLAGS='-Zsanitizer=address',RUSTDOCFLAGS='-Zsanitizer=address',ASAN_OPTIONS='detect_leaks=1:halt_on_error=1',CC=str(runner.out/'asan-cc'),CXX=str(runner.out/'asan-cxx'))
    runner.run('asan',['cargo','+'+nightly,'test','-Zbuild-std','--target','x86_64-unknown-linux-gnu','-p','knot-ffi','--test','ffi_cases','--locked'],env=env)
    evidence['passed']=True;evidence['rustc']=version
    (runner.out/'evidence.json').write_text(json.dumps(evidence,indent=2)+'\n',encoding='utf-8')

if __name__=='__main__':main()
