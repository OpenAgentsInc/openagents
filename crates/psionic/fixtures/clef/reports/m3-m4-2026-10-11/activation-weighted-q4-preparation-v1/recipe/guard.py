#!/usr/bin/env python3
"""Guard only this experiment's CPU preparation or explicitly granted model run."""
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
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('mode',choices=['cpu-tests','validate','pilot','full'])
    parser.add_argument('--corpus',type=Path);parser.add_argument('--encoded',type=Path);parser.add_argument('--run-dir',type=Path,required=True)
    args=parser.parse_args();gpu=args.mode in ('pilot','full')
    leases=set(os.environ.get('OPENAGENTS_LEASES','').split(','));needed={'memory','disk'}|({'gpu'} if gpu else set())
    if not needed<=leases or 'quiet' in leases or (not gpu and 'gpu' in leases):raise ValueError('incorrect resources; quiet leases are forbidden')
    run=args.run_dir.resolve()
    if HERE not in run.parents:raise ValueError('output must stay in this experiment scratch directory')
    run.mkdir(parents=True,exist_ok=False)
    cap=72 if gpu else 2;floor=12;deadline={'cpu-tests':60,'validate':60,'pilot':300,'full':1800}[args.mode]
    if args.mode=='cpu-tests':command=[str(PYTHON),str(HERE/'test_moments.py')]
    elif args.mode=='validate':
        if args.corpus is None:raise ValueError('corpus path required')
        command=[str(PYTHON),str(HERE/'validate_corpus.py'),'--corpus',str(args.corpus.resolve()),'--output',str(run/'encoded.json')]
    else:
        if args.corpus is None or args.encoded is None:raise ValueError('corpus and encoded paths required')
        command=[str(PYTHON),str(HERE/'collect.py'),'--corpus',str(args.corpus.resolve()),'--encoded',str(args.encoded.resolve()),'--mode',args.mode,'--output-dir',str(run/'results')]
    environment=dict(os.environ)
    for key in list(environment):
        if key.startswith('DYLD_') or key in {'LD_PRELOAD','LD_LIBRARY_PATH'}:del environment[key]
    environment.update(CLEF_ACTIVATION_GUARDED='1',PYTHONDONTWRITEBYTECODE='1',OMP_NUM_THREADS='2',OPENBLAS_NUM_THREADS='2',MKL_NUM_THREADS='2',VECLIB_MAXIMUM_THREADS='2',RAYON_NUM_THREADS='2',PYTORCH_ENABLE_MPS_FALLBACK='0',HF_HUB_OFFLINE='1',TRANSFORMERS_OFFLINE='1',CUDA_VISIBLE_DEVICES='')
    record={'schema':'clef.activation-owned-guard.v1','status':'preflight','mode':args.mode,'command':command,
            'max_group_gib':cap,'minimum_start_gib':cap+floor,'minimum_available_gib':floor,'deadline_seconds':deadline,
            'disk_floor_gib':25,'leases':sorted(leases),'source_sha256':{p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in HERE.glob('*.py')},
            'guard_dependency_sha256':{name:hashlib.sha256((ROOT/'crates/psionic/fixtures/clef/tools'/name).read_bytes()).hexdigest() for name in ['mac_quiet_bench.py','mac_memory.py']}}
    def save():(run/'guard.json').write_text(json.dumps(record,indent=2)+'\n')
    save();child=monitor=None;began=time.monotonic()
    def interrupt(signum,frame):raise KeyboardInterrupt(f'signal {signum}')
    signal.signal(signal.SIGINT,interrupt);signal.signal(signal.SIGTERM,interrupt)
    try:
        record['before']=available_memory()
        if record['before']['available_memory_bytes']<(cap+floor)*1024**3:raise ValueError('insufficient starting available memory')
        if shutil.disk_usage(run).free<27*1024**3:raise ValueError('insufficient starting free disk')
        with (run/'stdout.log').open('xb') as out:
            child=subprocess.Popen(command,stdin=subprocess.DEVNULL,stdout=out,stderr=out,env=environment,start_new_session=True)
            monitor=OwnedProcessGuard(child,cap*1024**3,run/'memory.jsonl',minimum_available_memory_bytes=floor*1024**3)
            record.update(status='running',owned_pid=child.pid);save()
            while child.poll() is None:
                monitor.guard()
                if time.monotonic()-began>deadline:raise TimeoutError('experiment deadline exceeded')
                if shutil.disk_usage(run).free<25*1024**3:raise ValueError('free disk floor crossed')
                time.sleep(.2)
            monitor.guard();record['exit_code']=child.returncode
            if child.returncode!=0:raise ValueError('owned child failed; inspect stdout.log')
            if gpu:
                report=json.loads((run/'results/receipt.json').read_text())
                if report.get('status')!='complete' or report.get('completed') is not True:raise ValueError('incomplete model collection')
            elif args.mode=='validate':
                if json.loads((run/'encoded.json').read_text()).get('status')!='prepared_not_model_executed':raise ValueError('incomplete corpus preparation')
            for name,expected in record['source_sha256'].items():
                if hashlib.sha256((HERE/name).read_bytes()).hexdigest()!=expected:raise ValueError('experiment source changed during run: '+name)
            for name,expected in record['guard_dependency_sha256'].items():
                if hashlib.sha256((ROOT/'crates/psionic/fixtures/clef/tools'/name).read_bytes()).hexdigest()!=expected:raise ValueError('guard dependency changed during run: '+name)
            record['source_hashes_rechecked_after_run']=True
            record['status']='complete'
    except BaseException as error:
        record.update(status='failed',error_type=type(error).__name__,error=str(error))
    finally:
        try:
            if monitor is not None:
                try:monitor.close()
                finally:record['memory']=monitor.receipt()
                if monitor.failure:record['status']='failed'
            elif child is not None:stop_owned_group(child)
        except BaseException as error:record.update(status='failed',cleanup_error=str(error))
        record['elapsed_seconds']=time.monotonic()-began;save()
    print(json.dumps({'status':record['status'],'mode':args.mode,'owned_group_stopped':record.get('memory',{}).get('owned_group_stopped')}))
    return 0 if record['status']=='complete' else 1


if __name__=='__main__':raise SystemExit(main())
