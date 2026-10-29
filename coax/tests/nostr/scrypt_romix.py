#!/usr/bin/env python3
"""Resumable ROMix compared with an independent byte-oriented RFC model."""
if not __debug__: raise SystemExit('tests require Python assertions')
import ctypes as C
import hashlib
import importlib.util
import json
import random
import subprocess
import sys
import tempfile
import time
from pathlib import Path

ROOT=Path(__file__).resolve().parents[3]
SOURCE=ROOT/'coax/lib/crypto/scrypt_romix.coax';COMPILER=ROOT/'coax/bin/coax'
spec=importlib.util.spec_from_file_location('blockmix_reference',ROOT/'coax/tests/nostr/scrypt_blockmix.py')
blockmix=importlib.util.module_from_spec(spec);spec.loader.exec_module(blockmix)


def reference(data,r,n):
    x=data;v=[]
    for _ in range(n):v.append(x);x=blockmix.reference(x,r)
    for _ in range(n):
        j=int.from_bytes(x[-64:-56],'little')%n
        x=blockmix.reference(bytes(a^b for a,b in zip(x,v[j])),r)
    return x


class View(C.Structure): _fields_=[('data',C.POINTER(C.c_uint8)),('count',C.c_int64)]
class State(C.Structure): _fields_=[('x',C.c_uint8*1024),('r',C.c_int64),('n',C.c_int64),('index',C.c_int64),('phase',C.c_int64)]
class StateView(C.Structure): _fields_=[('data',C.POINTER(State)),('count',C.c_int64)]


def nip49_native(directory,romix,mode,flags):
    pbsource=ROOT/'coax/lib/crypto/scrypt_pbkdf2.coax';cfile=directory/(mode+'-pb.c');lib=directory/(mode+'-pb.dylib')
    cfile.write_bytes(subprocess.run([str(COMPILER),'c',str(pbsource),'--library','--header',str(directory/'scrypt_pbkdf2.h'),*flags],capture_output=True,check=True).stdout)
    subprocess.run(['cc','-std=c11','-O2','-shared','-fPIC',str(cfile),'-o',str(lib)],check=True)
    pb=C.CDLL(str(lib)).scrypt_pbkdf2_one;pb.argtypes=[View,View,C.c_int64,View,C.c_bool];pb.restype=C.c_bool
    romix.romix_start.argtypes=[StateView,View,View,C.c_int64,C.c_int64,C.c_bool];romix.romix_start.restype=C.c_bool
    romix.romix_step.argtypes=[StateView,View,C.c_int64,C.c_bool];romix.romix_step.restype=C.c_bool
    romix.romix_finish.argtypes=[StateView,View,C.c_bool];romix.romix_finish.restype=C.c_bool
    password=b'nostr';salt=bytes(range(16));expected=hashlib.scrypt(password,salt=salt,n=65536,r=8,p=1,dklen=32,maxmem=134217728)
    password_buf=(C.c_uint8*len(password)).from_buffer_copy(password);salt_buf=(C.c_uint8*16).from_buffer_copy(salt)
    first=(C.c_uint8*1024)();mix=(C.c_uint8*1024)();last=(C.c_uint8*32)();memory=(C.c_uint8*(64*1024*1024))();state=State();view=StateView(C.pointer(state),1)
    start=time.monotonic()
    assert pb(View(password_buf,len(password)),View(salt_buf,16),1024,View(first,1024),False)
    assert romix.romix_start(view,View(first,1024),View(memory,len(memory)),8,65536,False)
    while state.phase!=3:assert romix.romix_step(view,View(memory,len(memory)),64,False)
    assert romix.romix_finish(view,View(mix,1024),False)
    assert pb(View(password_buf,len(password)),View(mix,1024),32,View(last,32),False)
    assert bytes(last)==expected
    print(mode,'NIP-49-size native scrypt matched hashlib; seconds',round(time.monotonic()-start,3),flush=True)


def main():
    rng=random.Random(791410);cases=[]
    for r in [1,2,8]:
        for n in [2,4,16]:
            for budget in [1,3,64]:
                data=rng.randbytes(128*r);cases.append({'r':r,'n':n,'budget':budget,'input':list(data),'expected':list(reference(data,r,n))})
    with tempfile.TemporaryDirectory(prefix='coax-romix-') as temp:
        d=Path(temp);fixtures=d/'cases.json';fixtures.write_text(json.dumps(cases))
        for mode,flags in [('checked',[]),('release',['--release'])]:
            cfile=d/(mode+'.c');lib=d/(mode+'.dylib')
            generated=subprocess.run([str(COMPILER),'c',str(SOURCE),'--library','--header',str(d/'scrypt_romix.h'),*flags],capture_output=True,check=True)
            cfile.write_bytes(generated.stdout)
            harness=ROOT/'coax/tests/nostr/scrypt_romix_wasm.c'
            subprocess.run(['cc','-std=c11','-O2','-shared','-fPIC','-I'+str(d),str(cfile),str(harness),'-o',str(lib)],check=True)
            obj=C.CDLL(str(lib));obj.rm_input.restype=C.POINTER(C.c_uint8);obj.rm_output.restype=C.POINTER(C.c_uint8)
            for mode_id in range(3):assert obj.rm_refusal(mode_id)==1
            for c in cases:
                inp=obj.rm_input()
                for i,b in enumerate(c['input']):inp[i]=b
                assert obj.rm_start(c['r'],c['n'])==1 and obj.rm_finish()==0
                calls=0
                while obj.rm_phase()!=3:
                    assert obj.rm_step(c['budget'])==1;calls+=1;assert calls<=2*c['n']
                assert obj.rm_index()==c['n'] and obj.rm_finish()==1
                assert list(obj.rm_output()[:len(c['expected'])])==c['expected']
            if '--nip49-native' in sys.argv:nip49_native(d,obj,mode,flags)
            mainfile=d/'main.c';mainfile.write_text('int rm_start(int,int); int rm_step(int); int rm_phase(void); int rm_finish(void); int rm_refusal(int); int main(void){for(int i=0;i<3;i++)if(!rm_refusal(i))return 1;if(!rm_start(8,16))return 1;while(rm_phase()!=3)if(!rm_step(3))return 1;return !rm_finish();}\n')
            subprocess.run(['cc','-std=c11','-O1','-fsanitize=address,undefined','-I'+str(d),str(cfile),str(harness),str(mainfile),'-o',str(d/'asan')],check=True)
            subprocess.run([str(d/'asan')],check=True)
            sdk=subprocess.check_output(['xcrun','--sdk','iphonesimulator','--show-sdk-path'],text=True).strip()
            subprocess.run(['xcrun','--sdk','iphonesimulator','clang','-arch','arm64','-isysroot',sdk,'-std=c11','-O2','-c',str(cfile),'-o',str(d/'ios.o')],check=True)
            module=d/(mode+'.wasm')
            subprocess.run(['/opt/homebrew/opt/llvm/bin/clang','--target=wasm32-wasip1','--sysroot=/opt/homebrew/opt/wasi-libc/share/wasi-sysroot','-resource-dir=/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes','-std=c11','-O2','-I'+str(d),'-mexec-model=reactor',str(cfile),str(harness),'-Wl,--export=rm_input,--export=rm_output,--export=rm_start,--export=rm_step,--export=rm_phase,--export=rm_index,--export=rm_finish,--export=rm_refusal','-o',str(module)],check=True)
            subprocess.run(['node',str(ROOT/'coax/tests/nostr/scrypt_romix_wasm.cjs'),str(module),str(fixtures)],check=True)
            print(mode,len(cases),'independent cases; C sha256',hashlib.sha256(cfile.read_bytes()).hexdigest(),'Wasm sha256',hashlib.sha256(module.read_bytes()).hexdigest(),flush=True)


if __name__=='__main__': main()
