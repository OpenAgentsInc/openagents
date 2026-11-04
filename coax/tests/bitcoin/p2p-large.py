#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: hashlib constructs full Bitcoin P2P frames across the one-MiB SHA boundary and four-million-byte protocol limit)
"""Large v1 frame reader/writer against independent hashlib bytes."""
import argparse
from pathlib import Path
import subprocess
import tempfile
import hashlib
import struct

ROOT=Path(__file__).resolve().parents[3]
DRIVER=r'''
#include "large.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
int main(int argc,char **argv){
  if(argc!=3)return 2;
  size_t n=(size_t)strtoull(argv[2],0,10),cap=n+25;
  if(n==0&&p2pl_boundaries()!=0)return 6;
  unsigned char *raw=malloc(n?n:1),*frame=malloc(cap);
  FILE *in=fopen(argv[1],"rb");if(!in||fread(raw,1,n,in)!=n)return 3;fclose(in);
  memset(frame,165,cap);
  int64_t written=p2pl_write((coax_v_u8){raw,(int64_t)n},(coax_v_u8){frame,(int64_t)(cap-1)});
  printf("%lld %u\n",(long long)written,frame[cap-1]);
  if(written<0){for(size_t i=0;i<cap;i++)if(frame[i]!=165)return 4;free(raw);free(frame);return 0;}
  FILE *out=fopen("frame","wb");if(!out||fwrite(frame,1,(size_t)written,out)!=(size_t)written)return 5;fclose(out);
  coax_v_u8 view={frame,written};
  printf("%lld %lld %lld\n",(long long)p2pl_read(view),(long long)p2pl_next(view),(long long)p2pl_read((coax_v_u8){frame,written-1}));
  if(n){size_t points[3]={0,n/2,n-1};for(int i=0;i<3;i++){frame[24+points[i]]^=1;printf("%lld ",(long long)p2pl_read(view));frame[24+points[i]]^=1;}puts("");}
  free(raw);free(frame);return 0;
}
'''

def run(compiler,mode):
    with tempfile.TemporaryDirectory(prefix='bitcoin-p2p-large-') as temporary:
        d=Path(temporary);h=d/'large.h';source=d/'large.c';driver=d/'driver.c';binary=d/'check'
        result=subprocess.run([str(compiler),'c','coax/tests/bitcoin/p2p-large.coax','--library','--header',str(h),*(['--release'] if mode=='release' else [])],cwd=ROOT,capture_output=True,text=True,timeout=840)
        assert result.returncode==0,result.stderr;source.write_text(result.stdout);driver.write_text(DRIVER)
        flags=['-O1','-g','-fsanitize=address,undefined','-fno-omit-frame-pointer'] if mode=='sanitized' else ['-O2']
        subprocess.run(['cc','-std=c11',*flags,str(source),str(driver),'-I'+str(d),'-lm','-o',str(binary)],check=True,capture_output=True,timeout=840)
        for n in (0,1,1048575,1048576,1048577,2097183,4000000,4000001):
            raw=bytes((i*73+(i>>16))&255 for i in range(n));(d/'payload').write_bytes(raw)
            r=subprocess.run([str(binary),str(d/'payload'),str(n)],cwd=d,capture_output=True,text=True,timeout=120)
            assert r.returncode==0,(mode,n,r.stdout,r.stderr)
            assert 'runtime error:' not in r.stderr and 'ERROR: AddressSanitizer' not in r.stderr,r.stderr
            if n>4000000:assert r.stdout=='-1 165\n';continue
            checksum=hashlib.sha256(hashlib.sha256(raw).digest()).digest()[:4]
            expected=bytes.fromhex('0a03cf40')+b'block'+bytes(7)+struct.pack('<I',n)+checksum+raw
            assert (d/'frame').read_bytes()==expected,(mode,n,'serialized frame differs')
            lines=r.stdout.splitlines();assert lines[:2]==[f'{n+24} 165',f'0 {n+24} 1'],(mode,n,lines)
            if n:assert lines[2]=='4 4 4 ',(mode,n,lines)
        print(f'P2P large {mode}: native codec boundary checks and eight sizes through 4,000,000 bytes, exact framing, fragmentation and three corruption positions pass',flush=True)

if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('--coax',type=Path,required=True);p.add_argument('--mode',choices=('debug','release','sanitized'),default='debug');a=p.parse_args();run(a.coax.resolve(),a.mode)
