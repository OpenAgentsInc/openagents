#!/usr/bin/env python3
# Firm reason: 2 (independent emitted-artifact, source and receipt equality oracle).
"""Rebind frozen B3 native observations only when actual final emitted artifacts match."""
import argparse,hashlib,json,os,subprocess,sys,time
from pathlib import Path
ROOT=Path('/tmp/openagents-12625-complete-20261101').resolve()
sys.path.insert(0,str(ROOT/'coax/tests'))
from b2_support import bounded_run,closure,source_inventory

def sha(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()
def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--compiler',type=Path,required=True);p.add_argument('--out',type=Path,required=True);a=p.parse_args();compiler=a.compiler.resolve();out=a.out.resolve();out.mkdir(parents=True,exist_ok=False);deadline=time.monotonic()+830;runs=[]
 previous=Path('/tmp/coax-12625-native/v3/coax');previous_sha='67e753aec35af4d35b31988fe2962ccb04f0b42ece3e86d465511972426dab80'
 if sha(previous)!=previous_sha:raise RuntimeError('prior image drift')
 current_sha=sha(compiler)
 if current_sha==previous_sha:raise RuntimeError('same compiler already qualified; retain its existing receipts')
 old={
  'typed':Path('/tmp/b3-rebased-975b-typed'),
  'raw-owner':Path('/tmp/b3-rebased-975b-owner'),
  'lab':Path('/tmp/b3-rebased-975b-lab-emission'),
  'factory':Path('/tmp/b3-rebased-975b-factory/generated'),
  'entry-red':Path('/tmp/b3-rebased-975b-entry/red'),
  'entry-green':Path('/tmp/b3-rebased-975b-entry/green')}
 profiles=[('raw-owner',ROOT/'coax/tests/systemone-owner.coax','systemone-owner',True),('typed',ROOT/'coax/tests/systemone-outcomes.coax','systemone-outcomes',False),('lab',old['lab']/'subset.coax','lab-consumer',True),('factory',old['factory']/'subset.coax','zeron',True),('entry-red',old['entry-red']/'subset.coax','lab-owner',True),('entry-green',old['entry-green']/'subset.coax','lab-owner',True)]
 files=closure(ROOT/'coax/tests/systemone-outcomes.coax');files.update(closure(ROOT/'apps/openagents-desktop/zeron/settings.coax'));before=source_inventory(files)
 inputs={str(path):sha(path) for path in [Path(__file__),*[row[1] for row in profiles]]}
 receipts={str(path):sha(path) for path in [Path('/tmp/b3-rebased-975b-typed/qualification.json'),Path('/tmp/b3-rebased-975b-lab-native/qualification.json'),Path('/tmp/b3-rebased-975b-factory/receipt.json'),Path('/tmp/b3-rebased-975b-entry/receipt.json'),Path('/tmp/b3-rebased-975b-owner/qualification.json')]}
 native_inputs={};native_tools={}
 def pin_source(path,identity):
  current=Path(path) if Path(path).is_absolute() else ROOT/path
  expected=identity['sha256'] if isinstance(identity,dict) else identity
  if sha(current)!=expected:raise RuntimeError('fresh native input drift: '+str(current))
  native_inputs[str(current)]=expected
 for receipt_path in receipts:
  receipt=json.loads(Path(receipt_path).read_text())
  for path,identity in receipt.get('source_before',{}).items():pin_source(path,identity)
  for path,identity in receipt.get('inputs',{}).items():pin_source(path,identity)
  for path,identity in receipt.get('tooling',{}).items():pin_source(path,identity)
  for row in receipt.get('projection',{}).get('files',[]):pin_source(row['path'],row['sha256'])
  for row in receipt.get('emission',{}).get('sources',[]):pin_source(row['path'],row['sha256'])
  for path,identity in receipt.get('input_stability',{}).get('owned_before',{}).items():pin_source(path,identity)
  for field in ['tools','tools_before']:
   for path,identity in receipt.get(field,{}).items():
    if sha(path)!=identity:raise RuntimeError('fresh native tool drift: '+path)
    native_tools[path]=identity
  tool=receipt.get('native_toolchain',{})
  if 'cc_path' in tool:
   if sha(tool['cc_path'])!=tool['cc_sha256']:raise RuntimeError('C compiler drift')
   native_tools[tool['cc_path']]=tool['cc_sha256']
 proof={'schema':1,'scope':__doc__,'status':'running','compiler':{'path':str(compiler),'sha256':current_sha},'prior_compiler':{'path':str(previous),'sha256':previous_sha},'source_before':before,'inputs_before':inputs,'prior_receipts':receipts,'runs':runs,'artifact_equivalence':[],'native_reexecuted':False,'fresh_native_inputs':native_inputs,'fresh_native_tools':native_tools}
 def save():
  proof['artifacts']={str(path.relative_to(out)):sha(path) for path in out.rglob('*') if path.is_file() and path.name!='qualification.json'};(out/'qualification.json').write_text(json.dumps(proof,indent=2)+'\n')
 def run(label,argv,expected=0,stdout=None):
  save();left=deadline-time.monotonic()
  if left<1:raise RuntimeError('whole-command deadline')
  started=time.monotonic();r=bounded_run(argv,cwd=ROOT,timeout=min(180,left),env={**os.environ,'ASAN_OPTIONS':'detect_leaks=0:halt_on_error=1','UBSAN_OPTIONS':'halt_on_error=1'});(stdout or out/(label+'.stdout')).write_bytes(r.stdout);(out/(label+'.stderr')).write_bytes(r.stderr);runs.append({'label':label,'argv':list(map(str,argv)),'exit_status':r.returncode,'expected':expected,'seconds':time.monotonic()-started});save()
  if r.returncode!=expected:raise RuntimeError(label+': '+r.stderr.decode(errors='replace')[-1800:])
  return r
 try:
  for label,source,stem,library in profiles:
   source=source.resolve();folder=out/label;folder.mkdir();args=[compiler,'c',source]
   if library:args+=['--library','--header',folder/(stem+'.h')]
   run(label+'-emit',args,stdout=folder/(stem+'.c'))
   for suffix in (['.c','.h'] if library else ['.c']):
    name=stem+suffix;prior=old[label]/name;actual=folder/name;row={'profile':label,'source':str(source),'source_sha256':sha(source),'artifact':name,'prior':str(prior),'prior_sha256':sha(prior),'actual_sha256':sha(actual),'bytes':actual.stat().st_size,'equal':prior.read_bytes()==actual.read_bytes()};proof['artifact_equivalence'].append(row)
    if not row['equal']:raise RuntimeError('emitted artifact differs: '+label+'/'+name+'; affected native observations must be rerun')
  run('typed-checks',[compiler,'test',ROOT/'coax/lib/systemone/outcomes.coax','--library','--evidence',out/'typed-checks.json'])
  run('typed-mutation',[compiler,'mutate',ROOT/'coax/lib/systemone/outcomes.coax','--library','--root-only','--jobs','2','--evidence',out/'typed-mutation.json'])
  run('owner-policy-mutation',[compiler,'mutate',ROOT/'coax/lib/systemone/owner-policy.coax','--library','--root-only','--jobs','2','--evidence',out/'owner-policy-mutation.json'])
  for name,code,line in [('ignored-start','E0907',2),('incomplete-terminal','E0904',3),('local-snapshot','E0383',4)]:
   r=run(name,[compiler,'check',ROOT/'coax/tests/systemone-outcomes'/(name+'.coax'),'--library','--json'],1);data=[json.loads(row) for row in (r.stdout+r.stderr).splitlines() if row.strip()]
   if len(data)!=1 or data[0]['code']!=code or data[0]['line']!=line:raise RuntimeError('diagnostic changed: '+name)
  proof['source_after']=source_inventory({**closure(ROOT/'coax/tests/systemone-outcomes.coax'),**closure(ROOT/'apps/openagents-desktop/zeron/settings.coax')});proof['inputs_unchanged']=proof['source_after']==before and all(sha(path)==h for path,h in inputs.items());proof['tools_unchanged']=sha(compiler)==current_sha and sha(previous)==previous_sha;proof['prior_receipts_unchanged']=all(sha(path)==h for path,h in receipts.items());proof['native_inputs_unchanged']=all(sha(path)==h for path,h in native_inputs.items()) and all(sha(path)==h for path,h in native_tools.items())
  if not all(proof[key] for key in ['inputs_unchanged','tools_unchanged','prior_receipts_unchanged','native_inputs_unchanged']):raise RuntimeError('input/tool/receipt drift')
  proof['status']='passed'
 except BaseException as exc:proof.update(status='failed',error=str(exc));raise
 finally:save()
 print(json.dumps({'status':proof['status'],'emitted_artifacts':len(proof['artifact_equivalence']),'native_reexecuted':False,'receipt':str(out/'qualification.json')}))
if __name__=='__main__':main()
