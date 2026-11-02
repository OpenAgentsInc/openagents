#!/usr/bin/env python3
"""Independent actual-so_post native caller-buffer checks; maintenance tooling."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import time
from b2_support import ROOT,H,closure,source_inventory,bounded_run

HERE=Path(__file__).resolve().parent
def sha(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()
def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--compiler',type=Path,default=H/'coax');p.add_argument('--out',type=Path,required=True);a=p.parse_args()
 out=a.out.resolve();out.mkdir(parents=True,exist_ok=False);compiler=a.compiler.resolve();cc=Path(shutil.which(os.environ.get('COAX_CC','cc'))).resolve();deadline=time.monotonic()+800
 files=closure(HERE/'b2-systemone-runtime.coax');files.update({x:x.read_bytes() for x in [HERE/'b2-systemone-runtime.c',Path(__file__),HERE/'b2_support.py']});before=source_inventory(files);tools={str(x):sha(x) for x in [compiler,cc]};artifacts={};runs=[]
 proof=dict(schema=1,scope=__doc__,source_before=before,tools_before=tools,runs=runs,status='running',profile_results={})
 def save():
  proof['source_after']={x:dict(bytes=(ROOT/x).stat().st_size,sha256=sha(ROOT/x)) for x in before};proof['inputs_unchanged']=before==proof['source_after'];proof['tools_unchanged']=all(sha(x)==h for x,h in tools.items());proof['artifacts']=artifacts;proof['artifacts_unchanged']=all(sha(out/x)==h for x,h in artifacts.items());(out/'qualification.json').write_text(json.dumps(proof,indent=2)+'\n')
 def run(label,argv):
  save()
  if not all(proof[x] for x in ['inputs_unchanged','tools_unchanged','artifacts_unchanged']):raise RuntimeError('input/tool/artifact drift')
  t=time.monotonic();left=deadline-t
  if left<=0:raise RuntimeError('focused deadline exhausted')
  try:r=bounded_run(argv,cwd=ROOT,timeout=min(180,left),env={**os.environ,'ASAN_OPTIONS':'detect_leaks=0:halt_on_error=1','UBSAN_OPTIONS':'halt_on_error=1'})
  except BaseException as exc:
   for stream,data in [('stdout',getattr(exc,'output',b'')),('stderr',getattr(exc,'stderr',b''))]:
    f=out/(label+'.'+stream);f.write_bytes(data or b'');artifacts[f.name]=sha(f)
   runs.append(dict(label=label,argv=list(map(str,argv)),exit_status=None,error=type(exc).__name__,seconds=time.monotonic()-t));save();raise
  for stream,data in [('stdout',r.stdout),('stderr',r.stderr)]:
   f=out/(label+'.'+stream);f.write_bytes(data);artifacts[f.name]=sha(f)
  runs.append(dict(label=label,argv=list(map(str,argv)),exit_status=r.returncode,seconds=time.monotonic()-t));save()
  if r.returncode:raise RuntimeError(label+': '+r.stderr.decode(errors='replace')[-1200:])
  return r.stdout
 try:
  c=out/'b2-systemone-runtime.c';h=out/'b2-systemone-runtime.h';c.write_bytes(run('emit',[compiler,'c',HERE/'b2-systemone-runtime.coax','--library','--header',h]));artifacts[c.name]=sha(c);artifacts[h.name]=sha(h)
  for name,flags in [('O0',['-O0']),('O2',['-O2']),('sanitized',['-O1','-g','-fsanitize=address,undefined','-fno-sanitize-recover=all','-fno-omit-frame-pointer'])]:
   binary=out/name;run(name+'-compile',[cc,'-std=c11',*flags,'-ffp-contract=off','-I',out,c,HERE/'b2-systemone-runtime.c','-lm','-o',binary]);artifacts[name]=sha(binary);observed=json.loads(run(name+'-run',[binary]));assert observed['buffer_cases']==45 and observed['default_empty_setup_cases']==1;proof['profile_results'][name]=observed
  proof['status']='passed'
 except BaseException as exc:proof.update(status='failed',error=str(exc));raise
 finally:
  save()
  if not all(proof[x] for x in ['inputs_unchanged','tools_unchanged','artifacts_unchanged']):raise RuntimeError('final input/tool/artifact drift')
  print(json.dumps(dict(status=proof['status'],receipt=str(out/'qualification.json'))))
if __name__=='__main__':main()
