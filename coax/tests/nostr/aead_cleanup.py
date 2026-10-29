#!/usr/bin/env python3
"""Finite-model AEAD cleanup policy check on native and Wasm."""
if not __debug__: raise SystemExit('tests require Python assertions')
import hashlib,subprocess,tempfile
from pathlib import Path
R=Path(__file__).resolve().parents[3];source=R/'coax/lib/crypto/aead_cleanup.coax';fixture=R/'coax/tests/nostr/aead_cleanup_fixture.c'
with tempfile.TemporaryDirectory(prefix='coax-aead-cleanup-') as temp:
 d=Path(temp)
 for mode,flags in [('checked',[]),('release',['--release'])]:
  c=d/(mode+'.c');c.write_bytes(subprocess.run([str(R/'coax/bin/coax'),'c',str(source),'--library','--header',str(d/'aead-cleanup.h'),*flags],capture_output=True,check=True).stdout)
  exe=d/(mode+'-asan');subprocess.run(['cc','-std=c11','-O1','-fsanitize=address,undefined','-DNATIVE_MAIN','-I'+str(d),str(c),str(fixture),'-o',str(exe)],check=True);subprocess.run([str(exe)],check=True)
  sdk=subprocess.check_output(['xcrun','--sdk','iphonesimulator','--show-sdk-path'],text=True).strip()
  subprocess.run(['xcrun','--sdk','iphonesimulator','clang','-arch','arm64','-isysroot',sdk,'-std=c11','-O2','-c',str(c),'-o',str(d/'ios.o')],check=True)
  wasm=d/(mode+'.wasm');subprocess.run(['/opt/homebrew/opt/llvm/bin/clang','--target=wasm32-wasip1','--sysroot=/opt/homebrew/opt/wasi-libc/share/wasi-sysroot','-resource-dir=/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes','-std=c11','-O2','-I'+str(d),'-mexec-model=reactor',str(c),str(fixture),'-Wl,--export=aead_cleanup_fixture','-o',str(wasm)],check=True)
  js='const fs=require("fs"),{WASI}=require("node:wasi");(async()=>{const w=new WASI({version:"preview1"});const {instance}=await WebAssembly.instantiate(fs.readFileSync(process.argv[1]),{wasi_snapshot_preview1:w.wasiImport});w.initialize(instance);if(instance.exports.aead_cleanup_fixture()!==0)process.exit(1)})().catch(e=>{console.error(e);process.exit(1)})'
  subprocess.run(['node','-e',js,str(wasm)],check=True);print(mode,'finite model passed native sanitizers and Wasm; iOS simulator compile; C',hashlib.sha256(c.read_bytes()).hexdigest(),'Wasm',hashlib.sha256(wasm.read_bytes()).hexdigest(),flush=True)
