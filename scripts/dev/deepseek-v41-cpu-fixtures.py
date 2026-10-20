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


if __name__ == '__main__':
    OUT.mkdir(parents=True, exist_ok=True)
    hyperconnections()
