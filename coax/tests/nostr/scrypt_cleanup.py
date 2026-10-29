#!/usr/bin/env python3
"""Exercise generated lifecycle policy on native and actual WebAssembly."""
if not __debug__: raise SystemExit('tests require Python assertions')
import hashlib
import subprocess
import tempfile
from pathlib import Path
ROOT=Path(__file__).resolve().parents[3]
SRC=ROOT/'coax/lib/crypto/scrypt_cleanup.coax'
FIXTURE=ROOT/'coax/tests/nostr/scrypt_cleanup_fixture.c'
with tempfile.TemporaryDirectory(prefix='coax-scrypt-cleanup-') as temp:
    d=Path(temp)
    for mode,flags in [('checked',[]),('release',['--release'])]:
        c=d/(mode+'.c'); header=d/'scrypt-cleanup.h'
        c.write_bytes(subprocess.run([str(ROOT/'coax/bin/coax'),'c',str(SRC),'--library','--header',str(header),*flags],check=True,capture_output=True).stdout)
        exe=d/(mode+'-asan')
        subprocess.run(['cc','-std=c11','-O1','-fsanitize=address,undefined','-DNATIVE_MAIN','-I'+str(d),str(c),str(FIXTURE),'-o',str(exe)],check=True)
        subprocess.run([str(exe)],check=True)
        sdk=subprocess.check_output(['xcrun','--sdk','iphonesimulator','--show-sdk-path'],text=True).strip()
        subprocess.run(['xcrun','--sdk','iphonesimulator','clang','-arch','arm64','-isysroot',sdk,'-std=c11','-O2','-I'+str(d),'-c',str(c),'-o',str(d/'ios.o')],check=True)
        wasm=d/(mode+'.wasm')
        subprocess.run(['/opt/homebrew/opt/llvm/bin/clang','--target=wasm32-wasip1','--sysroot=/opt/homebrew/opt/wasi-libc/share/wasi-sysroot','-resource-dir=/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes','-std=c11','-O2','-I'+str(d),'-mexec-model=reactor',str(c),str(FIXTURE),'-Wl,--export=cleanup_fixture','-o',str(wasm)],check=True)
        js='const fs=require("fs"),{WASI}=require("node:wasi"); (async()=>{const w=new WASI({version:"preview1"});const {instance}=await WebAssembly.instantiate(fs.readFileSync(process.argv[1]),{wasi_snapshot_preview1:w.wasiImport});w.initialize(instance);if(instance.exports.cleanup_fixture()!==0)process.exit(1)})().catch(()=>process.exit(1))'
        subprocess.run(['node','-e',js,str(wasm)],check=True)
        print(mode,'exhaustive finite policy model: native ASan/UBSan and Wasm passed; iOS simulator compile only; C',hashlib.sha256(c.read_bytes()).hexdigest(),'Wasm',hashlib.sha256(wasm.read_bytes()).hexdigest(),flush=True)
