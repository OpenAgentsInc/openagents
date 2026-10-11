#!/usr/bin/env python3
"""Check sampled full-attention rows and inverse DeltaNet output columns."""
import json,os,time
from pathlib import Path
for name in ('OMP_NUM_THREADS','OPENBLAS_NUM_THREADS','MKL_NUM_THREADS','VECLIB_MAXIMUM_THREADS'):os.environ[name]='2'
import numpy as np
from gguf_import import BASE,StreamingGguf,head_permutation,require
HERE=Path(__file__).resolve().parent
HF=BASE/'artifacts/clef-bf16'
output=HERE/'projection-fixtures.json';require(not output.exists(),'preserve output')
reader=StreamingGguf();index=json.loads((HF/'model.safetensors.index.json').read_text())['weight_map'];headers={};read_bytes=0;records=[]
def row(name,row):
    global read_bytes
    path=HF/index[name]
    if path not in headers:
        with path.open('rb') as f:
            size=int.from_bytes(f.read(8),'little');headers[path]=(8+size,json.loads(f.read(size)))
    base,header=headers[path];t=header[name];require(t['dtype']=='BF16','dtype')
    width=t['shape'][-1];read_bytes+=width*2;require(read_bytes<4*1024**2,'read bound')
    with path.open('rb') as f:f.seek(base+t['data_offsets'][0]+row*width*2);data=f.read(width*2)
    return (np.frombuffer(data,dtype='<u2').astype(np.uint32)<<16).view(np.float32)
def compare(gg,hf,r,columns=False):
    actual=reader.rows(gg,[r])[0]
    if columns:
        _,inverse=head_permutation();actual=actual.reshape(48,128)[inverse].reshape(-1)
    original=row(hf,r);difference=(actual-original).astype(np.float64)
    relative=float(np.linalg.norm(difference)/np.linalg.norm(original.astype(np.float64)))
    require(relative<.03,'projection mapping differs: '+gg)
    records.append({'gguf_tensor':gg,'hf_tensor':hf,'row':r,'relative_l2_error':relative})
for layer in (3,63):
    for gg,hf in [('attn_q','q_proj'),('attn_k','k_proj'),('attn_v','v_proj'),('attn_output','o_proj')]:
        name=f'blk.{layer}.{gg}.weight'
        count=reader.tensors[name]['shape'][1]
        for r in (0,127,128,255,256,count-1):
            compare(name,f'model.language_model.layers.{layer}.self_attn.{hf}.weight',r)
for r in (0,128,5119):compare('blk.0.ssm_out.weight','model.language_model.layers.0.linear_attn.out_proj.weight',r,True)
reader.close();output.write_text(json.dumps({'passed':True,'model_execution':False,'rows':records,'bf16_payload_bytes':read_bytes,'gguf_payload_bytes':reader.payload_read_bytes},indent=2)+'\n')
print(json.dumps({'passed':True,'rows':len(records),'max_relative_error':max(x['relative_l2_error'] for x in records)}))
