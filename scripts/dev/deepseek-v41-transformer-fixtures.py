#!/usr/bin/env python3
"""Small independent NumPy transformer fixtures from the pinned MIT reference.

Revision 1bae0e1088993b9b82de08f59de5be5a34e42170. No checkpoint or GPU.
"""
import json
from pathlib import Path
import numpy as np

OUT=Path(__file__).resolve().parents[2]/'crates/psionic/crates/psionic-models/src/testdata/deepseek_v41_cpu'


def write(name,data):
    (OUT/(name+'.json')).write_text(json.dumps(dict(revision='1bae0e1088993b9b82de08f59de5be5a34e42170',kind='synthetic_numpy',**data),indent=2,allow_nan=False)+'\n')


def norm(x,w):
    return w*(x/np.sqrt(np.mean(x*x,axis=-1,keepdims=True)+np.float32(1e-20)))


def mixes(x,part,tensors):
    get=lambda name:np.array(tensors['layers.1.'+name]['values'],dtype=np.float32).reshape(tensors['layers.1.'+name]['shape'])
    v=(get('hc_'+part+'_fn')@x.flatten())/np.sqrt(np.mean(x*x)+np.float32(1e-20))
    scale=get('hc_'+part+'_scale');base=get('hc_'+part+'_base');h=4;eps=np.float32(1e-6)
    pre=1/(1+np.exp(-(v[:h]*scale[0]+base[:h])))+eps
    post=2/(1+np.exp(-(v[h:2*h]*scale[1]+base[h:2*h])))
    comb=(v[2*h:]*scale[2]+base[2*h:]).reshape(h,h);comb=np.exp(comb-comb.max(axis=-1,keepdims=True));comb=comb/comb.sum(axis=-1,keepdims=True)+eps;comb/=comb.sum(axis=0,keepdims=True)+eps
    for _ in range(19):
        comb/=comb.sum(axis=-1,keepdims=True)+eps;comb/=comb.sum(axis=0,keepdims=True)+eps
    return pre,post,comb


def block():
    t={}
    def add(name,shape,scale=.2):
        x=(np.sin(np.arange(np.prod(shape))*.37+len(t)) * scale).astype(np.float32).reshape(shape)
        t['layers.1.'+name]=dict(shape=shape,values=x.flatten().tolist());return x
    def put(name,x):
        x=np.array(x,dtype=np.float32);t['layers.1.'+name]=dict(shape=list(x.shape),values=x.flatten().tolist());return x
    for part in ['attn','ffn']:
        add('hc_'+part+'_fn',[24,16],.03);put('hc_'+part+'_scale',[.05,-.1,.2]);put('hc_'+part+'_base',np.linspace(-.8,.6,24,dtype=np.float32))
    an=put('attn_norm.weight',np.linspace(.75,1.25,4,dtype=np.float32));fn=put('ffn_norm.weight',np.linspace(1.2,.8,4,dtype=np.float32))
    gw=add('ffn.gate.weight',[8,4],.4);bias=put('ffn.gate.bias',np.linspace(-.1,.1,8,dtype=np.float32))
    for stem in [f'ffn.experts.{i}' for i in range(8)]+['ffn.shared_experts']:
        add(stem+'.w1.weight',[6,4],.4);add(stem+'.w3.weight',[6,4],.4);add(stem+'.w2.weight',[4,6],.2)
    table=add('engram.embed.weight',[64,2],.08);wkv=add('engram.wkv.weight',[20,48],.03)
    qw=put('engram.q_weight',np.linspace(.75,1.25,16,dtype=np.float32).reshape(4,4));kw=put('engram.k_weight',np.linspace(1.2,.8,16,dtype=np.float32).reshape(4,4))
    get=lambda n:np.array(t['layers.1.'+n]['values'],dtype=np.float32).reshape(t['layers.1.'+n]['shape'])
    x=np.array([[1,-2,.5,3],[.75,1,-1,2],[2,-.5,1,.25],[-1,2,.75,-.5]],dtype=np.float32)
    prev=np.array([.1,.2,.3,.4],dtype=np.float32);attn=np.array([.25,-.5,.75,1],dtype=np.float32);hashes=[i*7%64 for i in range(24)]
    projected=wkv@table[hashes].flatten();key=projected[:16].reshape(4,4);value=projected[16:]
    rstd=1/np.sqrt(np.mean(x*x,axis=-1)+np.float32(1e-20)) / np.sqrt(np.mean(key*key,axis=-1)+np.float32(1e-20))
    dot=(x*(qw*kw)*key).sum(axis=-1)*rstd*np.float32(.5);gate=1/(1+np.exp(-np.copysign(np.sqrt(np.maximum(np.abs(dot),np.float32(1e-6))),dot)))
    h=x+gate[:,None]*value;ap,post,comb=mixes(h,'attn',t);ai=norm((h*prev[:,None]).sum(axis=0),an)
    after=post[:,None]*attn+comb.T@h;fp,post2,comb2=mixes(after,'ffn',t);fi=norm((after*ap[:,None]).sum(axis=0),fn)
    scores=np.sqrt(np.log1p(np.exp(gw@fi)));ids=np.argsort(-(scores+bias),kind='stable')[:2];weights=scores[ids].copy();weights/=weights.sum()+np.float32(1e-20);weights*=np.float32(1.5)
    def expert(stem,weight):
        gate=np.minimum(get(stem+'.w1.weight')@fi,10);up=np.clip(get(stem+'.w3.weight')@fi,-10,10)
        return get(stem+'.w2.weight')@((gate/(1+np.exp(-gate))*up)*weight)
    y=np.zeros(4,dtype=np.float32)
    for i,w in sorted(zip(ids,weights)):y+=expert(f'ffn.experts.{i}',w)
    y+=expert('ffn.shared_experts',np.float32(1));out=post2[:,None]*y+comb2.T@after
    expected={k:v.flatten().tolist() for k,v in dict(engram=h,attention_input=ai,attn_pre=ap,attn_comb=comb,after_attention=after,output=out,next_pre=fp,route_weights=weights).items()}
    write('block',dict(tensors=t,residual=x.flatten().tolist(),previous_pre=prev.tolist(),attention_output=attn.tolist(),hashes=hashes,expected=expected,expert_ids=ids.tolist()))


if __name__=='__main__':
    block()
