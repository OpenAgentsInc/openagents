#!/usr/bin/env python3
"""Compare the FFN-only weight replacement against both preserved controls."""
import json
from pathlib import Path
from gguf_import import BASE,require
from compare_final import tensor,logits
from compare_layers import metrics,sha
HERE=Path(__file__).resolve().parent
hdir=HERE/'hybrid-full-1/results';h=json.loads((hdir/'receipt.json').read_text())
qdir=HERE/'full-1/results';q=json.loads((qdir/'receipt.json').read_text())
fdir=HERE/'bf16-full-1/results';f=json.loads((fdir/'receipt.json').read_text())
clean=BASE/'reference27b/cpu-head-stage1/results';c=json.loads((clean/'text-014.json').read_text())
require(h['completed'] and h['original_ffn_matrices_loaded']==192 and h['input_ids']==q['input_ids']==f['input_ids']==c['input_ids'],'input/replacement coverage differs')
require(h['head_cross_swaps']['q4_hidden_q6_lexical']==q['logits'],'unchanged quantized head inputs no longer reproduce prior result')
output=HERE/'hybrid-full-1/ffn-control-comparison.json';require(not output.exists(),'preserve earlier comparison')
hh=tensor(hdir/'final-hidden.safetensors','hidden').reshape(321,5120)
fh=tensor(fdir/'final-hidden.safetensors','hidden').reshape(321,5120)
r={'kind':'text014_original_ffn_only_replacement_control','status':'complete','diagnostic_only':True,'not_q4_acceptance':True,
   'changed_weights':'Exactly192 FFN gate/up/down matrices; original BF16 values cast to f32.',
   'unchanged':'GGUF token/output embeddings, all other quantized/dense backbone weights, same official f32 operators and stored-negative-A adapter.',
   'hybrid_vs_original_f32_logits':logits(h['logits'],f['logits']),
   'hybrid_vs_original_bf16_mps_logits':logits(h['logits'],c['logits']),
   'hybrid_vs_quantized_f32_logits':logits(h['logits'],q['logits']),
   'hybrid_vs_original_f32_rows':metrics(hh,fh),
   'head_cross_swaps_vs_original_f32':{k:logits(v,f['logits']) for k,v in h['head_cross_swaps'].items()},
   'same_q4_head_result_exactly_reproduced':True,
   'sources':{'hybrid':sha(hdir/'receipt.json'),'quantized':sha(qdir/'receipt.json'),'original_f32':sha(fdir/'receipt.json'),'original_bf16_mps':sha(clean/'text-014.json')},
   'limitations':['One321-token case; not the full M4 suite.','Original FFN weights are substantially larger than Q4. This control is not a proposed product artifact or proof that Q6/Q8 will pass.','All192 FFN matrices changed together; no individual-layer causal attribution.']}
output.write_text(json.dumps(r,indent=2,allow_nan=False)+'\n')
print(json.dumps(r,indent=2))
