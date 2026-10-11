#!/usr/bin/env python3
"""Bound the standalone native encoder build or CPU-only export."""
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
TARGET=Path('@@CLEF_TARGET@@/release')
sys.path.insert(0,str(ROOT/'crates/psionic/fixtures/clef/tools'))
from mac_quiet_bench import OwnedProcessGuard,available_memory,stop_owned_group

def sha(path):
 h=hashlib.sha256()
 with Path(path).open('rb') as f:
  for raw in iter(lambda:f.read(1024**2),b''):h.update(raw)
 return h.hexdigest()
def require(value,message):
 if not value:raise ValueError(message)
def write(path,value):path.write_text(json.dumps(value,indent=2)+'\n')

def main():
 p=argparse.ArgumentParser();p.add_argument('mode',choices=['build','run']);p.add_argument('--run-dir',type=Path,required=True);p.add_argument('--build-dir',type=Path);a=p.parse_args()
 leases=set(os.environ.get('OPENAGENTS_LEASES','').split(','));needed={'memory','disk'}|({'build'} if a.mode=='build' else set())
 require(needed<=leases and not {'quiet','gpu'}&leases and (a.mode=='build' or 'build' not in leases),'incorrect resources')
 out=a.run_dir.resolve();require(HERE in out.parents and not out.exists(),'preserve prior output and use scratch');out.mkdir(parents=True)
 record={'status':'preparing','mode':a.mode,'bounds':{'group_gib':4 if a.mode=='build' else 2,'start_gib':16 if a.mode=='build' else 14,'floor_gib':12,'deadline_seconds':180 if a.mode=='build' else 60,'disk_floor_gib':25},'model_executed':False,'gpu_executed':False,'quiet_lease':False,'source_sha256':{'encoder_source.rs':sha(HERE/'encoder_source.rs'),'guard_encoder_source.py':sha(__file__)},'native_http_version_limit':'This export compiles current production parser/encoder/media sources directly, with the linked tokenizer library snapshot. It does not observe historical HTTP process IDs. Compare recorded current and Stage6 source identities before reusing old responses.'}
 source=read_source_identity();record['native_source_identity']=source
 copies={str(path):sha(path) for path in (HERE/'production-source').rglob('*.rs')}
 for name in ('encode.rs','json.rs','media.rs','media/resize.rs'):require(sha(HERE/'production-source/clef'/name)==sha(ROOT/'crates/psionic/crates/psionic-serve/src/clef'/name),'exact source copy differs')
 record['exact_source_copies']=copies
 env=dict(os.environ)
 for key in list(env):
  if key.startswith('DYLD_') or key in {'LD_LIBRARY_PATH','LD_PRELOAD'}:del env[key]
 env.update(CLEF_ENCODER_ONLY_GUARDED='1',RAYON_NUM_THREADS='2',OMP_NUM_THREADS='2',OPENBLAS_NUM_THREADS='2',MKL_NUM_THREADS='2',VECLIB_MAXIMUM_THREADS='2')
 if a.mode=='build':
  names={'psionic_models':'92720d7897a18b10','serde_json':'9edadf2d77c22b62','sha2':'1a19b6d3c0a9dc5f'}
  libraries={name:TARGET/'deps'/f'lib{name}-{identity}.rlib' for name,identity in names.items()}
  for name in ('image','base64','hex'):
   candidates=list((TARGET/'deps').glob('lib'+name+'-*.rlib'));require(bool(candidates),'missing image admission dependency');libraries[name]=max(candidates,key=lambda path:path.stat().st_mtime_ns)
  record['libraries']={name:{'path':str(path),'bytes':path.stat().st_size,'sha256':sha(path)} for name,path in libraries.items()}
  record['compiler']=subprocess.check_output(['rustup','run','1.97.1','rustc','-vV'],text=True)
  sysroot=Path(subprocess.check_output(['rustup','run','1.97.1','rustc','--print','sysroot'],text=True).strip())
  linker=sysroot/'lib/rustlib/aarch64-apple-darwin/bin/rust-lld';require(linker.is_file(),'pinned rust-lld unavailable')
  shim=out/'ld64.lld';shim.symlink_to(linker)
  sdk=subprocess.check_output(['xcrun','--sdk','macosx','--show-sdk-path'],text=True).strip()
  record['linker']={'path':str(linker),'sha256':sha(linker),'scratch_symlink':str(shim),'sdk':sdk,'flavor':'ld64.lld','threads':2,'lto_jobs':2}
  command=['rustup','run','1.97.1','rustc','--edition=2024','--crate-name','clef_native_encoder_only',str(HERE/'encoder_source.rs'),'-C','opt-level=1','-C','linker='+str(shim),'-C','linker-flavor=ld64.lld','-C','link-arg=-syslibroot','-C','link-arg='+sdk,'-C','link-arg=--threads=2','-C','link-arg=--thinlto-jobs=2','-C','link-arg=--lto-O1','-L','dependency='+str(TARGET/'deps'),'-o',str(out/'encoder')]
  # Keep only the most recently generated directory for each native library name.
  selected={}
  for path in (TARGET/'build').glob('*/out/*.a'):
   old=selected.get(path.name)
   if old is None or path.stat().st_mtime_ns>old.stat().st_mtime_ns:selected[path.name]=path
  record['native_libraries']={name:{'path':str(path),'sha256':sha(path)} for name,path in selected.items()}
  for directory in sorted({str(path.parent) for path in selected.values()}):command+=['-L','native='+directory]
  for name,path in libraries.items():command+=['--extern',f'{name}={path}']
 else:
  require(a.build_dir is not None,'build directory required');build=a.build_dir.resolve();built=json.loads((build/'guard.json').read_text());require(built['status']=='complete','build did not pass');require(sha(build/'encoder')==built['binary_sha256'],'encoder binary changed');require(sha(HERE/'encoder_source.rs')==built['source_sha256']['encoder_source.rs'],'encoder source changed since build')
  command=[str(build/'encoder'),str(BASE/'artifacts/Clef-Q4_K_M.gguf'),str(BASE/'27b-head-stage6-combined'),str(out/'native-encoded.json')];record['build_sha256']=sha(build/'guard.json');record['binary_sha256']=built['binary_sha256']
  old=json.loads((BASE/'cpu27b-diagnostic/streamed-gguf/full-1/results/receipt.json').read_text())['gguf_file_state'];actual=(BASE/'artifacts/Clef-Q4_K_M.gguf').stat();current=[actual.st_size,actual.st_ino,actual.st_mtime_ns,actual.st_ctime_ns];require(current==old,'GGUF stat no longer matches historical same-Q4 reference');record['gguf_state']=current
 record['command']=command;write(out/'guard.json',record);child=monitor=None;began=time.monotonic()
 def interrupt(signum,frame):raise KeyboardInterrupt(f'signal {signum}')
 signal.signal(signal.SIGTERM,interrupt);signal.signal(signal.SIGINT,interrupt)
 try:
  record['before']=available_memory();require(record['before']['available_memory_bytes']>=record['bounds']['start_gib']*1024**3 and shutil.disk_usage(out).free>=27*1024**3,'preflight resources insufficient')
  with (out/'stdout.log').open('xb') as log:
   child=subprocess.Popen(command,env=env,stdin=subprocess.DEVNULL,stdout=log,stderr=log,start_new_session=True)
   monitor=OwnedProcessGuard(child,record['bounds']['group_gib']*1024**3,out/'memory.jsonl',minimum_available_memory_bytes=12*1024**3)
   record.update(status='running',owned_pid=child.pid);write(out/'guard.json',record)
   while child.poll() is None:
    monitor.guard();require(time.monotonic()-began<record['bounds']['deadline_seconds'],'deadline exceeded');require(shutil.disk_usage(out).free>=25*1024**3,'disk floor crossed');time.sleep(.2)
   monitor.guard();record['exit_code']=child.returncode;require(child.returncode==0,'owned encoder command failed')
  for name,digest in record['source_sha256'].items():require(sha(HERE/name)==digest,'source changed during command')
  require(read_source_identity()==source,'native source changed during command')
  for path,digest in copies.items():require(sha(path)==digest,'exact source copy changed during command')
  if a.mode=='build':
   for v in record['libraries'].values():require(sha(v['path'])==v['sha256'],'library changed during build')
   for v in record['native_libraries'].values():require(sha(v['path'])==v['sha256'],'native library changed during build')
   record['binary_sha256']=sha(out/'encoder')
  else:
   actual=(BASE/'artifacts/Clef-Q4_K_M.gguf').stat();require([actual.st_size,actual.st_ino,actual.st_mtime_ns,actual.st_ctime_ns]==record['gguf_state'],'GGUF changed during encoder run')
   d=json.loads((out/'native-encoded.json').read_text());require(d['status']=='complete' and len(d['cases'])==40 and d['question_count']==104,'incomplete native encoding');record['output_sha256']=sha(out/'native-encoded.json')
  record.update(status='complete',sources_rechecked=True)
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

def read_source_identity():
 paths=['crates/psionic/crates/psionic-serve/src/clef/encode.rs','crates/psionic/crates/psionic-serve/src/clef/json.rs','crates/psionic/crates/psionic-serve/src/clef/media.rs','crates/psionic/crates/psionic-serve/src/clef/media/resize.rs','crates/psionic/crates/psionic-models/src/lib.rs','crates/psionic/crates/psionic-models/src/runtime_tokenizer.rs']
 archived=json.loads((BASE/'stage6/source-manifest.json').read_text());old={x['path']:x['sha256'] for x in archived['files']}
 return {'stage6_source_manifest_sha256':sha(BASE/'stage6/source-manifest.json'),'files':{p:{'current_sha256':sha(ROOT/p),'stage6_sha256':old.get(p),'matches_stage6':sha(ROOT/p)==old.get(p)} for p in paths}}

if __name__=='__main__':raise SystemExit(main())
