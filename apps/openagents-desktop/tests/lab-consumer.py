#!/usr/bin/env python3
# Firm reason: 2 (independent compiler, ABI, diagnostic and native-effect oracles; no product implementation).
"""Bounded exact-source actual Lab request consumer; not a full application build."""
import argparse,datetime,hashlib,json,os,re,shutil,subprocess,time
from pathlib import Path
import zeron_source_extract as extract
ROOT=Path(__file__).resolve().parents[3]
HERE=Path(__file__).resolve().parent
STUBS={'cl_getenv':'return "";','e_init':'','e_set_model':'','e_forward':'lab_consumer_effect(1);',
 'ps_configure':'lab_consumer_effect(2);','ps_forward':'lab_consumer_effect(3);','ps_url':'return "";',
 'gs_want':'lab_consumer_effect(4); return false;','lab_mc_start':'lab_consumer_effect(5); return "";',
 'lab_mc_pump':'lab_consumer_effect(6);','lab_mc_stop':'lab_consumer_effect(7); return false;'}
def sha(raw):return hashlib.sha256(raw).hexdigest()
def procedure(raw):
 clean=extract.mask(raw);start=clean.index('{');depth=1
 for end in range(start+1,len(clean)):
  if clean[end]=='{':depth+=1
  elif clean[end]=='}':
   depth-=1
   if not depth:return raw[:end+1]+'\n'
 raise RuntimeError('unterminated procedure')
def projection():
 index,files=extract.declarations(ROOT);selected={};stubs=[]
 def add(name):
  if name in selected:return
  row=dict(index[name]);selected[name]=row
  if row['kind']=='procedure':
   if '{' in extract.mask(row['text']):row['text']=procedure(row['text'])
   if name in STUBS:
    original=row['text'];signature=original[:extract.mask(original).index('{')].rstrip();row['text']=signature+' {'+STUBS[name]+'}\n'
    stubs.append(dict(name=name,path=row['path'],line=row['line'],signature=signature,source_sha256=sha(original.encode()),stub_sha256=sha(row['text'].encode())))
  clean=extract.mask(row['text'])
  for token in sorted(set(re.findall(r'\b[A-Za-z_]\w*\b',clean))):
   if token in index and index[token]['kind']!='procedure':add(token)
  for call in re.findall(r'\b([A-Za-z_]\w*)\s*\(',clean):
   if call in index:add(call)
 roots=['Lab','Session','Engine','PsEngine','GpuSwitch','lab_attach','lab_ask','lab_pump','lab_stop','s_init','s_user','s_text_of','LAB_TEST_ANSWER','so_owned_discard','so_request_cancel']
 for name in roots:add(name)
 helpers='''
lab_consumer_effect :: (code:int) #foreign;
lab_consumer_init :: (l:[]Lab,s:[]Session,e:[]Engine,p:[]PsEngine,g:[]GpuSwitch,body:[]u8,out:[]u8,err:[]u8,pool:[]u8) #export_c {
 l[0]=Lab.{};g[0]=GpuSwitch.{};s_init(s,pool);lab_attach(l,body,out,err);
 l[0].url_n=e_set(l[0].url[..],0,SO_PSIONIC_URL);s_user(s,"fix the failing test");l[0].part=0;
}
lab_consumer_ask :: (l:[]Lab,s:[]Session) #export_c {lab_ask(l,s);}
lab_consumer_pump :: (l:[]Lab,e:[]Engine,p:[]PsEngine,g:[]GpuSwitch,s:[]Session) #export_c {lab_pump(l,e,p,g,s,0.016);}
lab_consumer_stop :: (l:[]Lab,s:[]Session)->bool #export_c {return lab_stop(l,s);}
lab_consumer_text :: (s:[]Session,i:int)->string #export_c {return s_text_of(s,i);}
lab_consumer_fixture :: ()->string #export_c {return LAB_TEST_ANSWER;}
lab_consumer_stale :: (l:[]Lab,ticket:int)->bool #export_c {
 switch so_request_discard(l[0].owner[..],ticket) {case ok {return true;}case err(failure) {return false;}}
}
lab_consumer_cancel_ticket :: (l:[]Lab,ticket:int)->bool #export_c {
 switch so_request_cancel(l[0].owner[..],ticket) {case ok {return true;}case err(failure) {return false;}}
}
lab_consumer_again :: (l:[]Lab,s:[]Session) #export_c {
 s_turn_end(s,false,"");s_user(s,"next request");l[0].part=s[0].n-1;lab_ask(l,s);
}
'''
 text='#import "Basic";\n'+'\n'.join(row['text'] for row in selected.values())+helpers
 manifest=dict(scope=__doc__,roots=roots,files=files,declarations=[{k:v for k,v in row.items() if k!='text'} for row in selected.values()],stubs=stubs,subset_sha256=sha(text.encode()))
 return text,manifest

def main():
 parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--coax',required=True,type=Path);parser.add_argument('--out',required=True,type=Path);parser.add_argument('--generated-dir',type=Path,help='reuse a hash-verified successful emission only');parser.add_argument('--emit-only',action='store_true',help='produce the exact test projection for an independent host harness');args=parser.parse_args()
 out=args.out.resolve();out.mkdir(parents=True,exist_ok=False);compiler=args.coax.resolve();cc=Path(shutil.which(os.environ.get('COAX_CC','cc'))).resolve();deadline=time.monotonic()+800;runs=[]
 source,manifest=projection();(out/'subset.coax').write_text(source);(out/'projection.json').write_text(json.dumps(manifest,indent=2)+'\n')
 tools={str(path):sha(path.read_bytes()) for path in [compiler,cc]};tooling={str(path.relative_to(ROOT)):sha(path.read_bytes()) for path in [Path(__file__),HERE/'lab-consumer.c',HERE/'zeron_source_extract.py']}
 proof=dict(schema=1,emission_only=args.emit_only,status='running',recorded_utc=datetime.datetime.now(datetime.timezone.utc).isoformat(),projection=manifest,tools=tools,tooling=tooling,runs=runs)
 def run(label,argv,stdout=None):
  left=deadline-time.monotonic()
  if left<1:raise RuntimeError('whole-command deadline')
  start=time.monotonic();r=subprocess.run(list(map(str,argv)),cwd=ROOT,env={**os.environ,'ASAN_OPTIONS':'detect_leaks=0:halt_on_error=1','UBSAN_OPTIONS':'halt_on_error=1'},capture_output=True,timeout=min(180,left))
  (stdout or out/(label+'.stdout')).write_bytes(r.stdout);(out/(label+'.stderr')).write_bytes(r.stderr)
  runs.append(dict(label=label,argv=list(map(str,argv)),exit_status=r.returncode,seconds=time.monotonic()-start,stdout_sha256=sha(r.stdout),stderr_sha256=sha(r.stderr)))
  if r.returncode:raise RuntimeError(label+': '+r.stderr.decode(errors='replace')[-1400:])
 try:
  if args.generated_dir:
   previous=args.generated_dir.resolve();prior=json.loads((previous/'qualification.json').read_text())
   if prior['projection']!=manifest or prior['tools']!=tools or not any(r['label']=='emit' and r['exit_status']==0 for r in prior['runs']):raise RuntimeError('prior emission identity differs')
   for name in ['lab-consumer.c','lab-consumer.h']:
    raw=(previous/name).read_bytes()
    if sha(raw)!=prior['artifacts'][name]:raise RuntimeError('prior artifact drift')
    (out/name).write_bytes(raw)
   proof['reused_emission']=dict(receipt=str(previous/'qualification.json'),sha256=sha((previous/'qualification.json').read_bytes()))
  else:run('emit',[compiler,'c',out/'subset.coax','--library','--header',out/'lab-consumer.h'],out/'lab-consumer.c')
  for mode,flags in ([] if args.emit_only else [('O0',['-O0']),('O2',['-O2']),('sanitized',['-O1','-g','-fsanitize=address,undefined','-fno-sanitize-recover=all','-fno-omit-frame-pointer'])]):
   binary=out/mode;run(mode+'-compile',[cc,'-std=c11',*flags,'-ffp-contract=off','-I',out,out/'lab-consumer.c',HERE/'lab-consumer.c','-lm','-o',binary]);run(mode+'-run',[binary])
  after,after_manifest=projection()
  if after!=source or after_manifest!=manifest:raise RuntimeError('source drift')
  if any(sha(Path(path).read_bytes())!=identity for path,identity in tools.items()):raise RuntimeError('tool drift')
  if any(sha((ROOT/path).read_bytes())!=identity for path,identity in tooling.items()):raise RuntimeError('test tooling drift')
  proof['status']='passed'
 except BaseException as exc:proof.update(status='failed',error=str(exc));raise
 finally:
  proof['artifacts']={str(p.relative_to(out)):sha(p.read_bytes()) for p in out.rglob('*') if p.is_file()};(out/'qualification.json').write_text(json.dumps(proof,indent=2)+'\n')
 print(json.dumps(dict(status=proof['status'],runs=len(runs),receipt=str(out/'qualification.json'))))
if __name__=='__main__':main()
