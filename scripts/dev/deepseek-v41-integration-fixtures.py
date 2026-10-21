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
    write('projection',dict(tensors=tensors,input=x.tolist(),expected=expected))


if __name__ == '__main__':
    projection()
