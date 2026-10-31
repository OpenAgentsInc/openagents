#!/usr/bin/env python3
"""Protoc to Coax MP consistency; synthetic signatures grant no authority."""
# Firm reason: 3 (runs protoc 29.3, the reference protobuf implementation we test against)
import copy, ctypes as C, hashlib, json, os, pathlib, subprocess, sys, tempfile
R=pathlib.Path(__file__).resolve().parents[3]
compiler=os.environ.get('COAX_TEST_BIN',str(R/'coax/bin/coax'))
protoc=os.environ.get('PROTOC','protoc')
def run(*args,**kw):return subprocess.run(list(map(str,args)),check=True,**kw)
if run(protoc,'--version',capture_output=True).stdout.strip()!=b'libprotoc 29.3':raise SystemExit('requires protoc 29.3')
fixture=json.loads((R/'nips/block/NIP-MP.fixtures.json').read_text())
byname={x['name']:x['template'] for x in fixture['cases']}
with tempfile.TemporaryDirectory(prefix='coax-mp-adapter-') as tmp:
 d=pathlib.Path(tmp);gen=d/'gen';gen.mkdir()
 run(protoc,'-I'+str(R/'proto'),'--python_out='+str(gen),*[R/'proto'/x for x in ['openagents/options/v1alpha1/options.proto','nostr/event/v1alpha1/event.proto','nostr/block/mp/v1alpha1/project.proto']])
 sys.path.insert(0,str(gen));from nostr.block.mp.v1alpha1.project_pb2 import Project
 def project(template):
  event={'id':'00'*32,'pubkey':'11'*32,'created_at':0,'kind':30621,'tags':template['tags'],'content':template['content'],'sig':'22'*64,'verified':True,'authorized':True}
  p=Project();p.original.original_json=json.dumps(event,separators=(',',':'),ensure_ascii=False).encode();v=p.original.event
  v.id=bytes.fromhex(event['id']);v.pubkey=bytes.fromhex(event['pubkey']);v.created_at=0;v.kind=event['kind'];v.content=event['content'];v.sig=bytes.fromhex(event['sig'])
  for tag in event['tags']:
   v.tags.add().values.extend(tag)
   if len(tag)<2:continue
   name,value=tag[:2]
   if name=='d':p.identifier=value
   elif name=='name':p.name=value
   elif name=='description':p.description=value
   elif name=='buzz-channel':p.channel=value
   elif name=='buzz-visibility':p.visibility=value
   elif name=='a':
    m=p.members.add();m.coordinate=value
    if len(tag)>2:m.relay_hint=tag[2]
  return p
 full=project(byname['valid_full']);mini=project(byname['valid_minimal']);cases=[]
 def add(name,p,ok=True,wire_extra=b''):
  cases.append({'name':name,'wire':list(p.SerializeToString(deterministic=True)+wire_extra),'original':list(p.original.original_json),'consistent':ok})
 add('full synthetic event; authority flags are ignored',full)
 add('minimal',mini)
 add('unknown protobuf flag',full,wire_extra=bytes([0xc0,5,1]))
 template=copy.deepcopy(byname['valid_full'])
 for tag in template['tags']:
  if tag[0]=='a':tag.append('');break
 add('explicitly empty hint',project(template))
 template=copy.deepcopy(byname['valid_full'])
 for tag in template['tags']:
  if tag[0]=='name':tag.append('preserved');break
 add('singleton extra is preserved',project(template))
 template=copy.deepcopy(byname['valid_full']);template['tags'].append(copy.deepcopy(next(t for t in template['tags'] if t[0]=='a')))
 add('duplicate coordinate remains untrusted',project(template))
 for field in ('identifier','name','description','channel','visibility'):
  p=copy.deepcopy(full);setattr(p,field,getattr(p,field)+'!');add('mismatch '+field,p,False)
 for field in ('name','description','channel','visibility'):
  p=copy.deepcopy(full);p.ClearField(field);add('missing '+field,p,False)
 p=copy.deepcopy(full);p.members[0].coordinate+='!';add('coordinate mismatch',p,False)
 p=copy.deepcopy(full);p.members[0].relay_hint='';add('added empty hint',p,False)
 p=copy.deepcopy(full);p.members.add(coordinate='extra');add('extra member',p,False)
 p=copy.deepcopy(full);first=copy.deepcopy(p.members[0]);second=copy.deepcopy(p.members[1]);p.members[0].CopyFrom(second);p.members[1].CopyFrom(first);add('member order mismatch',p,False)
 p=copy.deepcopy(full);p.original.event.kind=1;add('original and event mismatch',p,False)
 p=copy.deepcopy(full);p.original.original_json=b'{}';add('bad source',p,False)
 wire=full.SerializeToString(deterministic=True)
 decoded=run(protoc,'-I'+str(R/'proto'),'--decode=nostr.block.mp.v1alpha1.Project',R/'proto/nostr/block/mp/v1alpha1/project.proto',input=wire,capture_output=True).stdout
 assert b'identifier: "platform"' in decoded and b'buzz-infra' in decoded
 path=d/'cases.json';path.write_text(json.dumps(cases))
 for mode,flags in [('checked',[]),('release',['--release'])]:
  c=d/(mode+'.c');h=d/'mp_adapter.h';c.write_bytes(run(compiler,'c',R/'coax/tests/nostr/mp_adapter_target.coax','--library','--header',h,'--run-steps','250000000',*flags,capture_output=True).stdout)
  common=['-I'+str(d),str(c),str(R/'coax/tests/nostr/mp_adapter_target.c')]
  lib=d/(mode+'.dylib');run('cc','-std=c11','-O1','-shared','-fPIC',*common,'-o',lib)
  obj=C.CDLL(str(lib));obj.mpa_input.restype=C.POINTER(C.c_uint8);obj.mpa_output.restype=C.POINTER(C.c_uint8);obj.mpa_test_run.argtypes=[C.c_int64];obj.mpa_test_run.restype=C.c_int64
  for case in cases:
   b=bytes(case['wire']);C.memmove(obj.mpa_input(),b,len(b));n=obj.mpa_test_run(len(b));assert (n>=0)==case['consistent'],case['name']
   if n>=0:assert C.string_at(obj.mpa_output(),n)==bytes(case['original']),case['name']
  good=bytes(cases[0]['wire']);raw=bytes(cases[0]['original'])
  main=d/'main.c';main.write_text('#include <stdint.h>\n#include <string.h>\nuint8_t *mpa_input(void),*mpa_output(void);int64_t mpa_test_run(int64_t);\nstatic uint8_t wire[]={'+','.join(map(str,good))+'};\nstatic uint8_t raw[]={'+','.join(map(str,raw))+'};\nint main(void){memcpy(mpa_input(),wire,sizeof wire);return mpa_test_run(sizeof wire)!=(int64_t)sizeof raw||memcmp(mpa_output(),raw,sizeof raw);}\n')
  exe=d/(mode+'-san');run('cc','-std=c11','-O1','-fsanitize=address,undefined','-fno-sanitize-recover=all',*common,main,'-o',exe);run(exe)
  wasm=d/(mode+'.wasm');run(os.environ.get('COAXIAL_CLANG','/opt/homebrew/opt/llvm/bin/clang'),'--target=wasm32-wasip1','--sysroot='+os.environ.get('COAXIAL_SYSROOT','/opt/homebrew/opt/wasi-libc/share/wasi-sysroot'),'-resource-dir='+os.environ.get('COAXIAL_RESOURCES','/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes'),'-std=c11','-O2','-mexec-model=reactor',*common,'-Wl,-z,stack-size=2097152,--export=mpa_input,--export=mpa_output,--export=mpa_test_run','-o',wasm)
  run('node',R/'coax/tests/nostr/mp_adapter_wasm.cjs',wasm,path)
  print(mode,len(cases),'protoc/native/Wasm cases; C SHA-256',hashlib.sha256(c.read_bytes()).hexdigest(),flush=True)
