#!/usr/bin/env python3
"""Run only the task-owned two-case head diagnostic under fixed resource bounds."""
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import threading
import time

ROOT = Path(os.environ['CLEF_WORKTREE'])
BASE = Path(os.environ['CLEF_DIAGNOSTIC_SCRATCH'])
SCRIPT = Path(__file__).with_name('head_capture.py')
PYTHON = Path(os.environ['CLEF_REFERENCE_PYTHON'])
OUTPUT = BASE / 'reference27b/head-inputs-014-034-parameter-views'
sys.path.insert(0, str(ROOT / 'crates/psionic/fixtures/clef/tools'))
from mac_quiet_bench import OwnedProcessGuard, available_memory, file_sha, stop_owned_group
from image_parity import write
GIB = 1024**3


def run(mode):
    if mode not in ('capture', 'evaluate', 'copy-probe', 'head-probe'):
        raise ValueError('choose capture, evaluate, copy-probe, or head-probe')
    leases = os.environ.get('OPENAGENTS_LEASES','').split(',')
    if 'memory' not in leases or (mode != 'evaluate' and 'gpu' not in leases):
        raise ValueError('required memory/GPU lease missing')
    if mode == 'evaluate' and 'gpu' in leases:
        raise ValueError('offline head evaluation must run after releasing the GPU lease')
    cap, start, deadline = (72,84,900) if mode == 'capture' else (4,16,300 if mode == 'evaluate' else 120)
    receipt = OUTPUT.with_name(OUTPUT.name + '-' + mode + '-guard.json')
    if receipt.exists():
        raise ValueError('guard receipt already exists; preserve the previous run')
    environment = dict(os.environ)
    bounds = {name:'2' for name in ('RAYON_NUM_THREADS','OMP_NUM_THREADS','OPENBLAS_NUM_THREADS',
                                    'MKL_NUM_THREADS','VECLIB_MAXIMUM_THREADS')}
    bounds.update(PYTORCH_ENABLE_MPS_FALLBACK='0', HF_HUB_OFFLINE='1', TRANSFORMERS_OFFLINE='1',
                  PYTHONDONTWRITEBYTECODE='1')
    environment.update(bounds)
    command = [str(PYTHON), str(SCRIPT), mode, '--output', str(OUTPUT)]
    script = SCRIPT
    if mode == 'evaluate':
        command.append('--allow-failed-capture')
    elif mode in ('copy-probe','head-probe'):
        script = SCRIPT.with_name('mps_gather_probe.py' if mode == 'copy-probe' else 'mps_head_parameter_views_probe.py')
        command = [str(PYTHON), str(script), str(OUTPUT.with_name(OUTPUT.name+'-'+mode+'.json'))]
    record = {'kind':'owned_official_head_diagnostic','mode':mode,'status':'preflight',
              'command':command,'script_sha256':file_sha(script),'guard_sha256':file_sha(Path(__file__)),
              'helper_sha256':file_sha(Path(__file__).with_name('immutable_probe_finite.py')),
              'effective_environment':bounds,'max_owned_group_gib':cap,'minimum_start_available_gib':start,
              'minimum_available_gib':12,'deadline_seconds':deadline,'conditions':'shared machine',
              'before':available_memory(), 'load_average_before':os.getloadavg()}
    write(receipt,record)
    if record['before']['available_memory_bytes'] < start * GIB:
        record.update(status='refused',reason='startup_available_memory')
        write(receipt,record)
        print(json.dumps(record),flush=True)
        return 1
    process = guard = timer = None
    expired = threading.Event()
    old_signals = {}
    began = time.monotonic()
    def interrupt(signum,_frame):
        raise KeyboardInterrupt(f'signal {signum}')
    try:
        for signum in [signal.SIGINT,signal.SIGTERM]:
            old_signals[signum] = signal.signal(signum,interrupt)
        with OUTPUT.with_name(OUTPUT.name+'-'+mode+'.log').open('wb') as log:
            process = subprocess.Popen(command,env=environment,stdin=subprocess.DEVNULL,
                                       stdout=log,stderr=log,start_new_session=True)
            guard = OwnedProcessGuard(process,cap*GIB,OUTPUT.with_name(OUTPUT.name+'-'+mode+'-memory.jsonl'),
                                      minimum_available_memory_bytes=12*GIB)
            record.update(status='running',owned_pid=process.pid)
            write(receipt,record)
            print(json.dumps({'model_active':True,'mode':mode,'owned_pid':process.pid}),flush=True)
            def expire():
                expired.set()
                guard.stop_process(grace=1)
            timer=threading.Timer(deadline,expire)
            timer.start()
            while process.poll() is None:
                guard.guard()
                time.sleep(.2)
            record['child_exit_code']=process.returncode
            guard.guard()
            record['status']='complete' if process.returncode==0 else 'failed'
    except BaseException as error:
        record.update(status='failed',error_type=type(error).__name__,error=str(error))
    finally:
        if timer is not None:
            timer.cancel()
            timer.join()
        try:
            if guard is not None:
                try:
                    guard.close()
                finally:
                    record['memory']=guard.receipt()
                if guard.failure:
                    record['status']='failed'
            elif process is not None:
                stop_owned_group(process)
        except BaseException as error:
            record.update(status='failed',cleanup_error=str(error))
        for signum,prior in old_signals.items():
            signal.signal(signum,prior)
        if expired.is_set():
            record.update(status='failed',error='deadline exceeded')
        record['elapsed_seconds']=time.monotonic()-began
        record['load_average_after']=os.getloadavg()
        write(receipt,record)
        print(json.dumps({'model_active':False,'mode':mode,'status':record['status'],
                          'owned_group_stopped':record.get('memory',{}).get('owned_group_stopped')}),flush=True)
    return 0 if record['status']=='complete' else 1

if __name__=='__main__':
    raise SystemExit(run(sys.argv[1]))
