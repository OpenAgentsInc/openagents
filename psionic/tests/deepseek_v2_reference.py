#!/usr/bin/env python3
"""Opt-in independent NumPy full-prefix equation oracle; no model framework.

Reads original BF16 directly. It never calls generated Coax or upstream code.
All query positions are evaluated together with an explicit causal mask, with
no incremental cache. Candidate input IDs suffice to prove every greedy choice:
the first different next token would fail the exact decision comparison.
"""
import argparse
import hashlib
import json
import math
from pathlib import Path
import struct
import time
import numpy as np

ROOT=Path(__file__).resolve().parents[2]
FIX=ROOT/'psionic/fixtures/deepseek'


class Weights:
    def __init__(self,paths):
        self.tensors={};self.maps=[]
        for path in paths:
            mm=np.memmap(path,dtype='u1',mode='r');self.maps.append(mm)
            n=struct.unpack('<Q',bytes(mm[:8]))[0];header=json.loads(bytes(mm[8:8+n]))
            for name,h in header.items():
                if name=='__metadata__':continue
                assert h['dtype']=='BF16';lo,hi=h['data_offsets']
                assert name not in self.tensors
                self.tensors[name]=(mm[8+n+lo:8+n+hi].view('<u2'),h['shape'])

    def get(self,name):
        raw,shape=self.tensors[name]
        return (raw.astype('<u4')<<16).view('<f4').reshape(shape).astype('f8')

    def project(self,name,x):
        return (x.astype('f8')@self.get(name).T).astype('f4')

    def norm(self,name,x):
        y=x.astype('f8');return (y/np.sqrt(np.sum(y*y,axis=-1,keepdims=True)/y.shape[-1]+1e-6)*self.get(name)).astype('f4')

    def ffn(self,p,x):
        gate=self.project(p+'gate_proj.weight',x).astype('f8');up=self.project(p+'up_proj.weight',x).astype('f8')
        e=np.exp(-np.abs(gate));act=(gate*(np.where(np.signbit(gate),e,1)/(1+e))*up).astype('f4')
        return self.project(p+'down_proj.weight',act)


def full(w,s,tokens,mode,verbose=False):
    n=len(tokens);a=s['attention'];h,nope,rope,v,z=[a[k] for k in ['heads','nope','rope','value','latent']];d=s['width'];k=s['selected'];layers=s['layers']
    x=w.get('model.embed_tokens.weight')[tokens].astype('f4');trace=np.empty((n,2*layers,d),'f4');ids=np.zeros((n,layers,8),'i8');weights=np.zeros((n,layers,8),'f8');margins=np.zeros((n,layers),'f8');caches=[]
    low=max(math.floor(rope*math.log(4096/(32*2*math.pi))/(2*math.log(10000))),0)
    high=min(math.ceil(rope*math.log(4096/(2*math.pi))/(2*math.log(10000))),rope-1)
    freq=[10000**(-2*i/rope)*(1-min(max((i-low)/(high-low),0),1)+min(max((i-low)/(high-low),0),1)/40) for i in range(rope//2)]
    cos=np.array([[math.cos(p*f) for f in freq] for p in range(n)],'f4').astype('f8')
    sin=np.array([[math.sin(p*f) for f in freq] for p in range(n)],'f4').astype('f8')
    def rotate(y):
        even=y[...,::2].astype('f8');odd=y[...,1::2].astype('f8');c=cos[:,None,:] if y.ndim==3 else cos;sn=sin[:,None,:] if y.ndim==3 else sin
        return np.concatenate([even*c-odd*sn,odd*c+even*sn],axis=-1).astype('f4')
    scale=(1+.1*.707*math.log(40))**2/math.sqrt(nope+rope)
    mask=np.triu(np.ones((n,n),bool),1)
    for layer in range(layers):
        begin=time.monotonic();p=f'model.layers.{layer}.';normal=w.norm(p+'input_layernorm.weight',x)
        query=w.project(p+'self_attn.q_proj.weight',normal).reshape(n,h,nope+rope)
        kv=w.project(p+'self_attn.kv_a_proj_with_mqa.weight',normal)
        latent=w.norm(p+'self_attn.kv_a_layernorm.weight',kv[:,:z]);key=rotate(kv[:,z:]);qr=rotate(query[:,:,nope:]);qn=query[:,:,:nope]
        wb=w.get(p+'self_attn.kv_b_proj.weight').reshape(h,nope+v,z)
        if mode=='compressed':
            absorbed=np.einsum('thi,hiz->thz',qn.astype('f8'),wb[:,:nope,:],optimize=True).astype('f4')
            scores=np.einsum('thz,sz->hts',absorbed.astype('f8'),latent.astype('f8'),optimize=True)
            caches.append(np.concatenate([latent,key],axis=-1))
        else:
            expanded=(latent.astype('f8')@wb.reshape(h*(nope+v),z).T).astype('f4').reshape(n,h,nope+v)
            scores=np.einsum('thi,shi->hts',qn.astype('f8'),expanded[:,:,:nope].astype('f8'),optimize=True)
            caches.append(np.concatenate([expanded.reshape(n,-1),key],axis=-1))
        scores+=np.einsum('thi,si->hts',qr.astype('f8'),key.astype('f8'),optimize=True);scores*=scale;scores[:,mask]=-np.inf
        probability=np.exp(scores-np.max(scores,axis=-1,keepdims=True));probability/=np.sum(probability,axis=-1,keepdims=True)
        # Coax divides the final weighted sum. Normalizing each probability is
        # algebraically independent; binary64 differences are below F32 rounding.
        if mode=='compressed':
            context=np.einsum('hts,sz->thz',probability,latent.astype('f8'),optimize=True).astype('f4')
            attended=np.einsum('thz,hvz->thv',context.astype('f8'),wb[:,nope:,:],optimize=True).astype('f4')
        else:attended=np.einsum('hts,shv->thv',probability,expanded[:,:,nope:].astype('f8'),optimize=True).astype('f4')
        x=(x.astype('f8')+w.project(p+'self_attn.o_proj.weight',attended.reshape(n,-1)).astype('f8')).astype('f4');trace[:,2*layer]=x
        normal=w.norm(p+'post_attention_layernorm.weight',x)
        if layer<s['dense_layers']:combined=w.ffn(p+'mlp.',normal)
        else:
            logits=w.project(p+'mlp.gate.weight',normal).astype('f8');scores=np.exp(logits-np.max(logits,axis=-1,keepdims=True));scores/=np.sum(scores,axis=-1,keepdims=True)
            order=np.argsort(-scores,axis=-1,kind='stable');chosen=order[:,:k];selected=np.take_along_axis(scores,chosen,axis=1)
            ids[:,layer,:k]=chosen;weights[:,layer,:k]=selected
            if k<s['experts']:margins[:,layer]=selected[:,-1]-np.take_along_axis(scores,order[:,k:k+1],axis=1)[:,0]
            parts=np.zeros((n,k,d),'f8')
            for expert in np.unique(chosen):
                rows,slots=np.nonzero(chosen==expert);parts[rows,slots]=w.ffn(p+f'mlp.experts.{expert}.',normal[rows]).astype('f8')
            routed=np.sum(parts*selected[:,:,None],axis=1).astype('f4')
            combined=(routed.astype('f8')+w.ffn(p+'mlp.shared_experts.',normal).astype('f8')).astype('f4')
        x=(x.astype('f8')+combined.astype('f8')).astype('f4');trace[:,2*layer+1]=x
        if verbose:print(mode,'reference layer',layer,'seconds',round(time.monotonic()-begin,3),flush=True)
    logits=w.project('lm_head.weight',w.norm('model.norm.weight',x))
    return dict(tokens=np.asarray(tokens,'i8'),logits=logits,trace=trace.reshape(n,-1),ids=ids,weights=weights,margins=margins,cache=np.array(caches))


def capture(path):
    raw=path.read_bytes();assert raw[:8]==b'DV2RUN01';v,t,l=struct.unpack_from('<qqq',raw,8)
    dtype=np.dtype([('token','<i8'),('position','<i8'),('logits','<f4',(v,)),('trace','<f4',(t,)),('routes',[('ids','<i8',(8,)),('weights','<f8',(8,)),('margin','<f8')],(l,))])
    assert (len(raw)-32)%dtype.itemsize==0;data=np.frombuffer(raw,offset=32,dtype=dtype);assert np.array_equal(data['position'],np.arange(len(data)))
    return dict(tokens=data['token'],logits=data['logits'],trace=data['trace'],ids=data['routes']['ids'],weights=data['routes']['weights'],margins=data['routes']['margin'])


def compare(ref,candidate,policy):
    report={}
    for key in ['logits','trace','weights']:
        rule=policy['route_weights' if key=='weights' else 'logits_and_residuals'];left=ref[key].astype('f8');right=candidate[key].astype('f8');error=np.abs(left-right);bound=rule['atol']+rule['rtol']*np.abs(left)
        report[key]=dict(max_abs=float(np.max(error)),max_bound_ratio=float(np.max(error/bound)),pass_bound=bool(np.all(np.isfinite(right)) and np.all(error<=bound)))
    report['ordered_router_ids_exact']=bool(np.array_equal(ref['ids'],candidate['ids']))
    report['greedy_every_position_exact']=bool(np.array_equal(np.argmax(ref['logits'],axis=1),np.argmax(candidate['logits'],axis=1)))
    report['minimum_reference_route_margin']=float(np.min(ref['margins'][:,1:]));report['maximum_margin_abs_error']=float(np.max(np.abs(ref['margins']-candidate['margins'])))
    report['passed']=all(report[k]['pass_bound'] for k in ['logits','trace','weights']) and report['ordered_router_ids_exact'] and report['greedy_every_position_exact']
    return report


def owned():
    for label in ['tiny','wide']:
        path=FIX/f'v2-{label}';data=json.loads((path/'reference.json').read_text());w=Weights([path/'model.safetensors'])
        # Read fixture dimensions as inert Coax metadata through the owned author.
        import sys
        sys.path.insert(0,str(Path(__file__).resolve().parent))
        from deepseek_v2_fixture import S,TOKENS
        s=S.copy()
        if label=='wide':s=dict(layers=3,width=6,vocab=11,dense_layers=1,dense_hidden=7,experts=4,selected=3,expert_hidden=5,shared=1,attention=dict(heads=3,nope=3,rope=4,value=2,latent=4,capacity=5))
        for mode in ['compressed','expanded']:
            r=full(w,s,TOKENS,mode)
            for key in ['logits','trace']:assert np.allclose(r[key],np.array([v[key] for v in data[mode]]),atol=2e-6,rtol=0),key
            assert np.allclose(r['cache'],np.array(data[mode][-1]['cache']),atol=2e-6,rtol=0)
            for t,item in enumerate(data[mode]):
                for l,route in enumerate(item['routes']):
                    if route:assert r['ids'][t,l,:s['selected']].tolist()==route['ids'] and np.allclose(r['weights'][t,l,:s['selected']],route['weights'],atol=1e-12,rtol=0)
        print('NumPy cache-free oracle matches independent scalar owned',label,'both forms',flush=True)


def main():
    p=argparse.ArgumentParser();p.add_argument('--owned',action='store_true');p.add_argument('--original',type=Path);p.add_argument('--capture',type=Path);p.add_argument('--mode',choices=['compressed','expanded']);a=p.parse_args()
    if a.owned:owned();return
    assert a.original and a.capture
    import sys
    sys.path.insert(0,str(Path(__file__).resolve().parent))
    from deepseek_v2_artifacts import SHAPE
    pins=json.loads((FIX/'v2-original/pins.json').read_text())['files'];paths=[]
    for pin in pins:
        file=a.original/pin['name'];digest=hashlib.sha256()
        with file.open('rb') as f:
            while block:=f.read(16<<20):digest.update(block)
        assert file.stat().st_size==pin['bytes'] and digest.hexdigest()==pin['sha256']
        if file.suffix=='.safetensors':paths.append(file)
    w=Weights(paths);policy=json.loads((ROOT/'psionic/tests/v2-qualification-policy.json').read_text());reports={}
    for mode in [a.mode] if a.mode else ['compressed','expanded']:
        meta=json.loads((a.capture/f'{mode}.json').read_text());assert meta['eos'] and meta['generated'][-1]==100001
        candidate=capture(a.capture/f'{mode}.bin');assert candidate['tokens'].tolist()==meta['prompt']+meta['generated'][:-1]
        begin=time.monotonic();ref=full(w,SHAPE,candidate['tokens'],mode,True)
        decisions=np.argmax(ref['logits'],axis=1)[len(meta['prompt'])-1:];assert decisions.tolist()==meta['generated']
        reports[mode]=compare(ref,candidate,policy);reports[mode]['reference_seconds']=time.monotonic()-begin
        np.savez_compressed(a.capture/f'{mode}-reference.npz',**ref)
        (a.capture/f'{mode}-comparison.json').write_text(json.dumps(reports[mode],indent=2)+'\n')
        assert reports[mode]['passed'],reports[mode]
    if not a.mode:(a.capture/'comparison.json').write_text(json.dumps(reports,indent=2)+'\n')
    print(json.dumps(reports,indent=2))


if __name__=='__main__':main()
