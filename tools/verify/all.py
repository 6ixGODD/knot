"""Run native or security gates and retain every command, exit code and duration."""
import argparse
import json
import os
from pathlib import Path
import sys
sys.dont_write_bytecode=True
from common import ROOT,Runner,cmake,sha

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--out',type=Path,required=True)
    parser.add_argument('--stage',choices=['all','native','security','fuzz','memory'],default='all')
    args=parser.parse_args();runner=Runner(args.out)
    runner.env['RUSTDOCFLAGS']='-D warnings'
    evidence=dict(passed=False,stage=args.stage,lockSha256=sha(ROOT/'Cargo.lock'),sourceFiles={str(p.relative_to(ROOT)):sha(p) for base in ['crates','bindings','tools','tests','cmake','examples'] for p in (ROOT/base).rglob('*') if p.is_file() and p.suffix in ['.rs','.py','.c','.cpp','.hpp','.h','.toml','.cmake','.txt']})
    def save(): (runner.out/'evidence.json').write_text(json.dumps(evidence,indent=2)+'\n',encoding='utf-8')
    save()
    if args.stage in ['all','native']:
        runner.cargo('fmt','fmt','--all','--','--check')
        runner.cargo('clippy','clippy','--workspace','--all-targets','--locked','--','-D','warnings')
        runner.cargo('test','test','--workspace','--locked')
        runner.cargo('doc','doc','--workspace','--no-deps','--locked')
        runner.cargo('release','build','--workspace','--release','--locked')
        native=runner.out/'p0'
        runner.run('p0',[sys.executable,ROOT/'tools/p0/run.py','--out',native,'--cmake',cmake()])
        components=runner.out/'bindings'
        command=[cmake(),'-S',ROOT/'tests/bindings','-B',components,'-DHOST_JSON_DIR='+str(native/'host-json'),'-DCMAKE_BUILD_TYPE=Release']
        if sys.platform=='win32':command+=['-A','x64']
        runner.run('bindings-configure',command)
        runner.run('bindings-build',[cmake(),'--build',components,'--config','Release','--parallel','1'])
        ctest=Path(cmake()).with_name('ctest.exe' if sys.platform=='win32' else 'ctest')
        runner.run('bindings-ctest',[ctest,'--test-dir',components,'-C','Release','--output-on-failure','--output-junit',runner.out/'bindings.xml'])
        client_env=dict(runner.env)
        if os.environ.get('KNOT_GO_CC'): client_env['CC']=os.environ['KNOT_GO_CC']
        runner.run('bindings-clients',[sys.executable,ROOT/'tests/bindings/run_native_clients.py','--build-dir',components,'--config','Release'],env=client_env)
        runner.run('docs',[sys.executable,ROOT/'tests/bindings/check_docs.py','--contracts',ROOT/'contracts','--out',runner.out/'docs'])
        package=runner.out/'package'
        runner.run('package',[sys.executable,ROOT/'tools/package/build.py','--out',package])
        runner.run('cross-language',[sys.executable,ROOT/'tests/cross-language/run.py','--out',runner.out/'cross-language','--package-dir',package])
        consume_env=dict(runner.env,CMAKE=cmake())
        runner.run('consumer',[sys.executable,ROOT/'tests/package/consume.py','--package-dir',package,'--out',runner.out/'consumer'],env=consume_env)
        runner.run('benchmark',[sys.executable,ROOT/'tools/verify/benchmark.py','--out',runner.out/'benchmark'])
    if args.stage in ['all','security','fuzz']:
        runner.run('fuzz',[sys.executable,ROOT/'tools/verify/fuzz.py','--seconds-per-target','300','--out',runner.out/'fuzz'],timeout=14400)
    if args.stage in ['all','security','memory']:
        runner.run('memory',[sys.executable,ROOT/'tools/verify/memory.py','--out',runner.out/'memory'],timeout=14400)
    runner.run('diff-check',['git','diff','--check'])
    evidence['passed']=True;save()

if __name__=='__main__':main()
