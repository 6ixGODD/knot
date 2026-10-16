"""Measure the fixed standard model in a fresh release process."""
import argparse
import ctypes
import json
import math
import os
import signal
import re
from pathlib import Path
import platform
import subprocess
import sys
import time
sys.dont_write_bytecode=True
from common import ROOT,Runner,sha
sys.path.insert(0,str(ROOT/'tests/benchmark'))
from generate import model

def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--out',type=Path,required=True);args=parser.parse_args();runner=Runner(args.out)
    product=runner.out/'model.json';product.write_text(json.dumps(model()),encoding='utf-8')
    pending=dict(passed=False,modelSha256=sha(product),phase='build',exitCode=None)
    def save_pending(): (runner.out/'evidence.json').write_text(json.dumps(pending,indent=2)+'\n',encoding='utf-8')
    save_pending()
    runner.cargo('build','build','-p','knot-ffi','--example','benchmark','--release','--locked')
    binary=Path(runner.env['CARGO_TARGET_DIR'])/'release/examples'/('benchmark.exe' if sys.platform=='win32' else 'benchmark')
    command=[str(binary),str(product)]
    pending['phase']='measure';save_pending()
    peak=0; started=time.monotonic()
    launch=command if sys.platform=='win32' else ['/usr/bin/time', '-l' if sys.platform=='darwin' else '-v', *command]
    measure_env=dict(runner.env,LC_ALL='C')
    with (runner.out/'measure.log').open('wb') as stream, (runner.out/'measure-memory.log').open('wb') as errors:
        process=subprocess.Popen(launch,cwd=ROOT,env=measure_env,stdout=stream,stderr=errors,start_new_session=sys.platform!='win32')
        if sys.platform=='win32':
            class Counters(ctypes.Structure):
                _fields_=[('cb',ctypes.c_ulong),('PageFaultCount',ctypes.c_ulong)]+[(name,ctypes.c_size_t) for name in ['PeakWorkingSetSize','WorkingSetSize','QuotaPeakPagedPoolUsage','QuotaPagedPoolUsage','QuotaPeakNonPagedPoolUsage','QuotaNonPagedPoolUsage','PagefileUsage','PeakPagefileUsage']]
            get=ctypes.windll.psapi.GetProcessMemoryInfo;get.argtypes=[ctypes.c_void_p,ctypes.POINTER(Counters),ctypes.c_ulong];get.restype=ctypes.c_int
        while process.poll() is None:
            if time.monotonic()-started>600:
                pending['timedOut']=True;save_pending()
                if sys.platform=='win32': process.kill()
                else: os.killpg(process.pid,signal.SIGKILL)
                process.wait();raise RuntimeError('Benchmark timeout')
            if sys.platform=='win32':
                counters=Counters();counters.cb=ctypes.sizeof(counters)
                if get(int(process._handle),ctypes.byref(counters),counters.cb): peak=max(peak,counters.PeakWorkingSetSize)
            time.sleep(0.05)
    if sys.platform=='win32':
        counters=Counters();counters.cb=ctypes.sizeof(counters)
        if not get(int(process._handle),ctypes.byref(counters),counters.cb): raise RuntimeError('Final process peak memory query failed')
        peak=max(peak,counters.PeakWorkingSetSize)
    else:
        report=(runner.out/'measure-memory.log').read_text(encoding='utf-8')
        pattern=r'(\d+)\s+maximum resident set size' if sys.platform=='darwin' else r'Maximum resident set size \(kbytes\):\s*(\d+)'
        found=re.search(pattern,report)
        if not found: raise RuntimeError('System time did not report peak resident memory')
        peak=int(found.group(1))*(1 if sys.platform=='darwin' else 1024)
    pending.update(argv=launch,exitCode=process.returncode,seconds=time.monotonic()-started,peakBytes=peak);save_pending()
    if process.returncode!=0 or peak<=0: raise RuntimeError('Benchmark or memory measurement failed')
    samples=json.loads((runner.out/'measure.log').read_text(encoding='utf-8'))
    summary={}
    for name,values in samples.items():
        if not name.endswith('Ms'): continue
        ordered=sorted(values)
        summary[name]=dict(samples=len(values),p50=ordered[math.ceil(len(values)*0.5)-1],p95=ordered[math.ceil(len(values)*0.95)-1])
    evidence=dict(passed=True,argv=launch,exitCode=process.returncode,seconds=time.monotonic()-started,platform=platform.platform(),peakBytes=peak,memoryMethod='OS peak working set' if sys.platform=='win32' else 'OS maximum resident set size via /usr/bin/time',rustc=runner.run('rustc',['rustc','-vV']),lockSha256=sha(ROOT/'Cargo.lock'),modelSha256=sha(product),scale=dict(questions=100,conditions=500,visualProperties=100,priceComponents=100),measurements=summary)
    if sys.platform=='win32': cpu=runner.run('cpu',['powershell','-NoProfile','-Command','(Get-CimInstance Win32_Processor).Name']).strip()
    elif sys.platform=='darwin': cpu=runner.run('cpu',['sysctl','-n','machdep.cpu.brand_string']).strip()
    else: cpu=next((line.split(':',1)[1].strip() for line in Path('/proc/cpuinfo').read_text().splitlines() if line.startswith('model name')),platform.machine())
    evidence.update(cpu=cpu,configuration='Release',backend='zen-engine 2.1.3',parallel=samples['parallel'],rustAllocationsFor100Evaluations=samples['rustAllocationsFor100Evaluations'],rustAllocatedBytesFor100Evaluations=samples['rustAllocatedBytesFor100Evaluations'],allocationScope='Rust GlobalAlloc only; includes measurement bookkeeping, excludes native C allocations')
    (runner.out/'evidence.json').write_text(json.dumps(evidence,indent=2)+'\n',encoding='utf-8');print(json.dumps(summary))

if __name__=='__main__':main()
