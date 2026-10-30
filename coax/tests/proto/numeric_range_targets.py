#!/usr/bin/env python3
"""Actual generated numeric policy on native, Wasm, iOS and protoc wire."""
import hashlib, os, pathlib, subprocess, tempfile
R=pathlib.Path(__file__).resolve().parents[3];coax=pathlib.Path(os.environ.get('COAX_TEST_BIN',str(R/'coax/bin/coax')))
def run(*args,**kw):return subprocess.run(list(map(str,args)),check=True,**kw)
protoc=os.environ.get('PROTOC','protoc')
# protoc generates legal wire, but does not itself enforce our custom range.
for text,expected in [('zero: 0',bytes([8,0])),('kind: 65535',bytes([16,255,255,3])),('values: 65535',bytes([26,3,255,255,3]))]:
 actual=subprocess.run([protoc,'-I'+str(R/'proto'),'-I'+str(R/'coax/tests/proto'),'--encode=coax.test.numeric.Ranges',str(R/'coax/tests/proto/numeric_range.proto')],input=text.encode(),capture_output=True,check=True).stdout
 assert actual==expected,(text,actual)
with tempfile.TemporaryDirectory(prefix='coax-numeric-range-') as temp:
 d=pathlib.Path(temp);fixture=R/'coax/tests/proto/numeric_range_native.c'
 for mode in ('checked','release'):
  src=d/(mode+'.c');header=d/'range.h';flags=['--release'] if mode=='release' else []
  with src.open('wb') as out:run(coax,'c',R/'coax/tests/proto/numeric_range_abi.coax','--library','--header',header,'--run-steps','250000000',*flags,stdout=out)
  common=['-I'+str(d),str(src),str(fixture)]
  exe=d/(mode+'-native');run('cc','-std=c11','-O1','-fsanitize=address,undefined',*common,'-o',exe);run(exe)
  wasm=d/(mode+'.wasm');run('/opt/homebrew/opt/llvm/bin/clang','--target=wasm32-wasip1','--sysroot=/opt/homebrew/opt/wasi-libc/share/wasi-sysroot','-resource-dir=/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes','-std=c11','-O2','-mexec-model=reactor',*common,'-Wl,--export=run_ranges','-o',wasm)
  run('node',R/'coax/tests/proto/numeric_range_wasm.cjs',wasm)
  sim=os.environ.get('COAX_IOS_SIMULATOR')
  if sim:
   sdk=subprocess.check_output(['xcrun','--sdk','iphonesimulator','--show-sdk-path'],text=True).strip();ios=d/(mode+'-ios')
   run('xcrun','--sdk','iphonesimulator','clang','-arch','arm64','-isysroot',sdk,'-std=c11','-O2',*common,'-o',ios)
   run('xcrun','simctl','spawn',sim,ios);print(mode,'iOS Simulator pass',flush=True)
  print(mode,'native ASan/UBSan and Wasm pass; generated C',hashlib.sha256(src.read_bytes()).hexdigest(),flush=True)
