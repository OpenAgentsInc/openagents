#!/usr/bin/env python3
"""Bounded exact-source Lab entry check, not full desktop/ownership qualification.

Types/defaults use the shared complete-declaration extractor. The production
lab_ask body is unchanged; only unreachable callees become signature-matched
test stubs that terminate through a foreign boundary. RED removes exactly the current entry guard as a fault injection. It is not
an historical source or full consumer qualification.
"""
import argparse
import datetime
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import time
import zeron_source_extract as extract

ROOT=Path(__file__).resolve().parents[3]
HERE=Path(__file__).resolve().parent
STUBS=['lab_body','lab_state','s_text_of','so_body','lab_fail','lab_url','cl_getenv','so_request_start']
GUARD='  // A repeated ask must not abandon the active owner or rewrite its backing.\n  if !so_owned_can_start(l[0].owner[0].phase,l[0].owner[0].generation) || !so_start_allowed(l[0].owner[0].call[0].h) { return; }\n'
COMPARED_FIELDS={
 'Lab':'phase owner ticket q part route via mc codex_due codex_effort fallback secs codex url url_n local remote body out err',
 'SoOwnedCall':'generation phase call',
 'SoCall':'h out err at last lines timeout_s done r',
 'SoResult':'status body problem stderr usage',
 'SoUsage':'ok input_tokens output_tokens',
 'SoBody':'buf at stage problem questions names kinds options',
 'Session':'pool np parts n live stopping noticed secs turns changed asking cost turn_ms hint',
 'SPart':'kind at n notice id tk arg detail before after input output failed resolved'}

def sha(raw):return hashlib.sha256(raw).hexdigest()
def proc(text):
 clean=extract.mask(text);start=clean.index('{');depth=1
 for end in range(start+1,len(clean)):
  if clean[end]=='{':depth+=1
  elif clean[end]=='}':
   depth-=1
   if not depth:return text[:end+1]+'\n'
 raise ValueError('unterminated procedure')

def projection():
 index,files=extract.declarations(ROOT)
 for name,fields in COMPARED_FIELDS.items():
  declared=set(re.findall(r'\b([A-Za-z_]\w*)\s*:(?!:)' ,extract.mask(index[name]['text'])))
  if declared!=set(fields.split()):raise RuntimeError('native named-field oracle needs review for changed '+name)
 actual=proc(index['lab_ask']['text']);selected={}
 def add(name):
  if name in selected:return
  row=index[name];selected[name]=row
  for token in sorted(set(re.findall(r'\b[A-Za-z_]\w*\b',extract.mask(row['text'])))):
   if token in index and index[token]['kind']!='procedure':add(token)
 for name in ['Lab','Session','SoClient','SO_OWN_IDLE','SO_OWN_LAST','SoStartFailure']:add(name)
 for token in sorted(set(re.findall(r'\b[A-Za-z_]\w*\b',extract.mask(actual)))):
  if token in index and index[token]['kind']!='procedure':add(token)
 chunks=['#import "Basic";\n','lab_owner_unexpected :: () #foreign;\n']+[r['text'] for r in selected.values()]
 stubs=[]
 for name in STUBS:
  original=proc(index[name]['text']);signature=original[:extract.mask(original).index('{')].rstrip()
  result=''
  if '-> string' in signature:result=' return "";'
  elif '-> bool' in signature:result=' return false;'
  elif name=='so_request_start':result=' return Result(int,SoStartFailure).{err=SoStartFailure.unavailable};'
  stub=signature+' { lab_owner_unexpected();'+result+' }\n';chunks.append(stub)
  stubs.append(dict(name=name,path=index[name]['path'],line=index[name]['line'],signature=signature,source_sha256=sha(original.encode()),stub_sha256=sha(stub.encode())))
 for name in ['so_start_allowed','so_owned_can_start']:chunks.append(proc(index[name]['text']))
 chunks.append(actual)
 chunks.append('lab_owner_init :: (l:[]Lab,s:[]Session) #export_c {l[0]=Lab.{};s[0]=Session.{};}\n')
 chunks.append('lab_owner_ask :: (l:[]Lab,s:[]Session) #export_c {lab_ask(l,s);}\n')
 return '\n'.join(chunks),dict(files=files,declarations=[{k:v for k,v in r.items() if k!='text'} for r in selected.values()],stubs=stubs,actual_lab_ask_sha256=sha(actual.encode()),guard=GUARD,compared_fields=COMPARED_FIELDS),actual

def main():
 parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--coax',required=True,type=Path);parser.add_argument('--out',required=True,type=Path);parser.add_argument('--generated-dir',type=Path,help='reuse exact hash-validated RED/GREEN emissions from a passed receipt; native checks still execute');args=parser.parse_args()
 out=args.out.resolve();out.mkdir(parents=True,exist_ok=False);compiler=args.coax.resolve();cc=shutil.which(os.environ.get('COAX_CC','cc'));deadline=time.monotonic()+800;runs=[]
 tooling={str(p.relative_to(ROOT)):sha(p.read_bytes()) for p in [Path(__file__),HERE/'lab-owner.c',HERE/'zeron_source_extract.py']}
 source,manifest,actual=projection()
 if actual.count(GUARD)!=1:raise RuntimeError('expected exact single production entry guard')
 red=actual.replace(GUARD,'',1)
 manifest.update(fault_injection='remove exact entry guard',red_lab_ask_sha256=sha(red.encode()),compiler_sha256=sha(compiler.read_bytes()),cc=cc,cc_sha256=sha(Path(cc).read_bytes()),scope='Exact lab_ask early active/invalid refusal only; complete authored types/defaults; signature-matched unreachable fatal stubs. Not full Lab generation, attachment, process lifecycle, B2, R2 or general ownership qualification.')
 (out/'projection.json').write_text(json.dumps(manifest,indent=2)+'\n')
 env={**os.environ,'ASAN_OPTIONS':'detect_leaks=0:halt_on_error=1','UBSAN_OPTIONS':'halt_on_error=1'}
 def run(label,cmd,expected=0,stdout=None):
  start=time.monotonic();remaining=deadline-start
  if remaining<1:raise RuntimeError('whole check deadline')
  result=subprocess.run(list(map(str,cmd)),cwd=ROOT,env=env,capture_output=True,timeout=min(180,remaining))
  (stdout or out/(label+'.stdout')).write_bytes(result.stdout);(out/(label+'.stderr')).write_bytes(result.stderr)
  runs.append(dict(label=label,argv=list(map(str,cmd)),exit_status=result.returncode,expected=expected,seconds=time.monotonic()-start,stdout_sha256=sha(result.stdout),stderr_sha256=sha(result.stderr)))
  if result.returncode!=expected:raise RuntimeError(label+' failed: '+result.stderr.decode(errors='replace')[-1200:])
 proof=dict(schema=1,projection=manifest,runs=runs,status='running',recorded_utc=datetime.datetime.now(datetime.timezone.utc).isoformat())
 try:
  prior=None
  if args.generated_dir:
   previous=args.generated_dir.resolve();prior=json.loads((previous/'receipt.json').read_text())
   if prior['status']!='passed' or prior['projection']!=manifest:raise RuntimeError('emission reuse source/producer identity differs')
   proof['reused_emission']=dict(receipt=str(previous/'receipt.json'),receipt_sha256=sha((previous/'receipt.json').read_bytes()))
  for name,body in [('red',source.replace(actual,red,1)),('green',source)]:
   folder=out/name;folder.mkdir();fixture=folder/'subset.coax';fixture.write_text(body)
   if prior:
    for artifact in ['subset.coax','lab-owner.c','lab-owner.h']:
     key=name+'/'+artifact;raw=(previous/key).read_bytes()
     if sha(raw)!=prior['artifacts'][key] or (artifact=='subset.coax' and raw!=body.encode()):raise RuntimeError('reused emitted artifact changed')
     (folder/artifact).write_bytes(raw)
   else:
    run(name+'-emit',[compiler,'c',fixture,'--library','--header',folder/'lab-owner.h'],stdout=folder/'lab-owner.c')
   for mode,flags in [('O0',['-O0']),('O2',['-O2']),('sanitized',['-O1','-g','-fsanitize=address,undefined','-fno-sanitize-recover=all','-fno-omit-frame-pointer'])]:
    binary=folder/mode
    run(name+'-'+mode+'-compile',[cc,'-std=c11',*flags,'-ffp-contract=off','-I',folder,folder/'lab-owner.c',HERE/'lab-owner.c','-lm','-o',binary])
    run(name+'-'+mode+'-run',[binary],expected=97 if name=='red' else 0)
  after,after_manifest,_=projection()
  if after!=source or after_manifest!= {k:v for k,v in manifest.items() if k in after_manifest}:raise RuntimeError('source projection changed during qualification')
  if sha(compiler.read_bytes())!=manifest['compiler_sha256'] or sha(Path(cc).read_bytes())!=manifest['cc_sha256']:raise RuntimeError('compiler/tool changed')
  if any(sha((ROOT/p).read_bytes())!=identity for p,identity in tooling.items()):raise RuntimeError('test tooling changed during qualification')
  proof['status']='passed'
 except BaseException as exc:
  proof.update(status='failed',error=str(exc));raise
 finally:
  proof['artifacts']={str(p.relative_to(out)):sha(p.read_bytes()) for p in out.rglob('*') if p.is_file()}
  proof['tooling']=tooling
  (out/'receipt.json').write_text(json.dumps(proof,indent=2)+'\n')
 print(json.dumps(dict(status=proof['status'],runs=len(runs),receipt=str(out/'receipt.json'))))

if __name__=='__main__':main()
