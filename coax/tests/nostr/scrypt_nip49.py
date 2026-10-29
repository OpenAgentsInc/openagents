#!/usr/bin/env python3
"""Resumable r=8,p=1 scrypt against OpenSSL-backed hashlib."""
if not __debug__: raise SystemExit('tests require Python assertions')
import ctypes as C
import hashlib
import json
import random
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT=Path(__file__).resolve().parents[3]
SOURCE=ROOT/'coax/lib/crypto/scrypt_nip49.coax';COMPILER=ROOT/'coax/bin/coax'
class View(C.Structure): _fields_=[('data',C.POINTER(C.c_uint8)),('count',C.c_int64)]
class Romix(C.Structure): _fields_=[('x',C.c_uint8*1024),('r',C.c_int64),('n',C.c_int64),('index',C.c_int64),('phase',C.c_int64)]
class State(C.Structure): _fields_=[('romix',Romix*1),('password',C.c_uint8*1024),('password_count',C.c_int64),('phase',C.c_int64)]
class StateView(C.Structure): _fields_=[('data',C.POINTER(State)),('count',C.c_int64)]


def large_native(obj):
    start=obj.nip49_scrypt_start;start.argtypes=[StateView,View,View,View,C.c_int64,C.c_bool];start.restype=C.c_bool
    step=obj.nip49_scrypt_step;step.argtypes=[StateView,View,C.c_int64,C.c_bool];step.restype=C.c_bool
    finish=obj.nip49_scrypt_finish;finish.argtypes=[StateView,View,C.c_bool];finish.restype=C.c_bool
    password=b'nostr';salt=bytes(range(16));expected=hashlib.scrypt(password,salt=salt,n=65536,r=8,p=1,dklen=32,maxmem=134217728)
    p=(C.c_uint8*len(password)).from_buffer_copy(password);s=(C.c_uint8*16).from_buffer_copy(salt);memory=(C.c_uint8*(64*1024*1024))();out=(C.c_uint8*32)();state=State();v=StateView(C.pointer(state),1)
    assert start(v,View(p,len(password)),View(s,16),View(memory,len(memory)),16,False)
    while state.romix[0].phase!=3:assert step(v,View(memory,len(memory)),64,False)
    assert finish(v,View(out,32),False) and bytes(out)==expected
    print('NIP-49-size native scrypt matched hashlib',flush=True)


def main():
    rng=random.Random(4901);cases=[]
    for log in [1,2,4,6]:
        for pn in [0,1,32,1024]:
            password=rng.randbytes(pn);salt=rng.randbytes(16)
            cases.append({'log':log,'budget':[1,3,64][pn%3],'password':list(password),'salt':list(salt),
                          'expected':list(hashlib.scrypt(password,salt=salt,n=1<<log,r=8,p=1,dklen=32,maxmem=134217728))})
    with tempfile.TemporaryDirectory(prefix='coax-nip49-scrypt-') as temp:
        d=Path(temp);fixtures=d/'cases.json';fixtures.write_text(json.dumps(cases))
        for mode,flags in [('checked',[]),('release',['--release'])]:
            cfile=d/(mode+'.c');lib=d/(mode+'.dylib');harness=ROOT/'coax/tests/nostr/scrypt_nip49_wasm.c'
            generated=subprocess.run([str(COMPILER),'c',str(SOURCE),'--library','--header',str(d/'scrypt_nip49.h'),*flags],capture_output=True,check=True)
            cfile.write_bytes(generated.stdout)
            subprocess.run(['cc','-std=c11','-O2','-shared','-fPIC','-I'+str(d),str(cfile),str(harness),'-o',str(lib)],check=True)
            obj=C.CDLL(str(lib));obj.sk_password.restype=C.POINTER(C.c_uint8);obj.sk_salt.restype=C.POINTER(C.c_uint8);obj.sk_output.restype=C.POINTER(C.c_uint8)
            for c in cases:
                for i,b in enumerate(c['password']):obj.sk_password()[i]=b
                for i,b in enumerate(c['salt']):obj.sk_salt()[i]=b
                assert obj.sk_start(len(c['password']),c['log'])==1 and obj.sk_finish()==0
                while obj.sk_phase()!=3:assert obj.sk_step(c['budget'])==1
                assert obj.sk_finish()==1 and obj.sk_finish()==0 and list(obj.sk_output()[:32])==c['expected']
            if '--nip49-native' in sys.argv:large_native(obj)
            mainfile=d/'main.c';mainfile.write_text('int sk_start(int,int); int sk_step(int); int sk_phase(void); int sk_finish(void); int main(void){if(!sk_start(1024,6))return 1;while(sk_phase()!=3)if(!sk_step(3))return 1;return !sk_finish();}\n')
            subprocess.run(['cc','-std=c11','-O1','-fsanitize=address,undefined','-I'+str(d),str(cfile),str(harness),str(mainfile),'-o',str(d/'asan')],check=True)
            subprocess.run([str(d/'asan')],check=True)
            sdk=subprocess.check_output(['xcrun','--sdk','iphonesimulator','--show-sdk-path'],text=True).strip()
            subprocess.run(['xcrun','--sdk','iphonesimulator','clang','-arch','arm64','-isysroot',sdk,'-std=c11','-O2','-c',str(cfile),'-o',str(d/'ios.o')],check=True)
            module=d/(mode+'.wasm')
            subprocess.run(['/opt/homebrew/opt/llvm/bin/clang','--target=wasm32-wasip1','--sysroot=/opt/homebrew/opt/wasi-libc/share/wasi-sysroot','-resource-dir=/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes','-std=c11','-O2','-I'+str(d),'-mexec-model=reactor',str(cfile),str(harness),'-Wl,--export=sk_password,--export=sk_salt,--export=sk_output,--export=sk_start,--export=sk_step,--export=sk_phase,--export=sk_finish','-o',str(module)],check=True)
            subprocess.run(['node',str(ROOT/'coax/tests/nostr/scrypt_nip49_wasm.cjs'),str(module),str(fixtures)],check=True)
            print(mode,len(cases),'independent cases; C sha256',hashlib.sha256(cfile.read_bytes()).hexdigest(),'Wasm sha256',hashlib.sha256(module.read_bytes()).hexdigest(),flush=True)


if __name__=='__main__':main()
