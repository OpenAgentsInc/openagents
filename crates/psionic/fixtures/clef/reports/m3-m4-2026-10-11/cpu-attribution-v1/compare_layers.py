#!/usr/bin/env python3
"""Compare bounded native and independent same-GGUF layer dump files."""
import argparse
import hashlib
import json
import os
from pathlib import Path
for name in ('OMP_NUM_THREADS','OPENBLAS_NUM_THREADS','MKL_NUM_THREADS','VECLIB_MAXIMUM_THREADS'):
    os.environ[name]='2'
import numpy as np
from safetensors.numpy import load_file
from gguf_import import require


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def metrics(candidate,reference):
    require(candidate.shape==reference.shape and np.isfinite(candidate).all() and np.isfinite(reference).all(),'shape or finite check failed')
    a,b=candidate.astype(np.float64),reference.astype(np.float64)
    difference=a-b
    reference_norm=np.linalg.norm(b,axis=-1)
    candidate_norm=np.linalg.norm(a,axis=-1)
    cosine=np.sum(a*b,axis=-1)/np.maximum(candidate_norm*reference_norm,np.finfo(np.float64).tiny)
    row_error=np.linalg.norm(difference,axis=-1)/np.maximum(reference_norm,np.finfo(np.float64).tiny)
    return {'values':int(a.size),'max_abs_error':float(np.max(np.abs(difference))),
            'rmse':float(np.sqrt(np.mean(difference*difference))),
            'relative_l2_error':float(np.linalg.norm(difference)/np.linalg.norm(b)),
            'minimum_token_cosine':float(np.min(cosine)), 'mean_token_cosine':float(np.mean(cosine)),
            'maximum_token_relative_l2_error':float(np.max(row_error)),
            'worst_token':int(np.argmax(row_error)),
            'last_token_relative_l2_error':float(row_error[-1])}


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--native-dir',type=Path,required=True)
    parser.add_argument('--reference-dir',type=Path,required=True)
    parser.add_argument('--output',type=Path,required=True)
    args=parser.parse_args();require(not args.output.exists(),'preserve previous comparison')
    receipt=json.loads((args.reference_dir/'receipt.json').read_text())
    require(receipt['completed'] and receipt['requested_layers']==64 and len(receipt['layers'])==64,'reference incomplete')
    n,d=len(receipt['input_ids']),5120
    result={'kind':'native_vs_official_same_gguf_cpu_f32_layers','diagnostic_only':True,
            'input_tokens':n,'hidden_size':d,'reference_receipt_sha256':sha(args.reference_dir/'receipt.json'),
            'native_dir':str(args.native_dir.resolve()),'reference_dir':str(args.reference_dir.resolve()),'layers':[]}
    for index in range(65):
        reference_path=args.reference_dir/(f'layer-{index:02d}.safetensors' if index<64 else 'final-hidden.safetensors')
        native_path=args.native_dir/(f'l_out-{index}.f32' if index<64 else 'result_norm.f32')
        require(native_path.stat().st_size==n*d*4,'native rows missing/truncated')
        reference=load_file(str(reference_path))['hidden'].reshape(n,d)
        native=np.fromfile(native_path,dtype='<f4').reshape(n,d)
        row={'layer':index if index<64 else 'final_norm','native_sha256':sha(native_path),'reference_sha256':sha(reference_path),**metrics(native,reference)}
        result['layers'].append(row)
    result['first_max_abs_above']={str(bound):next((row['layer'] for row in result['layers'] if row['max_abs_error']>bound),None) for bound in (1e-5,1e-4,1e-3,1e-2,.1)}
    result['status']='complete';args.output.write_text(json.dumps(result,indent=2,allow_nan=False)+'\n')
    print(json.dumps({'first_max_abs_above':result['first_max_abs_above'],'final_norm':result['layers'][-1]},indent=2))

if __name__=='__main__':main()
