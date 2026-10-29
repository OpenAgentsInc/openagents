#!/usr/bin/env python3
"""Large wipe policy, real observable host erasure and sanitizer checks."""
if not __debug__: raise SystemExit('tests require Python assertions')
import hashlib
import subprocess
import tempfile
from pathlib import Path

ROOT=Path(__file__).resolve().parents[3]
COMPILER=ROOT/'coax/bin/coax'
SOURCE=ROOT/'coax/lib/crypto/zeroize_large.coax'


def main():
    with tempfile.TemporaryDirectory(prefix='coax-zero-large-') as temp:
        d=Path(temp)
        subprocess.run([str(COMPILER),'header',str(ROOT/'coax/lib/crypto/zeroize.coax'),'-o',str(d/'zeroize-policy.h')],check=True)
        for mode,flags in [('checked',[]),('release',['--release'])]:
            cfile=d/(mode+'.c');exe=d/(mode+'-asan')
            result=subprocess.run([str(COMPILER),'c',str(SOURCE),'--library','--header',str(d/'zeroize-large-policy.h'),*flags],capture_output=True,check=True)
            cfile.write_bytes(result.stdout)
            subprocess.run(['cc','-std=c11','-O1','-fsanitize=address,undefined','-I'+str(d),'-I'+str(ROOT/'coax/lib/crypto/platform'),
                            str(cfile),str(ROOT/'coax/lib/crypto/platform/zeroize.c'),str(ROOT/'coax/lib/crypto/platform/zeroize_large.c'),
                            str(ROOT/'coax/tests/nostr/zeroize_large_native.c'),'-o',str(exe)],check=True)
            subprocess.run([str(exe)],check=True)
            sdk=subprocess.check_output(['xcrun','--sdk','iphonesimulator','--show-sdk-path'],text=True).strip()
            subprocess.run(['xcrun','--sdk','iphonesimulator','clang','-arch','arm64','-isysroot',sdk,'-std=c11','-O2','-I'+str(d),
                            '-I'+str(ROOT/'coax/lib/crypto/platform'),'-c',str(ROOT/'coax/lib/crypto/platform/zeroize_large.c'),'-o',str(d/'ios.o')],check=True)
            module=d/(mode+'.wasm')
            subprocess.run(['/opt/homebrew/opt/llvm/bin/clang','--target=wasm32-wasip1','--sysroot=/opt/homebrew/opt/wasi-libc/share/wasi-sysroot','-resource-dir=/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes',
                            '-std=c11','-O2','-I'+str(d),'-I'+str(ROOT/'coax/lib/crypto/platform'),'-mexec-model=reactor',str(cfile),
                            str(ROOT/'coax/lib/crypto/platform/zeroize.c'),str(ROOT/'coax/lib/crypto/platform/zeroize_large.c'),
                            str(ROOT/'coax/tests/nostr/zeroize_large_wasm.c'),'-Wl,--export=wipe_fixture','-o',str(module)],check=True)
            subprocess.run(['node',str(ROOT/'coax/tests/nostr/zeroize_large_wasm.cjs'),str(module)],check=True)
            print(mode,'64 MiB native wipe and sanitizers passed; C sha256',hashlib.sha256(cfile.read_bytes()).hexdigest(),
                  'Wasm sha256',hashlib.sha256(module.read_bytes()).hexdigest(),flush=True)


if __name__=='__main__':main()
