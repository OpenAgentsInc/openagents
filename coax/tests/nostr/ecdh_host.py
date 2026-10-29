#!/usr/bin/env python3
"""Identified scratch cleanup and retry tests for the native and Wasm hosts."""
import pathlib,subprocess,tempfile
ROOT=pathlib.Path(__file__).resolve().parents[3];C=ROOT/'coax/bin/coax';P=ROOT/'coax/lib/crypto/platform';T=ROOT/'coax/tests/nostr'
def run(*args):subprocess.run([str(x) for x in args],check=True)
with tempfile.TemporaryDirectory(prefix='coax-ecdh-host-') as tmp:
 d=pathlib.Path(tmp)
 for mode,flags in [('checked',[]),('release',['--release'])]:
  a=d/'ecdh.c';z=d/'zeroize.c';run(C,'c',ROOT/'coax/lib/crypto/ecdh.coax','--library','--header',d/'ecdh.h','-o',a,*flags);run(C,'c',ROOT/'coax/lib/crypto/zeroize.coax','--library','--header',d/'zeroize-policy.h','-o',z,*flags)
  sources=[a,z,P/'zeroize.c',P/'ecdh_host.c',T/'ecdh_host_fixture.c'];common=['-std=c11','-DCOAX_ECDH_TEST_EFFECT','-I'+str(d),'-I'+str(P)]
  for name,opts in [('native',['-O2']),('asan',['-O1','-g','-fsanitize=address,undefined','-fno-omit-frame-pointer'])]:
   exe=d/(mode+'-'+name);run('cc',*common,*opts,*sources,'-lm','-o',exe);run(exe);print(mode,name,'cleanup fixture passed',flush=True)
  wasm=d/(mode+'.wasm');run('/opt/homebrew/opt/llvm/bin/clang','--target=wasm32-wasip1','--sysroot=/opt/homebrew/opt/wasi-libc/share/wasi-sysroot','-resource-dir=/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes',*common,'-O2',*sources,'-Wl,-z,stack-size=1048576','-lm','-o',wasm);run('node',T/'ecdh_host_wasm.cjs',wasm);print(mode,'Wasm cleanup fixture passed',flush=True)
