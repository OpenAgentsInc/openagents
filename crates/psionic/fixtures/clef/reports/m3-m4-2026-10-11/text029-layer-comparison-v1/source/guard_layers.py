#!/usr/bin/env python3
"""Bound one CPU oracle or one archived native Metal layer capture."""
import argparse,hashlib,json,os,shutil,signal,subprocess,sys,time
from pathlib import Path
HERE=Path(__file__).resolve().parent;RUN=HERE.parent;BASE=RUN.parent
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
 p=argparse.ArgumentParser();p.add_argument('mode',choices=['cpu','native']);p.add_argument('--run-dir',type=Path,required=True);a=p.parse_args()
 leases=set(os.environ.get('OPENAGENTS_LEASES','').split(','));required={'memory','disk'}|({'gpu'} if a.mode=='native' else set());forbidden={'build','quiet'}|({'gpu'} if a.mode=='cpu' else set())
 require(required<=leases and not forbidden&leases,'incorrect resource leases')
 out=a.run_dir.resolve();require(HERE in out.parents and not out.exists(),'preserve previous scratch output');out.mkdir(parents=True)
 script=HERE/('cpu_reference.py' if a.mode=='cpu' else 'native_layers.py')
 command=[str(PYTHON),str(script),'--output-dir',str(out/'results')]
 previous=json.loads((RUN/'mps-full-1/guard.json').read_text())
 # Model/operator/tokenizer dependencies and historical HTTP inputs stay frozen.
 # Current product Rust may change; the native capture uses only the hashed archived executable.
 files=[Path(x) for x in previous['sources'] if not x.startswith(str(ROOT))]
 files += [ROOT/'crates/psionic/fixtures/clef/tools'/x for x in ('mac_quiet_bench.py','mac_memory.py')]
 files += [Path(__file__),script,RUN/'mps_reference.py',RUN/'mps-full-1/results/receipt.json',RUN/'mps-full-1/results/text-029-final.safetensors',HERE/'tokens-029.txt',HERE/'cpu-source-check.json',RUN/'encoder-exact-identity.json']
 if a.mode=='native':files += [BASE/'stage6'/x for x in ('psionic-serve-tests','source-manifest.json','test-binaries.json')]
 cap=12 if a.mode=='cpu' else 52;start=cap+12;deadline=300 if a.mode=='cpu' else 600
 record={'schema':'clef.text029-layer-capture-guard.v1','status':'preflight','mode':a.mode,'command':command,'bounds':{'group_gib':cap,'start_gib':start,'available_floor_gib':12,'gpu_executed':a.mode=='native','cpu_threads':2,'deadline_seconds':deadline,'disk_floor_gib':25,'disk_reservation_gib':2},'sources':{str(f):sha(f) for f in set(files)},'leases':sorted(leases),'latency_claim':False}
 gguf=BASE/'artifacts/Clef-Q4_K_M.gguf';old=json.loads((BASE/'cpu27b-diagnostic/streamed-gguf/full-1/results/receipt.json').read_text())['gguf_file_state'];require(state(gguf)==old,'GGUF changed from independently decoded CPU snapshot');record['gguf_state']=old
 encoder=json.loads((RUN/'encoder-source-run-1/guard.json').read_text());require(encoder['status']=='complete' and encoder['output_sha256']==sha(RUN/'encoder-source-run-1/native-encoded.json') and all(x['matches_stage6'] for x in encoder['native_source_identity']['files'].values()),'exact historical native encoding proof differs')
 write(out/'guard.json',record);env=dict(os.environ)
 for key in list(env):
  if key.startswith(('DYLD_','PSIONIC_CLEF_','MTL_','METAL_')) or key in {'LD_PRELOAD','LD_LIBRARY_PATH','PYTORCH_MPS_HIGH_WATERMARK_RATIO','PYTORCH_MPS_LOW_WATERMARK_RATIO','PYTORCH_MPS_FAST_MATH','PYTORCH_MPS_PREFER_METAL'}:del env[key]
 env.update(OMP_NUM_THREADS='2',OPENBLAS_NUM_THREADS='2',MKL_NUM_THREADS='2',VECLIB_MAXIMUM_THREADS='2',RAYON_NUM_THREADS='2',PYTORCH_ENABLE_MPS_FALLBACK='0',HF_HUB_OFFLINE='1',TRANSFORMERS_OFFLINE='1',PYTHONDONTWRITEBYTECODE='1',CUDA_VISIBLE_DEVICES='')
 if a.mode=='cpu':env['CLEF_TEXT029_CPU_GUARDED']='1'
 else:env.update(CLEF_TEXT029_NATIVE_GUARDED='1',PSIONIC_CLEF_GGUF=str(gguf),PSIONIC_CLEF_LAYER_TOKENS=str(HERE/'tokens-029.txt'),PSIONIC_CLEF_LAYER_DEVICE='cuda',PSIONIC_CLEF_TEST_DEVICE='metal',PSIONIC_CLEF_LAYER_CHUNK='512',PSIONIC_CLEF_METAL_ATTENTION='fused-tensorops',PSIONIC_CLEF_METAL_GEMM='weight-shapes',PSIONIC_CLEF_METAL_FFN_WEIGHTS='q4k-on-demand',PSIONIC_CLEF_METAL_DELTA='sequential',PSIONIC_CLEF_PREFIX_CACHE_BYTES='0')
 record['effective_environment']={k:v for k,v in env.items() if k.startswith(('PSIONIC_CLEF_','CLEF_TEXT029_')) or k in ('OMP_NUM_THREADS','OPENBLAS_NUM_THREADS','MKL_NUM_THREADS','VECLIB_MAXIMUM_THREADS','RAYON_NUM_THREADS','PYTORCH_ENABLE_MPS_FALLBACK','HF_HUB_OFFLINE','TRANSFORMERS_OFFLINE','PYTHONDONTWRITEBYTECODE','CUDA_VISIBLE_DEVICES')}
 child=monitor=None;began=time.monotonic()
 def interrupt(signum,frame):raise KeyboardInterrupt(f'signal {signum}')
 signal.signal(signal.SIGTERM,interrupt);signal.signal(signal.SIGINT,interrupt)
 try:
  record['before']=available_memory();require(record['before']['available_memory_bytes']>=start*1024**3 and shutil.disk_usage(out).free>=27*1024**3,'preflight resources insufficient')
  with (out/'stdout.log').open('xb') as log:
   child=subprocess.Popen(command,env=env,stdin=subprocess.DEVNULL,stdout=log,stderr=log,start_new_session=True)
   monitor=OwnedProcessGuard(child,cap*1024**3,out/'memory.jsonl',minimum_available_memory_bytes=12*1024**3)
   record.update(status='running',owned_pid=child.pid);write(out/'guard.json',record)
   while child.poll() is None:
    monitor.guard();require(time.monotonic()-began<deadline,'fixed deadline exceeded');require(shutil.disk_usage(out).free>=25*1024**3,'disk floor crossed');time.sleep(.2)
   monitor.guard();record['exit_code']=child.returncode;require(child.returncode==0,'owned '+a.mode+' layer capture failed')
  result=json.loads((out/'results/receipt.json').read_text());require(result['status']=='complete' and result['completed'],'layer capture incomplete')
  if a.mode=='cpu':require(len(result['layers'])==64 and result['full_pass_executed'] and len(result['input_ids'])==359,'CPU layer coverage differs')
  else:require(len(result['files'])==65 and result['all_layer_rows_checked'] and result['input_tokens']==359,'native layer coverage differs')
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
