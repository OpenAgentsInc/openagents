#!/usr/bin/env python3
"""Bound synthetic CPU quantizer checks without model or GPU work."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import sys
import time

HERE=Path(__file__).resolve().parent
ROOT=Path('<OPENAGENTS_WORKTREE>')
PYTHON=Path('<REFERENCE_ENV>/bin/python')
sys.path.insert(0,str(ROOT/'crates/psionic/fixtures/clef/tools'))
from mac_quiet_bench import OwnedProcessGuard,available_memory,stop_owned_group


def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--run-dir',type=Path,required=True);a=p.parse_args()
    resources=set(os.environ.get('OPENAGENTS_LEASES','').split(','))
    if not {'memory','disk'}<=resources or {'gpu','quiet','build'}&resources:raise ValueError('requires CPU memory/disk leases only')
    run=a.run_dir.resolve()
    if HERE not in run.parents:raise ValueError('run directory must remain in this scratch subdirectory')
    run.mkdir(parents=True,exist_ok=False)
    command=[str(PYTHON),str(HERE/'check_weighted.py'),'--output',str(run/'checks.json')]
    sources={str(path):hashlib.sha256(path.read_bytes()).hexdigest() for path in HERE.glob('*.py')}
    for name in ['mac_quiet_bench.py','mac_memory.py']:
        path=ROOT/'crates/psionic/fixtures/clef/tools'/name;sources[str(path)]=hashlib.sha256(path.read_bytes()).hexdigest()
    record={'status':'preflight','command':command,'cap_gib':2,'start_available_gib':14,'floor_available_gib':12,'deadline_seconds':60,'free_disk_floor_gib':25,'sources':sources,'gpu_executed':False,'model_executed':False}
    def save():(run/'guard.json').write_text(json.dumps(record,indent=2)+'\n')
    save();environment=dict(os.environ)
    for key in list(environment):
        if key.startswith('DYLD_') or key in {'LD_PRELOAD','LD_LIBRARY_PATH'}:del environment[key]
    environment.update(CLEF_ACTIVATION_GUARDED='1',PYTHONDONTWRITEBYTECODE='1',OMP_NUM_THREADS='1',OPENBLAS_NUM_THREADS='1',MKL_NUM_THREADS='1',VECLIB_MAXIMUM_THREADS='1')
    def interrupt(signum,frame):raise KeyboardInterrupt(f'signal {signum}')
    signal.signal(signal.SIGINT,interrupt);signal.signal(signal.SIGTERM,interrupt)
    child=monitor=None;began=time.monotonic()
    try:
        record['before']=available_memory()
        if record['before']['available_memory_bytes']<14*1024**3 or shutil.disk_usage(run).free<27*1024**3:raise ValueError('preflight resources insufficient')
        with (run/'stdout.log').open('xb') as out:
            child=subprocess.Popen(command,env=environment,stdin=subprocess.DEVNULL,stdout=out,stderr=out,start_new_session=True)
            monitor=OwnedProcessGuard(child,2*1024**3,run/'memory.jsonl',minimum_available_memory_bytes=12*1024**3)
            while child.poll() is None:
                monitor.guard()
                if time.monotonic()-began>60:raise TimeoutError('synthetic check deadline exceeded')
                if shutil.disk_usage(run).free<25*1024**3:raise ValueError('disk floor crossed')
                time.sleep(.2)
            monitor.guard();record['exit_code']=child.returncode
            if child.returncode!=0 or json.loads((run/'checks.json').read_text())['status']!='passed':raise ValueError('synthetic checks failed')
            for path,digest in sources.items():
                if hashlib.sha256(Path(path).read_bytes()).hexdigest()!=digest:raise ValueError('source changed during check')
            record.update(status='complete',source_hashes_rechecked=True)
    except BaseException as error:record.update(status='failed',error_type=type(error).__name__,error=str(error))
    finally:
        try:
            if monitor is not None:
                try:monitor.close()
                finally:record['memory']=monitor.receipt()
                if monitor.failure:record['status']='failed'
            elif child is not None:stop_owned_group(child)
        except BaseException as error:record.update(status='failed',cleanup_error=str(error))
        record['seconds']=time.monotonic()-began;save()
    print(json.dumps({'status':record['status'],'owned_group_stopped':record.get('memory',{}).get('owned_group_stopped')}))
    return 0 if record['status']=='complete' else 1


if __name__=='__main__':raise SystemExit(main())
