#!/usr/bin/env python3
"""Compare saved final rows and logits without executing any model."""
import argparse
import json
import math
from pathlib import Path
import numpy as np
from gguf_import import BASE,require
from compare_layers import metrics,sha


def tensor(path,name):
    with path.open('rb') as f:
        count=int.from_bytes(f.read(8),'little');require(count<1024**2,'tensor header exceeds bound')
        metadata=json.loads(f.read(count))[name]
        first,end=metadata['data_offsets'];require(end-first<32*1024**2,'saved tensor exceeds bound')
        f.seek(8+count+first);data=f.read(end-first)
    if metadata['dtype']=='BF16':array=(np.frombuffer(data,dtype='<u2').astype(np.uint32)<<16).view(np.float32)
    elif metadata['dtype']=='F32':array=np.frombuffer(data,dtype='<f4')
    else:raise ValueError('unexpected dtype')
    return array.reshape(metadata['shape'])


def logits(candidate,reference):
    require(len(candidate)==len(reference),'question count differs')
    rows=[]
    for left,right in zip(candidate,reference,strict=True):
        require(len(left)==len(right) and all(math.isfinite(x) for x in left+right),'logits invalid')
        lp=np.exp(np.array(left,dtype=np.float64)-max(left));lp/=lp.sum()
        rp=np.exp(np.array(right,dtype=np.float64)-max(right));rp/=rp.sum()
        rows.append({'candidate_logits':left,'reference_logits':right,'candidate_probabilities':lp.tolist(),'reference_probabilities':rp.tolist(),
                     'candidate_top':int(lp.argmax()),'reference_top':int(rp.argmax()),'max_abs_dp':float(np.max(np.abs(lp-rp))),
                     'max_abs_dlogit':max(abs(a-b) for a,b in zip(left,right,strict=True))})
    return {'questions':rows,'max_abs_dp':max(r['max_abs_dp'] for r in rows),'all_tops_agree':all(r['candidate_top']==r['reference_top'] for r in rows)}


def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--reference-dir',type=Path,required=True);p.add_argument('--output',type=Path,required=True)
    args=p.parse_args();require(not args.output.exists(),'preserve prior comparison')
    r=json.loads((args.reference_dir/'receipt.json').read_text());require(r['completed'],'reference incomplete')
    native=BASE/'stage5/native27b-head-014/inputs';n=json.loads((native/'dump.json').read_text())
    clean=BASE/'reference27b/cpu-head-stage1/results';c=json.loads((clean/'text-014.json').read_text())
    require(n['input_ids']==r['input_ids']==c['input_ids'],'token identity differs')
    independent=np.fromfile(args.reference_dir/'hidden.f32',dtype='<f4').reshape(321,5120)
    metal=np.fromfile(native/'hidden.f32',dtype='<f4').reshape(321,5120)
    bf16=tensor(clean/c['head_inputs']['file'],'hidden').reshape(321,5120)
    result={'kind':'same_gguf_cpu_f32_final_comparison','diagnostic_only':True,'status':'complete','independent_vs_native_metal_rows':metrics(independent,metal),
            'independent_vs_original_bf16_rows':metrics(independent,bf16),'independent_vs_native_metal_logits':logits(r['logits'],n['hf_head_logits']),
            'independent_vs_original_bf16_logits':logits(r['logits'],c['logits']),
            'source_hashes':{'independent_receipt':sha(args.reference_dir/'receipt.json'),'native_dump':sha(native/'dump.json'),'clean_case':sha(clean/'text-014.json')}}
    native_cpu=BASE/'stage5/native27b-layer-014/inputs/result_norm.f32'
    if native_cpu.exists():result['independent_vs_native_cpu_rows']=metrics(independent,np.fromfile(native_cpu,dtype='<f4').reshape(321,5120))
    args.output.write_text(json.dumps(result,indent=2,allow_nan=False)+'\n');print(json.dumps(result,indent=2))

if __name__=='__main__':main()
