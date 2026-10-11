#!/usr/bin/env python3
"""Replay the official CPU head with selected saved pooled inputs."""
import argparse,hashlib,importlib.util,json,math,os,sys
from pathlib import Path
HERE=Path(__file__).resolve().parent;RUN=HERE.parent;sys.path.insert(0,str(RUN))
import mps_reference as ref

def main():
 p=argparse.ArgumentParser();p.add_argument('--output-dir',type=Path,required=True);a=p.parse_args()
 resources=set(os.environ.get('OPENAGENTS_LEASES','').split(','));ref.require({'memory','disk'}<=resources and not {'gpu','build','quiet'}&resources and os.environ.get('CLEF_HEAD_REPLAY_GUARDED')=='1','CPU-only guarded replay required')
 ref.require(not a.output_dir.exists(),'preserve previous output');a.output_dir.mkdir(parents=True)
 result={'schema':'clef.text029-pooled-head-replay.v1','status':'running','gpu_executed':False,'trunk_executed':False,'scope':'Partial cross-swap. Native f16 exports contain pooled normalized rows only; every variant keeps full reference memory and lexical rows. This cannot establish full native-head equivalence.','variants':[]};path=a.output_dir/'receipt.json';ref.write(path,result);reader=None
 try:
  import numpy as np,torch,transformers
  from safetensors.torch import load_file
  torch.set_num_threads(2);torch.set_num_interop_threads(1);torch.set_default_device('cpu')
  ref.require(str(torch.__version__)=='2.9.1' and transformers.__version__=='5.10.2','runtime versions differ')
  ref.require(ref.sha(ref.HF/'joint_schema_model.py')==ref.SOURCE_SHA and ref.sha(ref.HF/'joint_head.safetensors')==ref.HEAD_SHA,'official head identity differs')
  spec=importlib.util.spec_from_file_location('pool_swap_official',ref.HF/'joint_schema_model.py');official=importlib.util.module_from_spec(spec);sys.modules[spec.name]=official;spec.loader.exec_module(official)
  full=json.loads((RUN/'mps-full-1/results/receipt.json').read_text());expected=next(x for x in full['cases'] if x['id']=='text-029');prior_input=next(x for x in full['inputs'] if x['id']=='text-029')
  processor=transformers.AutoProcessor.from_pretrained(ref.HF,local_files_only=True);raw=(ref.NATIVE/'text-029.request.json').read_bytes();record=official.encode_record(processor.tokenizer,json.loads(raw),processor=processor)
  ref.require(json.loads(json.dumps(ref.asdict(record)))==prior_input['record'],'fresh record differs')
  hidden=load_file(str(RUN/'mps-full-1/results/text-029-final.safetensors'),device='cpu')['hidden'];ref.require(ref.digest(ref.cpu_bytes(torch,hidden))==expected['hidden_sha256'],'saved final hidden changed')
  reader=ref.StreamingGguf();ref.require(reader.state==full['gguf_state'],'GGUF changed')
  ids=list(record.input_ids);ref.require(len(ids)==359 and record.questions[1].question_id=='日本1' and record.questions[1].option_ids[1]=='false','semantic swap coordinates differ');tokens=sorted({token for q in record.questions for first,end in q.option_spans for token in ids[first:end]});lexical=torch.from_numpy(reader.rows('output.weight',tokens));ref.require(ref.digest(ref.cpu_bytes(torch,lexical))==expected['lexical_sha256'],'saved lexical rows differ')
  mapping={token:i for i,token in enumerate(tokens)};remapped=[mapping.get(token,0) for token in ids]
  for q in record.questions:
   for start,end in q.option_spans:ref.require([tokens[i] for i in remapped[start:end]]==ids[start:end],'option lexical remapping differs')
  head_record=official.EncodedRecord(input_ids=tuple(remapped),questions=record.questions,record_id=record.record_id);head_ids=torch.tensor([remapped],dtype=torch.long);mask=torch.ones((1,len(ids)),dtype=torch.long)
  head=official.JointSchemaHead(**json.loads((ref.HF/'joint_head_config.json').read_text()));head.load_state_dict(load_file(str(ref.HF/'joint_head.safetensors'),device='cpu'),strict=True);head=head.float().eval()
  with torch.inference_mode():normal=head.hidden_norm(hidden);reference_pools={tuple(span):head._mean_span(normal[0],span).clone() for q in record.questions for span in [q.question_span,*q.option_spans]}
  normal_raw=ref.cpu_bytes(torch,normal);global_ref=normal[0,-1].clone();native_pools={};native_global=None;capture_rows={}
  for mode in ('staged','fused-fields'):
   rows=sorted((json.loads(x) for x in (ref.NATIVE/mode/'rows.jsonl').read_text().splitlines() if x),key=lambda r:r['question_index']);rows=[r for r in rows if r['request_sha256']=='sha256:'+prior_input['request_sha256']];ref.require(len(rows)==3,'native capture question count differs');capture_rows[mode]=rows
   for q,row in zip(record.questions,rows):
    ref.require(row['question']==q.question_id and row['rows'][2:]==['option:'+x for x in q.option_ids],'native pool order differs')
    with (ref.NATIVE/mode/'rows.f16').open('rb') as f:f.seek(row['offset']);data=f.read(len(row['rows'])*5120*2)
    values=torch.from_numpy(np.frombuffer(data,dtype='<f2').astype(np.float32).copy().reshape(-1,5120))
    if native_global is None:native_global=values[0].clone()
    ref.require(torch.equal(native_global,values[0]),'global export differs across questions/modes')
    for span,value in zip([q.question_span,*q.option_spans],values[1:]):
     span=tuple(span)
     if span in native_pools:ref.require(torch.equal(native_pools[span],value),'staged/fused pooled bytes differ')
     else:native_pools[span]=value.clone()
  question_spans={tuple(q.question_span) for q in record.questions};option_spans={tuple(s) for q in record.questions for s in q.option_spans};failed_false=tuple(record.questions[1].option_spans[1])
  variants=[('reference',{},None),('reference_pooled_f16',{s:v.half().float() for s,v in reference_pools.items()},global_ref.half().float()),('native_global',{},native_global),('native_questions',{s:native_pools[s] for s in question_spans},None),('native_options',{s:native_pools[s] for s in option_spans},None),('native_failed_false',{failed_false:native_pools[failed_false]},None),('native_all',native_pools,native_global),('native_all_except_failed_false',{s:v for s,v in native_pools.items() if s!=failed_false},native_global)]
  original_mean=head._mean_span;inputs_before={name:ref.cpu_bytes(torch,value) for name,value in [('hidden',hidden),('lexical',lexical),('ids',head_ids),('mask',mask)]}
  for name,overrides,global_override in variants:
   counts={'normal':0,'memory':0,'means':[]};parts={'options':[],'residuals':[]};handles=[]
   def normalized_hook(module,args,output):
    counts['normal']+=1;ref.require(ref.cpu_bytes(torch,output)==normal_raw,'head normalization changed')
    if global_override is None:return output
    changed=output.clone();changed[0,-1]=global_override;return changed
   def memory_hook(module,args):
    counts['memory']+=1;ref.require(tuple(args[0].shape)==(359,5120),'projected memory shape differs');return (normal[0],)
   def mean_override(values,span):
    key=tuple(span);counts['means'].append(key);return overrides[key] if key in overrides else original_mean(values,span)
   def fields_hook(module,args,output):parts['fields']=output.detach().clone()
   def options_hook(module,args,output):parts['options'].append(output.detach().clone())
   def residual_hook(module,args,output):parts['residuals'].append(output.detach().clone().squeeze(-1))
   try:
    handles=[head.hidden_norm.register_forward_hook(normalized_hook),head.memory_projection.register_forward_pre_hook(memory_hook),head.field_norm.register_forward_hook(fields_hook),head.option_norm.register_forward_hook(options_hook),head.residual_scorer.register_forward_hook(residual_hook)];head._mean_span=mean_override
    with torch.inference_mode():logits=head(hidden,head_ids,mask,[head_record],lexical)[0]
    values=[x.tolist() for x in logits];comparison=ref.compare_logits(values,expected['logits'])
    ref.require(counts['normal']==1 and counts['memory']==1 and len(counts['means'])==9 and set(counts['means'])==set(reference_pools),'hook coverage differs')
    decomposition=[]
    with torch.inference_mode():
     for i,q in enumerate(record.questions):
      question=overrides.get(tuple(q.question_span),reference_pools[tuple(q.question_span)]);global_value=global_ref if global_override is None else global_override;anchor=torch.nn.functional.normalize(question+global_value,dim=-1)
      option_lexical=torch.stack([lexical[head_ids[0,start:end]].mean(0) for start,end in q.option_spans]);prior=head.prior_logit_scale.clamp(max=math.log(100.)).exp()*torch.matmul(torch.nn.functional.normalize(option_lexical,dim=-1),anchor)
      options=parts['options'][i];field=parts['fields'][i].unsqueeze(0).expand_as(options);cosine=torch.nn.functional.cosine_similarity(field,options,dim=-1);joint=head.joint_logit_scale.clamp(max=math.log(100.)).exp()*cosine+parts['residuals'][i];scaled=torch.sigmoid(head.residual_gate)*joint
      ref.require(float((prior+scaled-logits[i]).abs().max())<=1e-6,'head contribution reconstruction differs')
      decomposition.append({'question':q.question_id,'prior':prior.tolist(),'scaled_joint':scaled.tolist()})
    if name=='reference':ref.require(all(torch.equal(value,torch.tensor(want,dtype=torch.float32)) for value,want in zip(logits,expected['logits'])),'exact reference head replay differs')
    row={'name':name,'logits':values,'probabilities':ref.distributions(values),'reference_comparison':comparison,'contributions':decomposition,'hook_counts':counts,'native_comparisons':{mode:ref.compare_logits(values,[r['logits'] for r in rows]) for mode,rows in capture_rows.items()}}
    result['variants'].append(row);ref.write(path,result)
   finally:
    for handle in handles:handle.remove()
    head._mean_span=original_mean
   ref.require(ref.cpu_bytes(torch,normal)==normal_raw and all(ref.cpu_bytes(torch,value)==inputs_before[name] for name,value in [('hidden',hidden),('lexical',lexical),('ids',head_ids),('mask',mask)]),'head replay mutated immutable inputs')
  reader.check_state();result.update(status='complete',passed=True,record=ref.asdict(record),baseline_exact=True,all_three_question_outputs_retained=True,acceptance_scope_unchanged=True)
 except BaseException as error:result.update(status='failed',error_type=type(error).__name__,error=str(error));raise
 finally:
  if reader is not None:reader.close()
  ref.write(path,result)

if __name__=='__main__':main()
