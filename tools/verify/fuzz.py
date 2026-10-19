"""Run all five native libFuzzer targets with fixed seeds and retained corpora."""
import argparse
import json
import os
from pathlib import Path
import re
import shutil
import sys
sys.dont_write_bytecode=True
from common import ROOT,Runner,sha

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--out',type=Path,required=True)
    parser.add_argument('--seconds-per-target',type=int,default=300)
    args=parser.parse_args()
    if args.seconds_per_target<60: parser.error('At least 60 seconds per target is required')
    runner=Runner(args.out)
    nightly=os.environ.get('KNOT_NIGHTLY','nightly-2026-08-29')
    version=runner.run('fuzz-version',['cargo','+'+nightly,'fuzz','--version'])
    # rustup diagnostics share the captured log but are not the tool version.
    versions=re.findall(r'^cargo-fuzz (\S+)\s*$',version,re.M)
    if versions!=['0.13.2']: raise RuntimeError('Install cargo-fuzz 0.13.2 with --locked')
    rustc=runner.run('nightly',['rustc','+'+nightly,'-vV'])
    if 'commit-hash: 17fd5b8a37b6667b6cc137f3cc35f09759768a3b' not in rustc: raise RuntimeError('Unexpected nightly compiler')
    # cargo-fuzz creates its own artifacts directory under the fuzz manifest.
    # Stage only the manifest/lock under outputs and reference the immutable sources.
    project=runner.out/'project';project.mkdir(exist_ok=True)
    manifest=(ROOT/'tests/fuzz/Cargo.toml').read_text(encoding='utf-8')
    manifest=manifest.replace('../../crates/',(ROOT/'crates').as_posix()+'/')
    manifest=manifest.replace('path = "fuzz_targets/','path = "'+(ROOT/'tests/fuzz/fuzz_targets').as_posix()+'/')
    manifest+='\n[lib]\npath = "'+(ROOT/'tests/fuzz/src/lib.rs').as_posix()+'"\n'
    (project/'Cargo.toml').write_text(manifest,encoding='utf-8')
    shutil.copy2(ROOT/'tests/fuzz/Cargo.lock',project/'Cargo.lock')
    runner.run('fetch',['cargo','+'+nightly,'fetch','--manifest-path',project/'Cargo.toml','--locked'])
    env=dict(runner.env,CARGO_TARGET_DIR=str(runner.out/'cargo-target'),CARGO_NET_OFFLINE='true')
    evidence=dict(passed=False,secondsPerTarget=args.seconds_per_target,seed=20261014,tool='cargo-fuzz '+versions[0],rustc=rustc,lockSha256=sha(ROOT/'tests/fuzz/Cargo.lock'),targets=[])
    def save(): (runner.out/'evidence.json').write_text(json.dumps(evidence,indent=2)+'\n',encoding='utf-8')
    save()
    for target in ['product','state','event','ast','operations']:
        corpus=runner.out/'corpus'/target;artifacts=runner.out/'artifacts'/target
        corpus.mkdir(parents=True,exist_ok=True);artifacts.mkdir(parents=True,exist_ok=True)
        if target in ('product','state','event'):
            filename={'product':'model.json','state':'initial-state.json','event':'event.json'}[target]
            for path in sorted((ROOT/'contracts/golden').glob('*/'+filename)):
                (corpus/path.parent.name).write_bytes(path.read_bytes())
        elif target=='ast':
            (corpus/'eq').write_text('{"field":"speaker_type","op":"eq","value":"active"}',encoding='utf-8')
        else: (corpus/'speaker').write_bytes((ROOT/'contracts/examples/speaker-events.json').read_bytes())
        argv=['cargo','+'+nightly,'fuzz','run','--fuzz-dir',str(project),target,str(corpus),'--','-seed=20261014',f'-max_total_time={args.seconds_per_target}','-timeout=10','-rss_limit_mb=3072','-max_len=4194304','-artifact_prefix='+str(artifacts)+os.sep]
        log=runner.run(target,argv,env=env,timeout=7200+args.seconds_per_target)
        if sha(project/'Cargo.lock') != evidence['lockSha256']: raise RuntimeError('Fuzz lock changed during execution')
        executions=re.findall(r'Done (\d+) runs in (\d+) second',log)
        if not executions or int(executions[-1][0])<1 or int(executions[-1][1])<args.seconds_per_target: raise RuntimeError(target+' did not run its full fuzz window')
        if any(artifacts.iterdir()): raise RuntimeError(target+' produced a crash artifact')
        evidence['targets'].append(dict(name=target,runs=int(executions[-1][0]),seconds=int(executions[-1][1]),corpusFiles=len(list(corpus.iterdir()))));save()
    evidence['passed']=True;save()

if __name__=='__main__':main()
