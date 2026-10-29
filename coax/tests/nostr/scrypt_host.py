#!/usr/bin/env python3
"""Exercise experimental Coax scrypt host effects on native, Wasm and iOS."""
if not __debug__: raise SystemExit('tests require Python assertions')
import hashlib,os,subprocess,tempfile
from pathlib import Path
R=Path(__file__).resolve().parents[3];P=R/'coax/lib/crypto/platform';compiler=R/'coax/bin/coax'
if os.environ.get('COAX_IOS_LARGE')=='1' and not os.environ.get('COAX_IOS_SIMULATOR'):
 raise SystemExit('COAX_IOS_LARGE requires a booted COAX_IOS_SIMULATOR')
with tempfile.TemporaryDirectory(prefix='coax-scrypt-host-') as temp:
 d=Path(temp)
 subprocess.run([str(compiler),'header',str(R/'coax/lib/crypto/zeroize.coax'),'-o',str(d/'zeroize-policy.h')],check=True)
 for mode,flags in [('checked',[]),('release',['--release'])]:
  paths=[]
  for name,src in [('scrypt-job','scrypt_job'),('scrypt-admission','scrypt_admission'),('zeroize-large-policy','zeroize_large')]:
   c=d/(name+'.c');c.write_bytes(subprocess.run([str(compiler),'c',str(R/f'coax/lib/crypto/{src}.coax'),'--library','--header',str(d/(name+'.h')),*flags],capture_output=True,check=True).stdout);paths.append(str(c))
  sdk=subprocess.check_output(['xcrun','--sdk','iphonesimulator','--show-sdk-path'],text=True).strip()
  for effect,defines in [('normal',[]),('fault',['-DCOAX_SCRYPT_TEST_EFFECT'])]:
   common=['-I'+str(d),'-I'+str(P),*defines,*paths,str(P/'zeroize.c'),str(P/'zeroize_large.c'),str(P/'scrypt_host.c'),str(R/'coax/tests/nostr/scrypt_host_fixture.c')]
   exe=d/(mode+'-'+effect+'-asan');subprocess.run(['cc','-std=c11','-O1','-fsanitize=address,undefined','-DNATIVE_MAIN',*common,'-o',str(exe)],check=True);subprocess.run([str(exe)],check=True)
   sim=os.environ.get('COAX_IOS_SIMULATOR')
   if sim:
    ios=d/(mode+'-'+effect+'-ios');subprocess.run(['xcrun','--sdk','iphonesimulator','clang','-arch','arm64','-isysroot',sdk,'-std=c11','-O2','-DNATIVE_MAIN',*common,'-o',str(ios)],check=True)
    subprocess.run(['xcrun','simctl','spawn',sim,str(ios)],check=True);print(mode,effect,'iOS simulator fixture',hashlib.sha256(ios.read_bytes()).hexdigest(),flush=True)
    if effect=='normal' and os.environ.get('COAX_IOS_LARGE')=='1':
     full=d/(mode+'-large-ios');large=R/'coax/tests/nostr/scrypt_host_large_ios.c'
     subprocess.run(['xcrun','--sdk','iphonesimulator','clang','-arch','arm64','-isysroot',sdk,'-std=c11','-O2',*common[:-1],str(large),'-o',str(full)],check=True)
     subprocess.run(['xcrun','simctl','spawn',sim,str(full)],check=True);print(mode,'iOS simulator 64 MiB host fixture',hashlib.sha256(full.read_bytes()).hexdigest(),flush=True)
   wasm=d/(mode+'-'+effect+'.wasm');subprocess.run(['/opt/homebrew/opt/llvm/bin/clang','--target=wasm32-wasip1','--sysroot=/opt/homebrew/opt/wasi-libc/share/wasi-sysroot','-resource-dir=/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes','-std=c11','-O2','-mexec-model=reactor',*common,'-Wl,--export=scrypt_host_fixture','-o',str(wasm)],check=True)
   js='const fs=require("fs"),{WASI}=require("node:wasi");(async()=>{const w=new WASI({version:"preview1"});const {instance}=await WebAssembly.instantiate(fs.readFileSync(process.argv[1]),{wasi_snapshot_preview1:w.wasiImport});w.initialize(instance);const r=instance.exports.scrypt_host_fixture();if(r!==0){console.error("fixture",r);process.exit(1)}})().catch(e=>{console.error(e);process.exit(1)})'
   subprocess.run(['node','-e',js,str(wasm)],check=True);print(mode,effect,'native sanitizer and Wasm host fixture passed; Wasm',hashlib.sha256(wasm.read_bytes()).hexdigest(),flush=True)
