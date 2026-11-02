#!/usr/bin/env python3
"""NIP-GS typed wire interoperability: protoc 29.3 to the Coax adapter on native sanitizers and Wasm.
Consistency of armor and projection only; no key lifting, signature or trust decision."""
# Firm reason: 3 (runs protoc 29.3, the reference protobuf implementation we test against)
import os as _os,sys as _sys;_sys.path.insert(0,_os.path.dirname(_os.path.abspath(__file__)));from wasm_toolchain import WASM_CLANG,WASM_SYSROOT,WASM_RESOURCES
import os, pathlib, re, shutil, subprocess, sys, tempfile
R=pathlib.Path(__file__).resolve().parents[3]
PROTOC=os.environ.get('PROTOC') or shutil.which('protoc')
COAX=os.environ.get('COAX_TEST_BIN',str(R/'coax/bin/coax'))
def run(*args,data=None,env=None):
 p=subprocess.run(list(map(str,args)),input=data,capture_output=True,env=env)
 if p.returncode:raise RuntimeError(f'{args[0]} failed {p.returncode}: {p.stderr[-2000:].decode(errors="replace")}')
 return p.stdout
if run(PROTOC,'--version').strip()!=b'libprotoc 29.3':raise SystemExit('requires pinned protoc 29.3')
run(sys.executable,R/'coax/tests/nostr/generate_gs_signature_fixtures.py','--check',env=dict(os.environ,PROTOC=PROTOC))
fixtures={m.group(1):bytes(int(x) for x in m.group(2).split(',')) for m in re.finditer(r'^GS_FIX_(\w+) :: \[\d+\]u8\.\[([0-9,]+)\];$',(R/'coax/tests/nostr/gs_signature_fixtures.coax').read_text(),re.M)}
good={'OK','OWNER','TRAILING_LF','ZERO_T','MAX_T','CONDITIONS','FULL'}
assert len(fixtures)==37 and good<=set(fixtures)
with tempfile.TemporaryDirectory(prefix='coax-gs-') as tmp:
 d=pathlib.Path(tmp);gen=d/'python';gen.mkdir();include=pathlib.Path(PROTOC).resolve().parent.parent/'include'
 for f in ['openagents/options/v1alpha1/options.proto','nostr/block/oa/v1alpha1/owner.proto','nostr/block/gs/v1alpha1/signature.proto']:
  run(PROTOC,'-I',R/'proto','-I',include,'--python_out='+str(gen),R/'proto'/f)
 sys.path.insert(0,str(gen));from nostr.block.gs.v1alpha1.signature_pb2 import GitSignature
 text=run(PROTOC,'-I',R/'proto','-I',include,'--decode=nostr.block.gs.v1alpha1.GitSignature',R/'proto/nostr/block/gs/v1alpha1/signature.proto',data=fixtures['OWNER'])
 assert b'BEGIN SIGNED MESSAGE' in text and b'created_at: 1700000000' in text and b'owner {' in text
 base=GitSignature.FromString(fixtures['OK'])
 extra=[('unknown field',fixtures['OK']+b'\x98\x06\x01',True),('truncated',fixtures['OK'][:-1],False),('empty',b'',False)]
 for field in ('armor','version','pubkey','signature','created_at'):
  g=GitSignature();g.CopyFrom(base);g.ClearField(field);extra.append(('no '+field,g.SerializeToString(),False))
 g=GitSignature();g.CopyFrom(base);g.armor=g.armor+b'x'*(4158-len(g.armor));extra.append(('armor at its bound',g.SerializeToString(),False))
 g.armor=g.armor+b'x';extra.append(('armor past its bound',g.SerializeToString(),False))
 g=GitSignature.FromString(fixtures['OWNER']);g.owner.ClearField('conditions');extra.append(('owner without conditions',g.SerializeToString(),False))
 runner=d/'run.mjs';runner.write_text("import{WASI}from'node:wasi';import{readFileSync}from'node:fs';const w=new WASI({version:'preview1',args:process.argv.slice(2),env:{},preopens:{}});const{instance}=await WebAssembly.instantiate(readFileSync(process.argv[2]),{wasi_snapshot_preview1:w.wasiImport});w.start(instance);\n")
 clang=WASM_CLANG;sysroot=WASM_SYSROOT;resources=WASM_RESOURCES
 for mode,flags in [('checked',[]),('release',['--release'])]:
  src=d/(mode+'.c');h=d/'block_git_signature.h';src.write_bytes(run(COAX,'c',R/'coax/lib/nostr/adapters/gs-projection.coax','--library','--header',h,*flags))
  common=['-std=c11','-O1','-I'+str(d),src,R/'coax/tests/nostr/block_git_signature_driver.c','-lm'];exe=d/(mode+'-san')
  run('cc',*common,'-fsanitize=address,undefined','-fno-sanitize-recover=all','-o',exe)
  wasm=d/(mode+'.wasm');run(clang,'--target=wasm32-wasip1','--sysroot='+sysroot,'-resource-dir='+resources,*common,'-Wl,-z,stack-size=1048576,--max-memory=67108864','-o',wasm)
  for target in ([exe],['node','--no-warnings',runner,wasm]):
   n=0
   for name,wire in sorted(fixtures.items()):
    ok,owner,count=map(int,run(*target,data=wire).split())
    assert ok==(name in good),(mode,target[0],name)
    if ok:assert owner==GitSignature.FromString(wire).HasField('owner') and count==len(GitSignature.FromString(wire).armor),name
    n+=1
   for name,wire,want in extra:
    ok,owner,count=map(int,run(*target,data=wire).split());assert ok==want,(mode,target[0],name);n+=1
  print(mode,n,'cases: pinned NIP-GS vectors, armor/JSON/projection divergence, bounds, presence and unknowns; native sanitizers and Wasm',flush=True)
