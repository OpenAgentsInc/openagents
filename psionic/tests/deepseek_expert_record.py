#!/usr/bin/env python3
"""Replay retained original experts/MoE without weights, NumPy or inference."""
import argparse
import array
import gzip
import hashlib
import json
import math
from pathlib import Path
import struct
import sys
ROOT=Path(__file__).resolve().parents[2]


def sha(b):return hashlib.sha256(b).hexdigest()
def f32(x):return struct.unpack('<f',struct.pack('<f',x))[0]
def values(raw,code):
    a=array.array(code);a.frombytes(raw)
    if sys.byteorder!='little':a.byteswap()
    return a


def main():
    p=argparse.ArgumentParser();p.add_argument('--root',type=Path,default=ROOT/'psionic/fixtures/deepseek/expert-cpu-original');a=p.parse_args();d=a.root;m=json.loads((d/'manifest.json').read_text())
    for name,pin in m['files'].items():
        b=(d/name).read_bytes();assert len(b)==pin['bytes'] and sha(b)==pin['sha256'],name
    def raw(name):return gzip.decompress((d/name).read_bytes()) if name.endswith('.gz') else (d/name).read_bytes()
    policy=(ROOT/'psionic/tests/expert-qualification-policy.json').read_bytes();mixture_policy=(ROOT/'psionic/tests/expert-mixture-policy.json').read_bytes();assert sha(policy)==m['pair_policy_sha256'] and sha(mixture_policy)==m['mixture_policy_sha256'];pair=json.loads(policy);mix=json.loads(mixture_policy)
    def close(actual,reference,rule):
        assert len(actual)==len(reference)
        assert all(math.isfinite(x) and math.isfinite(y) and abs(x-y)<=rule['atol']+rule['rtol']*abs(y) for x,y in zip(actual,reference))
    for which in ['pair','mixture']:
        build=json.loads(raw(which+'/build-identity.json'));assert sha(raw(which+'/executed.c.gz'))==build['files'][m[which+'_executed_c_path']]
    pair_report=json.loads(raw('pair/comparison.json'));assert pair_report['policy_sha256']==sha(policy)
    for mode in ['routed','shared']:
        meta=json.loads(raw('pair/'+mode+'.json'));assert meta['width']==5120 and meta['hidden']==2304
        for kind,n in [('gate',2304),('up',2304),('activation',2304),('output',5120)]:
            actual=raw(f'pair/{mode}-{kind}.f32.gz');reference=raw(f'pair/{mode}-reference-{kind}.f32.gz');assert len(actual)==len(reference)==4*n
            close(values(actual,'f'),values(reference,'f'),pair['intermediate_and_output']);assert actual==reference # observed stronger equality, not an altered acceptance policy
        assert raw(f'pair/{mode}-down-grid.f64.gz')==raw(f'pair/{mode}-reference-down-grid.f64.gz')
        gate=values(raw(f'pair/{mode}-gate.f32.gz'),'f');up=values(raw(f'pair/{mode}-up.f32.gz'),'f');assert meta['gate_clamped']==sum(v>10 for v in gate) and meta['up_clamped']==sum(abs(v)>10 for v in up)
    report=json.loads(raw('mixture/comparison.json'));assert report['policy_sha256']==sha(mixture_policy)
    meta=json.loads(raw('mixture/mixture.json'));assert (meta['experts'],meta['selected'],meta['width'],meta['hidden'])==(384,6,5120,2304)
    for kind,code,rule,n in [('gate-logits','f',mix['intermediate_and_output'],384),('scores','d',mix['router_scores'],384),('ranked','d',mix['router_scores'],384),('weights','f',mix['route_weights'],6),('output','f',mix['intermediate_and_output'],5120)]:
        suffix='f32' if code=='f' else 'f64';actual=values(raw(f'mixture/mixture-{kind}.{suffix}.gz'),code);reference=values(raw(f'mixture/mixture-reference-{kind}.{suffix}.gz'),code);assert len(actual)==len(reference)==n;close(actual,reference,rule)
        if kind in ['gate-logits','weights','output']:assert actual.tobytes()==reference.tobytes()
    ids=struct.unpack('<6q',raw('mixture/mixture-ids.i64'));reference_ids=struct.unpack('<6q',raw('mixture/mixture-reference-ids.i64'));assert ids==reference_ids and len(set(ids))==6
    ranked=values(raw('mixture/mixture-ranked.f64.gz'),'d');scores=values(raw('mixture/mixture-scores.f64.gz'),'d');order=sorted(range(384),key=lambda i:(-ranked[i],i));assert ids==tuple(order[:6]) and meta['excluded']==order[6]
    margin=ranked[order[5]]-ranked[order[6]];assert abs(meta['margin']-margin)<=mix['router_scores']['atol'] and margin>=0
    weights=values(raw('mixture/mixture-weights.f32.gz'),'f');den=math.fsum(scores[i] for i in ids)+1e-20;assert list(weights)==[f32(scores[i]/den*1.5) for i in ids]
    parts=[values(raw(f'mixture/mixture-reference-expert-{i}.f32.gz'),'f') for i in ids];shared=values(raw('mixture/mixture-reference-shared.f32.gz'),'f');routed=values(raw('mixture/mixture-reference-routed.f32.gz'),'f');combined=values(raw('mixture/mixture-reference-output.f32.gz'),'f')
    assert all(len(row)==5120 for row in [*parts,shared,routed,combined])
    for j in range(5120):assert routed[j]==f32(math.fsum(row[j] for row in parts)) and combined[j]==f32(routed[j]+shared[j])
    print('Original experts/MoE: pinned executed builds, exact gate/FFN values, ordered 384/6 route, margins, pre-down weights and shared combination replay pass')


if __name__=='__main__':main()
