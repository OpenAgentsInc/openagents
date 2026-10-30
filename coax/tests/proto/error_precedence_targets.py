#!/usr/bin/env python3
"""Generated Coax error precedence across native guard pages, Wasm and iOS."""
import hashlib, os, pathlib, subprocess, tempfile
R=pathlib.Path(__file__).resolve().parents[3]
coax=pathlib.Path(os.environ.get('COAX_TEST_BIN',str(R/'coax/bin/coax')))
def run(*args,**kw):return subprocess.run(list(map(str,args)),check=True,**kw)
with tempfile.TemporaryDirectory(prefix='coax-error-precedence-') as temp:
 d=pathlib.Path(temp);fixture=R/'coax/tests/proto/error_precedence_native.c'
 for mode in ('checked','release'):
  header=d/'precedence.h';src=d/(mode+'.c'); flags=['--release'] if mode=='release' else []
  with src.open('wb') as out:run(coax,'c',R/'coax/tests/proto/error_precedence_abi.coax','--library','--header',header,'--run-steps','250000000',*flags,stdout=out)
  common=['-I'+str(d),str(src),str(fixture)]
  exe=d/(mode+'-native');run('cc','-std=c11','-D_DARWIN_C_SOURCE','-O1','-fsanitize=address,undefined',*common,'-o',exe);run(exe)
  wasm=d/(mode+'.wasm');run('/opt/homebrew/opt/llvm/bin/clang','--target=wasm32-wasip1','--sysroot=/opt/homebrew/opt/wasi-libc/share/wasi-sysroot','-resource-dir=/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes','-std=c11','-O2','-mexec-model=reactor',*common,'-Wl,--export=probe_bad,--export=probe_wire','-o',wasm)
  run('node',R/'coax/tests/proto/error_precedence_wasm.cjs',wasm)
  sim=os.environ.get('COAX_IOS_SIMULATOR')
  if sim:
   sdk=subprocess.check_output(['xcrun','--sdk','iphonesimulator','--show-sdk-path'],text=True).strip();ios=d/(mode+'-ios')
   run('xcrun','--sdk','iphonesimulator','clang','-arch','arm64','-isysroot',sdk,'-std=c11','-D_DARWIN_C_SOURCE','-O2',*common,'-o',ios)
   run('xcrun','simctl','spawn',sim,ios);print(mode,'simulator guard-page pass',flush=True)
  print(mode,'native ASan/UBSan and Wasm pass; C sha256',hashlib.sha256(src.read_bytes()).hexdigest(),flush=True)
