#!/usr/bin/env python3
"""Inspect existing arrays only; no Torch, model forward, GPU, or allocation of weights."""
import hashlib,json,struct
from pathlib import Path
import numpy as np
BASE=Path(__file__).resolve().parents[2];HERE=Path(__file__).resolve().parent;RUN=HERE.parent

def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def tensor(path,key):
 with path.open('rb') as f:
  n=struct.unpack('<Q',f.read(8))[0];h=json.loads(f.read(n));row=h[key];a,b=row['data_offsets'];assert b-a<=8*1024**2;f.seek(8+n+a);raw=f.read(b-a)
 if row['dtype']=='BF16':value=(np.frombuffer(raw,dtype='<u2').astype(np.uint32)<<16).view(np.float32)
 else:assert row['dtype']=='F32';value=np.frombuffer(raw,dtype='<f4').copy()
 return value.reshape(row['shape'])
def error(a,b):
 a=a.astype(np.float64);b=b.astype(np.float64);delta=a-b
 return {'max_abs':float(abs(delta).max()),'relative_l2':float(np.linalg.norm(delta)/np.linalg.norm(b)),'cosine':float(a@b/np.linalg.norm(a)/np.linalg.norm(b)),'native_norm':float(np.linalg.norm(a)),'reference_norm':float(np.linalg.norm(b))}
head=BASE/'artifacts/clef-bf16/joint_head.safetensors';hidden_path=RUN/'mps-full-1/results/text-029-final.safetensors';hidden=tensor(hidden_path,'hidden')[0].astype(np.float64);weight=tensor(head,'hidden_norm.weight').astype(np.float64);bias=tensor(head,'hidden_norm.bias').astype(np.float64)
mean=hidden.mean(axis=1,keepdims=True);centered=hidden-mean;normalized=centered/np.sqrt((centered*centered).mean(axis=1,keepdims=True)+1e-5)*weight+bias
encoded=json.loads((RUN/'encoder-source-run-1/native-encoded.json').read_text())['cases'][29]
request=(BASE/'27b-head-stage6-combined/text-029.request.json').read_bytes();digest='sha256:'+hashlib.sha256(request).hexdigest();result={'tokens':359,'hidden':5120,'request_sha256':digest,'mps_final_sha256':sha(hidden_path),'method':'NumPy float64 LayerNorm and means of saved final MPS f32 rows; diagnostic only, not bitwise official head replay. Native export rounded f32 pooled rows to f16.','questions':[]}
for mode in ('staged','fused-fields'):
 directory=BASE/'27b-head-stage6-combined'/mode;rows=sorted((json.loads(s) for s in (directory/'rows.jsonl').read_text().splitlines() if s),key=lambda r:r['offset']);rows=[r for r in rows if r['request_sha256']==digest]
 for q,row in zip(encoded['questions'],rows):
  spans=[q['question_span']]+q['option_spans'];reference=np.stack([normalized[-1]]+[normalized[a:b].mean(axis=0) for a,b in spans])
  with (directory/'rows.f16').open('rb') as f:f.seek(row['offset']);raw=f.read(len(row['rows'])*5120*2)
  native=np.frombuffer(raw,dtype='<f2').astype(np.float64).reshape(-1,5120)
  result['questions'].append({'mode':mode,'question':q['id'],'question_span_tokens':q['question_span'][1]-q['question_span'][0],'rows':[{'label':label,**error(a,b)} for label,a,b in zip(row['rows'],native,reference)],'question_plus_global_anchor':error(native[0]+native[1],reference[0]+reference[1]),'native_pooled_export_sha256':hashlib.sha256(raw).hexdigest()})
(HERE/'pooled-inspection.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result,indent=2))
