#!/usr/bin/env python3
"""Opt-in independent NumPy original-expert comparison; no upstream runtime."""
import argparse
from decimal import Decimal,localcontext
import hashlib
import json
from pathlib import Path
import struct
import sys
import time
import numpy as np
sys.path.insert(0,str(Path(__file__).resolve().parent))
from deepseek_expert_oracle import scalar,quantize,activate,f32
ROOT=Path(__file__).resolve().parents[2]


def main():
    p=argparse.ArgumentParser();p.add_argument('original',type=Path);p.add_argument('capture',type=Path);p.add_argument('--mixture',action='store_true');a=p.parse_args()
    record=json.loads((ROOT/'psionic/fixtures/deepseek/expert-original.json').read_text());assert a.original.stat().st_size==record['pin']['bytes']
    with a.original.open('rb') as f:n=struct.unpack('<Q',f.read(8))[0];raw=f.read(n)
    assert hashlib.sha256(raw).hexdigest()==record['raw_header_sha256'];header=json.loads(raw);payload=n+8;mm=np.memmap(a.original,mode='r',dtype='u1')
    def tensor(name):
        h=header[name];assert h==record['reviewed'][name];lo,hi=h['data_offsets'];raw=np.asarray(mm[payload+lo:payload+hi])
        if h['dtype']=='BF16':return (raw.view('<u2').astype('<u4')<<16).view('<f4').reshape(h['shape'])
        if h['dtype']=='F32':return raw.view('<f4').reshape(h['shape'])
        return raw.reshape(h['shape'])
    def matrix(prefix,branch,shared):
        data=tensor(prefix+branch+'.weight');scales=tensor(prefix+branch+'.scale');assert np.all(scales!=255)
        if shared:
            assert np.all((data&127)!=127);lut=np.array([scalar('fp8',c,127) if c&127!=127 else np.nan for c in range(256)],dtype='f8');values=lut[data]
            factors=np.exp2(scales.astype('f8')-127);factors=np.repeat(np.repeat(factors,32,axis=0),32,axis=1)[:values.shape[0],:values.shape[1]]
        else:
            codes=np.empty((data.shape[0],data.shape[1]*2),'u1');codes[:,::2]=data&15;codes[:,1::2]=data>>4
            values=np.array([scalar('fp4',c,127) for c in range(16)],'f8')[codes];factors=np.repeat(np.exp2(scales.astype('f8')-127),32,axis=1)
        return values*factors
    if a.mixture:
        policy=(ROOT/'psionic/tests/expert-mixture-policy.json').read_bytes();rules=json.loads(policy);meta=json.loads((a.capture/'mixture.json').read_text());x=np.fromfile(a.capture/'mixture-input.f32',dtype='<f4');assert len(x)==5120
        assert np.array_equal(x,np.array([f32(((i*17)%29-14)/8) for i in range(5120)],'f4'))
        begin=time.monotonic();logits=(tensor('layers.0.ffn.gate.weight').astype('f8')@x.astype('f8')).astype('f4');bias=tensor('layers.0.ffn.gate.bias').astype('f8');scores=[]
        with localcontext() as ctx:
            ctx.prec=80
            for v in logits:
                d=Decimal.from_float(float(v));soft=d if d>20 else (1+d.exp()).ln();scores.append(soft.sqrt())
            ranked=[s+Decimal.from_float(float(b)) for s,b in zip(scores,bias)];order=sorted(range(384),key=lambda i:(-ranked[i],i));ids=order[:6];den=sum(scores[i] for i in ids)+Decimal('1e-20');weights=np.array([float(Decimal('1.5')*scores[i]/den) for i in ids],'f4');margin=float(ranked[ids[-1]]-ranked[order[6]])
        def execute_original(prefix,route):
            q=np.array(quantize(x.tolist())['values'],'f8');gate=(matrix(prefix,'w1',prefix.endswith('shared_experts.'))@q).astype('f4');up=(matrix(prefix,'w3',prefix.endswith('shared_experts.'))@q).astype('f4');act=activate(gate.tolist(),up.tolist(),float(route),10);grid=np.array(quantize(act['values'])['values'],'f8');out=(matrix(prefix,'w2',prefix.endswith('shared_experts.'))@grid).astype('f4');return out,act
        parts=[];gc=uc=0
        for expert_id,weight in zip(ids,weights):
            out,act=execute_original(f'layers.0.ffn.experts.{expert_id}.',weight);parts.append(out);gc+=act['gate_clamped'];uc+=act['up_clamped'];out.astype('<f4').tofile(a.capture/f'mixture-reference-expert-{expert_id}.f32')
        shared,act=execute_original('layers.0.ffn.shared_experts.',1);gc+=act['gate_clamped'];uc+=act['up_clamped'];routed=np.sum(np.array(parts,'f8'),axis=0).astype('f4');combined=(routed.astype('f8')+shared.astype('f8')).astype('f4')
        report=dict(ordered_ids_exact=np.array_equal(np.fromfile(a.capture/'mixture-ids.i64',dtype='<i8'),np.array(ids,'i8')),clamp_counts_exact=gc==meta['gate_clamped'] and uc==meta['up_clamped'],reference_margin=margin,margin_abs_error=abs(meta['margin']-margin),reference_excluded=order[6],excluded_exact=meta['excluded']==order[6])
        for name,reference,dtype,rule in [('gate-logits',logits,'f4',rules['intermediate_and_output']),('scores',np.array([float(s) for s in scores]),'f8',rules['router_scores']),('ranked',np.array([float(s) for s in ranked]),'f8',rules['router_scores']),('weights',weights,'f4',rules['route_weights']),('output',combined,'f4',rules['intermediate_and_output'])]:
            actual=np.fromfile(a.capture/f'mixture-{name}.{dtype.replace("f4","f32").replace("f8","f64")}',dtype='<'+dtype);assert actual.shape==reference.shape
            error=np.abs(actual.astype('f8')-reference.astype('f8'));bound=rule['atol']+rule['rtol']*np.abs(reference.astype('f8'));report[name]=dict(max_abs=float(np.max(error)),max_bound_ratio=float(np.max(error/bound)),bit_identical=actual.tobytes()==reference.astype(dtype).tobytes(),passed=bool(np.all(np.isfinite(actual)) and np.all(error<=bound)));reference.astype('<'+dtype).tofile(a.capture/f'mixture-reference-{name}.{dtype.replace("f4","f32").replace("f8","f64")}')
        np.array(ids,'<i8').tofile(a.capture/'mixture-reference-ids.i64');shared.astype('<f4').tofile(a.capture/'mixture-reference-shared.f32');routed.astype('<f4').tofile(a.capture/'mixture-reference-routed.f32');report['reference_seconds']=time.monotonic()-begin;report['policy_sha256']=hashlib.sha256(policy).hexdigest();report['passed']=report['ordered_ids_exact'] and report['clamp_counts_exact'] and report['excluded_exact'] and report['margin_abs_error']<=rules['router_scores']['atol'] and all(report[k]['passed'] for k in ['gate-logits','scores','ranked','weights','output'])
        (a.capture/'comparison.json').write_text(json.dumps(report,indent=2)+'\n');assert report['passed'],report;print(json.dumps(report,indent=2));return
    policy=(ROOT/'psionic/tests/expert-qualification-policy.json').read_bytes();rule=json.loads(policy)['intermediate_and_output'];reports={}
    for shared in [False,True]:
        begin=time.monotonic();tag='shared' if shared else 'routed';meta=json.loads((a.capture/f'{tag}.json').read_text());prefix='layers.0.ffn.'+('shared_experts.' if shared else 'experts.0.')
        x=np.fromfile(a.capture/f'{tag}-input.f32',dtype='<f4');expected=np.array([f32(((i*17)%29-14)/8) for i in range(5120)],'f4');assert np.array_equal(x,expected)
        q=np.array(quantize(x.tolist())['values'],'f8');gate=(matrix(prefix,'w1',shared)@q).astype('f4');up=(matrix(prefix,'w3',shared)@q).astype('f4')
        act=activate(gate.tolist(),up.tolist(),f32(meta['route']),10);weighted=np.array(act['values'],'f4');down_grid=np.array(quantize(weighted.tolist())['values'],'f8');out=(matrix(prefix,'w2',shared)@down_grid).astype('f4')
        report={}
        for name,reference in [('gate',gate),('up',up),('activation',weighted),('output',out)]:
            actual=np.fromfile(a.capture/f'{tag}-{name}.f32',dtype='<f4');assert actual.shape==reference.shape
            error=np.abs(actual.astype('f8')-reference.astype('f8'));bound=rule['atol']+rule['rtol']*np.abs(reference.astype('f8'))
            report[name]=dict(values=reference.size,max_abs=float(np.max(error)),max_bound_ratio=float(np.max(error/bound)),bit_identical=actual.tobytes()==reference.tobytes(),passed=bool(np.all(np.isfinite(actual)) and np.all(error<=bound)))
            reference.astype('<f4').tofile(a.capture/f'{tag}-reference-{name}.f32')
        actual_grid=np.fromfile(a.capture/f'{tag}-down-grid.f64',dtype='<f8');report['down_grid_exact']=actual_grid.tobytes()==down_grid.astype('<f8').tobytes();down_grid.astype('<f8').tofile(a.capture/f'{tag}-reference-down-grid.f64')
        report['clamp_counts_exact']=meta['gate_clamped']==act['gate_clamped'] and meta['up_clamped']==act['up_clamped'];report['reference_seconds']=time.monotonic()-begin
        report['passed']=all(report[k]['passed'] for k in ['gate','up','activation','output']) and report['down_grid_exact'] and report['clamp_counts_exact'];reports[tag]=report
        (a.capture/f'{tag}-comparison.json').write_text(json.dumps(report,indent=2)+'\n');assert report['passed'],report
    (a.capture/'comparison.json').write_text(json.dumps(dict(policy_sha256=hashlib.sha256(policy).hexdigest(),experts=reports),indent=2)+'\n');print(json.dumps(reports,indent=2))


if __name__=='__main__':main()
