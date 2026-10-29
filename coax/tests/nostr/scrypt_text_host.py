#!/usr/bin/env python3
"""NFKC-to-scrypt bridge; local test tools only."""
if not __debug__: raise SystemExit('tests require Python assertions')
import hashlib,os,subprocess,tempfile
from pathlib import Path
R=Path(__file__).resolve().parents[3];P=R/'coax/lib/crypto/platform';compiler=R/'coax/bin/coax'
assert hashlib.scrypt('Å'.encode(),salt=bytes(range(16)),n=2,r=8,p=1,dklen=32).hex()=='958f3e9f16c33fe660425a098b1da4d40284ba2ea0ed8c8e83c652a06ff57efe'
if os.environ.get('COAX_IOS_LARGE')=='1' and not os.environ.get('COAX_IOS_SIMULATOR'):
 raise SystemExit('COAX_IOS_LARGE requires a booted COAX_IOS_SIMULATOR')
if os.environ.get('COAX_IOS_LARGE')=='1':
 assert hashlib.scrypt('Å'.encode(),salt=bytes(range(16)),n=65536,r=8,p=1,dklen=32,maxmem=134217728).hex()=='7be855a36723a348295c59816da532375ec3f8d9ac8974918231de9a813a6ecd'
with tempfile.TemporaryDirectory(prefix='coax-scrypt-text-') as temp:
 d=Path(temp)
 subprocess.run([str(compiler),'header',str(R/'coax/lib/crypto/zeroize.coax'),'-o',str(d/'zeroize-policy.h')],check=True)
 for mode,flags in [('checked',[]),('release',['--release'])]:
  paths=[]
  for name,src in [('scrypt-job','crypto/scrypt_job'),('scrypt-admission','crypto/scrypt_admission'),('zeroize-large-policy','crypto/zeroize_large'),('nfkc','text/nfkc')]:
   c=d/(name+'.c');c.write_bytes(subprocess.run([str(compiler),'c',str(R/f'coax/lib/{src}.coax'),'--library','--header',str(d/(name+'.h')),*flags],capture_output=True,check=True).stdout);paths.append(str(c))
  sdk=subprocess.check_output(['xcrun','--sdk','iphonesimulator','--show-sdk-path'],text=True).strip()
  for effect,defines in [('normal',[]),('fault',['-DCOAX_SCRYPT_TEXT_TEST_EFFECT'])]:
   common=['-I'+str(d),'-I'+str(P),*defines,*paths,str(P/'zeroize.c'),str(P/'zeroize_large.c'),str(P/'scrypt_host.c'),str(P/'scrypt_text_host.c'),str(R/'coax/tests/nostr/scrypt_text_host_fixture.c')]
   exe=d/(mode+'-'+effect+'-asan');subprocess.run(['cc','-std=c11','-O1','-fsanitize=address,undefined','-DNATIVE_MAIN',*common,'-o',str(exe)],check=True);subprocess.run([str(exe)],check=True)
   sim=os.environ.get('COAX_IOS_SIMULATOR')
   if sim:
    ios=d/(mode+'-'+effect+'-ios');subprocess.run(['xcrun','--sdk','iphonesimulator','clang','-arch','arm64','-isysroot',sdk,'-std=c11','-O2','-DNATIVE_MAIN',*common,'-o',str(ios)],check=True)
    subprocess.run(['xcrun','simctl','spawn',sim,str(ios)],check=True);print(mode,effect,'iOS simulator',hashlib.sha256(ios.read_bytes()).hexdigest(),flush=True)
    if effect=='normal' and os.environ.get('COAX_IOS_LARGE')=='1':
     full=d/(mode+'-large-ios');large=R/'coax/tests/nostr/scrypt_text_host_large_ios.c'
     subprocess.run(['xcrun','--sdk','iphonesimulator','clang','-arch','arm64','-isysroot',sdk,'-std=c11','-O2',*common[:-1],str(large),'-o',str(full)],check=True)
     subprocess.run(['xcrun','simctl','spawn',sim,str(full)],check=True);print(mode,'iOS simulator full 64 MiB normalized scrypt',hashlib.sha256(full.read_bytes()).hexdigest(),flush=True)
   wasm=d/(mode+'-'+effect+'.wasm');subprocess.run(['/opt/homebrew/opt/llvm/bin/clang','--target=wasm32-wasip1','--sysroot=/opt/homebrew/opt/wasi-libc/share/wasi-sysroot','-resource-dir=/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes','-std=c11','-O2','-mexec-model=reactor',*common,'-Wl,-z,stack-size=1048576,--export=scrypt_text_host_fixture','-o',str(wasm)],check=True)
   js='const fs=require("fs"),{WASI}=require("node:wasi");(async()=>{const w=new WASI({version:"preview1"});const {instance}=await WebAssembly.instantiate(fs.readFileSync(process.argv[1]),{wasi_snapshot_preview1:w.wasiImport});w.initialize(instance);const r=instance.exports.scrypt_text_host_fixture();if(r!==0){console.error("fixture",r);process.exit(1)}})().catch(e=>{console.error(e);process.exit(1)})'
   subprocess.run(['node','-e',js,str(wasm)],check=True);print(mode,effect,'native sanitizer and Wasm fixture passed; Wasm',hashlib.sha256(wasm.read_bytes()).hexdigest(),flush=True)
