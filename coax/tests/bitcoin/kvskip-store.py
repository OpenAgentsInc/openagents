#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: Python's v1 log encoder/replayer and ordered dictionary compare both native table implementations and every final-frame cut)
"""The indexed arena keeps the original KV v1 bytes and recovery decisions."""
import argparse
import ctypes as C
import importlib.util
from pathlib import Path
import random
import struct
import subprocess
import tempfile

ROOT=Path(__file__).resolve().parents[3]
spec=importlib.util.spec_from_file_location('kv_oracle',Path(__file__).with_name('kvstore.py'));old=importlib.util.module_from_spec(spec);spec.loader.exec_module(old)
class Open(C.Structure):
    _fields_=[('code',C.c_int64),('used',C.c_int64),('seq',C.c_uint64),('frames',C.c_int64),('keep',C.c_int64),('cut',C.c_bool),('mask',C.c_uint64)]
class Arena(old.Store):
    def fresh(self):
        keep,seed=old.bview(bytes(range(16)));self.used=self.lib.ks_init(old.view(self.table,self.size),seed);assert self.used==256
    def open(self,data):
        self.fresh();keep,wire=old.bview(data);r=self.lib.kv_open_arena_vartime(wire,old.view(self.table,self.size),self.used);self.used,self.seq,self.mask=r.used,r.seq,r.mask;return r.code,r.keep,bool(r.cut)
    def items(self):
        f,f5=old.fields(5);at=0;result=[]
        while self.lib.kv_entry_c(old.view(self.table,self.size),self.used,at,f5)==0:
            result.append((bytes(self.table[f[1]:f[1]+f[2]]),bytes(self.table[f[3]:f[3]+f[4]])));at=f[0];assert len(result)<10000
        return result

DRIVER=r'''
#include "store.h"
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
static void word(uint64_t v,int n){for(int i=0;i<n;i++)putchar((int)((v>>(8*i))&255));}
int main(int argc,char **argv){
 if(argc!=2)return 2;FILE *f=fopen(argv[1],"rb");if(!f)return 3;fseek(f,0,SEEK_END);long n=ftell(f);rewind(f);if(n<0||n>16777216)return 4;
 unsigned char *wire=malloc((size_t)n+1),*table=malloc(8388608),*out=malloc(8388608),seed[16];if(!wire||!table||!out)return 5;
 if(fread(wire,1,(size_t)n,f)!=(size_t)n)return 6;fclose(f);for(int i=0;i<16;i++)seed[i]=(unsigned char)i;
 coax_v_u8 t={table,8388608};int64_t initial=ks_init(t,(coax_v_u8){seed,16});coax_t_KvOpen r=kv_open_arena_vartime((coax_v_u8){wire,n},t,initial);
 word((uint64_t)r.code,4);word((uint64_t)r.keep,8);word((uint64_t)r.cut,1);word(r.seq,8);
 if(!r.code){int64_t size=kv_snapshot_vartime(t,r.used,r.seq,r.mask,(coax_v_u8){out,8388608});if(size<20)return 7;fwrite(out,1,(size_t)size,stdout);}
 free(wire);free(table);free(out);return 0;
}
'''

def fixture():
    rng=random.Random(12704);keys=[b'a',b'aa',b'ab',b'\0',b'\0\1',*[rng.randbytes(rng.randrange(1,41)) for _ in range(40)]];batches=[]
    for i in range(24):batches.append(old.random_ops(rng,keys,rng.randrange(1,13)))
    return batches,keys

def bindings(lib):
    V,I,U=old.View,C.c_int64,C.c_uint64
    signatures={'kv_batch_new':[V],'kv_batch_put_vartime':[V,I,V,V],'kv_batch_delete_vartime':[V,I,V],
        'kv_snapshot_vartime':[V,I,U,U,V],'kv_open_c':[V,V,old.View64],'kv_commit_c':[V,I,U,V,U,V,old.View64],
        'kv_get_c':[V,I,V,V],'kv_seek_c':[V,I,V],'kv_entry_c':[V,I,I,old.View64],'ks_init':[V,V]}
    for name,args in signatures.items():getattr(lib,name).argtypes=args;getattr(lib,name).restype=I
    lib.kv_open_arena_vartime.argtypes=[V,V,I];lib.kv_open_arena_vartime.restype=Open

def run(compiler,mode):
    with tempfile.TemporaryDirectory(prefix='bitcoin-kvskip-store-') as temporary:
        d=Path(temporary);source=d/'store.c';header=d/'store.h';driver=d/'driver.c';binary=d/'check'
        r=subprocess.run([str(compiler),'c','coax/lib/bitcoin/kvstore.coax','--library','--header',str(header),*(['--release'] if mode=='release' else [])],cwd=ROOT,capture_output=True,text=True,timeout=840);assert r.returncode==0,r.stderr;source.write_text(r.stdout);driver.write_text(DRIVER)
        flags=['-O1','-g','-fsanitize=address,undefined','-fno-omit-frame-pointer'] if mode=='sanitized' else ['-O2']
        r=subprocess.run(['cc','-std=c11',*flags,str(source),str(driver),'-I'+str(d),'-lm','-o',str(binary)],capture_output=True,text=True,timeout=840);assert r.returncode==0,r.stderr
        batches,keys=fixture();native=None
        if mode!='sanitized':
            shared=d/'store.dylib';r=subprocess.run(['cc','-std=c11','-O2','-shared','-fPIC',str(source),'-lm','-o',str(shared)],capture_output=True,text=True,timeout=840);assert r.returncode==0,r.stderr;native=C.CDLL(str(shared));bindings(native)
        cases=0
        for mask in (0,0x0123456789abcdef):
            journal=old.header(mask);model={};frames=[]
            if native:
                fast=Arena(native,1<<22);fast.fresh();fast.mask=mask;flat=old.Store(native,1<<18);flat.mask=mask
            for seq,ops in enumerate(batches,1):
                encoded=old.frame(seq,ops,mask);journal+=encoded;frames.append(encoded);model=old.apply(model,ops)
                if native:
                    assert fast.commit(ops)==encoded and flat.commit(ops)==encoded
                    assert fast.items()==flat.items()==sorted(model.items())
                    for key in keys:assert fast.get(key)==flat.get(key)==model.get(key)
                    for query in (b'',b'a',b'aaa',b'\xff'):
                        candidate=next((k for k in sorted(model) if k>=query),None);assert fast.seek(query)==flat.seek(query)==candidate
                    expected=old.header(mask)+old.frame(seq,[('put',k,v) for k,v in sorted(model.items())],mask)
                    assert fast.snapshot()==flat.snapshot()==expected
            base=journal[:-len(frames[-1])];variants=[journal,*[base+frames[-1][:i] for i in range(len(frames[-1]))]]
            for i in range(0,len(journal),max(1,len(journal)//40)):
                damaged=bytearray(journal);damaged[i]^=1;variants.append(bytes(damaged)+b'x')
            for wire in variants:
                code,state,seq,keep,cut=old.replay(wire);expected=struct.pack('<IQBQ',code,keep,cut,seq)
                if code==0:
                    expected+=old.header(mask)
                    if seq:expected+=old.frame(seq,[('put',k,v) for k,v in sorted(state.items())],mask)
                (d/'journal').write_bytes(wire);r=subprocess.run([str(binary),str(d/'journal')],capture_output=True,timeout=30)
                assert r.returncode==0,(mode,r.returncode,r.stderr.decode());assert r.stdout==expected,(mode,code,seq,keep,cut,'replay/snapshot differs')
                assert b'runtime error:' not in r.stderr and b'ERROR: AddressSanitizer' not in r.stderr,r.stderr;cases+=1
        print(f'indexed KV store {mode}: canonical frames/snapshots and {cases} full, torn and corrupt file replays match independent v1 bytes; native update/get/seek parity passes where linked',flush=True)

if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('--coax',type=Path,required=True);p.add_argument('--mode',choices=('debug','release','sanitized'),default='debug');a=p.parse_args();run(a.coax.resolve(),a.mode)
