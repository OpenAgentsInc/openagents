#!/usr/bin/env python3
"""NIP-PL and NIP-RS envelope interoperability: protoc 29.3 to the Coax adapters on native sanitizers and Wasm.
Envelope consistency only; no signature, decryption, expiration window or acceptance."""
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
run(sys.executable,R/'coax/tests/nostr/generate_pl_rs_fixtures.py','--check',env=dict(os.environ,PROTOC=PROTOC))
def load(name,prefix):return {m.group(1):bytes(int(x) for x in m.group(2).split(',')) for m in re.finditer(r'^'+prefix+r'(\w+) :: \[\d+\]u8\.\[([0-9,]+)\];$',(R/'coax/tests/nostr'/name).read_text(),re.M)}
sets=[('pl','PL',load('pl_lease_fixtures.coax','PL_FIX_'),{'OK','NO_ALT','ZERO','MAX','D64'},'nostr.block.pl.v1alpha1.PushLease','nostr/block/pl/v1alpha1/lease.proto',b'installation: "f3a1'),
      ('rs','RS',load('rs_read_state_fixtures.coax','RS_FIX_'),{'OK','MORE_TAGS','EXTRA_D_VALUE','T_NO_VALUE','VALUE_NOT_T'},'nostr.block.rs.v1alpha1.ReadStateEvent','nostr/block/rs/v1alpha1/read_state.proto',b'slot: "0123')]
clang=WASM_CLANG;sysroot=WASM_SYSROOT;resources=WASM_RESOURCES
include=pathlib.Path(PROTOC).resolve().parent.parent/'include'
with tempfile.TemporaryDirectory(prefix='coax-pl-rs-') as tmp:
 d=pathlib.Path(tmp)
 runner=d/'run.mjs';runner.write_text("import{WASI}from'node:wasi';import{readFileSync}from'node:fs';const w=new WASI({version:'preview1',args:process.argv.slice(2),env:{},preopens:{}});const{instance}=await WebAssembly.instantiate(readFileSync(process.argv[2]),{wasi_snapshot_preview1:w.wasiImport});w.start(instance);\n")
 for short,macro,fixtures,good,message,proto,needle in sets:
  assert good<=set(fixtures)
  text=run(PROTOC,'-I',R/'proto','-I',include,'--decode='+message,R/'proto'/proto,data=fixtures['OK'])
  assert needle in text and b'original {' in text,short
  extra=[('unknown field',fixtures['OK']+b'\x98\x06\x01',True),('truncated',fixtures['OK'][:-1],False),('empty',b'',False)]
  for mode,flags in [('checked',[]),('release',['--release'])]:
   w=d/short/mode;w.mkdir(parents=True);src=w/'a.c';h=w/'block_pl_rs.h'
   src.write_bytes(run(COAX,'c',R/f'coax/lib/nostr/adapters/{short}-projection.coax','--library','--header',h,'--run-steps','250000000',*flags))
   common=['-std=c11','-O1','-D'+(macro if short=='pl' else 'RS'),'-I'+str(w),src,R/'coax/tests/nostr/block_pl_rs_driver.c','-lm'];exe=w/'san'
   run('cc',*common,'-fsanitize=address,undefined','-fno-sanitize-recover=all','-o',exe)
   wasm=w/'a.wasm';run(clang,'--target=wasm32-wasip1','--sysroot='+sysroot,'-resource-dir='+resources,*common,'-Wl,-z,stack-size=1048576,--max-memory=67108864','-o',wasm)
   for target in ([exe],['node','--no-warnings',runner,wasm]):
    n=0
    for name,wire in sorted(fixtures.items()):
     ok,count=map(int,run(*target,data=wire).split());assert ok==(name in good),(short,mode,target[0],name);assert ok==0 or count>0;n+=1
    for name,wire,want in extra:
     ok,count=map(int,run(*target,data=wire).split());assert ok==want,(short,mode,target[0],name);n+=1
   print(short,mode,n,'cases: envelope fixtures, unknowns and truncation; native sanitizers and Wasm',flush=True)
