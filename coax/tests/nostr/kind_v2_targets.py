#!/usr/bin/env python3
"""Versioned Event profile boundary checks using generated native/Wasm/iOS code."""
import os as _os,sys as _sys;_sys.path.insert(0,_os.path.dirname(_os.path.abspath(__file__)));from wasm_toolchain import WASM_CLANG,WASM_SYSROOT,WASM_RESOURCES
import hashlib,os,pathlib,subprocess,tempfile
R=pathlib.Path(__file__).resolve().parents[3];coax=pathlib.Path(os.environ.get('COAX_TEST_BIN',str(R/'coax/bin/coax')))
def run(*args,**kw):return subprocess.run(list(map(str,args)),check=True,**kw)
protoc=os.environ.get('PROTOC','protoc')
for n,expected in [(0,bytes([24,0])),(65535,bytes([24,255,255,3])),(65536,bytes([24,128,128,4]))]:
 encoded=subprocess.run([protoc,'-I'+str(R/'proto'),'--encode=nostr.event.v1alpha2.UnsignedEvent',str(R/'proto/nostr/event/v1alpha2/event.proto')],input=f'kind: {n}'.encode(),capture_output=True,check=True).stdout
 assert encoded==expected,(n,encoded)
with tempfile.TemporaryDirectory(prefix='coax-kind-v2-') as temp:
 d=pathlib.Path(temp);fixture=R/'coax/tests/nostr/kind_v2_native.c'
 for mode in ('checked','release'):
  src=d/(mode+'.c');header=d/'kind.h';flags=['--release'] if mode=='release' else []
  with src.open('wb') as out:run(coax,'c',R/'coax/tests/nostr/kind_v2_abi.coax','--library','--header',header,'--run-steps','250000000',*flags,stdout=out)
  common=['-I'+str(d),str(src),str(fixture)];exe=d/(mode+'-native')
  run('cc','-std=c11','-O1','-fsanitize=address,undefined',*common,'-o',exe);run(exe)
  wasm=d/(mode+'.wasm');run(WASM_CLANG,'--target=wasm32-wasip1','--sysroot='+WASM_SYSROOT,'-resource-dir='+WASM_RESOURCES,'-std=c11','-O2','-mexec-model=reactor',*common,'-Wl,--export=run_kinds','-o',wasm)
  run('node',R/'coax/tests/nostr/kind_v2_wasm.cjs',wasm)
  sim=os.environ.get('COAX_IOS_SIMULATOR')
  if sim:
   sdk=subprocess.check_output(['xcrun','--sdk','iphonesimulator','--show-sdk-path'],text=True).strip();ios=d/(mode+'-ios')
   run('xcrun','--sdk','iphonesimulator','clang','-arch','arm64','-isysroot',sdk,'-std=c11','-O2',*common,'-o',ios);run('xcrun','simctl','spawn',sim,ios);print(mode,'iOS Simulator pass',flush=True)
  print(mode,'native ASan/UBSan and Wasm pass; generated C',hashlib.sha256(src.read_bytes()).hexdigest(),flush=True)
