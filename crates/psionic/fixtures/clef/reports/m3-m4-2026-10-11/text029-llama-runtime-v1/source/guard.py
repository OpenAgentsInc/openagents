"""Own and bound one CPU-only build, capture, or saved-row replay."""
import argparse
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import sys
import time
from common import BASE, CPU, ENCODED, GGUF, HERE, ISOLATION, PYTHON, ROOT, RUN
from common import clean_environment, frozen_input, require, sha, state, upstream_identity, write

sys.path.insert(0,str(ROOT / 'crates/psionic/fixtures/clef/tools'))
from mac_quiet_bench import OwnedProcessGuard, available_memory, stop_owned_group

LIMITS = {
    'build': {'group_gib':8,'start_gib':20,'available_floor_gib':12,'deadline_seconds':600,'output_gib':2},
    'capture': {'group_gib':32,'start_gib':44,'available_floor_gib':12,'deadline_seconds':600,'output_gib':2},
    'replay': {'group_gib':4,'start_gib':16,'available_floor_gib':12,'deadline_seconds':120,'output_gib':2},
}

def output_bytes(directory):
    total=0
    for path in directory.rglob('*'):
        try:
            if path.is_file(): total += path.stat().st_size
        except FileNotFoundError:
            # The compiler can remove a temporary file between listing and stat.
            continue
    return total

def main():
    p=argparse.ArgumentParser()
    p.add_argument('--mode', choices=LIMITS, required=True)
    p.add_argument('--run-dir', type=Path, required=True)
    p.add_argument('--build-receipt', type=Path)
    p.add_argument('--capture-receipt', type=Path)
    a=p.parse_args()
    leases=set(os.environ.get('OPENAGENTS_LEASES','').split(','))
    needed={'memory','disk'} | ({'build'} if a.mode == 'build' else set())
    require(needed <= leases and not {'gpu','quiet'} & leases and (a.mode == 'build' or 'build' not in leases), 'CPU-only resource scope differs')
    require((a.build_receipt is not None) == (a.mode == 'capture') and (a.capture_receipt is not None) == (a.mode == 'replay'), 'mode receipt arguments differ')
    out=a.run_dir.resolve(); require(HERE in out.parents and not out.exists(), 'preserve previous output'); out.mkdir(parents=True)
    limits={**LIMITS[a.mode], 'threads':2, 'disk_floor_gib':25, 'gpu_executed':False}
    descriptor=HERE / 'source-check.json'
    frozen=json.loads(descriptor.read_text())
    require(frozen['status'] == 'source-only' and frozen['input']['record'] == frozen_input()['record'], 'frozen descriptor differs')
    files={str(descriptor):sha(descriptor)}
    for path,digest in frozen['sources'].items():
        require(sha(path) == digest, 'frozen source/input changed: '+path); files[path]=digest
    require(upstream_identity() == frozen['upstream'], 'upstream checkout changed')
    inputs=frozen_input()
    command=[str(PYTHON),str(HERE / (a.mode + '.py')),'--output-dir',str(out / 'results')]
    if a.mode == 'capture':
        a.build_receipt=a.build_receipt.resolve()
        require(HERE in a.build_receipt.parents, 'unrelated build receipt')
        built=json.loads(a.build_receipt.read_text())
        prior_guard=a.build_receipt.parent.parent / 'guard.json'
        proof=json.loads(prior_guard.read_text())
        require(proof['status']=='complete' and proof['mode']=='build' and proof['memory']['owned_group_stopped']
                and proof['result_sha256']==sha(a.build_receipt) and built['status']=='complete', 'build proof incomplete')
        require(proof['sources'].get(str(descriptor))==sha(descriptor)
                and built['driver_sha256']==sha(HERE/'driver.cpp'), 'build used a different frozen source snapshot')
        binary=Path(built['binary']).resolve()
        require(HERE in binary.parents and sha(binary)==built['binary_sha256'], 'built binary differs')
        for f in [a.build_receipt,prior_guard,binary]: files[str(f)]=sha(f)
        command += ['--build-receipt',str(a.build_receipt)]
    if a.mode == 'replay':
        a.capture_receipt=a.capture_receipt.resolve()
        require(HERE in a.capture_receipt.parents, 'unrelated capture receipt')
        captured=json.loads(a.capture_receipt.read_text())
        prior_guard=a.capture_receipt.parent.parent / 'guard.json'
        proof=json.loads(prior_guard.read_text())
        require(proof['status']=='complete' and proof['mode']=='capture' and proof['memory']['owned_group_stopped']
                and proof['result_sha256']==sha(a.capture_receipt) and captured['completed'], 'capture proof incomplete')
        require(proof['sources'].get(str(descriptor))==sha(descriptor), 'capture used a different frozen source snapshot')
        for f in [a.capture_receipt,prior_guard]: files[str(f)]=sha(f)
        for row in captured['files']:
            f=a.capture_receipt.parent / 'rows' / row['name']
            require(sha(f)==row['sha256'], 'new saved capture changed');files[str(f)]=row['sha256']
        previous=json.loads((RUN / 'mps-full-1/guard.json').read_text())
        for path,digest in previous['sources'].items():
            # The cache-response self-test changed after the old reference. Its guard
            # implementation is unchanged and the current file is explicitly frozen.
            if Path(path)==ROOT/'crates/psionic/fixtures/clef/tools/mac_quiet_bench.py':
                require(files[path]=='7018be065710be9ffe5dfc2d289770015ca617e2fdd3da253a98e07ba1bc997b', 'current guard dependency differs')
                continue
            require(sha(path)==digest, 'prior reference source/input changed: '+path); files[path]=digest
        for mode in ('cpu','native','comparison'):
            prior=ISOLATION / (mode+'-1')
            g=json.loads((prior / 'guard.json').read_text()); r=prior / 'results/receipt.json'
            require(g['status']=='complete' and g['memory']['owned_group_stopped'] and g['result_sha256']==sha(r), 'prior capture proof differs')
            files[str(prior / 'guard.json')]=sha(prior / 'guard.json');files[str(r)]=sha(r)
        require(json.loads((ISOLATION/'comparison-1/results/receipt.json').read_text())['observer_consistency_screen']['passed'], 'prior observer screen failed')
        for f in sorted((ISOLATION/'cpu-1/results').glob('*.safetensors')) + sorted((ISOLATION/'native-1/results/rows').glob('*.f32')):
            files[str(f)]=sha(f)
        command += ['--capture-receipt',str(a.capture_receipt)]
    record={'schema':'clef.text029-llama-guard.v1','status':'preflight','mode':a.mode,'command':command,
            'bounds':limits,'leases':sorted(leases),'sources':files,'upstream':frozen['upstream'],
            'gguf_state':inputs['gguf_state'],'gguf_sha256_from_prior_verification':inputs['gguf_sha256_from_prior_verification'],
            'scope':'Diagnostic only. Frozen Torch acceptance remains unchanged.'}
    write(out/'guard.json',record)
    child=monitor=None; began=time.monotonic()
    def interrupt(signum,frame): raise KeyboardInterrupt(f'signal {signum}')
    signal.signal(signal.SIGTERM,interrupt);signal.signal(signal.SIGINT,interrupt)
    try:
        record['before']=available_memory()
        require(record['before']['available_memory_bytes'] >= limits['start_gib']*1024**3
                and shutil.disk_usage(out).free >= (25+limits['output_gib'])*1024**3, 'preflight resources insufficient')
        with (out/'stdout.log').open('xb') as log:
            child=subprocess.Popen(command,env=clean_environment(),stdin=subprocess.DEVNULL,stdout=log,stderr=log,start_new_session=True)
            monitor=OwnedProcessGuard(child,limits['group_gib']*1024**3,out/'memory.jsonl',minimum_available_memory_bytes=12*1024**3)
            record.update(status='running',owned_pid=child.pid);write(out/'guard.json',record)
            while child.poll() is None:
                monitor.guard()
                require(time.monotonic()-began<limits['deadline_seconds'], 'fixed deadline exceeded')
                require(shutil.disk_usage(out).free>=25*1024**3, 'disk floor crossed')
                require(output_bytes(out)<=limits['output_gib']*1024**3, 'output size exceeded')
                time.sleep(.2)
            monitor.guard();record['exit_code']=child.returncode
            require(child.returncode==0, 'owned CPU '+a.mode+' failed')
        result=json.loads((out/'results/receipt.json').read_text())
        require(result['status']=='complete', 'result incomplete')
        if a.mode=='capture': require(result['completed'] and len(result['files'])==65, 'layer coverage incomplete')
        if a.mode=='replay': require(result['completed'] and len(result['layers'])==64 and len(result['heads'])==3 and result['A_head_replay_exact'], 'head/layer replay incomplete')
        for path,digest in files.items(): require(sha(path)==digest, 'source/input changed during run: '+path)
        require(state(GGUF)==inputs['gguf_state'] and upstream_identity()==frozen['upstream'], 'GGUF/upstream changed')
        record.update(status='complete',sources_rechecked=True,result_sha256=sha(out/'results/receipt.json'))
    except BaseException as error:
        record.update(status='failed',error_type=type(error).__name__,error=str(error))
    finally:
        try:
            if monitor is not None:
                try: monitor.close()
                finally: record['memory']=monitor.receipt()
                if monitor.failure: record['status']='failed'
            elif child is not None: stop_owned_group(child)
        except BaseException as error: record.update(status='failed',cleanup_error=str(error))
        record['seconds']=time.monotonic()-began;write(out/'guard.json',record)
    print(json.dumps({'status':record['status'],'mode':a.mode,'owned_group_stopped':record.get('memory',{}).get('owned_group_stopped')}))
    return 0 if record['status']=='complete' else 1

if __name__=='__main__': raise SystemExit(main())
