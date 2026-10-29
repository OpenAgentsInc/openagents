#!/usr/bin/env python3
"""Verify every retained original step without weights, NumPy or inference."""
import array
import gzip
import hashlib
import json
import math
from pathlib import Path
import struct
import sys

ROOT=Path(__file__).resolve().parents[2]
DEST=ROOT/'psionic/fixtures/deepseek/v2-cpu-original'


def sha(b):return hashlib.sha256(b).hexdigest()


def xor(a,b):
    assert len(a)==len(b)
    return bytes(x^y for x,y in zip(a,b))


def floats(raw,code):
    a=array.array(code);a.frombytes(raw)
    if sys.byteorder!='little':a.byteswap()
    return a


def records(raw):
    assert raw[:8]==b'DV2RUN01';v,t,l=struct.unpack_from('<qqq',raw,8);assert (v,t,l)==(102400,110592,27)
    width=16+4*(v+t)+l*136;assert (len(raw)-32)%width==0
    for pos,start in enumerate(range(32,len(raw),width)):
        token,index=struct.unpack_from('<qq',raw,start);assert index==pos;start+=16
        logits=floats(raw[start:start+4*v],'f');start+=4*v;trace=floats(raw[start:start+4*t],'f');start+=4*t
        routes=[]
        for layer in range(l):
            ids=struct.unpack_from('<8q',raw,start);weights=struct.unpack_from('<8d',raw,start+64);margin=struct.unpack_from('<d',raw,start+128)[0];start+=136
            routes.append((ids,weights,margin))
        yield token,logits,trace,routes


def main():
    m=json.loads((DEST/'manifest.json').read_text());policy=(ROOT/'psionic/tests/v2-qualification-policy.json').read_bytes();assert sha(policy)==m['policy_sha256'];p=json.loads(policy)
    for name,pin in m['files'].items():
        raw=(DEST/name).read_bytes();assert len(raw)==pin['bytes'] and sha(raw)==pin['sha256'],name
    build=json.loads((DEST/'build-identity.json').read_text());assert sha(gzip.decompress((DEST/'executed.c.gz').read_bytes()))==build['files'][m['executed_c_path']]
    for name,digest in m['rebuild_sources'].items():assert sha(gzip.decompress((DEST/name).read_bytes()))==digest
    compressed=gzip.decompress((DEST/'compressed-candidate.bin.gz').read_bytes())
    expanded=xor(compressed,gzip.decompress((DEST/'expanded-candidate.xor.gz').read_bytes()))
    metas=[]
    for mode,candidate in [('compressed',compressed),('expanded',expanded)]:
        reference=xor(candidate,gzip.decompress((DEST/f'{mode}-reference.xor.gz').read_bytes()));assert sha(candidate)==m['captures'][mode]['candidate_sha256'] and sha(reference)==m['captures'][mode]['reference_sha256']
        meta=json.loads((DEST/f'{mode}.json').read_text());metas.append(meta);assert meta['eos'] and meta['stop']==1 and meta['generated'][-1]==100001
        expected=meta['prompt']+meta['generated'][:-1];actual=list(records(candidate));oracle=list(records(reference));assert len(actual)==len(oracle)==len(expected)==meta['steps']
        for pos,(a,b) in enumerate(zip(actual,oracle)):
            assert a[0]==b[0]==expected[pos]
            # These retained original captures happened to agree bit for bit;
            # this observation is stronger than the frozen acceptance policy.
            assert a[1].tobytes()==b[1].tobytes() and a[2].tobytes()==b[2].tobytes()
            for key,left,right in [('logits',a[1],b[1]),('trace',a[2],b[2])]:
                rule=p['logits_and_residuals'];assert all(math.isfinite(x) and math.isfinite(y) and abs(x-y)<=rule['atol']+rule['rtol']*abs(y) for x,y in zip(left,right)),(mode,pos,key)
            chosen=max(range(len(a[1])),key=a[1].__getitem__);assert chosen==max(range(len(b[1])),key=b[1].__getitem__)
            if pos>=len(meta['prompt'])-1:assert chosen==meta['generated'][pos-len(meta['prompt'])+1]
            for layer,(left,right) in enumerate(zip(a[3],b[3])):
                assert left[0]==right[0];rule=p['route_weights'];assert all(math.isfinite(x) and math.isfinite(y) and abs(x-y)<=rule['atol']+rule['rtol']*abs(y) for x,y in zip(left[1],right[1]))
                assert math.isfinite(left[2]) and math.isfinite(right[2]) and left[2]>=0 and right[2]>=0
                if layer:assert len(set(left[0][:6]))==6 and all(0<=x<64 for x in left[0][:6])
        # Independently decode the original-derived byte table, stripping EOS by ID.
        table=(ROOT/'psionic/fixtures/deepseek/v2-original/v2-token-bytes.bin').read_bytes();count=struct.unpack_from('<I',table,8)[0];pool=16+8*count;answer=b''
        for token in meta['generated'][:-1]:
            assert 0<=token<count and token!=100000;lo,n=struct.unpack_from('<II',table,16+8*token);answer+=table[pool+lo:pool+lo+n]
        assert answer==(DEST/f'{mode}.txt').read_bytes()
    assert metas[0]['generated']==metas[1]['generated'] and (DEST/'compressed.txt').read_bytes()==(DEST/'expanded.txt').read_bytes()
    for a,b in zip(records(compressed),records(expanded)):
        assert a[0]==b[0]
        rule=p['logits_and_residuals']
        for left,right in [(a[1],b[1]),(a[2],b[2])]:assert all(abs(x-y)<=rule['atol']+rule['rtol']*abs(x) for x,y in zip(left,right))
        for left,right in zip(a[3],b[3]):
            assert left[0]==right[0];rule=p['route_weights'];assert all(abs(x-y)<=rule['atol']+rule['rtol']*abs(x) for x,y in zip(left[1],right[1]))
    print('V2 retained original: all 62 steps, all logits/residuals, ordered routes/weights, exact greedy IDs and typed EOS verify')


if __name__=='__main__':main()
