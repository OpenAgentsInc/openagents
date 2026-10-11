#!/usr/bin/env python3
"""Guard only the owned one-layer MPS reference process; no automatic retries."""
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
BASE=HERE.parent
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
 p=argparse.ArgumentParser();p.add_argument('mode',choices=['pilot','full']);p.add_argument('--native-encoded',type=Path,required=True);p.add_argument('--run-dir',type=Path,required=True);a=p.parse_args()
 leases=set(os.environ.get('OPENAGENTS_LEASES','').split(','));require({'gpu','memory','disk'}<=leases and not {'quiet','build'}&leases,'incorrect resource leases')
 out=a.run_dir.resolve();require(HERE in out.parents and not out.exists(),'preserve previous output and stay in scratch');out.mkdir(parents=True)
 encoded=a.native_encoded.resolve();require(encoded.parent.parent==HERE and encoded.name=='native-encoded.json','encoding must be from this experiment')
 encoder_guard=json.loads((encoded.parent/'guard.json').read_text());require(encoder_guard['status']=='complete' and encoder_guard['output_sha256']==sha(encoded),'native encoder guard/hash differs')
 require(encoder_guard['memory']['owned_group_stopped'],'encoder child cleanup incomplete')
 require(all(row['matches_stage6'] for row in encoder_guard['native_source_identity']['files'].values()),'encoder/parser/tokenizer source differs from historical native captures')
 if a.mode=='full':
  require(os.environ.get('CLEF_SAME_Q4_FULL_APPROVED')=='1','full collection requires a separate explicit handoff after pilot review')
  recovery=json.loads((HERE/'mps-pilot-recovery-1/guard.json').read_text());proof=HERE/'mps-pilot-recovery-1/results/receipt.json';result=json.loads(proof.read_text())
  require(recovery['status']=='complete' and recovery['memory']['owned_group_stopped'] and recovery['result_sha256']==sha(proof),'CPU pilot recovery proof differs')
  require(result['status']=='complete' and result['passed'] and result['acceleration_screen_passed'] and result['saved_final_norm_exact'] and result['numerical_functions_unchanged'],'CPU pilot recovery did not pass')
  require(result['new_checker_sha256']==sha(HERE/'mps_reference.py'),'reference source changed after pilot recovery')
 command=[str(PYTHON),str(HERE/'mps_reference.py'),a.mode,'--native-encoded',str(encoded),'--output-dir',str(out/'results')]
 files=[HERE/'mps_reference.py',Path(__file__),BASE/'cpu27b-diagnostic/streamed-gguf/gguf_import.py',BASE/'cpu27b-diagnostic/streamed-gguf/stream_reference.py',BASE/'cpu27b-diagnostic/streamed-gguf/full-1/results/receipt.json',BASE/'27b-head-stage6-combined/summary.json',BASE/'27b-head-stage6-combined/staged/rows.jsonl',BASE/'27b-head-stage6-combined/fused-fields/rows.jsonl',encoded,encoded.parent/'guard.json']
 if a.mode=='full':files += [HERE/'mps-pilot-recovery-1/guard.json',HERE/'mps-pilot-recovery-1/results/receipt.json',HERE/'mps-pilot-1/guard.json',HERE/'mps-pilot-1/results/receipt.json',HERE/'mps-pilot-1/executed-source/mps_reference.py']
 files += [HERE/'encoder-exact-identity.json',PYTHON.parent.parent/'lib/python3.12/site-packages/transformers/models/qwen3_5/modeling_qwen3_5.py']
 files += [ROOT/path for path in encoder_guard['native_source_identity']['files']]
 files += [ROOT/'crates/psionic/fixtures/clef/tools'/x for x in ('mac_quiet_bench.py','mac_memory.py')]
 files += sorted((BASE/'27b-head-stage6-combined').glob('text-???.request.json'))
 files += sorted((BASE/'27b-head-stage6-combined').glob('*-text-???.response.json'))
 files += [BASE/'artifacts/clef-bf16'/x for x in ('joint_schema_model.py','joint_head.safetensors','joint_head_config.json','config.json','tokenizer.json','tokenizer_config.json','processor_config.json')]
 record={'schema':'clef.same-q4-mps-owned-guard.v1','status':'preflight','mode':a.mode,'command':command,'bounds':{'group_gib':12,'start_gib':24,'available_floor_gib':12,'mps_gib':8,'threads':2,'deadline_seconds':300 if a.mode=='pilot' else 1800,'disk_floor_gib':25},'sources':{str(f):sha(f) for f in files},'leases':sorted(leases),'source_identity_limit':encoder_guard['native_http_version_limit']}
 gguf=BASE/'artifacts/Clef-Q4_K_M.gguf';old=json.loads((BASE/'cpu27b-diagnostic/streamed-gguf/full-1/results/receipt.json').read_text())['gguf_file_state'];require(state(gguf)==old,'GGUF changed from independently decoded CPU snapshot');record['gguf_state']=old
 write(out/'guard.json',record);env=dict(os.environ)
 for key in list(env):
  if key.startswith('DYLD_') or key in {'LD_PRELOAD','LD_LIBRARY_PATH','PYTORCH_MPS_HIGH_WATERMARK_RATIO','PYTORCH_MPS_LOW_WATERMARK_RATIO','PYTORCH_MPS_FAST_MATH','PYTORCH_MPS_PREFER_METAL'}:del env[key]
 env.update(CLEF_SAME_Q4_MPS_GUARDED='1',OMP_NUM_THREADS='2',OPENBLAS_NUM_THREADS='2',MKL_NUM_THREADS='2',VECLIB_MAXIMUM_THREADS='2',RAYON_NUM_THREADS='2',PYTORCH_ENABLE_MPS_FALLBACK='0',HF_HUB_OFFLINE='1',TRANSFORMERS_OFFLINE='1',PYTHONDONTWRITEBYTECODE='1',CUDA_VISIBLE_DEVICES='')
 record['effective_environment']={k:v for k,v in env.items() if k in ('CLEF_SAME_Q4_MPS_GUARDED','OMP_NUM_THREADS','OPENBLAS_NUM_THREADS','MKL_NUM_THREADS','VECLIB_MAXIMUM_THREADS','RAYON_NUM_THREADS','PYTORCH_ENABLE_MPS_FALLBACK','HF_HUB_OFFLINE','TRANSFORMERS_OFFLINE','PYTHONDONTWRITEBYTECODE','CUDA_VISIBLE_DEVICES')}
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
    monitor.guard();require(time.monotonic()-began<record['bounds']['deadline_seconds'],'fixed deadline exceeded');require(shutil.disk_usage(out).free>=25*1024**3,'disk floor crossed');time.sleep(.2)
   monitor.guard();record['exit_code']=child.returncode;require(child.returncode==0,'owned MPS reference failed')
  result=json.loads((out/'results/receipt.json').read_text());require(result['status']=='complete' and result['completed'] and len(result['layers'])==64,'incomplete independent trunk')
  require(len(result['cases'])==(1 if a.mode=='pilot' else 40),'case count differs')
  if a.mode=='pilot':require(result['cases'][0]['acceleration_screen_passed'],'CPU-to-MPS acceleration screen failed')
  else:
   acceptance=result['runtime_acceptance'];require(acceptance['requests']==40 and acceptance['questions']==104 and acceptance['probability_limit']==.02 and acceptance['all_tops_required'] is True and acceptance['passed'] is True,'full same-Q4 runtime acceptance failed')
   require(set(acceptance['modes'])=={'staged','fused-fields'} and all(v['questions']==104 and v['top_matches']==104 and v['max_abs_probability']<=.02 and v['passed'] is True for v in acceptance['modes'].values()),'full mode aggregate acceptance differs')
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
 print(json.dumps({'status':record['status'],'mode':a.mode,'owned_group_stopped':record.get('memory',{}).get('owned_group_stopped')}))
 return 0 if record['status']=='complete' else 1

if __name__=='__main__':raise SystemExit(main())
