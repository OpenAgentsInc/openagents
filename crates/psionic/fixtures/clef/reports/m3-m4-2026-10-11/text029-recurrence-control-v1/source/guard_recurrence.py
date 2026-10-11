#!/usr/bin/env python3
"""Bound one fixed CPU full-trunk recurrent Delta control."""
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
RUN=HERE.parent
BASE=RUN.parent
ROOT=Path('@@CLEF_WORKTREE@@')
PYTHON=Path('@@CLEF_PYTHON_ENV@@/bin/python')
sys.path.insert(0,str(ROOT/'crates/psionic/fixtures/clef/tools'))
from mac_quiet_bench import OwnedProcessGuard,available_memory,stop_owned_group

def sha(path):
 h=hashlib.sha256()
 with Path(path).open('rb') as f:
  for raw in iter(lambda:f.read(1024**2),b''):h.update(raw)
 return h.hexdigest()
def require(value,message):
 if not value:raise ValueError(message)
def write(path,value):
 temp=path.with_suffix(path.suffix+'.partial');temp.write_text(json.dumps(value,indent=2,allow_nan=False)+'\n');temp.replace(path)
def state(path):
 s=path.stat();return [s.st_size,s.st_ino,s.st_mtime_ns,s.st_ctime_ns]

def main():
 p=argparse.ArgumentParser();p.add_argument('--run-dir',type=Path,required=True);a=p.parse_args()
 leases=set(os.environ.get('OPENAGENTS_LEASES','').split(','));require({'memory','disk'}<=leases and not {'gpu','quiet','build'}&leases,'CPU-only resources required')
 out=a.run_dir.resolve();require(HERE in out.parents and not out.exists(),'preserve prior output');out.mkdir(parents=True)
 command=[str(PYTHON),str(HERE/'recurrence_reference.py'),'--output-dir',str(out/'results')]
 previous=json.loads((RUN/'mps-full-1/guard.json').read_text());files=[Path(x) for x in previous['sources']]
 files += [Path(__file__),HERE/'recurrence_reference.py',RUN/'mps_reference.py',RUN/'mps-full-1/results/receipt.json',RUN/'mps-full-1/results/text-029-final.safetensors',BASE/'27b-head-stage6-combined/staged/rows.f16',BASE/'27b-head-stage6-combined/fused-fields/rows.f16']
 files += [HERE/'layer35_math.py', HERE/'layer0_math.py', HERE/'recurrence-control-plan.md']
 files += [HERE/'precision_hash.py', HERE/'precision-1/guard.json', HERE/'precision-1/results/receipt.json', HERE/'precision-1/executed-source/manifest.json']
 precision_guard=HERE/'precision-2/guard.json';precision_receipt=HERE/'precision-2/results/receipt.json'
 prior_guard=json.loads(precision_guard.read_text());prior=json.loads(precision_receipt.read_text())
 require(prior_guard['status']=='complete' and prior_guard['memory']['owned_group_stopped']
         and prior_guard['sources_rechecked'] and prior_guard['result_sha256']==sha(precision_receipt)
         and prior['completed'] and prior['A_head_replay_exact'] and len(prior['layers'])==64,'completed C proof differs')
 cname='C_projection_and_attention_half'
 files += [precision_guard,precision_receipt,HERE/'precision_reference.py',HERE/'guard_precision.py',HERE/'precision-source-check-repair1.json']
 files += [HERE/'precision-2/results'/v['variants'][cname]['file'] for v in prior['layers']]
 files += [HERE/'precision-2/results'/prior['heads'][cname]['head_inputs_file']]
 for mode in ('cpu','native','comparison','layer0'):
  guard=HERE/(mode+'-1')/'guard.json';metadata=json.loads(guard.read_text());proof=guard.parent/'results/receipt.json'
  require(metadata['status']=='complete' and metadata['memory']['owned_group_stopped'] and metadata['result_sha256']==sha(proof),'capture proof or cleanup differs: '+mode)
  files += [guard,proof]
 require(json.loads((HERE/'comparison-1/results/receipt.json').read_text())['observer_consistency_screen']['passed'],'observer consistency screen did not pass')
 files += [HERE/'cpu-1/results'/f'layer-{i:02d}.safetensors' for i in range(64)]
 files += [HERE/'native-1/results/rows'/f'l_out-{i}.f32' for i in range(64)]
 files += [HERE/'cpu-1/results/final-hidden.safetensors', HERE/'native-1/results/rows/result_norm.f32']
 files += [HERE/'stage6-source-evidence'/name for name in ('clef_prefill.metal','clef_fused_attention.metal','clef_metal.rs')]
 frozen_file=HERE/'recurrence-source-check.json';frozen=json.loads(frozen_file.read_text())
 require(frozen['status']=='source-only' and frozen['gpu_executed'] is False,'frozen source descriptor differs')
 for path,digest in frozen['sources'].items():require(sha(path)==digest,'frozen executed source changed: '+path)
 files += [frozen_file]+[Path(path) for path in frozen['sources']]
 record={'schema':'clef.text029-recurrence-replay-guard.v1','status':'preflight','mode':'cpu-full-recurrence-control','command':command,'bounds':{'group_gib':12,'start_gib':24,'available_floor_gib':12,'gpu_executed':False,'threads':2,'deadline_seconds':300,'disk_floor_gib':25},'sources':{str(f):sha(f) for f in files},'leases':sorted(leases)}
 gguf=BASE/'artifacts/Clef-Q4_K_M.gguf';old=json.loads((BASE/'cpu27b-diagnostic/streamed-gguf/full-1/results/receipt.json').read_text())['gguf_file_state'];require(state(gguf)==old,'GGUF changed from independently decoded CPU snapshot');record['gguf_state']=old
 write(out/'guard.json',record);env=dict(os.environ)
 for key in list(env):
  if key.startswith('DYLD_') or key in {'LD_PRELOAD','LD_LIBRARY_PATH','PYTORCH_MPS_HIGH_WATERMARK_RATIO','PYTORCH_MPS_LOW_WATERMARK_RATIO','PYTORCH_MPS_FAST_MATH','PYTORCH_MPS_PREFER_METAL'}:del env[key]
 env.update(CLEF_RECURRENCE_GUARDED='1',OMP_NUM_THREADS='2',OPENBLAS_NUM_THREADS='2',MKL_NUM_THREADS='2',VECLIB_MAXIMUM_THREADS='2',RAYON_NUM_THREADS='2',PYTORCH_ENABLE_MPS_FALLBACK='0',HF_HUB_OFFLINE='1',TRANSFORMERS_OFFLINE='1',PYTHONDONTWRITEBYTECODE='1',CUDA_VISIBLE_DEVICES='')
 record['effective_environment']={k:v for k,v in env.items() if k in ('CLEF_RECURRENCE_GUARDED','OMP_NUM_THREADS','OPENBLAS_NUM_THREADS','MKL_NUM_THREADS','VECLIB_MAXIMUM_THREADS','RAYON_NUM_THREADS','PYTORCH_ENABLE_MPS_FALLBACK','HF_HUB_OFFLINE','TRANSFORMERS_OFFLINE','PYTHONDONTWRITEBYTECODE','CUDA_VISIBLE_DEVICES')}
 child=monitor=None;began=time.monotonic()
 def interrupt(signum,frame):raise KeyboardInterrupt(f'signal {signum}')
 signal.signal(signal.SIGTERM,interrupt);signal.signal(signal.SIGINT,interrupt)
 try:
  record['before']=available_memory();require(record['before']['available_memory_bytes']>=24*1024**3 and shutil.disk_usage(out).free>=27*1024**3,'preflight resources insufficient')
  with (out/'stdout.log').open('xb') as log:
   child=subprocess.Popen(command,env=env,stdin=subprocess.DEVNULL,stdout=log,stderr=log,start_new_session=True)
   monitor=OwnedProcessGuard(child,12*1024**3,out/'memory.jsonl',minimum_available_memory_bytes=12*1024**3)
   record.update(status='running',owned_pid=child.pid);write(out/'guard.json',record)
   while child.poll() is None:
    monitor.guard();require(time.monotonic()-began<record['bounds']['deadline_seconds'],'fixed deadline exceeded');require(shutil.disk_usage(out).free>=25*1024**3,'disk floor crossed');require(sum(f.stat().st_size for f in out.rglob('*') if f.is_file())<=2*1024**3,'output byte cap exceeded');time.sleep(.2)
   monitor.guard();record['exit_code']=child.returncode;require(child.returncode==0,'owned CPU recurrence replay failed')
  result=json.loads((out/'results/receipt.json').read_text());require(result['status']=='complete' and result['completed'] and result['fixtures']['attention']['passed'] and result['fixtures']['delta']['passed'] and len(result['layers'])==64 and len(result['heads'])==1 and result['A_head_replay_exact'] and result['C_head_replay_exact'] and result['layer0_WXR_exact'] and result['call_counts']=={'recurrent_delta':48,'online_attention':16},'CPU recurrence diagnostic incomplete')
  for f,digest in record['sources'].items():require(sha(f)==digest,'source/input changed during run: '+f)
  require(state(gguf)==old,'GGUF changed during run');record.update(status='complete',sources_rechecked=True,result_sha256=sha(out/'results/receipt.json'))
 except BaseException as error:record.update(status='failed',error_type=type(error).__name__,error=str(error))
 finally:
  try:
   if monitor is not None:
    try:monitor.close()
    finally:record['memory']=monitor.receipt()
    if monitor.failure:record['status']='failed'
   elif child is not None:stop_owned_group(child)
  except BaseException as error:record.update(status='failed',cleanup_error=str(error))
  record['seconds']=time.monotonic()-began;write(out/'guard.json',record)
 print(json.dumps({'status':record['status'],'mode':'cpu-full-recurrence-control','owned_group_stopped':record.get('memory',{}).get('owned_group_stopped')}))
 return 0 if record['status']=='complete' else 1

if __name__=='__main__':raise SystemExit(main())
