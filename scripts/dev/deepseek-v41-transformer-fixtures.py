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


def transformer_forward(tokens):
    b=json.loads((OUT/'block.json').read_text())['tensors']
    a=json.loads((OUT/'projection.json').read_text())['tensors']
    def get(t,prefix,name):
        v=t[prefix+name];return np.array(v['values'],dtype=np.float32).reshape(v['shape'])
    bw=lambda n:get(b,'layers.1.',n)
    aw=lambda n:get(a,'layers.2.attn.',n)
    embed=(np.sin(np.arange(32)*.31)*.2).astype(np.float32).reshape(8,4)
    head=(np.cos(np.arange(32)*.27)*.3).astype(np.float32).reshape(8,4)
    nw=np.array([.75,1,1.25,.9],dtype=np.float32)
    globals={n:dict(shape=list(v.shape),values=v.flatten().tolist()) for n,v in {'embed.weight':embed,'head.weight':head,'norm.weight':nw}.items()}
    def rotate(x,pos,width):
        x=x.reshape(-1,width).copy();aa=x[:,-2].copy();bb=x[:,-1].copy();c=np.cos(np.float32(pos));s=np.sin(np.float32(pos));x[:,-2]=aa*c-bb*s;x[:,-1]=aa*s+bb*c;return x.flatten()
    n=len(tokens);h=np.repeat(embed[tokens,None,:],4,axis=1);pre=np.zeros((n,4),dtype=np.float32);pre[:,0]=1
    multipliers=json.loads((OUT/'engram.json').read_text())['multipliers']
    hashes=[]
    for pos in range(n):
        look=[tokens[pos-i] if pos>=i else 2 for i in range(4)];layers=[]
        for ms in multipliers:
            rolling=look[0]*ms[0];row=[]
            for gram in range(1,4):
                rolling^=look[gram]*ms[gram]
                for hd in range(8):row.append(rolling%2+((gram-1)*8+hd)*2)
            layers.append(row)
        hashes.append(layers)
    caches={i:dict(kv=[],keys=[],values=[],gates=[]) for i in [2,8,14,20]};selections={};traces={}
    for layer in range(40):
        if layer in [1,14]:
            slot=0 if layer==1 else 1
            for p in range(n):
                kv=bw('engram.wkv.weight')@bw('engram.embed.weight')[hashes[p][slot]].flatten()
                key=kv[:16].reshape(4,4);value=kv[16:];rstd=1/np.sqrt(np.mean(h[p]*h[p],axis=-1)+np.float32(1e-20)) / np.sqrt(np.mean(key*key,axis=-1)+np.float32(1e-20))
                dot=(h[p]*(bw('engram.q_weight')*bw('engram.k_weight'))*key).sum(axis=-1)*rstd*np.float32(.5)
                gate=1/(1+np.exp(-np.copysign(np.sqrt(np.maximum(np.abs(dot),np.float32(1e-6))),dot)));h[p]+=gate[:,None]*value
        am=[mixes(h[p],'attn',b) for p in range(n)]
        inputs=np.array([norm((h[p]*pre[p,:,None]).sum(axis=0),bw('attn_norm.weight')) for p in range(n)])
        ratio=0 if layer<2 else 2 if layer<20 else 1;owner=max((i for i in caches if i<=layer),default=None);index=max((i for i in [2,8,14,20,24,28,32,36] if i<=layer),default=None)
        ring=[];selected_rows=[];attention=[]
        for pos,x in enumerate(inputs):
            qr=norm(aw('wq_a.weight')@x,aw('q_norm.weight'));q=rotate(aw('wq_b.weight')@qr,pos,4).reshape(4,4)
            ring.append(rotate(norm(aw('wkv.weight')@x,aw('kv_norm.weight')),pos,4));ring=ring[-4:]
            if layer==owner:
                cache=caches[owner];cache['values'].append(aw('compressor.wkv.weight')@x)
                if ratio>1:cache['gates'].append(aw('compressor.wgate.weight')@x)
                if (pos+1)%ratio==0:
                    if ratio==1:latent=cache['values'][0]
                    else:
                        gate=np.array(cache['gates']);gate=np.exp(gate-gate.max(axis=0));gate/=gate.sum(axis=0);latent=(np.array(cache['values'])*gate).sum(axis=0)
                    latent=norm(latent,aw('compressor.norm.weight'));gp=pos+1-ratio
                    cache['keys'].append(rotate(norm(aw('indexer.wk.weight')@latent,aw('indexer.k_norm.weight')),gp,4));cache['kv'].append(rotate(latent,gp,4));cache['values']=[];cache['gates']=[]
            kv=list(ring)
            if owner is not None:
                visible=(pos+1)//ratio
                if layer==index:
                    iq=rotate(aw('indexer.wq_b.weight')@qr,pos,4).reshape(2,4);keys=np.array(caches[owner]['keys'][:visible],dtype=np.float32).reshape(-1,4)
                    iw=(aw('indexer.weights_proj.weight')@x)*(np.float32(.5)*np.float32(2**-.5));scores=(np.maximum(iq@keys.T,0)*iw[:,None]).sum(axis=0)
                    ids=np.sort(np.argsort(-scores,kind='stable')[:min(3,visible)]).tolist();selected_rows.append(ids)
                else:ids=selections[index][pos]
                kv.extend(caches[owner]['kv'][i] for i in ids)
            kv=np.array(kv,dtype=np.float32);scores=(q@kv.T)*np.float32(.5);logits=np.concatenate([scores,aw('attn_sink')[:,None]],axis=-1);probs=np.exp(logits-logits.max(axis=-1,keepdims=True));probs/=probs.sum(axis=-1,keepdims=True)
            heads=rotate((probs[:,:-1]@kv).flatten(),-pos,4);low=np.einsum('grd,gd->gr',aw('wo_a.weight').reshape(2,2,8),heads.reshape(2,8)).flatten();attention.append(aw('wo_b.weight')@low)
        if layer==index:selections[layer]=selected_rows
        all_ids=[];all_weights=[]
        for p in range(n):
            ap,post,comb=am[p];h[p]=post[:,None]*attention[p]+comb.T@h[p];fp,post2,comb2=mixes(h[p],'ffn',b)
            fi=norm((h[p]*ap[:,None]).sum(axis=0),bw('ffn_norm.weight'));scores=np.sqrt(np.log1p(np.exp(bw('ffn.gate.weight')@fi)));ids=np.argsort(-(scores+bw('ffn.gate.bias')),kind='stable')[:2];weights=scores[ids].copy();weights/=weights.sum()+np.float32(1e-20);weights*=np.float32(1.5)
            def expert(stem,weight):
                gate=np.minimum(bw(stem+'.w1.weight')@fi,10);up=np.clip(bw(stem+'.w3.weight')@fi,-10,10)
                return bw(stem+'.w2.weight')@((gate/(1+np.exp(-gate))*up)*weight)
            y=np.zeros(4,dtype=np.float32)
            for eid,weight in sorted(zip(ids,weights)):y+=expert(f'ffn.experts.{eid}',weight)
            y+=expert('ffn.shared_experts',np.float32(1));h[p]=post2[:,None]*y+comb2.T@h[p];pre[p]=fp;all_ids.append(ids.tolist());all_weights.append(weights.tolist())
        if layer in [0,1,2,14,20,39]:traces[layer]=dict(residuals=h.reshape(n,16).tolist(),pre_mixes=pre.tolist(),expert_ids=all_ids,route_weights=all_weights)
    logits=head@norm((h[-1]*pre[-1,:,None]).sum(axis=0),nw)
    return dict(global_tensors=globals,tokens=tokens,logits=logits.tolist(),layers=traces)


def transformer():
    write('transformer',transformer_forward([0,3,4,5]))


def greedy():
    prompt=[0,3,4,5];tokens=[];logits=[]
    for _ in range(3):
        row=transformer_forward(prompt+tokens)['logits'];logits.append(row)
        tokens.append(int(np.argmax(row)))
    assert 7 not in tokens
    write('greedy',dict(prompt=prompt,tokens=tokens,logits=logits))


if __name__=='__main__':
    block()
    transformer()
    greedy()
