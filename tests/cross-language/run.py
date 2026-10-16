"""Compare frozen cases through Rust, both native linkages, C++ DTOs and Python."""
import argparse
import ctypes as C
import json
import os
from pathlib import Path
import sys
sys.dont_write_bytecode=True
ROOT=Path(__file__).resolve().parents[2]
sys.path.insert(0,str(ROOT/'tools/verify'))
from common import Runner,cmake
sys.path.insert(0,str(ROOT/'bindings/python'))
import knot

def normal(value, metadata_only=False):
    value=json.loads(json.dumps(value))
    if isinstance(value,dict) and 'error' in value:
        error=value['error']
        error.pop('message',None)
        if metadata_only: error.pop('details',None)
    return value

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--out',type=Path,required=True)
    parser.add_argument('--package-dir',type=Path)
    args=parser.parse_args(); runner=Runner(args.out)
    package=args.package_dir or runner.out.parent/'package'
    receipt=json.loads((package/'package.json').read_text(encoding='utf-8'))
    sdk=(package/receipt['packageDirectory']).resolve()
    runner.cargo('rust-reference','build','-p','knot-ffi','--example','reference','--release','--locked')
    build=runner.out/'native'
    command=[cmake(),'-S',ROOT/'tests/cross-language','-B',build,'-DCMAKE_PREFIX_PATH='+str(sdk),'-DHOST_JSON_DIR='+str(ROOT/'vendor/nlohmann'),'-DCMAKE_BUILD_TYPE=Release']
    if sys.platform=='win32': command+=['-A','x64']
    runner.run('configure',command)
    runner.run('build',[cmake(),'--build',build,'--config','Release','--parallel','1'])
    env=dict(runner.env)
    env['PATH']=str(sdk/'bin')+os.pathsep+env.get('PATH','')
    env['LD_LIBRARY_PATH']=str(sdk/'lib');env['DYLD_LIBRARY_PATH']=str(sdk/'lib')
    library=sdk/('bin/knot.dll' if sys.platform=='win32' else ('lib/libknot.dylib' if sys.platform=='darwin' else 'lib/libknot.so'))
    native=build/'Release' if sys.platform=='win32' else build
    suffix='.exe' if sys.platform=='win32' else ''
    reference=Path(runner.env['CARGO_TARGET_DIR'])/'release/examples'/('reference'+suffix)
    root=ROOT/'contracts/golden'
    cases=json.loads((root/'index.json').read_text(encoding='utf-8'))['cases']
    evidence=dict(passed=False,cases=[],cppThreads=False,go=False)
    def save(): (runner.out/'evidence.json').write_text(json.dumps(evidence,indent=2)+'\n',encoding='utf-8')
    save()
    for case in cases:
        name=case['id']; directory=root/name
        arguments=[case['operation'],directory/'model.json',directory/'initial-state.json',directory/'event.json']
        expected=json.loads((directory/'expected-result.json').read_text(encoding='utf-8'))
        wanted=dict(status=case['expectedStatus'],result=normal(expected))
        consumers={}
        cpp_interfaces={}
        for language, binary in [('rust',reference),('c_shared',native/('c_shared'+suffix)),('c_static',native/('c_static'+suffix)),('cpp_shared',native/('cpp_shared'+suffix)),('cpp_static',native/('cpp_static'+suffix))]:
            actual=json.loads(runner.run(name+'-'+language,[binary,*arguments],env=env))
            metadata_only=False
            if language.startswith('cpp'):
                interface=actual.get('interface','dto')
                if interface not in ('dto','raw-abi'): raise RuntimeError('Unknown C++ interface')
                cpp_interfaces[language]=interface
                metadata_only=interface=='dto'
            if actual['status'] != wanted['status'] or normal(actual['result'],metadata_only) != normal(expected,metadata_only):
                (runner.out/(name+'-'+language+'-mismatch.json')).write_text(json.dumps(dict(actual=actual,expected=wanted),indent=2),encoding='utf-8')
                raise RuntimeError(f'{name}: {language} mismatch')
            consumers[language]=True
        # Raw wire bytes pass through the shipped Python ABI signatures and ownership decoder.
        try:
            with knot.Model((directory/'model.json').read_bytes(),library=library) as model:
                op=case['operation']
                if op=='model_create': result=model.info
                elif op=='initial': result=model._consume(model._lib.knot_initial(model._handle))
                else:
                    state=(directory/'initial-state.json').read_bytes();sb=knot._input(state)
                    if op=='evaluate': raw=model._lib.knot_evaluate(model._handle,sb,len(state))
                    elif op=='apply':
                        event=(directory/'event.json').read_bytes();eb=knot._input(event)
                        raw=model._lib.knot_apply(model._handle,sb,len(state),eb,len(event))
                    else: raise RuntimeError('Unsupported operation '+op)
                    result=model._consume(raw)
                actual=dict(status=0,result=result)
        except knot.Error as error:
            actual=dict(status=error.status,result=dict(schemaVersion='1.0',error=dict(code=error.code,path=error.path,ruleIds=error.rule_ids)))
        if actual['status'] != wanted['status'] or normal(actual['result'],True) != normal(expected,True): raise RuntimeError(name+': Python mismatch')
        consumers['python']=True
        evidence['cases'].append(dict(id=name,consumers=consumers,cppInterfaces=cpp_interfaces));save()
    directory=root/'T22'
    for mode in ['shared','static']:
        runner.run('cpp-threads-'+mode,[native/('cpp_'+mode+suffix),'threads',directory/'model.json',directory/'initial-state.json',directory/'event.json'],env=env)
    evidence['cppThreads']=True
    # T23 repetition is in one process with the original descriptor freed twice each time.
    with knot.Model((root/'T23/model.json').read_bytes(),library=library) as model:
        for _ in range(1000):
            raw=model._lib.knot_initial(model._handle)
            if raw.status: raise RuntimeError('T23 failed')
            model._lib.knot_result_free(C.byref(raw));model._lib.knot_result_free(C.byref(raw))
            if raw.json.data or raw.json.len: raise RuntimeError('Original descriptor not cleared')
    # Public typed Python path executes the full Speaker sequence and restore.
    with knot.Model((ROOT/'contracts/examples/speaker.json').read_bytes(),library=library) as model:
        result=model.initial();totals=[]
        for event in json.loads((ROOT/'contracts/examples/speaker-events.json').read_text(encoding='utf-8')):
            result=model.apply(result.state,event);totals.append(result.snapshot['price']['totalMinor'])
        if totals[-2:] != [282800,159900]: raise RuntimeError('Python Speaker totals')
        if model.evaluate(result.state).snapshot != result.snapshot: raise RuntimeError('Python restore')
    goenv=dict(env,CGO_ENABLED='1',GOCACHE=str(runner.out/'go-cache'),GOTMPDIR=str(runner.out/'go-tmp'))
    if os.environ.get('KNOT_GO_CC'): goenv['CC']=os.environ['KNOT_GO_CC']
    Path(goenv['GOTMPDIR']).mkdir(exist_ok=True)
    goenv['CGO_CFLAGS']='-I "'+str(sdk/'include')+'"'
    linkdir=sdk/'lib/shared' if sys.platform=='win32' else sdk/'lib'
    goenv['CGO_LDFLAGS']='-L "'+str(linkdir)+'" -lknot'
    if sys.platform!='win32': goenv['CGO_LDFLAGS']+=' "-Wl,-rpath,'+str(linkdir)+'"'
    go=runner.run('go-smoke',['go','run',ROOT/'examples/go/smoke.go',ROOT/'contracts/examples/speaker.json',ROOT/'contracts/examples/speaker-events.json'],cwd=ROOT/'bindings/go',env=goenv)
    outputs=[json.loads(line) for line in go.splitlines() if line.startswith('{')]
    if [v['snapshot']['price']['totalMinor'] for v in outputs[-2:]] != [282800,159900]: raise RuntimeError('Go Speaker output')
    evidence['go']=True;evidence['passed']=True;save()

if __name__=='__main__':main()
