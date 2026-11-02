#!/usr/bin/env python3
# Firm reason: 2 (independent compiler, ABI, diagnostic and native-effect oracles; no product implementation).
"""Bounded typed async API checks, sharp authority mutation and real offline transcript."""
import argparse,hashlib,json,os,shutil,time
from pathlib import Path
from b2_support import ROOT,closure,source_inventory,bounded_run
HERE=Path(__file__).resolve().parent
NEGATIVE={'ignored-start':('E0907',2),'incomplete-terminal':('E0904',3),'local-snapshot':('E0383',4)}
def sha(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()
def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--compiler',required=True,type=Path);p.add_argument('--out',required=True,type=Path);p.add_argument('--reuse',type=Path,help='retain completed exact stages after runner-only correction');a=p.parse_args();out=a.out.resolve();out.mkdir(parents=True,exist_ok=False);compiler=a.compiler.resolve();cc=Path(shutil.which(os.environ.get('COAX_CC','cc'))).resolve()
 files=closure(HERE/'systemone-outcomes.coax')
 for name in NEGATIVE:files.update(closure(HERE/'systemone-outcomes'/(name+'.coax')))
 for path in [Path(__file__),HERE/'b2_support.py',HERE/'systemone-outcomes-child.c',HERE/'systemone-outcomes.out',ROOT/'coax/platform/process-native.c',ROOT/'coax/platform/process-native.h']:files[path]=path.read_bytes()
 before=source_inventory(files);tools={str(path):sha(path) for path in [compiler,cc]};runs=[];deadline=time.monotonic()+800
 proof=dict(schema=1,status='running',scope=__doc__,source_before=before,tools=tools,runs=runs,negative=NEGATIVE)
 retained={};previous=None
 if a.reuse:
  previous=a.reuse.resolve();prior=json.loads((previous/'qualification.json').read_text());runner=str(Path(__file__).relative_to(ROOT))
  if {k:v for k,v in prior['source_before'].items() if k!=runner}!={k:v for k,v in before.items() if k!=runner} or prior['tools']!=tools:raise RuntimeError('reuse changed non-runner input')
  if any(sha(previous/name)!=h for name,h in prior['artifacts'].items()):raise RuntimeError('prior artifact drift')
  retained={row['label']:row for row in prior['runs'] if row['exit_status']==row['expected']}
  proof['retained_results']=dict(receipt=str(previous/'qualification.json'),sha256=sha(previous/'qualification.json'),labels=sorted(retained),reason='runner-only diagnostic-stream correction')
  for name in ['checks.json','mutation.json']:
   if (previous/name).is_file():shutil.copyfile(previous/name,out/name)
 def run(label,argv,expected=0,stdout=None,env=None):
  if label in retained:
   row=retained[label];runs.append(dict(label=label,retained_from=str(previous),original=row,exit_status=row['exit_status'],expected=expected))
   for stream in ['stdout','stderr']:shutil.copyfile(previous/(label+'.'+stream),out/(label+'.'+stream))
   return (out/(label+'.stdout')).read_bytes()
  left=deadline-time.monotonic()
  if left<1:raise RuntimeError('whole-check deadline')
  start=time.monotonic();r=bounded_run(argv,cwd=ROOT,timeout=min(180,left),env={**os.environ,'ASAN_OPTIONS':'detect_leaks=0:halt_on_error=1','UBSAN_OPTIONS':'halt_on_error=1',**(env or {})});(stdout or out/(label+'.stdout')).write_bytes(r.stdout);(out/(label+'.stderr')).write_bytes(r.stderr);runs.append(dict(label=label,argv=list(map(str,argv)),exit_status=r.returncode,expected=expected,seconds=time.monotonic()-start))
  if r.returncode!=expected:raise RuntimeError(label+': '+r.stderr.decode(errors='replace')[-1800:])
  return r.stdout
 try:
  run('checks',[compiler,'test',ROOT/'coax/lib/systemone/outcomes.coax','--library','--evidence',out/'checks.json'])
  run('mutation',[compiler,'mutate',ROOT/'coax/lib/systemone/outcomes.coax','--library','--root-only','--jobs','2','--evidence',out/'mutation.json'])
  for name,(code,line) in NEGATIVE.items():
   raw=run(name,[compiler,'check',HERE/'systemone-outcomes'/(name+'.coax'),'--library','--json'],1);diagnostics=[json.loads(row) for row in (raw+(out/(name+'.stderr')).read_bytes()).splitlines() if row.strip()]
   if len(diagnostics)!=1 or diagnostics[0]['code']!=code or diagnostics[0]['line']!=line:raise RuntimeError('unexpected diagnostic '+name)
  generated=out/'systemone-outcomes.c';run('emit',[compiler,'c',HERE/'systemone-outcomes.coax'],stdout=generated)
  expected=(HERE/'systemone-outcomes.out').read_bytes()
  for mode,flags in [('O0',['-O0']),('O2',['-O2']),('sanitized',['-O1','-g','-fsanitize=address,undefined','-fno-sanitize-recover=all','-fno-omit-frame-pointer'])]:
   child=out/('child-'+mode);binary=out/mode;run(mode+'-child',[cc,'-std=c11',*flags,HERE/'systemone-outcomes-child.c','-o',child]);run(mode+'-compile',[cc,'-std=c11',*flags,'-ffp-contract=off',generated,ROOT/'coax/platform/process-native.c','-lm','-o',binary]);actual=run(mode+'-run',[binary],env={'COAX_SYSTEMONE_TEST_CHILD':str(child)})
   if actual!=expected:raise RuntimeError(mode+' transcript differs from literal oracle')
  after={path:dict(bytes=(ROOT/path).stat().st_size,sha256=sha(ROOT/path)) for path in before};proof['source_after']=after;proof['inputs_unchanged']=before==after;proof['tools_unchanged']=all(sha(path)==h for path,h in tools.items())
  if not proof['inputs_unchanged'] or not proof['tools_unchanged']:raise RuntimeError('source/tool drift')
  proof['status']='passed'
 except BaseException as exc:proof.update(status='failed',error=str(exc));raise
 finally:proof['artifacts']={str(path.relative_to(out)):sha(path) for path in out.rglob('*') if path.is_file()};(out/'qualification.json').write_text(json.dumps(proof,indent=2)+'\n')
 print(json.dumps(dict(status=proof['status'],receipt=str(out/'qualification.json'))))
if __name__=='__main__':main()
