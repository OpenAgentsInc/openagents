#!/usr/bin/env python3
"""Firm reason2: independent Torch equations, no model checkpoint or runtime role."""
import argparse
import hashlib
import json
from pathlib import Path
import numpy as np
import torch

# Frozen before observing CPU/GPU candidate outputs. Never widen on failure.
ATOL=2e-5
RTOL=2e-5

def main():
    p=argparse.ArgumentParser();p.add_argument("directory",type=Path);args=p.parse_args()
    assert torch.__version__.split("+")[0]=="2.9.0"
    torch.set_num_threads(2)
    results=[];words=0
    for line in (args.directory/"cases.jsonl").read_text().splitlines():
        case=json.loads(line);prefix=args.directory/f"{case['index']:03d}"
        def read(tag,dtype='<f4'):return np.frombuffer(Path(str(prefix)+f'-{tag}.bin').read_bytes(),dtype=dtype).copy()
        def tensor(tag):return torch.from_numpy(read(tag))
        op=case['op']
        if op in ('embed','inject'):
            t,w,v=case['tokens'],case['width'],case['vocab']
            image=torch.from_numpy(read('map','<i8'));visual=tensor('visual').reshape(128,w);selected=image>=0
            if op=='embed':
                embedding=torch.from_numpy(read('embedding','<u2')).view(torch.bfloat16).float().reshape(v,w)
                expected=embedding[torch.from_numpy(read('ids','<i8'))].clone()
                expected[selected]=visual[image[selected]]
            else:
                expected=tensor('x').reshape(t,w).clone()
                expected[selected]=expected[selected]+visual[image[selected]]
        elif op=='rotate':
            t,h,d=case['tokens'],case['heads'],case['dim'];x=tensor('x').reshape(t,h,d);rope=tensor('rope').reshape(t,d//2,2)
            a,b=x.chunk(2,dim=-1);cosine=rope[:,:,0][:,None,:];sine=rope[:,:,1][:,None,:]
            expected=torch.cat((a*cosine+(-b)*sine,b*cosine+a*sine),dim=-1)
        else:
            a,b=tensor('x'),tensor('b');expected=a+b if op=='add' else a*b
        expected=expected.reshape(-1).numpy();assert np.isfinite(expected).all()
        for backend in ['debug','release','sanitized','metal']:
            actual=read(backend);assert actual.shape==expected.shape and np.isfinite(actual).all()
            error=np.abs(actual.astype(np.float64)-expected.astype(np.float64));budget=ATOL+RTOL*np.abs(expected.astype(np.float64));passed=bool(np.all(error<=budget))
            results.append({'case':case['index'],'op':op,'backend':backend,'words':len(expected),'max_abs_error':float(error.max(initial=0)),'max_budget_fraction':float((error/budget).max(initial=0)),'passed':passed});words+=len(expected)
    root=Path(__file__).resolve().parents[3]
    sources=['psionic/lib/robotics/groot-text-kernels.coax','psionic/tests/robotics/groot-text-kernels-native.c','psionic/tests/robotics/check-groot-text-kernels.sh','psionic/tests/robotics/groot_text_kernels_reference.py']
    record={'schema':'psionic-groot-text-kernels-reference-v1','date':'2026-10-31','torch':torch.__version__,'numpy':np.__version__,'passed':all(x['passed']for x in results),'tolerance_frozen_before_observation':{'atol':ATOL,'rtol':RTOL},'words':words,'results':results,'source_sha256':{s:hashlib.sha256((root/s).read_bytes()).hexdigest()for s in sources},'scope':'Owned embedding/half-rotation/deepstack/add/multiply primitives on realMetal and nativeCPU; original checkpoint/fullVLA execution is separate.'}
    (args.directory/'reference.json').write_text(json.dumps(record,indent=2)+'\n')
    assert record['passed'],'Frozen tolerance failed; inspect without widening'
    print(f"GR00T text Torch{torch.__version__}: {len(results)}comparisons,{words}values; maxbudgetfraction{max(x['max_budget_fraction']for x in results):.6g}; realMetal+CPU passed")

if __name__=='__main__':main()
