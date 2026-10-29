#!/usr/bin/env python3
"""Independent CPU Transformers 5.3.0 reference; run only on CoderOS.

The original checkpoint must have been authenticated beforehand. The additive mask implements the same two-block visibility as the pinned
upstream serving patch. Full weights are converted to float32 for comparison with the
Psionic wide CPU profile. This script is tooling, not a runtime dependency.
"""
import json,sys
from pathlib import Path
import torch
from transformers import Qwen3_5ForConditionalGeneration
import transformers.models.qwen3_5.modeling_qwen3_5 as module

def score(logits):
 v=logits.float();selected=v[torch.tensor([32,33])]
 mass=(torch.logsumexp(selected,dim=0)-torch.logsumexp(v,dim=0)).exp().item()
 p=torch.softmax(selected,dim=0)
 return {'logits':[float(x) for x in selected], 'probabilities':[float(x) for x in p], 'slot_mass':mass}

def main():
 if len(sys.argv)!=4:raise SystemExit('usage: elephant_reference.py ORIGINAL_DIR TOKEN_IDS OUTPUT_JSON')
 directory=Path(sys.argv[1]);lines=[int(x) for x in Path(sys.argv[2]).read_text().splitlines()];prefix=lines[0];tokens=lines[1:]
 if not 0<=prefix<len(tokens):raise ValueError('prefix')
 torch.set_num_threads(4)
 print('loading original Elephant checkpoint in float32',flush=True)
 model=Qwen3_5ForConditionalGeneration.from_pretrained(str(directory),dtype=torch.float32,local_files_only=True,attn_implementation='eager').eval()
 capture=[]
 def save_layer(module,inputs,output):capture.append(output.detach().float().cpu())
 handle=model.model.language_model.layers[-1].register_forward_hook(save_layer)
 with torch.inference_mode():
  one=model(input_ids=torch.tensor([[32]]),use_cache=False,return_dict=True,logits_to_keep=1)
  one_score=score(one.logits[0,-1]);one_score['hidden_first4']=[float(x) for x in capture[-1][0,-1,:4]]
  del one;capture.clear()
  original=module.create_causal_mask
  def create_mask(*args,**kwargs):
   mask=torch.zeros((1,1,len(tokens),len(tokens)),dtype=torch.float32)
   mask[:,:,0:prefix,prefix:]=torch.finfo(mask.dtype).min
   return mask
  module.create_causal_mask=create_mask
  print(f'computing block-bidirectional decision: prefix {prefix}, suffix {len(tokens)-prefix}',flush=True)
  decision=model(input_ids=torch.tensor([tokens]),use_cache=False,return_dict=True,logits_to_keep=1)
  decision_score=score(decision.logits[0,-1]);decision_score['hidden_first4']=[float(x) for x in capture[-1][0,-1,:4]]
  module.create_causal_mask=original
 handle.remove();result={'torch':torch.__version__,'profile':'float32 eager CPU','token32':one_score,'decision':decision_score,'prefix':prefix,'tokens':len(tokens)}
 Path(sys.argv[3]).write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result,indent=2),flush=True)
if __name__=='__main__':main()
