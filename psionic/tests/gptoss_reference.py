#!/usr/bin/env python3
"""Owned independent dense full-prefix NumPy oracle. Never imports upstream code.
Use with a NumPy-equipped Python; not a runtime dependency. Original bytes stay
outside the repository. Tolerances are fixed before reading candidate results.
"""
import argparse,contextlib,hashlib,io,json,math,sys,time
from pathlib import Path
import numpy as np
ROOT=Path(__file__).resolve().parents[2]
TOLERANCE=5e-4
LUT=np.array([0,.5,1,1.5,2,3,4,6,-0.,-.5,-1,-1.5,-2,-3,-4,-6],dtype=np.float64)
class Original:
    def __init__(self,root):
        self.t={};self.maps=[]
        for path in sorted((root/'original').glob('*.safetensors')):
            raw=np.memmap(path,dtype=np.uint8,mode='r');self.maps.append(raw)
            n=int.from_bytes(raw[:8].tobytes(),'little');header=json.loads(raw[8:8+n].tobytes())
            for name,v in header.items():
                if name=='__metadata__':continue
                a,b=v['data_offsets'];data=raw[8+n+a:8+n+b]
                if v['dtype']=='BF16':data=(data.view('<u2').astype(np.uint32)<<16).view(np.float32)
                else:assert v['dtype']=='U8'
                self.t[name]=data.reshape(v['shape'])
        self.c=json.loads((root/'original/config.json').read_text())
    def bf(self,name):return self.t[name]
    def mx(self,name,e):
        packed=self.t[name+'.blocks'][e];scale=self.t[name+'.scales'][e]
        codes=np.empty((*packed.shape[:-1],32),dtype=np.uint8)
        codes[...,::2]=packed&15;codes[...,1::2]=packed>>4
        assert np.all(scale<=252)
        return np.ldexp(LUT[codes],scale.astype(np.int16)[...,None]-127).reshape(packed.shape[0],-1)
def project(w,x,b=None):
    # Independent BLAS reductions in float64; storage/bias staging matches profile.
    out=np.empty((len(x),len(w)),np.float32)
    for begin in range(0,len(w),512):
        end=min(begin+512,len(w));out[:,begin:end]=(x.astype(np.float64)@w[begin:end].astype(np.float64).T).astype(np.float32)
    if b is not None:out=(out.astype(np.float64)+b.astype(np.float64)).astype(np.float32)
    return out
def norm(x,w):
    z=x.astype(np.float64);return (z/np.sqrt(np.sum(z*z,axis=-1,keepdims=True)/x.shape[-1]+1e-5)*w).astype(np.float32)
def rotate(x):
    # Direct powers and whole sequence; no candidate rotary table or ring cache.
    out=x.copy();lo=32*math.log(4096/(32*2*math.pi))/math.log(150000);hi=32*math.log(4096/(2*math.pi))/math.log(150000)
    for t in range(len(x)):
        for i in range(32):
            ramp=min(1,max(0,(i-lo)/(hi-lo)));a=t/150000**(i/32)*(1-ramp+ramp/32)
            co=np.float32(math.cos(a)*(1+.1*math.log(32)));si=np.float32(math.sin(a)*(1+.1*math.log(32)))
            lhs=x[t,:,i].astype(np.float64);rhs=x[t,:,i+32].astype(np.float64)
            out[t,:,i]=(lhs*float(co)-rhs*float(si)).astype(np.float32);out[t,:,i+32]=(rhs*float(co)+lhs*float(si)).astype(np.float32)
    return out
def run(m,tokens):
    c=m.c;L=c['num_hidden_layers'];D=c['hidden_size'];H=c['num_attention_heads'];K=c['num_key_value_heads'];M=c['intermediate_size'];E=c['num_experts'];top=c['experts_per_token'];Q=H*64;kn=K*64
    x=m.bf('embedding.weight')[tokens].copy();traces=[[] for _ in tokens];routes=[[] for _ in tokens];mixes=[[] for _ in tokens]
    for l in range(L):
        p=f'block.{l}.';z=project(m.bf(p+'attn.qkv.weight'),norm(x,m.bf(p+'attn.norm.scale')),m.bf(p+'attn.qkv.bias'))
        qs=rotate(z[:,:Q].reshape(-1,H,64));ks=rotate(z[:,Q:Q+kn].reshape(-1,K,64));vs=z[:,Q+kn:].reshape(-1,K,64);ctx=np.empty((len(x),H,64),np.float32)
        for t in range(len(x)):
            first=max(0,t-127) if l%2==0 else 0
            for h in range(H):
                kh=h//(H//K);scores=np.sum(ks[first:t+1,kh].astype(np.float64)*qs[t,h].astype(np.float64),axis=1)/8;sink=float(m.bf(p+'attn.sinks')[h]);maximum=max(float(max(scores)),sink)
                e=np.exp(scores-maximum);den=math.fsum(e)+math.exp(sink-maximum);ctx[t,h]=(np.sum(vs[first:t+1,kh].astype(np.float64)*e[:,None],axis=0)/den).astype(np.float32)
        residual=(x.astype(np.float64)+project(m.bf(p+'attn.out.weight'),ctx.reshape(-1,Q),m.bf(p+'attn.out.bias')).astype(np.float64)).astype(np.float32)
        normalized=norm(residual,m.bf(p+'mlp.norm.scale'));gate=project(m.bf(p+'mlp.gate.weight'),normalized,m.bf(p+'mlp.gate.bias'));x=np.empty_like(residual)
        for t in range(len(x)):
            ids=sorted(range(E),key=lambda i:(-float(gate[t,i]),i))[:top];exp=[math.exp(float(gate[t,i])-float(gate[t,ids[0]])) for i in ids];weights=np.array([v/math.fsum(exp) for v in exp],np.float32);ys=[]
            for e in ids:
                up=project(m.mx(p+'mlp.mlp1_weight',e),normalized[t:t+1],m.bf(p+'mlp.mlp1_bias')[e])[0].astype(np.float64);g=np.minimum(up[::2],7);lin=np.clip(up[1::2],-7,7);z=1.702*g
                sig=np.empty_like(z);mask=z>=0;sig[mask]=1/(1+np.exp(-z[mask]));ez=np.exp(z[~mask]);sig[~mask]=ez/(1+ez)
                act=(g*sig*(lin+1)).astype(np.float32)
                ys.append(project(m.mx(p+'mlp.mlp2_weight',e),act[None,:],m.bf(p+'mlp.mlp2_bias')[e])[0])
            combined=np.sum(np.array(ys,np.float64)*weights.astype(np.float64)[:,None],axis=0)
            x[t]=(residual[t].astype(np.float64)+combined).astype(np.float32);traces[t].extend(residual[t].tolist()+x[t].tolist());routes[t].extend(ids);mixes[t].extend(weights.tolist())
        print('reference layer',l+1,'/',L,flush=True)
    logits=project(m.bf('unembedding.weight'),norm(x,m.bf('norm.scale')))
    return [dict(logits=logits[i],trace=np.array(traces[i],np.float32),routes=np.array(routes[i],dtype='<i8'),weights=np.array(mixes[i],np.float32)) for i in range(len(tokens))]
def main():
    ap=argparse.ArgumentParser();ap.add_argument('cache',type=Path);ap.add_argument('candidate',type=Path);ap.add_argument('output',type=Path);ap.add_argument('--tokens',default='200006,173781');args=ap.parse_args();tokens=list(map(int,args.tokens.split(',')))
    start=time.monotonic();ref=run(Original(args.cache),tokens);summary={'numpy_version':np.__version__,'python_version':sys.version,'reference_source_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),'profile':'gpt-oss-cpu-wide-v1','reference':'owned NumPy whole-prefix layer traversal; float64 independent BLAS reductions; no upstream code executed','tokens':tokens,'tolerance_relative_plus_absolute':TOLERANCE,'steps':[]}
    for i,r in enumerate(ref):
        step={}
        for field,expected in r.items():
            suffix='i64' if field=='routes' else 'f32';path=args.candidate/f'step{i}.{field}.{suffix}';got=np.fromfile(path,dtype=expected.dtype);assert got.shape==expected.shape
            if field=='routes':assert np.array_equal(got,expected);step[field]={'exact':True}
            else:
                error=np.abs(got.astype(np.float64)-expected.astype(np.float64));scaled=error/(1+np.abs(expected.astype(np.float64)));assert np.all(np.isfinite(got)) and np.max(scaled)<TOLERANCE,(i,field,float(np.max(scaled)))
                step[field]={'maximum_absolute_error':float(np.max(error)),'maximum_scaled_error':float(np.max(scaled))}
            step[field]['candidate_sha256']=hashlib.sha256(path.read_bytes()).hexdigest()
        step['top_token']=int(np.argmax(r['logits']));summary['steps'].append(step)
    summary['reference_seconds']=time.monotonic()-start;args.output.mkdir(parents=True,exist_ok=True)
    np.savez_compressed(args.output/'reference.npz',**{f'{i}_{k}':v for i,r in enumerate(ref) for k,v in r.items()})
    config=io.StringIO()
    with contextlib.redirect_stdout(config):np.show_config()
    (args.output/'reference-build.txt').write_text(config.getvalue())
    (args.output/'qualification.json').write_text(json.dumps(summary,indent=2)+'\n');print(json.dumps(summary,indent=2))
if __name__=='__main__':main()
