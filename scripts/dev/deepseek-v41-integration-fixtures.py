#!/usr/bin/env python3
"""Generate independent synthetic fixtures for the V4.1 CPU integration.

Uses the public MIT reference at 1bae0e1088993b9b82de08f59de5be5a34e42170.
These small NumPy equations do not run the official device kernels.
"""
import json
from pathlib import Path
import numpy as np

OUT = Path(__file__).resolve().parents[2] / 'crates/psionic/crates/psionic-models/src/testdata/deepseek_v41_cpu'


def write(name, data):
    (OUT / (name + '.json')).write_text(json.dumps(dict(revision='1bae0e1088993b9b82de08f59de5be5a34e42170', kind='synthetic_numpy', **data), indent=2, allow_nan=False) + '\n')


def projection():
    tensors = {}
    def matrix(name, rows, cols):
        w = (np.sin(np.arange(rows*cols)*.7 + len(tensors))*.2).astype(np.float32).reshape(rows,cols)
        tensors[name] = dict(shape=[rows,cols],values=w.flatten().tolist())
        return w
    def vector(name, count):
        w = np.linspace(.75,1.25,count,dtype=np.float32)
        tensors[name] = dict(shape=[count],values=w.tolist())
        return w
    def norm(x,w):return w*(x/np.sqrt(np.mean(x*x)+np.float32(1e-20)))
    def m(part,r,c):return matrix('layers.2.attn.'+part+'.weight',r,c)
    def n(part,c):return vector('layers.2.attn.'+part+'.weight',c)
    x=np.array([.25,-.5,1,2],dtype=np.float32)
    qr=norm(m('wq_a',3,4)@x,n('q_norm',3));q=m('wq_b',16,3)@qr
    kv=norm(m('wkv',4,4)@x,n('kv_norm',4))
    cv=m('compressor.wkv',4,4)@x;cg=m('compressor.wgate',4,4)@x;n('compressor.norm',4)
    ik=norm(m('indexer.wk',4,4)@cv,n('indexer.k_norm',4));iq=m('indexer.wq_b',8,3)@qr;iw=m('indexer.weights_proj',2,4)@x
    oa=m('wo_a',4,8).reshape(2,2,8)
    low=np.einsum('grd,gd->gr',oa,q.reshape(2,8)).flatten();out=m('wo_b',4,4)@low
    embed=matrix('embed.weight',5,4)[2];nw=vector('norm.weight',4);hw=matrix('head.weight',5,4)
    residual=np.array([[1,2,-1,.5],[-2,1,3,-.25]],dtype=np.float32)
    logits=hw@norm((residual*np.array([.25,.75],dtype=np.float32)[:,None]).sum(axis=0),nw)
    expected={k:v.flatten().tolist() for k,v in dict(qr=qr,q=q,kv=kv,compress_value=cv,compress_gate=cg,index_key=ik,index_query=iq,index_weights=iw,output=out,embedding=embed,residual=residual,logits=logits).items()}
    vector('layers.2.attn.attn_sink',4)
    write('projection',dict(tensors=tensors,input=x.tolist(),expected=expected))


def rope():
    import math
    dim=64
    x=np.sin(np.arange(136)*.4).astype(np.float32)
    positions=[0,127,128,129,65535,65536,1048575]
    cases=[]
    for compressed in [False,True]:
        base=160000 if compressed else 10000
        powers=np.array([math.pow(base,float(np.float32(2*i)/np.float32(dim))) for i in range(dim//2)],dtype=np.float32)
        freqs=np.float32(1)/powers
        if compressed:
            correction=lambda b:dim*math.log(65536/(b*2*math.pi))/(2*math.log(base))
            low=max(math.floor(correction(32)),0);high=min(math.ceil(correction(1)),dim-1)
            ramp=np.clip((np.arange(dim//2,dtype=np.float32)-low)/max(high-low,1e-3),0,1)
            smooth=1-ramp;freqs=freqs/16*(1-smooth)+freqs*smooth
        outputs=[]
        for pos in positions:
            angle=np.float32(pos)*freqs
            z=x.reshape(2,68).copy();tail=z[:,4:].reshape(2,32,2)
            a=tail[:,:,0].copy();b=tail[:,:,1].copy()
            tail[:,:,0]=a*np.cos(angle)-b*np.sin(angle);tail[:,:,1]=a*np.sin(angle)+b*np.cos(angle)
            outputs.append(z.flatten().tolist())
        cases.append(dict(compressed=compressed,frequencies=freqs.tolist(),positions=positions,outputs=outputs))
    write('rope',dict(input=x.tolist(),cases=cases))


def quantization():
    import math, struct
    def fp8(code):
        exponent=(code>>3)&15;mantissa=code&7
        return np.float32(mantissa*2**-9 if exponent==0 else (1+mantissa/8)*2**(exponent-7))
    f8=np.array([fp8(i) for i in range(127)],dtype=np.float32)
    f4=np.array([0,.5,1,1.5,2,3,4,6],dtype=np.float32)
    def encode(x,table,sign):
        distances=np.abs(table-np.abs(np.float32(x)))
        minimum=distances.min();ids=np.flatnonzero(distances==minimum)
        code=next((int(i) for i in ids if i%2==0),int(ids[0]))
        return code | (sign if np.signbit(x) else 0)
    cases=[]
    for kind,maximum,block in [('fp8',448,32),('index_fp4',6,32),('compressed_fp4',6,16)]:
        first=[maximum,-0.,.25,.75,1.0625,1.1875,1.25,1.75,2.5,3.5,5,-maximum]
        x=np.array((first+[0.]*(block-len(first)))*2,dtype=np.float32);x[block:]*=2
        codes=[];scales=[];decoded=[]
        for start in range(0,len(x),block):
            values=x[start:start+block];amax=max(np.max(np.abs(values)),np.float32(1e-4 if kind=='fp8' else 6*2**(-9 if kind=='compressed_fp4' else -126)))
            if kind=='compressed_fp4':
                byte=encode(np.float32(amax/6),f8,128);scale=f8[byte]
            else:
                v=np.float32(amax*np.float32(1/maximum));bits=struct.unpack('<I',struct.pack('<f',v))[0]
                byte=(bits>>23)+int(bool(bits&0x7fffff));scale=np.float32(2.**(byte-127))
            scales.append(byte)
            for v in values:
                code=encode(np.float32(v/scale),f8 if kind=='fp8' else f4,128 if kind=='fp8' else 8);codes.append(code)
                sign=128 if kind=='fp8' else 8;table=f8 if kind=='fp8' else f4
                decoded.append(float(np.copysign(table[code&(sign-1)],-1 if code&sign else 1)*scale))
        if kind!='fp8':codes=[a|(b<<4) for a,b in zip(codes[::2],codes[1::2])]
        cases.append(dict(kind=kind,input=x.tolist(),codes=codes,scales=scales,decoded=decoded))
    def bf(x):
        x=np.asarray(x,dtype=np.float32);u=x.view(np.uint32);return ((u+np.uint32(0x7fff)+((u>>16)&1))&np.uint32(0xffff0000)).view(np.float32)
    q=bf([.71,-.42]);kv=bf(np.stack([np.sin(np.arange(70)*.4),np.cos(np.arange(70)*.3)],axis=-1));ids=list(range(70));sinks=[.2]
    maximum=np.float32(-1e30);denom=np.float32(0);out=np.zeros(2,dtype=np.float32)
    for start in range(0,70,64):
        scores=(kv[start:start+64]@q)*np.float32(.5);old=maximum;maximum=max(maximum,scores.max());rescale=np.exp(old-maximum)
        weights=np.exp(scores-maximum);denom=denom*rescale+weights.sum();out=out*rescale+bf(weights)@kv[start:start+64]
    denom+=np.exp(np.float32(sinks[0])-maximum);out=bf(out/denom)
    write('quantization',dict(cases=cases,query=q.tolist(),kv=kv.flatten().tolist(),indices=ids,sinks=sinks,attention=out.tolist()))


def shared_state():
    # Independent layerwise NumPy attention with the actual 40-layer schedule,
    # tiny geometry and synthetic projection weights. Not a full transformer.
    f=json.loads((OUT/'projection.json').read_text())
    tensors=f['tensors']
    def w(part):
        t=tensors['layers.2.attn.'+part];return np.array(t['values'],dtype=np.float32).reshape(t['shape'])
    def norm(x,weight):return weight*(x/np.sqrt(np.mean(x*x)+np.float32(1e-20)))
    def rotate(x,pos,width):
        x=x.reshape(-1,width).copy();a=x[:,-2].copy();b=x[:,-1].copy()
        c=np.cos(np.float32(pos));s=np.sin(np.float32(pos));x[:,-2]=a*c-b*s;x[:,-1]=a*s+b*c
        return x.flatten()
    caches={i:dict(kv=[],keys=[],values=[],gates=[]) for i in [2,8,14,20]}
    selections={};outputs={}
    for layer in range(40):
        ratio=0 if layer<2 else 2 if layer<20 else 1
        owner=max((i for i in caches if i<=layer),default=None)
        index=max((i for i in [2,8,14,20,24,28,32,36] if i<=layer),default=None)
        ring=[];ys=[];selected_rows=[]
        for pos in range(7):
            x=np.array([.25,-.5,1,2],dtype=np.float32)+np.float32(layer)*np.float32(.03)+np.sin(np.float32(pos)*np.float32(.4))*np.float32(.2)
            qr=norm(w('wq_a.weight')@x,w('q_norm.weight'))
            q=rotate(w('wq_b.weight')@qr,pos,4).reshape(4,4)
            ring.append(rotate(norm(w('wkv.weight')@x,w('kv_norm.weight')),pos,4));ring=ring[-4:]
            if layer==owner:
                cache=caches[owner];cache['values'].append(w('compressor.wkv.weight')@x)
                if ratio>1:cache['gates'].append(w('compressor.wgate.weight')@x)
                if (pos+1)%ratio==0:
                    if ratio==1:latent=cache['values'][0]
                    else:
                        v=np.array(cache['values']);gate=np.array(cache['gates']);gate=np.exp(gate-gate.max(axis=0));gate/=gate.sum(axis=0)
                        latent=(v*gate).sum(axis=0)
                    latent=norm(latent,w('compressor.norm.weight'));gp=pos+1-ratio
                    key=norm(w('indexer.wk.weight')@latent,w('indexer.k_norm.weight'))
                    cache['keys'].append(rotate(key,gp,4));cache['kv'].append(rotate(latent,gp,4));cache['values']=[];cache['gates']=[]
            kv=list(ring)
            if owner is not None:
                visible=(pos+1)//ratio
                if layer==index:
                    iq=rotate(w('indexer.wq_b.weight')@qr,pos,4).reshape(2,4)
                    keys=np.array(caches[owner]['keys'][:visible],dtype=np.float32).reshape(-1,4)
                    iw=(w('indexer.weights_proj.weight')@x)*(np.float32(4**-.5)*np.float32(2**-.5))
                    scores=(np.maximum(iq@keys.T,0)*iw[:,None]).sum(axis=0)
                    ids=np.sort(np.argsort(-scores,kind='stable')[:min(3,visible)]).tolist();selected_rows.append(ids)
                else:ids=selections[index][pos]
                kv.extend(caches[owner]['kv'][i] for i in ids)
            kv=np.array(kv,dtype=np.float32)
            scores=(q@kv.T)*np.float32(.5)
            logits=np.concatenate([scores,w('attn_sink')[:,None]],axis=-1)
            probs=np.exp(logits-logits.max(axis=-1,keepdims=True));probs/=probs.sum(axis=-1,keepdims=True)
            heads=rotate((probs[:,:-1]@kv).flatten(),-pos,4)
            oa=w('wo_a.weight').reshape(2,2,8)
            low=np.einsum('grd,gd->gr',oa,heads.reshape(2,8)).flatten();ys.extend((w('wo_b.weight')@low).tolist())
        if layer==index:selections[layer]=selected_rows
        outputs[layer]=ys
    write('shared_state',dict(outputs=outputs))


if __name__ == '__main__':
    projection()
    rope()
    quantization()
    shared_state()
