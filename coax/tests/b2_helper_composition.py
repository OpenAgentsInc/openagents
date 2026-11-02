#!/usr/bin/env python3
"""Focused B2 actual-source/CK_run oracles, without compiler/bootstrap gates.

The checker emission strips ONLY physical #test/#forall/#authority regions.
Every other byte and dependency is retained in a closed private snapshot.
System One internal queries use a byte-mapped flattening of its real load
closure; this is not module-resolution or full compiler-image qualification.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import time

ROOT=Path(__file__).resolve().parents[2]
HERE=Path(__file__).resolve().parent
FIXTURES=HERE/'b2-helper-composition'
H=Path('/tmp/coax-chapter30-full/final-controls-qualification/normal167/snapshot/coax/bin')

from b2_support import ROOT,H,sha,file_sha,mask,closure,strip_regions,source_inventory,bounded_run

def validate_checker_reuse(directory,compiler_files,compiler):
 receipt=directory.parent/'qualification.json';record=json.loads(receipt.read_text())
 if (record.get('status') not in ('passed','emitted') or not all(record.get(k) is True for k in ('inputs_unchanged','tools_unchanged','artifacts_unchanged')) or
     record['tools_before']['compiler_sha256']!=file_sha(compiler) or not record.get('runtime_projection')):
  raise ValueError('reused checker lacks a successful source/producer-bound emission receipt')
 for path,raw in compiler_files.items():
  if record['source_before'].get(str(path.relative_to(ROOT)))!=dict(bytes=len(raw),sha256=sha(raw)):raise ValueError('reused checker source closure differs')
 for name in ('checker.c','checker.h'):
  if file_sha(directory/name)!=record['artifacts'].get('generated/'+name):raise ValueError('reused checker C/header changed')
 return dict(kind='validated retained generated checker',receipt=str(receipt),receipt_sha256=file_sha(receipt),sha256={n:file_sha(directory/n) for n in ('checker.c','checker.h')})

def normalized(origins):
 return sorted([{k:o[k] for k in ('kind','root','role','path')} for o in origins],key=lambda x:json.dumps(x,sort_keys=True))
def expect_origins(observed,expected):
 if expected=='static-only':return bool(observed) and all(o['kind']==1 for o in observed)
 return normalized(observed)==sorted(expected,key=lambda x:json.dumps(x,sort_keys=True))
def judge(case,observed,manifest,offset):
 if case['outcome']=='resource-reject':
  return not observed['ok'] and observed['fatal'] and len(observed['errors'])==1 and observed['errors'][0]['code']==case['diagnostic_code'] and not observed['procedures']
 if case['outcome']=='reject':
  expected=dict(case['diagnostic']);expected['line']+=offset
  return not observed['ok'] and not observed['fatal'] and observed['errors']==[expected]
 if not observed['ok'] or observed['fatal'] or observed['errors']:return False
 found=[p for p in observed['procedures'] if p['name']==case['query_procedure']]
 if len(found)!=1 or found[0]['borrow_status']!=case['borrow_status']:return False
 queries=[q for q in found[0]['queries'] if q['path']==[]]
 if case['origins'] is not None and (not queries or not all(q['status']==(2 if case['borrow_status']==1 else 3) and expect_origins(q['origins'],case['origins']) for q in queries)):return False
 if case.get('projection'):
  expected=manifest['actual_so_post_queries'];post=[p for p in observed['procedures'] if p['name']==expected['procedure']]
  if len(post)!=1 or post[0]['borrow_status']!=expected['borrow_status'] or post[0]['retain_incomplete'] is not expected['retain_incomplete']:return False
  for field in expected['fields']:
   rows=[q for q in post[0]['queries'] if q['path']==field['path']]
   if not rows or any(q['status']!=2 or not expect_origins(q['origins'],field['origins']) for q in rows):return False
 return True

def main():
 parser=argparse.ArgumentParser(description=__doc__)
 parser.add_argument('--compiler',type=Path,default=H/'coax');parser.add_argument('--native-dir','--lexer-dir',dest='native_dir',type=Path,default=H)
 parser.add_argument('--native-source-root',type=Path,help='checkout supplying the selected lexer/parser sources; defaults to native-dir/../..')
 parser.add_argument('--out',type=Path,required=True);parser.add_argument('--only',action='append')
 parser.add_argument('--profile',action='append',choices=['O0','O2','sanitized']);parser.add_argument('--checker-generated-dir',type=Path)
 parser.add_argument('--emit-only',action='store_true');args=parser.parse_args()
 out=args.out.resolve();out.mkdir(parents=True,exist_ok=False);compiler=args.compiler.resolve();cc=Path(shutil.which(os.environ.get('COAX_CC','cc'))).resolve();deadline=time.monotonic()+820
 manifest=json.loads((FIXTURES/'fixtures.json').read_text());cases=manifest['cases']
 if args.only:
  if set(args.only)-{c['id'] for c in cases}:raise ValueError('unknown selected fixture')
  cases=[c for c in cases if c['id'] in args.only]
 compiler_files=closure(ROOT/'coax/lib/compiler/checker.coax');system_files=closure(ROOT/'coax/lib/systemone/transport.coax')
 inputs={**compiler_files,**system_files,**{p:p.read_bytes() for p in FIXTURES.rglob('*') if p.is_file()},**{p:p.read_bytes() for p in [Path(__file__),HERE/'b2-helper-observer.c',HERE/'b2_support.py',*[ROOT/'coax/bootstrap'/n for n in ('lexer.c','lexer.h','parser.c','parser.h')]]}}
 before=source_inventory(inputs);tools=dict(compiler_sha256=file_sha(compiler),cc_path=str(cc),cc_sha256=file_sha(cc))
 proof=dict(schema=1,scope=__doc__,source_before=before,tools_before=tools,commands=[],cases=[],status='running');artifacts={}
 def save():
  proof['source_after']={p:dict(bytes=(ROOT/p).stat().st_size,sha256=file_sha(ROOT/p)) for p in before}
  proof['inputs_unchanged']=proof['source_after']==before
  proof['tools_unchanged']=file_sha(compiler)==tools['compiler_sha256'] and file_sha(cc)==tools['cc_sha256']
  proof['artifacts']=artifacts
  proof['artifacts_unchanged']=all(p.is_file() and file_sha(p)==h for p,h in [(out/p,h) for p,h in artifacts.items()])
  (out/'qualification.json').write_text(json.dumps(proof,indent=2)+'\n')
 def command(label,argv):
  save()
  if not proof['inputs_unchanged'] or not proof['tools_unchanged'] or not proof['artifacts_unchanged']:raise RuntimeError('input/tool/artifact changed before '+label)
  remaining=deadline-time.monotonic()
  if remaining<=0:raise RuntimeError('focused gate deadline exhausted')
  tick=time.monotonic()
  try:r=bounded_run(argv,cwd=ROOT,timeout=min(180,remaining))
  except BaseException as exc:
   for stream,raw in [('stdout',getattr(exc,'output',b'')),('stderr',getattr(exc,'stderr',b''))]:
    p=out/(label+'.'+stream);p.write_bytes(raw or b'');artifacts[p.name]=file_sha(p)
   proof['commands'].append(dict(label=label,argv=list(map(str,argv)),exit_status=None,error=type(exc).__name__,seconds=time.monotonic()-tick));save();raise
  for stream,raw in [('stdout',r.stdout),('stderr',r.stderr)]:
   p=out/(label+'.'+stream);p.write_bytes(raw);artifacts[str(p.relative_to(out))]=sha(raw)
  proof['commands'].append(dict(label=label,argv=list(map(str,argv)),exit_status=r.returncode,seconds=time.monotonic()-tick));save()
  if r.returncode:raise RuntimeError(label+': '+r.stderr.decode(errors='replace')[-1800:])
  return r.stdout
 try:
  # Reuse only lexer/parser source closures and ABI from the named H snapshot.
  historical=args.native_source_root.resolve() if args.native_source_root else args.native_dir.resolve().parent.parent
  native_before={n:file_sha(args.native_dir/n) for n in ('lexer.c','lexer.h','parser.c','parser.h')};proof['native_artifacts_before']=native_before
  canonical_native={n:file_sha(ROOT/'coax/bootstrap'/n) for n in native_before};proof['canonical_native_artifacts']=canonical_native
  if native_before!=canonical_native:raise RuntimeError('selected lexer/parser C/header tuple differs from canonical native ABI snapshots')
  for name in ['lexer','parser']:
   live=closure(ROOT/('coax/lib/compiler/'+name+'.coax'))
   for path,raw in live.items():
    old=historical/path.relative_to(ROOT)
    if not old.is_file() or old.read_bytes()!=raw:raise RuntimeError('historical '+name+' source/ABI closure differs: '+str(path))
  generated=out/'generated';generated.mkdir()
  if args.checker_generated_dir:
   proof['checker_producer']=validate_checker_reuse(args.checker_generated_dir,compiler_files,compiler)
   for name in ['checker.c','checker.h']:(generated/name).write_bytes((args.checker_generated_dir/name).read_bytes())
  else:
   snapshot=out/'runtime-snapshot';regions=[]
   for path,raw in compiler_files.items():
    relative=path.relative_to(ROOT);target=snapshot/relative;target.parent.mkdir(parents=True,exist_ok=True)
    text,removed=strip_regions(raw);target.write_text(text);regions.append(dict(path=str(relative),source_sha256=sha(raw),projected_sha256=sha(text.encode()),removed=removed))
   proof['runtime_projection']=regions
   module=snapshot/'coax/lib/compiler/checker.coax'
   raw=command('emit-checker',[compiler,'c',module,'--library','--header',generated/'checker.h'])
   (generated/'checker.c').write_bytes(raw)
  for name in ['lexer.c','lexer.h','parser.c','parser.h']:(generated/name).write_bytes((args.native_dir/name).read_bytes())
  for p in generated.iterdir():artifacts[str(p.relative_to(out))]=file_sha(p)
  if args.emit_only:proof['status']='emitted';return
  # Exact whole-file flattening for actual System One internal query observations.
  flattened='#import "Basic";\n';maps=[]
  for path,raw in reversed(list(system_files.items())):
   text,removed=strip_regions(raw,flatten=True);start=flattened.count('\n')+1
   maps.append(dict(path=str(path.relative_to(ROOT)),source_sha256=sha(raw),start_line=start,removed=removed));flattened+=text+'\n'
  proof['systemone_projection']=maps
  profiles=args.profile or ['O0','O2','sanitized'];failures=[]
  for profile in profiles:
   flags={'O0':['-O0'],'O2':['-O2'],'sanitized':['-O1','-g','-fsanitize=address,undefined','-fno-sanitize-recover=all','-fno-omit-frame-pointer']}[profile]
   binary=out/profile
   command(profile+'-compile',[cc,'-std=c11',*flags,'-ffp-contract=off','-I',generated,generated/'lexer.c',generated/'parser.c',generated/'checker.c',HERE/'b2-helper-observer.c','-lm','-o',binary]);artifacts[profile]=file_sha(binary)
   for case in cases:
    text=(FIXTURES/case['source']).read_text();offset=0
    if case.get('projection'):
     text=re.sub(r'(?m)^#load[^\n]+','',text);offset=flattened.count('\n');text=flattened+text
    fixture=out/(case['id']+'.coax')
    if not fixture.exists():fixture.write_text(text);artifacts[fixture.name]=file_sha(fixture)
    raw=command(profile+'-'+case['id'],[binary,fixture]);observed=json.loads(raw);passed=judge(case,observed,manifest,offset)
    proof['cases'].append(dict(id=case['id'],profile=profile,expected=case,observed=observed,line_offset=offset,passed=passed))
    if not passed:failures.append(profile+':'+case['id'])
  if failures:raise AssertionError('literal B2 oracle mismatch: '+', '.join(failures))
  proof['status']='passed'
 except BaseException as exc:
  proof.update(status='failed',error=str(exc));raise
 finally:
  if 'native_before' in locals():
   proof['native_artifacts_after']={n:file_sha(args.native_dir/n) for n in native_before}
   if proof['native_artifacts_after']!=native_before:proof.update(status='failed',error='native lexer/parser artifacts changed')
  save()
  if not proof['inputs_unchanged'] or not proof['tools_unchanged'] or not proof['artifacts_unchanged']:raise RuntimeError('final input/tool/artifact drift')
  if proof.get('error')=='native lexer/parser artifacts changed':raise RuntimeError(proof['error'])
  print(json.dumps(dict(status=proof['status'],cases=len(proof['cases']),receipt=str(out/'qualification.json'))))

if __name__=='__main__':main()
