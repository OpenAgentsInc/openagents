#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: Python's byte-key dictionary and ordering decide every native arena lookup, update, deletion, seek and scan)
"""Indexed arena against an independent ordered model, including stage copies."""
import argparse
from pathlib import Path
import random
import struct
import subprocess
import tempfile

ROOT=Path(__file__).resolve().parents[3]
DRIVER=r'''
#define _GNU_SOURCE
#include "skip.h"
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
static void word(uint64_t x,int n){for(int i=0;i<n;i++)putchar((int)((x>>(8*i))&255));}
static uint64_t readword(FILE *f,int n){uint64_t x=0;for(int i=0;i<n;i++){int c=fgetc(f);if(c<0)exit(12);x|=(uint64_t)c<<(8*i);}return x;}
static void row(coax_t_KsEntry e){word((uint64_t)e.key.count,2);word((uint64_t)e.value.count,4);fwrite(e.key.data,1,(size_t)e.key.count,stdout);fwrite(e.value.data,1,(size_t)e.value.count,stdout);}
int main(int argc,char **argv){
  if(argc!=2)return 2;unsigned char seed[16];for(int i=0;i<16;i++)seed[i]=(unsigned char)i;
  if(!strcmp(argv[1],"offset")){
    size_t cap=((size_t)1<<32)+4096;unsigned char *p=mmap(0,cap,PROT_READ|PROT_WRITE,MAP_ANON|MAP_PRIVATE,-1,0);if(p==MAP_FAILED)return 3;
    coax_v_u8 t={p,(int64_t)cap};if(ks_init(t,(coax_v_u8){seed,16})!=256)return 4;
    int64_t used=((int64_t)1<<32)+256;unsigned char k[2]={0,1},v[3]={7,8,9};
    used=ks_put(t,used,(coax_v_u8){k,2},(coax_v_u8){v,3});
    coax_t_KsFind f=ks_find(t,used,(coax_v_u8){k,2});coax_t_KsEntry e=ks_entry(t,used,0);
    if(used!=((int64_t)1<<32)+284||!f.found||f.at!=((int64_t)1<<32)+273||e.code||e.next!=used||e.key.count!=2||e.value.count!=3||memcmp(e.value.data,v,3))return 5;
    munmap(p,cap);puts("64-bit arena offsets pass");return 0;
  }
  FILE *in=fopen(argv[1],"rb");if(!in)return 6;
  size_t cap=16777216;unsigned char *p=malloc(cap),*stage=malloc(cap),key[1024],value[1048576];if(!p||!stage)return 7;
  memset(p,165,cap);coax_v_u8 t={p,(int64_t)cap};int64_t used=ks_init(t,(coax_v_u8){seed,16});
  int op;while((op=fgetc(in))!=EOF){
    if(op==4){putchar('s');coax_t_KsEntry e=ks_entry(t,used,0);int count=0;while(!e.code){if(++count>1000000)return 8;putchar(1);row(e);e=ks_entry(t,used,e.next);}putchar(0);continue;}
    if(op==5){memset(stage,165,cap);memcpy(stage,p,(size_t)used);unsigned char *old=p;p=stage;stage=old;t.data=p;putchar('c');continue;}
    int64_t kn=(int64_t)readword(in,2);if(kn>1024||fread(key,1,(size_t)kn,in)!=(size_t)kn)return 9;
    coax_v_u8 k={key,kn};
    if(op==1){int64_t vn=(int64_t)readword(in,4);if(vn>1048576||fread(value,1,(size_t)vn,in)!=(size_t)vn)return 10;used=ks_put(t,used,k,(coax_v_u8){value,vn});if(used<0)return 11;putchar('p');}
    else if(op==2){used=ks_delete(t,used,k);putchar('d');}
    else if(op==3){coax_t_KsFind f=ks_find(t,used,k);coax_t_KsEntry e=ks_entry(t,used,f.at);putchar('g');putchar(f.found?1:0);putchar(!e.code?1:0);if(!e.code)row(e);}
    else return 13;
  }
  fclose(in);free(p);free(stage);return 0;
}
'''

def fixture():
    rng=random.Random(20261104);model={};ops=bytearray();want=bytearray()
    keys=sorted({bytes(rng.randrange(256) for _ in range(rng.randrange(1,41))) for _ in range(500)}|{b'a',b'aa',b'ab',b'a\0',b'\0',b'\xff'})
    def row(k):return struct.pack('<HI',len(k),len(model[k]))+k+model[k]
    def get(k):
        ops.extend(b'\3'+struct.pack('<H',len(k))+k);candidate=next((x for x in sorted(model) if x>=k),None)
        want.extend(b'g'+bytes([k in model,candidate is not None]));
        if candidate is not None:want.extend(row(candidate))
    def scan():
        ops.append(4);want.extend(b's'+b''.join(b'\1'+row(k) for k in sorted(model))+b'\0')
    for i in range(5000):
        k=rng.choice(keys)
        if rng.randrange(4)==0:ops.extend(b'\2'+struct.pack('<H',len(k))+k);model.pop(k,None);want.extend(b'd')
        else:
            v=bytes(rng.randrange(256) for _ in range(rng.randrange(129)));ops.extend(b'\1'+struct.pack('<H',len(k))+k+struct.pack('<I',len(v))+v);model[k]=v;want.extend(b'p')
        get(k);get(rng.choice(keys));get(b'')
        if i%50==0:scan();ops.append(5);want.extend(b'c');scan()
    scan();return ops,want

def run(compiler,mode):
    with tempfile.TemporaryDirectory(prefix='bitcoin-kvskip-') as temporary:
        d=Path(temporary);h=d/'skip.h';source=d/'skip.c';driver=d/'driver.c';binary=d/'check'
        r=subprocess.run([str(compiler),'c','coax/lib/bitcoin/kvskip.coax','--library','--header',str(h),*(['--release'] if mode=='release' else [])],cwd=ROOT,text=True,capture_output=True,timeout=840);assert r.returncode==0,r.stderr
        source.write_text(r.stdout);driver.write_text(DRIVER);flags=['-O1','-g','-fsanitize=address,undefined','-fno-omit-frame-pointer'] if mode=='sanitized' else ['-O2']
        r=subprocess.run(['cc','-std=c11','-D_DARWIN_C_SOURCE',*flags,str(source),str(driver),'-I'+str(d),'-lm','-o',str(binary)],capture_output=True,text=True,timeout=840);assert r.returncode==0,r.stderr
        ops,want=fixture();(d/'ops').write_bytes(ops)
        r=subprocess.run([str(binary),str(d/'ops')],capture_output=True,timeout=120);assert r.returncode==0,(r.returncode,r.stderr.decode());assert r.stdout==want,'native lookup/scan differs from Python ordered dictionary'
        r=subprocess.run([str(binary),'offset'],capture_output=True,timeout=120);assert r.returncode==0,(r.returncode,r.stderr.decode());assert r.stdout==b'64-bit arena offsets pass\n'
        assert b'runtime error:' not in r.stderr and b'ERROR: AddressSanitizer' not in r.stderr,r.stderr
        print(f'indexed KV {mode}: 5,000 updates, 15,000 immediate seeks, 201 complete scans, 100 dirty stage copies and offsets above 4 GiB match the independent ordered dictionary',flush=True)

if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('--coax',type=Path,required=True);p.add_argument('--mode',choices=('debug','release','sanitized'),default='debug');a=p.parse_args();run(a.coax.resolve(),a.mode)
