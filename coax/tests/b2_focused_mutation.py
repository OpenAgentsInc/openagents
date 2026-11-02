#!/usr/bin/env python3
"""Exact-source B2 helper-only mutation projection, never a whole-checker gate.

Selected newly named helpers are physical entry-source declarations. Every
remaining runtime declaration/load is a dependency, including modified old
call sites. Only authored B2 checks run. Moved old bodies are labeled below;
this does not claim mutation qualification of the old modified call sites.
"""
import argparse
import json
import os
from pathlib import Path
import re
import subprocess
import time
from b2_support import ROOT,sha,file_sha,mask,closure,strip_regions,source_inventory,bounded_run

AFFECTED_CHECKS=(
 'return borrow local aliases ', 'return borrow parameter aliases ',
 'return borrow unknown alias origins ', 'return borrow alias indexing ',
 'return borrow descriptor-only queries ', 'return borrow resolved helper endpoints ',
 'return borrow storage behind a passed-on helper slice ')

NAMES={
 'CB_call_writes':'new helper-write collector',
 'CB_external_local':'new original-source local witness helper',
 'CB_call_values':'moved existing call-substitution body, independently included in selected scope',
 'CB_alias_writes_mode':'mostly moved existing alias-write body plus external-witness mode'}
def span(raw,name):
 text=raw.decode();clean=mask(text);m=re.search(r'(?m)^'+name+r'\s*::',clean)
 if not m:raise ValueError('missing selected declaration '+name)
 begin=clean.index('{',m.end());depth=1;end=begin+1
 while depth:
  if clean[end]=='{':depth+=1
  elif clean[end]=='}':depth-=1
  end+=1
 return len(text[:m.start()].encode()),len(text[:end].encode())
def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--compiler',required=True,type=Path);p.add_argument('--out',required=True,type=Path);p.add_argument('--jobs',type=int,default=4);p.add_argument('--prepare-only',action='store_true');a=p.parse_args()
 out=a.out.resolve();out.mkdir(parents=True,exist_ok=False);compiler=a.compiler.resolve();source=ROOT/'coax/lib/compiler/checker.coax';files=closure(source)
 before=source_inventory(files);tooling={str(x.relative_to(ROOT)):file_sha(x) for x in [Path(__file__),Path(__file__).with_name('b2_support.py')]};compiler_sha=file_sha(compiler)
 snapshot=out/'runtime-snapshot';raw=source.read_bytes();projected,checks=strip_regions(raw);base=bytearray(projected.encode());moved=[];entry=b'#load "runtime-snapshot/coax/lib/compiler/checker.coax";\n#authority;\n'
 for name,scope in NAMES.items():
  start,end=span(raw,name);body=raw[start:end];entry_start=len(entry);entry+=b'\n'+body+b'\n';moved.append(dict(name=name,scope=scope,source_start=start,source_end=end,source_sha256=sha(body),entry_start=entry_start+1,entry_end=len(entry)-1))
  for i in range(start,end):
   if base[i]!=10:base[i]=32
 selected=[]
 for fact in checks:
  body=raw[fact['start']:fact['end']]
  named=re.match(rb'\s*#(?:test|forall)\s+"([^"]+)"',body)
  title=named[1].decode() if named else ''
  if fact['kind'] in ('test','forall') and (title.startswith('B2 ') or title.startswith(AFFECTED_CHECKS)):
   entry+=b'\n'+body+b'\n';selected.append(dict(fact,title=title,scope='new B2 check' if title.startswith('B2 ') else 'affected existing alias/call-substitution check'))
 if not selected:raise ValueError('no focused B2 checks')
 regions=[]
 for path,content in files.items():
  target=snapshot/path.relative_to(ROOT);target.parent.mkdir(parents=True,exist_ok=True);text,removed=strip_regions(content)
  data=bytes(base) if path==source else text.encode();target.write_bytes(data);regions.append(dict(path=str(path.relative_to(ROOT)),source_sha256=sha(content),projected_sha256=sha(data),removed=removed))
 entry_path=out/'b2-helpers.coax';entry_path.write_bytes(entry)
 proof=dict(schema=1,scope=__doc__,selected_helpers=moved,selected_checks=selected,source_before=before,tooling_before=tooling,compiler=dict(path=str(compiler),sha256=compiler_sha),runtime_projection=regions,entry_sha256=sha(entry),status='prepared')
 (out/'projection.json').write_text(json.dumps(proof,indent=2)+'\n')
 if a.prepare_only:print(json.dumps(dict(status='prepared',entry=str(entry_path))));return
 argv=[str(compiler),'mutate',str(entry_path),'--root-only','--jobs',str(a.jobs),'--evidence',str(out/'mutation-evidence.json')];start=time.monotonic()
 try:
  r=bounded_run(argv,cwd=ROOT,timeout=830);(out/'mutation.stdout').write_bytes(r.stdout);(out/'mutation.stderr').write_bytes(r.stderr)
  proof.update(command=argv,exit_status=r.returncode,seconds=time.monotonic()-start,status='passed' if r.returncode==0 else 'failed')
 except BaseException as exc:
  if hasattr(exc,'output'):(out/'mutation.stdout').write_bytes(exc.output or b'');(out/'mutation.stderr').write_bytes(exc.stderr or b'')
  proof.update(status='failed',error=str(exc));raise
 finally:
  proof['source_after']={path:dict(bytes=(ROOT/path).stat().st_size,sha256=file_sha(ROOT/path)) for path in before};proof['inputs_unchanged']=before==proof['source_after'];proof['tooling_unchanged']=all(file_sha(ROOT/path)==h for path,h in tooling.items());proof['compiler_unchanged']=file_sha(compiler)==compiler_sha
  proof['artifacts']={str(x.relative_to(out)):file_sha(x) for x in out.rglob('*') if x.is_file() and x.name not in ['qualification.json']}
  (out/'qualification.json').write_text(json.dumps(proof,indent=2)+'\n')
 if not proof['inputs_unchanged'] or not proof['tooling_unchanged'] or not proof['compiler_unchanged']:raise RuntimeError('mutation source/tool drift')
 print((out/'mutation.stdout').read_text()[-4000:]);print(json.dumps(dict(status=proof['status'],record=str(out/'qualification.json'))))
 if proof['status']!='passed':raise SystemExit(1)
if __name__=='__main__':main()
