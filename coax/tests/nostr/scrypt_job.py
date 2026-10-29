#!/usr/bin/env python3
"""Checked/release scrypt lifecycle with real wipe effects on native and Wasm."""
if not __debug__: raise SystemExit('tests require Python assertions')
import hashlib
import os
import subprocess
import tempfile
from pathlib import Path
ROOT=Path(__file__).resolve().parents[3]
COMPILER=ROOT/'coax/bin/coax'
JOB=ROOT/'coax/lib/crypto/scrypt_job.coax'
ZERO=ROOT/'coax/lib/crypto/zeroize.coax'
LARGE=ROOT/'coax/lib/crypto/zeroize_large.coax'
PLATFORM=ROOT/'coax/lib/crypto/platform'
FIXTURE=ROOT/'coax/tests/nostr/scrypt_job_fixture.c'
assert hashlib.scrypt(b'nostr',salt=bytes(range(16)),n=2,r=8,p=1,dklen=32).hex()=='34b83ae9bd8d9d3bbb6aa6f73ac6b4229fb5bb49044317d392c732351ab51c17'
if os.environ.get('COAX_IOS_LARGE')=='1' and not os.environ.get('COAX_IOS_SIMULATOR'):
    raise SystemExit('COAX_IOS_LARGE requires a booted COAX_IOS_SIMULATOR')
with tempfile.TemporaryDirectory(prefix='coax-scrypt-job-') as temp:
    d=Path(temp)
    subprocess.run([str(COMPILER),'header',str(ZERO),'-o',str(d/'zeroize-policy.h')],check=True)
    for mode,flags in [('checked',[]),('release',['--release'])]:
        c=d/(mode+'.c'); z=d/(mode+'-zero.c')
        c.write_bytes(subprocess.run([str(COMPILER),'c',str(JOB),'--library','--header',str(d/'scrypt-job.h'),*flags],check=True,capture_output=True).stdout)
        z.write_bytes(subprocess.run([str(COMPILER),'c',str(LARGE),'--library','--header',str(d/'zeroize-large-policy.h'),*flags],check=True,capture_output=True).stdout)
        common=['-I'+str(d),'-I'+str(PLATFORM),str(c),str(z),str(PLATFORM/'zeroize.c'),str(PLATFORM/'zeroize_large.c'),str(FIXTURE)]
        exe=d/(mode+'-asan')
        subprocess.run(['cc','-std=c11','-O1','-fsanitize=address,undefined','-DNATIVE_MAIN',*common,'-o',str(exe)],check=True)
        subprocess.run([str(exe)],check=True)
        sdk=subprocess.check_output(['xcrun','--sdk','iphonesimulator','--show-sdk-path'],text=True).strip()
        subprocess.run(['xcrun','--sdk','iphonesimulator','clang','-arch','arm64','-isysroot',sdk,'-std=c11','-O2','-c',str(c),'-o',str(d/'ios.o')],check=True)
        device=os.environ.get('COAX_IOS_SIMULATOR')
        if device:
            ios=d/(mode+'-ios')
            subprocess.run(['xcrun','--sdk','iphonesimulator','clang','-arch','arm64','-isysroot',sdk,'-std=c11','-O2','-DNATIVE_MAIN',*common,'-o',str(ios)],check=True)
            subprocess.run(['xcrun','simctl','spawn',device,str(ios)],check=True)
            print(mode,'iOS Simulator fixture passed; binary',hashlib.sha256(ios.read_bytes()).hexdigest(),flush=True)
            if os.environ.get('COAX_IOS_LARGE')=='1':
                large_ios=d/(mode+'-ios-large')
                large_fixture=ROOT/'coax/tests/nostr/scrypt_job_large_ios.c'
                subprocess.run(['xcrun','--sdk','iphonesimulator','clang','-arch','arm64','-isysroot',sdk,'-std=c11','-O2',*common[:-1],str(large_fixture),'-o',str(large_ios)],check=True)
                subprocess.run(['xcrun','simctl','spawn',device,str(large_ios)],check=True)
                print(mode,'iOS Simulator 64 MiB fixture passed; binary',hashlib.sha256(large_ios.read_bytes()).hexdigest(),flush=True)
        wasm=d/(mode+'.wasm')
        subprocess.run(['/opt/homebrew/opt/llvm/bin/clang','--target=wasm32-wasip1','--sysroot=/opt/homebrew/opt/wasi-libc/share/wasi-sysroot','-resource-dir=/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes','-std=c11','-O2','-mexec-model=reactor',*common,'-Wl,--export=scrypt_job_fixture','-o',str(wasm)],check=True)
        js='const fs=require("fs"),{WASI}=require("node:wasi"); (async()=>{const w=new WASI({version:"preview1"});const {instance}=await WebAssembly.instantiate(fs.readFileSync(process.argv[1]),{wasi_snapshot_preview1:w.wasiImport});w.initialize(instance);if(instance.exports.scrypt_job_fixture()!==0)process.exit(1)})().catch(()=>process.exit(1))'
        subprocess.run(['node','-e',js,str(wasm)],check=True)
        print(mode,'independent scrypt vector and actual wipe success/cancel/failure: native ASan/UBSan and Wasm passed; iOS',('executed' if device else 'compile only'),'; C',hashlib.sha256(c.read_bytes()).hexdigest(),'Wasm',hashlib.sha256(wasm.read_bytes()).hexdigest(),flush=True)
