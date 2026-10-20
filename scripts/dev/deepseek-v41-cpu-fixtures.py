#!/usr/bin/env python3
"""Generate small CPU math fixtures; requires NumPy, no checkpoint or GPU.

Equations reimplemented from DeepSeek's MIT-licensed reference, revision
1bae0e1088993b9b82de08f59de5be5a34e42170. These synthetic float32 fixtures
are not outputs from the official TileLang kernels or full checkpoint.
"""
import json
from pathlib import Path
import numpy as np

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / 'crates/psionic/crates/psionic-models/src/testdata/deepseek_v41_cpu'
REV = '1bae0e1088993b9b82de08f59de5be5a34e42170'


def write(name, data):
    payload = {'revision': REV, 'kind': 'synthetic_numpy_float32', **data}
    (OUT / (name + '.json')).write_text(json.dumps(payload, indent=2, allow_nan=False) + '\n')


def hyperconnections():
    # model.py:950-975 and kernel.py:407-459; NumPy broadcasting mirrors
    # the reference's dimensions, including source/target residual orientation.
    h, d = 4, 3
    x = np.array([[-1, 2, .5], [3, -.25, 1], [0, -2, 4], [.75, 1.25, -3]], dtype=np.float32)
    fn = np.array([np.sin(np.arange(h*d) + j) * .3 for j in range((h+2)*h)], dtype=np.float32)
    base = np.linspace(-1, 1.5, (h+2)*h, dtype=np.float32)
    scale = np.array([.3, -.5, 1.2], dtype=np.float32)
    eps, norm_eps = np.float32(1e-6), np.float32(1e-20)
    mixes = (fn @ x.flatten()) / np.sqrt(np.mean(x*x) + norm_eps)
    pre = 1/(1+np.exp(-(mixes[:h]*scale[0]+base[:h])))+eps
    post = 2/(1+np.exp(-(mixes[h:2*h]*scale[1]+base[h:2*h])))
    comb = (mixes[2*h:]*scale[2]+base[2*h:]).reshape(h,h)
    comb = np.exp(comb-comb.max(axis=-1,keepdims=True))
    comb = comb/comb.sum(axis=-1,keepdims=True)+eps
    comb /= comb.sum(axis=0,keepdims=True)+eps
    for _ in range(19):
        comb /= comb.sum(axis=-1,keepdims=True)+eps
        comb /= comb.sum(axis=0,keepdims=True)+eps
    previous = np.array([.1,.2,.3,.4], dtype=np.float32)
    collapsed = (previous[:,None]*x).sum(axis=0)
    sublayer = np.array([.75,-1.5,2], dtype=np.float32)
    expanded = post[:,None]*sublayer + (comb[:,:,None]*x[:,None,:]).sum(axis=0)
    write('hyperconnections', dict(streams=h,width=d,residual=x.flatten().tolist(),projection=fn.flatten().tolist(),
          scale=scale.tolist(),base=base.tolist(),previous_pre=previous.tolist(),sublayer=sublayer.tolist(),
          pre=pre.tolist(),post=post.tolist(),comb=comb.flatten().tolist(),collapsed=collapsed.tolist(),expanded=expanded.flatten().tolist()))


def moe():
    # model.py:794-906, no activation quantization or BF16 conversion.
    x = np.array([1.25,-.75,2.0], dtype=np.float32)
    gate = np.array([np.cos(np.arange(3)+j)*.8 for j in range(8)], dtype=np.float32)
    bias = np.array([0,0,2,-2,0,-1,.5,-.5], dtype=np.float32)
    score = np.sqrt(np.log1p(np.exp(gate @ x)))
    ids = np.argsort(-(score+bias),kind='stable')[:3]
    weights = score[ids].copy();weights /= weights.sum()+np.float32(1e-20);weights *= np.float32(1.5)
    w1 = [np.sin(np.arange(12)+j).reshape(4,3).astype(np.float32)*7 for j in range(9)]
    w3 = [np.cos(np.arange(12)+j).reshape(4,3).astype(np.float32)*8 for j in range(9)]
    w2 = [np.sin(np.arange(12)+j*.5).reshape(3,4).astype(np.float32)*.2 for j in range(9)]
    def expert(j,w):
        g=np.minimum(w1[j]@x,10);u=np.clip(w3[j]@x,-10,10)
        return w2[j] @ ((g/(1+np.exp(-g))*u)*w)
    y = np.zeros(3,dtype=np.float32)
    for j,w in sorted(zip(ids,weights)):
        y += expert(j,w)
    y += expert(8,np.float32(1))
    write('moe',dict(input=x.tolist(),gate=gate.flatten().tolist(),bias=bias.tolist(),ids=ids.tolist(),weights=weights.tolist(),output=y.tolist(),
          w1=[w.flatten().tolist() for w in w1],w2=[w.flatten().tolist() for w in w2],w3=[w.flatten().tolist() for w in w3]))


def attention():
    # model.py:429-486, 558-563 and kernel.py:311-389, float32 boundaries.
    q=np.array([[.5,-1,2],[-2,.75,.25]],dtype=np.float32)
    kv=np.array([[1,-2,.5],[.25,1,2],[-1,.5,3],[4,-1,-2]],dtype=np.float32)
    sinks=np.array([.75,-.25],dtype=np.float32)
    ids=np.array([0,-1,2,3]);valid=ids[ids>=0]
    logits=q@kv[valid].T/np.sqrt(np.float32(3))
    all_scores=np.concatenate([logits,sinks[:,None]],axis=-1)
    weights=np.exp(all_scores-all_scores.max(axis=-1,keepdims=True));weights/=weights.sum(axis=-1,keepdims=True)
    y=weights[:,:-1]@kv[valid]
    iw=np.array([.75,-.2],dtype=np.float32)
    score=(np.maximum(q@kv.T,0)*(iw/np.sqrt(np.float32(3))/np.sqrt(np.float32(2)))[:,None]).sum(axis=0)
    values=np.array([[1,2,-3],[4,-1,2]],dtype=np.float32)
    gates=np.array([[0,2,-1],[1,-2,3]],dtype=np.float32)
    norm=np.array([1,.5,1.5],dtype=np.float32)
    cw=np.exp(gates-gates.max(axis=0,keepdims=True));cw/=cw.sum(axis=0,keepdims=True)
    compressed=(values*cw).sum(axis=0);compressed=norm*(compressed/np.sqrt(np.mean(compressed**2)+np.float32(1e-20)))
    write('attention',dict(query=q.flatten().tolist(),kv=kv.flatten().tolist(),sinks=sinks.tolist(),indices=ids.tolist(),attention=y.flatten().tolist(),index_weights=iw.tolist(),scores=score.tolist(),values=values.flatten().tolist(),gates=gates.flatten().tolist(),norm=norm.tolist(),compressed=compressed.tolist()))


def engram():
    # engram.py:68-184, using the same NumPy RNG and exact integer products.
    import math
    seen=set();primes=[]
    for layer in [1,14]:
        row=[]
        for gram in range(3):
            n=15999999
            for head in range(8):
                n+=1
                while n in seen or n%2==0 or any(n%d==0 for d in range(3,math.isqrt(n)+1,2)):
                    n+=1
                seen.add(n);row.append(n)
        primes.append(row)
    bound=(np.iinfo(np.int64).max//99092)//2
    multipliers=[(np.random.default_rng(10007*layer).integers(0,bound,size=(4,),dtype=np.int64)*2+1).tolist() for layer in [1,14]]
    compressed=[4,5,None,6,7,8,99091]
    hashes=[]
    for pos in range(len(compressed)):
        tokens=[];blocked=False
        for shift in range(4):
            source=compressed[pos-shift] if pos>=shift else None
            blocked=blocked or source is None
            tokens.append(2 if blocked else source)
        layers=[]
        for ps,ms in zip(primes,multipliers):
            rolling=tokens[0]*ms[0];row=[];offset=0
            for gram in range(1,4):
                rolling^=tokens[gram]*ms[gram]
                for head in range(8):
                    p=ps[(gram-1)*8+head];row.append(rolling%p+offset);offset+=p
            layers.append(row)
        hashes.append(layers)
    x=np.array([[1,-2,3],[10,-.1,.25]],dtype=np.float32)
    keys=np.array([[.25,2,-1],[-.1,3,7]],dtype=np.float32)
    value=np.array([.25,-1.5,2],dtype=np.float32)
    qw=np.array([[1,.5,2],[.2,1,1.5]],dtype=np.float32)
    kw=np.array([[.5,1,1],[2,.5,1]],dtype=np.float32)
    rstd=(1/np.sqrt(np.mean(x*x,axis=-1)+np.float32(1e-20)))*(1/np.sqrt(np.mean(keys*keys,axis=-1)+np.float32(1e-20)))
    dot=(x*(qw*kw)*keys).sum(axis=-1)*rstd*np.float32(3**-.5)
    gate=1/(1+np.exp(-np.copysign(np.sqrt(np.maximum(np.abs(dot),np.float32(1e-6))),dot)))
    output=x+gate[:,None]*value
    write('engram',dict(primes=primes,multipliers=multipliers,compressed=compressed,hashes=hashes,residual=x.flatten().tolist(),keys=keys.flatten().tolist(),value=value.tolist(),q_weight=qw.flatten().tolist(),k_weight=kw.flatten().tolist(),output=output.flatten().tolist()))


if __name__ == '__main__':
    OUT.mkdir(parents=True, exist_ok=True)
    hyperconnections()
    moe()
    attention()
    engram()
