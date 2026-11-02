#!/usr/bin/env python3
# Firm reason: 2 (independent compiler, ABI, diagnostic and native-effect oracles; no product implementation).
"""Exact actual Lab consumer with real offline child effects and six owner generations."""
import argparse,hashlib,json,os,runpy,shutil,sys,time
from pathlib import Path
HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[2]
sys.path.insert(0,str(ROOT/'coax/tests'))
from b2_support import bounded_run

def sha(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()
def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--generated-dir',type=Path,required=True);p.add_argument('--out',type=Path,required=True);a=p.parse_args();old=a.generated_dir.resolve();out=a.out.resolve();out.mkdir(parents=True,exist_ok=False);prior=json.loads((old/'qualification.json').read_text());cc=Path(shutil.which(os.environ.get('COAX_CC','cc'))).resolve()
 producer=runpy.run_path(str(HERE/'lab-consumer.py'));source,manifest=producer['projection']()
 if prior['status']!='passed' or prior['projection']!=manifest or (old/'subset.coax').read_text()!=source:raise RuntimeError('current exact source projection differs')
 for name in ['subset.coax','projection.json','lab-consumer.c','lab-consumer.h']:
  if sha(old/name)!=prior['artifacts'][name]:raise RuntimeError('prior emitted artifact drift')
  shutil.copyfile(old/name,out/name)
 for path,identity in prior['tools'].items():
  if sha(path)!=identity:raise RuntimeError('prior producer drift')
 inputs={str(path.relative_to(ROOT)):sha(path) for path in [Path(__file__),HERE/'lab-consumer.py',HERE/'lab-consumer-process.c',HERE/'lab-consumer-process-child.c',HERE/'zeron_source_extract.py',ROOT/'coax/tests/b2_support.py',ROOT/'coax/platform/process-native.c',ROOT/'coax/platform/process-native.h']};tools={str(cc):sha(cc)};runs=[];deadline=time.monotonic()+800
 proof=dict(schema=1,status='running',scope=__doc__,projection=manifest,inputs=inputs,tools=tools,reused_emission=dict(receipt=str(old/'qualification.json'),sha256=sha(old/'qualification.json'),compiler=prior['tools']),runs=runs)
 def run(label,argv,env=None):
  left=deadline-time.monotonic()
  if left<1:raise RuntimeError('whole deadline')
  start=time.monotonic();r=bounded_run(argv,cwd=ROOT,timeout=min(180,left),env={**os.environ,'ASAN_OPTIONS':'detect_leaks=0:halt_on_error=1','UBSAN_OPTIONS':'halt_on_error=1',**(env or {})});(out/(label+'.stdout')).write_bytes(r.stdout);(out/(label+'.stderr')).write_bytes(r.stderr);runs.append(dict(label=label,argv=list(map(str,argv)),exit_status=r.returncode,seconds=time.monotonic()-start))
  if r.returncode:raise RuntimeError(label+': '+r.stderr.decode(errors='replace')[-1800:])
  return r.stdout
 try:
  for mode,flags in [('O0',['-O0']),('O2',['-O2']),('sanitized',['-O1','-g','-fsanitize=address,undefined','-fno-sanitize-recover=all','-fno-omit-frame-pointer'])]:
   work=out/mode;work.mkdir();child=work/'curl';binary=work/'check';run(mode+'-child',[cc,'-std=c11',*flags,HERE/'lab-consumer-process-child.c','-o',child]);run(mode+'-compile',[cc,'-std=c11',*flags,'-ffp-contract=off','-I',out,'-I',ROOT/'coax/platform',out/'lab-consumer.c',ROOT/'coax/platform/process-native.c',HERE/'lab-consumer-process.c','-lm','-o',binary]);raw=run(mode+'-run',[binary,work/'response.json'],env={'PATH':str(work)+os.pathsep+os.environ.get('PATH','')});observed=json.loads(raw)
   if observed!={'actual_lab_native_process_cases':6,'generations':6,'all_transcript_borrows_retained':True,'cancel_reaped':True,'stale_tickets_inert':True}:raise RuntimeError('incomplete native result')
  after,after_manifest=producer['projection']()
  if after!=source or after_manifest!=manifest or any(sha(ROOT/path)!=h for path,h in inputs.items()) or any(sha(path)!=h for path,h in tools.items()):raise RuntimeError('source/tool drift')
  proof['status']='passed'
 except BaseException as exc:proof.update(status='failed',error=str(exc));raise
 finally:proof['artifacts']={str(path.relative_to(out)):sha(path) for path in out.rglob('*') if path.is_file()};(out/'qualification.json').write_text(json.dumps(proof,indent=2)+'\n')
 print(json.dumps(dict(status=proof['status'],receipt=str(out/'qualification.json'))))
if __name__=='__main__':main()
