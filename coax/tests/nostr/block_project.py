#!/usr/bin/env python3
"""NIP-MP typed wire interoperability, not signature or project admission."""
import json, os, shutil, subprocess, sys, tempfile
from pathlib import Path
ROOT=Path(__file__).resolve().parents[3]
PROTOC=os.environ.get('PROTOC') or shutil.which('protoc')
if not PROTOC: raise SystemExit('set PROTOC to pinned protoc 29.3')
COAX=os.environ.get('COAX_TEST_BIN',str(ROOT/'coax/bin/coax'))
if not __debug__: raise SystemExit('run without optimized Python')
def run(*args, data=None):
 p=subprocess.run(list(map(str,args)),input=data,capture_output=True)
 if p.returncode:raise RuntimeError(f'{args[0]} failed {p.returncode}: {p.stderr[-2000:].decode(errors="replace")}')
 return p.stdout
if run(PROTOC,'--version').strip()!=b'libprotoc 29.3':raise RuntimeError('requires pinned protoc 29.3')
fixtures=json.loads((ROOT/'nips/block/NIP-MP.fixtures.json').read_text())
byname={case['name']:case for case in fixtures['cases']}
with tempfile.TemporaryDirectory(prefix='coax-mp-') as tmp:
 d=Path(tmp);gen=d/'python';gen.mkdir()
 include=Path(PROTOC).resolve().parent.parent/'include'
 for file in ['openagents/options/v1alpha1/options.proto','nostr/event/v1alpha1/event.proto','nostr/block/mp/v1alpha1/project.proto']:
  run(PROTOC,'-I',ROOT/'proto','-I',include,'--python_out='+str(gen),ROOT/'proto'/file)
 sys.path.insert(0,str(gen))
 from nostr.block.mp.v1alpha1.project_pb2 import Project
 def project(template):
  tags=template['tags'];e={'id':'00'*32,'pubkey':'11'*32,'created_at':0,'kind':30621,'tags':tags,'content':template['content'],'sig':'22'*64}
  p=Project();p.original.original_json=json.dumps(e,ensure_ascii=False,separators=(',',':')).encode()
  v=p.original.event;v.id=bytes.fromhex(e['id']);v.pubkey=bytes.fromhex(e['pubkey']);v.created_at=0;v.kind=e['kind'];v.content=e['content'];v.sig=bytes.fromhex(e['sig'])
  for tag in tags:v.tags.add().values.extend(tag)
  for tag in tags:
   if tag[0]=='d':p.identifier=tag[1]
   elif tag[0]=='name':p.name=tag[1]
   elif tag[0]=='description':p.description=tag[1]
   elif tag[0]=='buzz-channel':p.channel=tag[1]
   elif tag[0]=='buzz-visibility':p.visibility=tag[1]
   elif tag[0]=='a':
    m=p.members.add();m.coordinate=tag[1]
    if len(tag)>2:m.relay_hint=tag[2]
  return p
 samples=[project(byname[n]['template']) for n in ('valid_minimal','valid_full','valid_member_dtag_contains_colon','valid_uninterpreted_metadata_values','valid_member_cap_boundary')]
 expected=samples[1].SerializeToString(deterministic=True)
 text=run(PROTOC,'-I',ROOT/'proto','-I',include,'--decode=nostr.block.mp.v1alpha1.Project',ROOT/'proto/nostr/block/mp/v1alpha1/project.proto',data=expected)
 assert b'identifier: "platform"' in text and b'buzz-infra' in text
 runner=d/'run.mjs';runner.write_text("import{WASI}from'node:wasi';import{readFileSync}from'node:fs';const w=new WASI({version:'preview1',args:process.argv.slice(2),env:{},preopens:{}});const{instance}=await WebAssembly.instantiate(readFileSync(process.argv[2]),{wasi_snapshot_preview1:w.wasiImport});w.start(instance);\n")
 for mode,flags in [('checked',[]),('release',['--release'])]:
  src=d/(mode+'.c');h=d/'block_project.h';src.write_bytes(run(COAX,'c',ROOT/'coax/tests/nostr/mp_projection_checks.coax','--library','--header',h,*flags))
  common=['-std=c11','-O1','-I'+str(d),src,ROOT/'coax/tests/nostr/block_project_driver.c','-lm'];exe=d/(mode+'-san')
  run('cc',*common,'-fsanitize=address,undefined','-fno-sanitize-recover=all','-o',exe)
  clang=os.environ.get('COAXIAL_CLANG','/opt/homebrew/opt/llvm/bin/clang');sysroot=os.environ.get('COAXIAL_SYSROOT','/opt/homebrew/opt/wasi-libc/share/wasi-sysroot');resources=os.environ.get('COAXIAL_RESOURCES','/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes')
  wasm=d/(mode+'.wasm');run(clang,'--target=wasm32-wasip1','--sysroot='+sysroot,'-resource-dir='+resources,*common,'-Wl,-z,stack-size=1048576,--max-memory=67108864','-o',wasm)
  for target in ([exe],['node','--no-warnings',runner,wasm]):
   def read(op,data):return int(run(*target,op,data=data))
   def copy(data):return run(*target,'copy',data=data)
   for p in samples:
    raw=p.SerializeToString(deterministic=True);assert read('0',raw)==0
    q=Project.FromString(copy(raw));assert q==p and q.original.original_json==p.original.original_json
   assert read('1',expected)==8 and read('2',expected)==2 and read('3',expected)==0
   assert read('2',samples[4].SerializeToString())==64
   hinted=project(byname['valid_full']['template']);hinted.members[0].relay_hint='';assert read('3',hinted.SerializeToString())==1
   assert read('0',Project(identifier='x').SerializeToString())!=0
   bounded=Project();bounded.CopyFrom(samples[0]);bounded.name='x'*256;assert read('0',bounded.SerializeToString())==0
   oversized=Project();oversized.CopyFrom(samples[0]);oversized.name='x'*257;assert read('0',oversized.SerializeToString())!=0
   too_many=Project();too_many.CopyFrom(samples[0]);[setattr(too_many.members.add(),'coordinate',str(i)) for i in range(65)];assert read('0',too_many.SerializeToString())!=0
   assert read('0',expected[:-1])!=0
   unknown=expected+b'\x98\x06\x01';assert read('0',unknown)==0 and read('4',unknown)==99 and b'\x98\x06\x01' in copy(unknown)
   nested=Project();nested.CopyFrom(samples[0]);nested.original.ParseFromString(nested.original.SerializeToString()+b'\x98\x06\x01');nested_wire=nested.SerializeToString();assert read('0',nested_wire)==0 and b'\x98\x06\x01' in Project.FromString(copy(nested_wire)).original.SerializeToString()
   # Structural decoding deliberately does not prove original/projection agreement.
   mismatch=Project();mismatch.CopyFrom(samples[0]);mismatch.identifier='other';assert read('0',mismatch.SerializeToString())==0
  print(mode,'pinned source fixtures, limits, presence, unknowns and protoc; native sanitizers and Wasm',flush=True)
