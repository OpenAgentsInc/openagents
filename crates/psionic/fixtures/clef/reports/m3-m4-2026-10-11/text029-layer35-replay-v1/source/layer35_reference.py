#!/usr/bin/env python3
"""Replay layer35 with exact CPU input, native input, and documented half boundaries."""
import argparse,gc,json,os,sys,time,types
from pathlib import Path
HERE=Path(__file__).resolve().parent;RUN=HERE.parent
sys.path.insert(0,str(RUN));import mps_reference as ref
from layer35_math import online_attention,fixtures

def main():
 p=argparse.ArgumentParser();p.add_argument('--output-dir',type=Path,required=True);a=p.parse_args()
 resources=set(os.environ.get('OPENAGENTS_LEASES','').split(','));ref.require({'memory','disk'}<=resources and not {'gpu','build','quiet'}&resources and os.environ.get('CLEF_LAYER35_GUARDED')=='1','CPU-only guarded layer replay required')
 ref.require(not a.output_dir.exists(),'preserve previous output');a.output_dir.mkdir(parents=True)
 result={'schema':'clef.text029-layer35-same-input.v1','status':'running','layer':35,'gpu_executed':False,'head_executed':False,'artifact_weight_files_modified':False,'C_in_memory_weight_rounding':True,'variants':[],'limits':'Diagnostic only. All104tops/.02 fullruntime gate and prior within-pipeline1e-3 criterion remain unchanged.','rounding_model_limit':'C models half boundaries and32-key online exponential rounding. CPU reduction order, transcendental implementations, fused multiply-add and residual-in-GEMM accumulation are not bitwise Metal emulations.'};path=a.output_dir/'receipt.json';ref.write(path,result);reader=None
 try:
  import torch,transformers
  from transformers.models.qwen3_5 import modeling_qwen3_5 as modeling
  from safetensors.torch import load_file,save_file
  import numpy as np
  torch.set_num_threads(2);torch.set_num_interop_threads(1);torch.set_default_device('cpu')
  ref.require(str(torch.__version__)=='2.9.1' and transformers.__version__=='5.10.2' and ref.sha(Path(modeling.__file__))==ref.MODELING_SHA,'official operators/version differ')
  raw=json.loads((ref.HF/'config.json').read_text())['text_config'];config=modeling.Qwen3_5TextConfig.from_dict(raw);config._attn_implementation='eager';config.use_cache=False;config.dtype=torch.float32
  ref.require(config.num_hidden_layers==64 and config.hidden_size==5120 and config.intermediate_size==17408 and config.layer_types[35]=='full_attention' and config.attention_dropout==0,'layer35 configuration differs')
  cpu=json.loads((HERE/'cpu-1/results/receipt.json').read_text());native=json.loads((HERE/'native-1/results/receipt.json').read_text());comparison=json.loads((HERE/'comparison-1/results/receipt.json').read_text())
  ref.require(comparison['observer_consistency_screen']['passed'],'native observer is not equivalent')
  cp=[];np_rows=[]
  for index in (34,35):
   row=cpu['layers'][index];f=HERE/'cpu-1/results'/row['file'];ref.require(ref.sha(f)==row['file_sha256'],'CPU layer file changed');value=load_file(str(f),device='cpu')['hidden'];ref.require(ref.digest(ref.cpu_bytes(torch,value))==row['hidden_sha256'],'CPU hidden bytes differ');cp.append(value)
   f=HERE/'native-1/results/rows'/f'l_out-{index}.f32';meta=next(x for x in native['files'] if x['name']==f.name);ref.require(ref.sha(f)==meta['sha256'],'native layer file changed');np_rows.append(torch.from_numpy(np.fromfile(f,dtype='<f4').copy().reshape(1,359,5120)))
  for value in cp+np_rows:ref.require(value.dtype==torch.float32 and tuple(value.shape)==(1,359,5120) and bool(torch.isfinite(value).all()),'saved layer shape/finiteness differs')
  full=json.loads((RUN/'mps-full-1/results/receipt.json').read_text());record=next(x['record'] for x in full['inputs'] if x['id']=='text-029');q=record['questions'][1]
  ref.require(q['question_id']=='日本1' and q['option_ids']==['true','false'] and q['question_span']==[192,194] and q['option_spans']==[[205,223],[229,247]],'diagnostic spans differ')
  position_ids=torch.arange(359).view(1,-1);rotary=modeling.Qwen3_5TextRotaryEmbedding(config,device=torch.device('cpu'))
  with torch.inference_mode():rope=rotary(cp[0],position_ids[None,...].expand(3,1,-1));mask=modeling.create_causal_mask(config=config,inputs_embeds=cp[0],attention_mask=torch.ones((1,359),dtype=torch.long),past_key_values=None,position_ids=position_ids)
  result['arithmetic_fixtures']=fixtures(torch,modeling);reader=ref.StreamingGguf();ref.require(reader.state==cpu['gguf_file_state'],'GGUF identity changed')
  with torch.device('meta'):layer=modeling.Qwen3_5DecoderLayer(config,35)
  arrays,negative=ref.layer_arrays(reader,35);ref.require(negative is None and set(arrays)==set(layer.state_dict()),'full-attention layer parameters differ');layer.load_state_dict({k:torch.from_numpy(v) for k,v in arrays.items()},strict=True,assign=True);del arrays,negative;layer.eval()
  ref.require(all(p.dtype==torch.float32 and p.device.type=='cpu' for p in layer.parameters()),'layer device/dtype differs')
  result['parameter_identity_before']=ref.layer_hashes(torch,layer,'cpu');result['gguf_state']=reader.state;result['payload_read_bytes']=reader.payload_read_bytes;ref.write(path,result)
  original_eager=modeling.eager_attention_forward
  for name,hidden,round_half in [('A_cpu_input_f32',cp[0],False),('B_native_input_f32',np_rows[0],False),('C_native_input_half_boundaries',np_rows[0],True)]:
   start=time.monotonic();directory=a.output_dir/name;directory.mkdir();points={};handles=[];originals={};calls={'attention':0};before=ref.cpu_bytes(torch,hidden)
   def capture(key,value,allow_negative_infinity=False):
    ref.require(key not in points and value.device.type=='cpu' and value.dtype==torch.float32,'checkpoint repeated/device/dtype differs')
    okay=torch.isfinite(value) | (torch.isneginf(value) if allow_negative_infinity else False);ref.require(bool(okay.all()),'nonfinite checkpoint '+key)
    f=directory/(key+'.safetensors');save_file({'value':value.detach().contiguous()},str(f));points[key]={'file':str(f.relative_to(a.output_dir)),'shape':list(value.shape),'bytes':value.numel()*4,'raw_sha256':ref.digest(ref.cpu_bytes(torch,value)),'file_sha256':ref.sha(f),'negative_infinity_allowed':allow_negative_infinity}
   if round_half:
    with torch.no_grad():
     for module in layer.modules():
      if isinstance(module,torch.nn.Linear):module.weight.copy_(module.weight.half().float())
    ref.require(all(bool(torch.isfinite(p).all()) for p in layer.parameters()),'half-rounded weight became nonfinite')
   weight_hashes=ref.layer_hashes(torch,layer,'cpu')
   def wrap_linear(key,module):
    original=module.forward
    def forward(self,values):
     used=values.half().float() if round_half else values;capture(key+'.input',used);output=original(used);capture(key+'.output',output);return output
    originals[key]=(module,original);module.forward=types.MethodType(forward,module)
   def attention(module,query,key,value,attention_mask,scaling,dropout=0.0,**kwargs):
    calls['attention']+=1;ref.require(dropout==0 and not module.training,'dropout/training differs');capture('attention.q_after_rope',query);capture('attention.k_after_rope',key);capture('attention.v_before_cache',value)
    if round_half:return online_attention(torch,modeling,module,query,key,value,attention_mask,scaling,capture)
    output,p=original_eager(module,query,key,value,attention_mask,scaling=scaling,dropout=dropout,**kwargs);capture('attention.probabilities',p);capture('attention.output',output);return output,p
   def post_norm(module,args,output):capture('mixer.residual',args[0]);capture('ffn.norm',output)
   try:
    for key,module in layer.named_modules():
     if isinstance(module,torch.nn.Linear):wrap_linear(key,module)
    ref.require(len(originals)==7,'linear checkpoint count differs')
    handles=[layer.input_layernorm.register_forward_hook(lambda m,a,o:capture('mixer.norm',o)),layer.post_attention_layernorm.register_forward_hook(post_norm),layer.self_attn.register_forward_hook(lambda m,a,o:capture('mixer.output',o[0])),layer.mlp.register_forward_hook(lambda m,a,o:capture('ffn.output',o))]
    modeling.eager_attention_forward=attention
    with torch.inference_mode():output=layer(hidden,position_embeddings=rope,attention_mask=mask,position_ids=position_ids,past_key_values=None,use_cache=False)
    capture('layer.output',output);ref.require(len(points)==(29 if round_half else 25),'checkpoint coverage differs');ref.require(calls['attention']==1 and ref.cpu_bytes(torch,hidden)==before,'attention/input invariance differs');ref.require(ref.layer_hashes(torch,layer,'cpu')==weight_hashes,'forward mutated parameters')
    baseline_exact=ref.cpu_bytes(torch,output)==ref.cpu_bytes(torch,cp[1]) if name.startswith('A_') else None
    if baseline_exact is not None:ref.require(baseline_exact,'A official baseline does not reproduce savedCPU layer35')
    entry={'name':name,'seconds':time.monotonic()-start,'baseline_exact':baseline_exact,'input_sha256':ref.digest(before),'parameter_hashes':weight_hashes,'checkpoints':points,'output_vs_native':ref.row_errors(output,np_rows[1]),'output_vs_cpu':ref.row_errors(output,cp[1]),'false_span_vs_native':ref.row_errors(output[:,229:247],np_rows[1][:,229:247]),'false_span_vs_cpu':ref.row_errors(output[:,229:247],cp[1][:,229:247])};result['variants'].append(entry);ref.write(path,result)
   finally:
    modeling.eager_attention_forward=original_eager
    for handle in handles:handle.remove()
    for module,original in originals.values():module.forward=original
   del output;gc.collect();reader.check_state()
  ref.require(len(result['variants'])==3,'variant coverage differs')
  result['checkpoint_comparisons']=[]
  for left,right in ((1,0),(2,1)):
   lhs,rhs=result['variants'][left],result['variants'][right]
   for key in sorted(set(lhs['checkpoints']) & set(rhs['checkpoints'])):
    lp,rp=lhs['checkpoints'][key],rhs['checkpoints'][key]
    ref.require(ref.sha(a.output_dir/lp['file'])==lp['file_sha256'] and ref.sha(a.output_dir/rp['file'])==rp['file_sha256'],'checkpoint file changed')
    lv=load_file(str(a.output_dir/lp['file']),device='cpu')['value'];rv=load_file(str(a.output_dir/rp['file']),device='cpu')['value']
    result['checkpoint_comparisons'].append({'left':lhs['name'],'right':rhs['name'],'checkpoint':key,'metrics':ref.row_errors(lv,rv)})
    del lv,rv
  result.update(status='complete',completed=True,original_artifact_unchanged=True)
 except BaseException as error:result.update(status='failed',error_type=type(error).__name__,error=str(error));raise
 finally:
  if reader is not None:reader.close()
  ref.write(path,result)

if __name__=='__main__':main()
