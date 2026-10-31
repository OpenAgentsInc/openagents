#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: recomputes our answer outside Coax, so it must not share our implementation)
"""Owned source-equation oracle: enumerate grids, round BF16 independently.

No upstream modules are imported. BF16 input/output is serialized as bits;
Python binary64 holds every grid value and F32 operation exactly before its
explicit rounding boundary. The reference CUDA cast saturates finite FP8.
"""
import argparse
import bisect
import gzip
import hashlib
import json
import math
from pathlib import Path
import struct

ROOT=Path(__file__).resolve().parents[2]
DEST=ROOT/'psionic/fixtures/deepseek'
def f32(x):
    try:return struct.unpack('<f',struct.pack('<f',x))[0]
    except OverflowError:return math.copysign(math.inf,x)
def bits(x):return struct.unpack('<I',struct.pack('<f',x))[0]
def bf(b):return struct.unpack('<f',struct.pack('<I',b<<16))[0]
def bf_round(x):
    # Independent neighboring-BF16 choice, rather than the implementation's bias.
    b=bits(x);sign=b&0x80000000;a=b&0x7fffffff
    if a>=0x7f800000:return (b>>16)
    lo=a>>16;hi=lo+1
    low=bf(lo);high=bf(hi)
    mag=abs(x)
    if math.isinf(high):
        # Half an ulp beyond the largest BF16 is the overflow threshold.
        code=hi if mag>=bf(0x7f7f)+2**119 else lo
    else:code=min((lo,hi),key=lambda c:(abs(bf(c)-mag),c%2))
    return code|(sign>>16)
FP4=[0,.5,1,1.5,2,3,4,6]
FP8=[(c&7)*2**-9 if c<8 else (1+(c&7)/8)*2**((c>>3)-7) for c in range(127)]
def nearest(x,table):
    mag=abs(x);at=bisect.bisect_left(table,mag);choices={min(at,len(table)-1),max(0,at-1)}
    return min(choices,key=lambda c:(abs(table[c]-mag),c%2))
def block(kind,inputs):
    xs=[bf(b) for b in inputs]
    if not all(math.isfinite(x) for x in xs):return (1,0,[],[])
    amax=max(map(abs,xs))
    if kind==1:
        raw=f32(max(amax,6*2**-9)/6);scale_code=nearest(raw,FP8);scale=FP8[scale_code]
    else:
        inv=f32(1/(448 if kind==0 else 6));floor=f32(1e-4) if kind==0 else 6*2**-126
        product=f32(max(amax,floor)*inv)
        frac,exponent=math.frexp(product);power=exponent-1 if frac==.5 else exponent
        scale_code=power+127;scale=2.0**power
    codes=[];out=[];grid=FP8 if kind==0 else FP4;sign_bit=128 if kind==0 else 8
    for b,x in zip(inputs,xs):
        normalized=f32(x/scale);code=nearest(normalized,grid)|(sign_bit if b&0x8000 else 0)
        value=math.copysign(grid[code&(sign_bit-1)]*scale,x)
        rounded=bf_round(f32(value))
        if rounded&0x7f80==0x7f80:return (2,0,[],[])
        codes.append(code);out.append(rounded)
    return (0,scale_code,codes,out)
def matrix(kind,inputs,width):
    group=16 if kind==1 else 32;codes=[];scales=[];output=[]
    for at in range(0,len(inputs),group):
        status,s,c,y=block(kind,inputs[at:at+group]);assert status==0
        codes+=c;scales.append(s);output+=y
    data=codes if kind==0 else [codes[i]|codes[i+1]<<4 for i in range(0,len(codes),2)]
    return dict(kind=kind,width=width,rows=len(inputs)//width,input=inputs,data=data,scales=scales,output=output)
def make():
    fixtures=[]
    # Mixed signs, zero, subnormals, two scale regimes per row, and both tails.
    for kind in range(3):
        width=128 if kind==2 else 512;x=[]
        for i in range(3*width):
            v=f32(((i*37)%101-50)/16*2**((i//16)%9-5))
            x.append(bf_round(v))
        x[0]=0x8000;x[1]=0;x[2]=1;x[-1]=0x8080
        fixtures.append(matrix(kind,x,width))
    return fixtures
def exhaustive():
    output=bytearray()
    for kind in range(3):
        group=16 if kind==1 else 32
        for b in range(65536):
            status,s,c,y=block(kind,[b]*group)
            output+=struct.pack('<BBBH',status,s,c[0] if c else 0,y[0] if y else 0)
    return bytes(output)
def anchored():
    output=bytearray()
    for kind in range(3):
        group=16 if kind==1 else 32;anchor=bf_round(448 if kind==0 else 6)
        for b in range(65536):
            status,s,c,y=block(kind,[anchor,b]+[0]*(group-2))
            output+=struct.pack('<BBBBHH',status,s,c[0] if c else 0,c[1] if c else 0,y[0] if y else 0,y[1] if y else 0)
    return bytes(output)
def lanes():
    output=bytearray()
    for kind in range(3):
        for scale in range(256):
            for code in range(256 if kind==0 else 16):
                status=0;out=0
                if (kind==1 and not 0<scale<127) or (kind!=1 and scale==255) or (kind==0 and code&127==127):status=1
                else:
                    sign=128 if kind==0 else 8;value=(FP8 if kind==0 else FP4)[code&(sign-1)]*(FP8[scale] if kind==1 else 2.0**(scale-127))
                    value=math.copysign(value,-1 if code&sign else 1);out=bf_round(f32(value))
                    if out&0x7f80==0x7f80:status=2;out=0
                output+=struct.pack('<BH',status,out)
    return bytes(output)
def coax(fixtures):
    s='// Owned independent source-equation fixtures. Regenerate with deepseek_cache_oracle.py.\n'
    for i,f in enumerate(fixtures):
        for key,typ in [('input','u16'),('data','u8'),('scales','u8'),('output','u16')]:
            values=f[key];s+=f'DKG_{i}_{key.upper()} :: [{len(values)}]{typ}.['+','.join(map(str,values))+'];\n'
    return s
def header(fixtures):
    s='#include <stdint.h>\n'
    for i,f in enumerate(fixtures):
        for key,typ in [('input','uint16_t'),('data','uint8_t'),('scales','uint8_t'),('output','uint16_t')]:
            s+=f'static const {typ} dkg_{i}_{key}[]={{'+','.join(map(str,f[key]))+'};\n'
    source=json.loads((DEST/'cache-source.json').read_text())['original']
    s+='typedef struct {int64_t context,chunk,drafts,cache,step;} DkgBudget;\nstatic const DkgBudget dkg_budgets[]={\n'
    for context in [1,2,3,127,128,129,8192,1048576]:
        for drafts in range(4):
            for chunk in sorted({1,min(context,128),context}):
                cache=(source['n_layers']+drafts)*source['window_size']*(source['head_dim']+source['head_dim']//32)
                for layer in source['kv_source_layers']:
                    rows=context//source['compress_ratios'][layer]
                    cache+=rows*(source['head_dim']//2+source['head_dim']//16+source['index_head_dim']//2+source['index_head_dim']//32)
                # Count actual live codec buffers for each role, then take the
                # largest reusable lane. Other model intermediates are separate.
                widths=[(0,source['head_dim']),(1,source['head_dim']),(2,source['index_n_heads']*source['index_head_dim'])]
                step=max(6*chunk*w+2*chunk*(w if k==0 else w//2)+2*chunk*(w//(16 if k==1 else 32)) for k,w in widths)
                s+='{'+','.join(map(str,(context,chunk,drafts,cache,step)))+'},\n'
    s+='};\n'
    return s
def verify_sources():
    source=json.loads((DEST/'cache-source.json').read_text())
    for key,role in [('model.py','model'),('kernel.py','kernel'),('config.json','config')]:
        raw=gzip.decompress((DEST/f'cache-reference-{role}.txt.gz').read_bytes())
        assert hashlib.sha256(raw).hexdigest()==source['sources'][key]['sha256'],key
        if role=='config':
            original=json.loads(raw)
            assert all(original[k]==v for k,v in source['original'].items())
def main():
    p=argparse.ArgumentParser();p.add_argument('--write',action='store_true');p.add_argument('--header',action='store_true');p.add_argument('--sources',action='store_true');a=p.parse_args()
    if a.header:
        print(header(json.loads((DEST/'cache-goldens.json').read_text())),end='');return
    verify_sources()
    if a.sources:
        print('Cache source pins and retained original configuration agree');return
    fixtures=make();files={'cache-goldens.coax':coax(fixtures).encode(),'cache-goldens.json':(json.dumps(fixtures,separators=(',',':'))+'\n').encode(),'cache-bf16.bin':exhaustive(),'cache-anchored.bin':anchored(),'cache-lanes.bin':lanes()}
    for name,data in files.items():
        path=DEST/name
        if a.write:path.write_bytes(data)
        else:assert path.read_bytes()==data,name
    print('Cache oracle: three source grids, all 65,536 BF16 patterns/profile, mixed original-width rows agree')
if __name__=='__main__':main()
