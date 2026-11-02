#!/usr/bin/env python3
# Firm reason: 2 (independent compiler, ABI, diagnostic and native-effect oracles; no product implementation).
"""New canonical owner + real native host; reuse exact already-passed emission."""
import argparse,hashlib,json,os,shutil,time
from pathlib import Path
from b2_support import ROOT,closure,source_inventory,bounded_run
HERE=Path(__file__).resolve().parent
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--generated-dir',type=Path,required=True);p.add_argument('--out',type=Path,required=True);a=p.parse_args()
 old=a.generated_dir.resolve();prior=json.loads((old/'qualification.json').read_text());out=a.out.resolve();out.mkdir(parents=True,exist_ok=False);cc=Path(shutil.which(os.environ.get('COAX_CC','cc'))).resolve();files=closure(HERE/'systemone-owner.coax');before=source_inventory(files)
 if prior['status']!='passed' or any(prior['source_before'].get(path)!=identity for path,identity in before.items()):raise RuntimeError('owner source differs from passed emission')
 for name in ['systemone-owner.c','systemone-owner.h']:
  if sha(old/name)!=prior['artifacts'][name]:raise RuntimeError('generated artifact drift')
  shutil.copyfile(old/name,out/name)
 inputs={str(p.relative_to(ROOT)):sha(p) for p in [HERE/'systemone-owned-process.c',Path(__file__),HERE/'b2_support.py',HERE/'systemone-fakecurl.c',ROOT/'coax/platform/process-native.c',ROOT/'coax/platform/process-native.h']};tools={str(cc):sha(cc)};runs=[];deadline=time.monotonic()+800
 proof=dict(schema=1,status='running',scope=__doc__,owner_inputs=before,inputs=inputs,tools=tools,reused_emission=dict(receipt=str(old/'qualification.json'),sha256=sha(old/'qualification.json')),runs=runs)
 def run(label,argv):
  left=deadline-time.monotonic()
  if left<1:raise RuntimeError('whole deadline')
  start=time.monotonic();r=bounded_run(argv,cwd=ROOT,timeout=min(180,left),env={**os.environ,'ASAN_OPTIONS':'detect_leaks=0:halt_on_error=1','UBSAN_OPTIONS':'halt_on_error=1'});(out/(label+'.stdout')).write_bytes(r.stdout);(out/(label+'.stderr')).write_bytes(r.stderr);runs.append(dict(label=label,argv=list(map(str,argv)),exit_status=r.returncode,seconds=time.monotonic()-start))
  if r.returncode:raise RuntimeError(label+': '+r.stderr.decode(errors='replace')[-1200:])
 try:
  for mode,flags in [('O0',['-O0']),('O2',['-O2']),('sanitized',['-O1','-g','-fsanitize=address,undefined','-fno-sanitize-recover=all','-fno-omit-frame-pointer'])]:
   child=out/('child-'+mode);binary=out/mode;run(mode+'-child',[cc,'-std=c11',*flags,HERE/'systemone-fakecurl.c','-o',child]);run(mode+'-compile',[cc,'-std=c11',*flags,'-ffp-contract=off','-I',out,'-I',ROOT/'coax/platform',out/'systemone-owner.c',ROOT/'coax/platform/process-native.c',HERE/'systemone-owned-process.c','-lm','-o',binary]);run(mode+'-run',[binary,child])
  if before!=source_inventory(closure(HERE/'systemone-owner.coax')) or any(sha(ROOT/path)!=h for path,h in inputs.items()) or any(sha(path)!=h for path,h in tools.items()):raise RuntimeError('source/tool drift')
  proof['status']='passed'
 except BaseException as exc:proof.update(status='failed',error=str(exc));raise
 finally:proof['artifacts']={str(path.relative_to(out)):sha(path) for path in out.rglob('*') if path.is_file()};(out/'qualification.json').write_text(json.dumps(proof,indent=2)+'\n')
 print(json.dumps(dict(status=proof['status'],receipt=str(out/'qualification.json'))))
if __name__=='__main__':main()
