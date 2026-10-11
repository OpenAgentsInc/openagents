#!/usr/bin/env python3
"""Separate checkpoint quantization from BF16 arithmetic using saved results."""
import json
from pathlib import Path
import numpy as np
from gguf_import import BASE,require
from compare_final import tensor,logits
from compare_layers import metrics,sha
HERE=Path(__file__).resolve().parent
qdir=HERE/'full-1/results';fdir=HERE/'bf16-full-1/results'
q=json.loads((qdir/'receipt.json').read_text());f=json.loads((fdir/'receipt.json').read_text())
require(q['completed'] and f['completed'] and q['input_ids']==f['input_ids'],'streamed passes incomplete or token mismatch')
clean=BASE/'reference27b/cpu-head-stage1/results';c=json.loads((clean/'text-014.json').read_text())
native=BASE/'stage5/native27b-head-014/inputs';n=json.loads((native/'dump.json').read_text())
require(q['input_ids']==c['input_ids']==n['input_ids'],'native/clean token mismatch')
output=HERE/'bf16-full-1/control-comparison.json';require(not output.exists(),'preserve previous output')
qh=tensor(qdir/'final-hidden.safetensors','hidden').reshape(321,5120)
fh=tensor(fdir/'final-hidden.safetensors','hidden').reshape(321,5120)
bh=tensor(clean/c['head_inputs']['file'],'hidden').reshape(321,5120)
nh=np.fromfile(native/'hidden.f32',dtype='<f4').reshape(321,5120)
r={'kind':'text014_same_operators_quantization_and_dtype_controls','diagnostic_only':True,'status':'complete',
   'q4_f32_vs_original_f32_rows':metrics(qh,fh),'original_f32_vs_original_bf16_mps_rows':metrics(fh,bh),
   'native_metal_vs_original_f32_rows':metrics(nh,fh),
   'q4_f32_vs_original_f32_logits':logits(q['logits'],f['logits']),
   'original_f32_vs_original_bf16_mps_logits':logits(f['logits'],c['logits']),
   'native_metal_vs_original_f32_logits':logits(n['hf_head_logits'],f['logits']),
   'head_cross_swaps_vs_original_f32':{k:logits(v,f['logits']) for k,v in f['head_cross_swaps'].items()},
   'head_cross_swaps_vs_original_bf16_mps':{k:logits(v,c['logits']) for k,v in f['head_cross_swaps'].items()},
   'layer_rows':[], 'sources':{'q4_f32':sha(qdir/'receipt.json'),'original_f32':sha(fdir/'receipt.json'),'original_bf16_mps':sha(clean/'text-014.json'),'native_metal':sha(native/'dump.json')},
   'limitations':['One321-token case; not the full acceptance suite.','GGUF transformation uses stored negative-A values while original checkpoint uses its exact A_log.','Full quantized checkpoint differs in embeddings, projection quantization and FFN weights; this does not isolate FFN Q4 alone.']}
for i in range(64):
    qh=tensor(qdir/f'layer-{i:02d}.safetensors','hidden').reshape(321,5120)
    fh=tensor(fdir/f'layer-{i:02d}.safetensors','hidden').reshape(321,5120)
    r['layer_rows'].append({'layer':i,**metrics(qh,fh)})
output.write_text(json.dumps(r,indent=2,allow_nan=False)+'\n')
print(json.dumps({k:v for k,v in r.items() if k not in ('layer_rows','head_cross_swaps_vs_original_bf16_mps','head_cross_swaps_vs_original_f32')},indent=2))
print(json.dumps({'head_swaps':{k:{'max_abs_dp':v['max_abs_dp'],'all_tops_agree':v['all_tops_agree']} for k,v in r['head_cross_swaps_vs_original_f32'].items()}},indent=2))
