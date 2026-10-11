#!/usr/bin/env python3
"""Replay only the CPU head after the completed MPS pilot's checker failure."""
import argparse
import ast
import json
import os
from pathlib import Path
import importlib.util
import sys
import time

import mps_reference as ref

def main():
 p=argparse.ArgumentParser();p.add_argument('--output-dir',type=Path,required=True);a=p.parse_args()
 resources=set(os.environ.get('OPENAGENTS_LEASES','').split(','));ref.require({'memory','disk'}<=resources and not {'gpu','build','quiet'}&resources and os.environ.get('CLEF_PILOT_RECOVERY_GUARDED')=='1','CPU-only recovery guard required')
 ref.require(not a.output_dir.exists(),'preserve previous results');a.output_dir.mkdir(parents=True)
 failed=ref.HERE/'mps-pilot-1';before=json.loads((failed/'results/receipt.json').read_text());guard=json.loads((failed/'guard.json').read_text())
 record={'schema':'clef.same-q4-mps-pilot-cpu-recovery.v1','status':'running','gpu_executed':False,'trunk_executed':False,'failed_guard_sha256':ref.sha(failed/'guard.json'),'failed_result_sha256':ref.sha(failed/'results/receipt.json'),'old_source_sha256':ref.sha(failed/'executed-source/mps_reference.py'),'new_checker_sha256':ref.sha(ref.HERE/'mps_reference.py'),'frozen_limits':{'relative_l2':1e-4,'max_abs_logit':1e-3,'max_abs_probability':1e-4,'all_tops':True}};path=a.output_dir/'receipt.json';ref.write(path,record);reader=None
 try:
  ref.require(before['status']=='failed' and before['error']=='native HTTP answer type differs' and len(before['layers'])==64,'unexpected failed pilot state')
  ref.require(guard['memory']['owned_group_stopped'] and guard['memory']['memory_rejection'] is None and guard['memory']['sampling_error'] is None,'pilot resource/cleanup failed')
  ref.require(record['old_source_sha256']==guard['sources'][str(ref.HERE/'mps_reference.py')],'old executed source hash differs')
  old_functions={v.name:ast.dump(v,include_attributes=False) for v in ast.parse((failed/'executed-source/mps_reference.py').read_text()).body if isinstance(v,ast.FunctionDef)}
  new_functions={v.name:ast.dump(v,include_attributes=False) for v in ast.parse((ref.HERE/'mps_reference.py').read_text()).body if isinstance(v,ast.FunctionDef)}
  for name in ('loaded_layer','forward_one','finish_head','row_errors','compare_logits'):ref.require(old_functions[name]==new_functions[name],'pilot numerical function changed: '+name)
  record['numerical_functions_unchanged']=True
  import torch,transformers
  from transformers.models.qwen3_5 import modeling_qwen3_5 as modeling
  from safetensors.torch import load_file
  ref.require(str(torch.__version__)=='2.9.1' and transformers.__version__=='5.10.2' and ref.sha(Path(modeling.__file__))==ref.MODELING_SHA,'runtime/operator identity differs')
  ref.require(ref.sha(ref.HF/'joint_schema_model.py')==ref.SOURCE_SHA and ref.sha(ref.HF/'joint_head.safetensors')==ref.HEAD_SHA,'official head identity differs')
  torch.set_num_threads(2);torch.set_num_interop_threads(1);torch.set_default_device('cpu')
  spec=importlib.util.spec_from_file_location('recovery_official',ref.HF/'joint_schema_model.py');official=importlib.util.module_from_spec(spec);sys.modules[spec.name]=official;spec.loader.exec_module(official)
  processor=transformers.AutoProcessor.from_pretrained(ref.HF,local_files_only=True)
  cases=ref.prepare_cases(official,processor,json.loads((ref.HERE/'encoder-source-run-1/native-encoded.json').read_text()),'full')
  contracts=[]
  for case in cases:
   for mode in ('staged','fused-fields'):
    rows=[json.loads(x) for x in (ref.NATIVE/mode/'rows.jsonl').read_text().splitlines()];selected=sorted((row for row in rows if row['request_sha256']=='sha256:'+case['request_sha256']),key=lambda row:row['question_index'])
    contracts.append({'case':case['id'],'mode':mode,**ref.native_response(case,mode,[row['logits'] for row in selected])})
  record['actual_official_typed_record_contracts']={'requests':40,'questions_per_mode':104,'responses':80,'passed':True,'checks':contracts};ref.write(path,record)
  case=next(x for x in cases if x['id']=='text-014');hidden=load_file(str(failed/'results/layer-63.safetensors'),device='cpu')['hidden']
  ref.require(ref.digest(ref.cpu_bytes(torch,hidden))==before['layers'][63]['requests'][0]['hidden_sha256'],'saved pre-final-norm hidden changed')
  record['input_layer63']={'file_sha256':ref.sha(failed/'results/layer-63.safetensors'),'raw_sha256':ref.digest(ref.cpu_bytes(torch,hidden))}
  old=json.loads((ref.CPU/'full-1/results/receipt.json').read_text());ref.require(ref.sha(ref.CPU/'full-1/results/receipt.json')=='4bc4e7bc7732bdbb732889d5376dab1c887782a3434ec86620cbc0a666bc8fe5','CPU oracle identity differs')
  reader=ref.StreamingGguf();ref.require(reader.state==old['gguf_file_state']==before['gguf_state'],'GGUF identity differs')
  config=modeling.Qwen3_5TextConfig.from_dict(json.loads((ref.HF/'config.json').read_text())['text_config'])
  head=official.JointSchemaHead(**json.loads((ref.HF/'joint_head_config.json').read_text()));head.load_state_dict(load_file(str(ref.HF/'joint_head.safetensors'),device='cpu'),strict=True);head=head.to(device='cpu',dtype=torch.float32).eval()
  final,result=ref.finish_head(torch,official,reader,config,case,hidden,head)
  saved=load_file(str(failed/'results/text-014-final.safetensors'),device='cpu')['hidden'];ref.require(ref.cpu_bytes(torch,final)==ref.cpu_bytes(torch,saved),'replayed final norm differs from saved pilot output')
  record['saved_final_norm_exact']=True;record['saved_final_file_sha256']=ref.sha(failed/'results/text-014-final.safetensors')
  cpu=load_file(str(ref.CPU/'full-1/results/final-hidden.safetensors'),device='cpu')['hidden'];ref.require(ref.digest(ref.cpu_bytes(torch,cpu))==old['hidden_sha256'],'CPU final rows changed')
  result.update(id='text-014',cpu_rows=ref.row_errors(final,cpu),cpu_logits=old['logits'],cpu_comparison=ref.compare_logits(result['logits'],old['logits']))
  result['native_stage6_comparisons']=ref.native_comparisons(case,result['logits'],old['logits'])
  r=result['cpu_rows'];c=result['cpu_comparison'];record['result']=result
  record['acceleration_screen_passed']=r['relative_l2'] is not None and r['relative_l2']<=1e-4 and c['max_abs_logit']<=1e-3 and c['max_abs_probability']<=1e-4 and c['all_tops_match'];ref.write(path,record)
  ref.require(record['acceleration_screen_passed'],'fixed CPU-to-MPS acceleration screen failed');reader.check_state();record.update(status='complete',passed=True,normalization_count=1,head_device='cpu',head_dtype='f32')
 except BaseException as error:record.update(status='failed',error_type=type(error).__name__,error=str(error));raise
 finally:
  if reader is not None:reader.close()
  ref.write(path,record)

if __name__=='__main__':main()
