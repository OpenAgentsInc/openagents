#!/usr/bin/env python3
"""Independent same-GGUF f32 reference, with one official decoder layer on MPS."""
import argparse
from dataclasses import asdict
import gc
import hashlib
import importlib.util
import json
import math
import os
from pathlib import Path
import sys
import struct
import time
import types

HERE=Path(__file__).resolve().parent
BASE=HERE.parent
CPU=BASE/'cpu27b-diagnostic/streamed-gguf'
HF=BASE/'artifacts/clef-bf16'
NATIVE=BASE/'27b-head-stage6-combined'
sys.path.insert(0,str(CPU))
from gguf_import import StreamingGguf,fixture_checks,layer_arrays,ordinary_norm_to_hf,require
from stream_reference import make_negative_a_adapter,MODELING_SHA,SOURCE_SHA,HEAD_SHA
GIB=1024**3
PINS={'gguf_import.py':'57a152754b1dd224de24b111d7824fc6ecb3c7f4bb1149c6a5a3d78fdb41bc32','stream_reference.py':'b1b61674dc9afc362f29bef3bf228ed4d5e6c7cf0150bf203023d28362462041'}

def sha(path):
 h=hashlib.sha256()
 with Path(path).open('rb') as f:
  for raw in iter(lambda:f.read(1024**2),b''):h.update(raw)
 return h.hexdigest()
def digest(raw):return hashlib.sha256(raw).hexdigest()
def write(path,value):
 temporary=path.with_suffix(path.suffix+'.partial');temporary.write_text(json.dumps(value,indent=2,allow_nan=False)+'\n');temporary.replace(path)
def cpu_bytes(torch,tensor):
 require(tensor.device.type=='cpu','owned CPU tensor required')
 return tensor.detach().contiguous().view(torch.uint8).numpy().tobytes()
def snapshot(torch,tensor):
 require(tensor.device.type=='mps' and tensor.dtype in (torch.float32,torch.int64,torch.bool),'same-dtype MPS snapshot required')
 copied=tensor.detach().to(device='cpu',dtype=tensor.dtype,copy=True);torch.mps.synchronize()
 raw=cpu_bytes(torch,copied);del copied
 return raw

def owned_tensor(torch,raw,shape):
 import numpy as np
 value=torch.from_numpy(np.frombuffer(raw,dtype='<f4').copy().reshape(shape))
 require(cpu_bytes(torch,value)==raw,'owned CPU reconstruction differs')
 return value

def transport_test(torch):
 result=[]
 for sliced in (False,True):
  source=torch.arange(2048,dtype=torch.float32).reshape(32,64)/128-8
  if sliced:source=source[:,::2]
  original=cpu_bytes(torch,source);device=source.to(device='mps',dtype=source.dtype,copy=True);torch.mps.synchronize()
  first=snapshot(torch,device);require(first==original and cpu_bytes(torch,source)==original,'initial f32 transport changed bytes')
  later=torch.arange(8192,device='mps',dtype=torch.float32)*.125;torch.mps.synchronize()
  require(snapshot(torch,device)==original and first==original and cpu_bytes(torch,source)==original,'later MPS work changed owned/source bytes')
  result.append({'noncontiguous':sliced,'bytes':len(original),'sha256':digest(original)})
  del source,device,later
 gc.collect();torch.mps.synchronize();torch.mps.empty_cache();torch.mps.synchronize()
 return {'passed':True,'cases':result,'driver_after_cleanup':torch.mps.driver_allocated_memory()}

def layer_hashes(torch,layer,device):
 values={**dict(layer.named_parameters()),**dict(layer.named_buffers())};result={}
 for name,value in values.items():
  require(value.device.type==device and value.dtype==torch.float32,'unexpected parameter/buffer device or dtype')
  raw=snapshot(torch,value) if device=='mps' else cpu_bytes(torch,value)
  result[name]={'shape':list(value.shape),'sha256':digest(raw),'bytes':len(raw)}
 return result

def versions(layer):
 return {name:(tuple(value.shape),str(value.dtype),str(value.device),value.data_ptr(),value._version) for name,value in list(layer.named_parameters())+list(layer.named_buffers())}

def loaded_layer(torch,modeling,config,index,reader,adapter):
 began=time.monotonic()
 with torch.device('meta'):layer=modeling.Qwen3_5DecoderLayer(config,index)
 arrays,negative_a=layer_arrays(reader,index)
 require(set(arrays)==set(layer.state_dict()),'official layer keys differ')
 state={name:torch.from_numpy(value) for name,value in arrays.items()};layer.load_state_dict(state,strict=True,assign=True)
 if negative_a is not None:
  layer.linear_attn.register_buffer('gguf_negative_a',torch.from_numpy(negative_a));layer.linear_attn.forward=types.MethodType(adapter,layer.linear_attn)
 del arrays,state,negative_a
 layer.eval();before=layer_hashes(torch,layer,'cpu');decoded=time.monotonic()
 layer=layer.to(device='mps',dtype=torch.float32);torch.mps.synchronize()
 after=layer_hashes(torch,layer,'mps');require(before==after,'weight transfer changed f32 bytes')
 return layer,{'load_decode_hash_seconds':decoded-began,'upload_readback_hash_seconds':time.monotonic()-decoded,'parameter_hashes':before}

def forward_one(torch,modeling,config,layer,index,hidden):
 before=cpu_bytes(torch,hidden);shape=list(hidden.shape);length=shape[1]
 position_ids=torch.arange(length,dtype=torch.long,device='cpu').view(1,-1)
 rotary=modeling.Qwen3_5TextRotaryEmbedding(config,device=torch.device('cpu'))
 with torch.inference_mode():
  positions=position_ids[None,...].expand(3,1,-1)
  cpu_rope=rotary(hidden,positions)
  cpu_mask=modeling.create_causal_mask(config=config,inputs_embeds=hidden,attention_mask=torch.ones((1,length),dtype=torch.long),past_key_values=None,position_ids=position_ids) if index%4==3 else None
  device=hidden.to(device='mps',dtype=torch.float32,copy=True)
  rope=tuple(value.to(device='mps',dtype=value.dtype,copy=True) for value in cpu_rope)
  mask=cpu_mask.to(device='mps',dtype=cpu_mask.dtype,copy=True) if cpu_mask is not None else None
  ids=position_ids.to(device='mps',dtype=position_ids.dtype,copy=True)
  torch.mps.synchronize();require(snapshot(torch,device)==before,'hidden upload changed bytes')
  auxiliary=[('position_ids',position_ids,ids)]+[(f'rotary_{i}',a,b) for i,(a,b) in enumerate(zip(cpu_rope,rope))]
  if cpu_mask is not None:auxiliary.append(('causal_mask',cpu_mask,mask))
  auxiliary_bytes={name:cpu_bytes(torch,a) for name,a,b in auxiliary}
  require(all(snapshot(torch,b)==auxiliary_bytes[name] for name,a,b in auxiliary),'auxiliary upload changed bytes')
  old=versions(layer)
  output=layer(device,position_embeddings=rope,attention_mask=mask,position_ids=ids,past_key_values=None,use_cache=False)
  torch.mps.synchronize()
  require(list(output.shape)==shape and bool(torch.isfinite(output).all().item()),'invalid official layer output')
  raw=snapshot(torch,output)
  require(snapshot(torch,device)==before and cpu_bytes(torch,hidden)==before,'official layer mutated hidden input')
  require(versions(layer)==old,'official layer mutated parameter/buffer identities')
  require(all(cpu_bytes(torch,a)==auxiliary_bytes[name] and snapshot(torch,b)==auxiliary_bytes[name] for name,a,b in auxiliary),'official layer mutated auxiliary input bytes')
 return owned_tensor(torch,raw,shape),digest(raw),{name:digest(value) for name,value in auxiliary_bytes.items()}

def row_errors(candidate,reference):
 import numpy as np
 a=candidate.detach().cpu().numpy().astype(np.float64);b=reference.detach().cpu().numpy().astype(np.float64)
 require(a.shape==b.shape and np.isfinite(a).all() and np.isfinite(b).all(),'row comparison shape/finiteness differs')
 error=float(np.linalg.norm(a-b));den=float(np.linalg.norm(b));relative=(error/den if den else (0.0 if error==0 else None))
 av=a.reshape(-1,a.shape[-1]);bv=b.reshape(-1,b.shape[-1]);norms=np.linalg.norm(av,axis=1)*np.linalg.norm(bv,axis=1)
 cos=[]
 for x,y,n in zip(av,bv,norms):cos.append(float(np.dot(x,y)/n) if n else (1.0 if np.array_equal(x,y) else -1.0))
 return {'values':a.size,'max_abs_error':float(np.max(np.abs(a-b))),'relative_l2':relative,'zero_reference_norm':den==0,'minimum_token_cosine':min(cos)}

def distributions(logits):
 import numpy as np
 result=[]
 for values in logits:
  a=np.array(values,dtype=np.float64);require(a.ndim==1 and a.size>0 and np.isfinite(a).all(),'invalid logits');e=np.exp(a-a.max());p=e/e.sum();result.append(p.tolist())
 return result

def compare_logits(candidate,reference):
 import numpy as np
 require(len(candidate)==len(reference),'question count differs')
 cp,rp=distributions(candidate),distributions(reference);rows=[]
 for a,b,p,q in zip(candidate,reference,cp,rp):
  require(len(a)==len(b),'option count differs')
  rows.append({'max_abs_logit':float(np.max(np.abs(np.array(a)-b))),'max_abs_probability':float(np.max(np.abs(np.array(p)-q))),'same_top':int(np.argmax(a))==int(np.argmax(b))})
 return {'questions':rows,'max_abs_logit':max(x['max_abs_logit'] for x in rows),'max_abs_probability':max(x['max_abs_probability'] for x in rows),'all_tops_match':all(x['same_top'] for x in rows)}

def prepare_cases(official,processor,native,mode):
 require(native['status']=='complete' and len(native['cases'])==40 and native['question_count']==104,'native encoding incomplete')
 result=[]
 for index,row in enumerate(native['cases']):
  expected=f'text-{index:03}';require(row['id']==expected,'case order differs')
  raw=(NATIVE/(expected+'.request.json')).read_bytes();require(digest(raw)==row['request_sha256'],'request bytes changed')
  encoded=official.encode_record(processor.tokenizer,json.loads(raw),processor=processor)
  require(encoded.media is None and list(encoded.input_ids)==row['input_ids'],'native/official IDs or media differ')
  require(len(encoded.questions)==len(row['questions']),'native/official question counts differ')
  for q,want in zip(encoded.questions,row['questions']):
   actual={'id':q.question_id,'type':q.question_type,'question_span':list(q.question_span),'option_spans':[list(x) for x in q.option_spans],'option_ids':list(q.option_ids)}
   require(actual==want,'native/official spans/options differ')
  require(0<len(encoded.input_ids)<=1090,'unexpected suite length')
  if mode=='full' or expected=='text-014':result.append({'id':expected,'request_sha256':digest(raw),'record':encoded,'record_json':asdict(encoded)})
 require(len(result)==(40 if mode=='full' else 1),'selected cases differ')
 return result

def finish_head(torch,official,reader,config,case,hidden,head):
 norm=__import__('transformers.models.qwen3_5.modeling_qwen3_5',fromlist=['Qwen3_5RMSNorm']).Qwen3_5RMSNorm(5120,eps=config.rms_norm_eps)
 norm.weight=torch.nn.Parameter(torch.from_numpy(ordinary_norm_to_hf(reader.full('output_norm.weight'))),requires_grad=False)
 with torch.inference_mode():hidden=norm(hidden)
 encoded=case['record'];ids=list(encoded.input_ids)
 tokens=sorted({token for q in encoded.questions for first,end in q.option_spans for token in ids[first:end]})
 lexical=torch.from_numpy(reader.rows('output.weight',tokens));mapping={token:i for i,token in enumerate(tokens)};remapped=[mapping.get(token,0) for token in ids]
 for q in encoded.questions:
  for first,end in q.option_spans:require([tokens[x] for x in remapped[first:end]]==ids[first:end],'lexical remapping differs')
 record=official.EncodedRecord(input_ids=tuple(remapped),questions=encoded.questions,record_id=encoded.record_id)
 hidden_before=cpu_bytes(torch,hidden);lexical_before=cpu_bytes(torch,lexical)
 head_ids=torch.tensor([remapped],dtype=torch.long);mask=torch.ones((1,len(ids)),dtype=torch.long);ids_before=cpu_bytes(torch,head_ids);mask_before=cpu_bytes(torch,mask)
 with torch.inference_mode():logits=head(hidden,head_ids,mask,[record],lexical)[0]
 require(cpu_bytes(torch,hidden)==hidden_before and cpu_bytes(torch,lexical)==lexical_before and cpu_bytes(torch,head_ids)==ids_before and cpu_bytes(torch,mask)==mask_before,'CPU head mutated input bytes')
 values=[value.float().tolist() for value in logits];distributions(values)
 return hidden,{'logits':values,'probabilities':distributions(values),'hidden_sha256':digest(hidden_before),'lexical_sha256':digest(lexical_before),'option_token_ids':tokens}

def native_response(case,mode,native_logits):
 path=NATIVE/(mode+'-'+case['id']+'.response.json');response=json.loads(path.read_bytes())
 request=json.loads((NATIVE/(case['id']+'.request.json')).read_bytes());questions=case['record'].questions
 require(response['model']==request['model'] and set(response['answers'])==set(request['questions']),'native HTTP model/questions differ')
 usage=response['usage'];tokens=len(case['record'].input_ids)
 require(type(usage['input_tokens']) is int and usage['input_tokens']==tokens and type(usage['output_tokens']) is int and usage['output_tokens']==0 and type(usage['cached_input_tokens']) is int and usage['cached_input_tokens']==0,'native HTTP token/cache count differs')
 meta=response['psionic'];require(meta['prompt_tokens']==tokens and meta['truncated_state_tokens']==0 and meta['backend']=='metal' and meta['execution_mode']=='native' and meta['prefill_chunk']==512 and meta['accumulate']=='f32','native HTTP execution metadata differs')
 require(meta['artifact_digest']=='sha256:9997675e4c9e9016273372b5fdedbdda7ad14285bc722949e4b9daca62bf3822' and meta['head_digest']=='sha256:'+HEAD_SHA and meta['head_source']=='safetensors','native HTTP artifact differs')
 require(meta['timing']['cached'] is False and response.get('cached',False) is False,'native HTTP unexpectedly cached')
 # Native softmax subtracts the maximum in f32, then exp and sum in f64. This small
 # consistency bound covers floating-point arithmetic, not model acceptance.
 tolerance=1e-10
 def finite(value):return type(value) in (float,int) and math.isfinite(value)
 for q,values in zip(questions,native_logits):
  require(values and all(math.isfinite(value) for value in values),'invalid captured native logits')
  maximum=max(values);exps=[math.exp(struct.unpack('<f',struct.pack('<f',value-maximum))[0]) for value in values];total=sum(exps);probabilities=[value/total for value in exps]
  answer=response['answers'][q.question_id];require(answer['type']==q.question_type==request['questions'][q.question_id]['type'],'native HTTP answer type differs')
  options=list(q.option_ids);require(len(options)==len(probabilities),'native logits/options differ')
  if q.question_type=='noul':
   require(set(options)=={'false','true'} and finite(answer['noul']) and 0<=answer['noul']<=1 and abs(answer['noul']-probabilities[options.index('true')])<=tolerance,'native noul/logits disagree')
  else:
   p=answer['probabilities'];require(set(p)==set(options),'native probability keys differ')
   require(all(finite(p[key]) and 0<=p[key]<=1 and abs(p[key]-v)<=tolerance for key,v in zip(options,probabilities)) and abs(sum(p.values())-1)<=tolerance,'native HTTP probabilities/logits disagree')
   require(finite(answer['confidence']) and abs(answer['confidence']-max(p.values()))<=tolerance,'native confidence/probabilities disagree')
   if q.question_type=='choice':require(answer['choice'] in p and max(p.values())-p[answer['choice']]<=tolerance,'native selected answer disagrees')
   else:
    require(q.question_type=='score' and options==[str(i) for i in range(len(options))],'native score options differ')
    expected=sum(i*p[str(i)] for i in range(len(options)));require(finite(answer['score']) and abs(answer['score']-expected)<=tolerance*len(options),'native score/probabilities disagree')
 return {'response_sha256':sha(path),'contract_passed':True,'input_tokens':tokens,'questions':len(questions),'logit_probability_consistency_tolerance':tolerance}

def native_comparisons(case,mps_logits,cpu_logits):
 result={}
 for mode in ('staged','fused-fields'):
  path=NATIVE/mode/'rows.jsonl';rows=[json.loads(line) for line in path.read_text().split('\n') if line]
  selected=[r for r in rows if r['request_sha256']=='sha256:'+case['request_sha256']];selected.sort(key=lambda r:r['question_index'])
  require(len(selected)==len(case['record'].questions),'native capture question count differs')
  for i,(row,q) in enumerate(zip(selected,case['record'].questions)):
   require(row['question_index']==i and row['question']==q.question_id and row['rows'][2:]==['option:'+v for v in q.option_ids],'native capture ordering differs')
   require(row['psionic']['artifact_digest']=='sha256:9997675e4c9e9016273372b5fdedbdda7ad14285bc722949e4b9daca62bf3822' and row['psionic']['head_digest']=='sha256:'+HEAD_SHA,'native artifacts differ')
  native=[row['logits'] for row in selected]
  result[mode]={'capture_sha256':sha(path),'http':native_response(case,mode,native),'mps_vs_native':compare_logits(mps_logits,native)}
  if cpu_logits is not None:result[mode]['cpu_vs_native']=compare_logits(cpu_logits,native)
 return result

def full_acceptance(cases):
 require(len(cases)==40 and [x['id'] for x in cases]==[f'text-{i:03}' for i in range(40)],'full runtime request coverage differs')
 result={'requests':40,'questions':104,'probability_limit':.02,'all_tops_required':True,'modes':{}}
 for mode in ('staged','fused-fields'):
  rows=[q for case in cases for q in case['native_stage6_comparisons'][mode]['mps_vs_native']['questions']]
  require(len(rows)==104 and all(case['native_stage6_comparisons'][mode]['http']['contract_passed'] for case in cases),'full runtime question/HTTP coverage differs')
  maximum=max(row['max_abs_probability'] for row in rows);tops=sum(row['same_top'] for row in rows)
  result['modes'][mode]={'questions':len(rows),'top_matches':tops,'max_abs_probability':maximum,'passed':tops==104 and maximum<=.02}
 result['passed']=all(row['passed'] for row in result['modes'].values())
 return result


def main():
 p=argparse.ArgumentParser();p.add_argument('mode',choices=['pilot','full']);p.add_argument('--native-encoded',type=Path,required=True);p.add_argument('--output-dir',type=Path,required=True);a=p.parse_args()
 resources=set(os.environ.get('OPENAGENTS_LEASES','').split(','));require({'gpu','memory','disk'}<=resources and 'quiet' not in resources,'GPU/memory/disk leases required; quiet forbidden')
 require(os.environ.get('CLEF_SAME_Q4_MPS_GUARDED')=='1','owned guard required');require(not a.output_dir.exists(),'preserve prior output');a.output_dir.mkdir(parents=True)
 record={'schema':'clef.same-q4-independent-mps.v1','status':'validating','completed':False,'mode':a.mode,'trunk':'Pinned official Transformers decoder operators, independently decoded same GGUF f32 weights, one layer on MPS at a time','head':'Official external BF16 head weights loaded and executed on CPU in f32','native_encoded_sha256':sha(a.native_encoded),'cpu_pilot_limits':{'relative_l2':1e-4,'max_abs_logit':1e-3,'max_abs_probability':1e-4,'all_tops':True},'layers':[],'cases':[],'gpu_group_gib':12,'mps_gib':8,'cpu_threads':2};path=a.output_dir/'receipt.json';write(path,record);reader=None
 try:
  for name,want in PINS.items():require(sha(CPU/name)==want,'independent source changed')
  import torch,transformers
  from transformers.models.qwen3_5 import modeling_qwen3_5 as modeling
  from safetensors.torch import load_file,save_file
  require(str(torch.__version__)=='2.9.1' and transformers.__version__=='5.10.2','runtime versions differ')
  require(sha(HF/'config.json')=='c42e88892bd3fd84e8276b2ad90df58c1c3b797676ea161035006a72ad468c58' and sha(HF/'joint_head_config.json')=='890be585d75b981201eb96a35f98a9967afe37bc8d220cfdea2e72d56507534f','official configuration changed')
  require(sha(Path(modeling.__file__))==MODELING_SHA and sha(HF/'joint_schema_model.py')==SOURCE_SHA and sha(HF/'joint_head.safetensors')==HEAD_SHA,'official operators or head differ')
  require(modeling.FusedRMSNormGated is None and modeling.causal_conv1d_fn is None and modeling.chunk_gated_delta_rule is None and modeling.fused_recurrent_gated_delta_rule is None,'optional GPU operator changed reference')
  torch.set_num_threads(2);torch.set_num_interop_threads(1);torch.set_default_device('cpu');require(torch.backends.mps.is_available(),'MPS unavailable')
  torch.mps.set_per_process_memory_fraction(min(1.0,8*GIB/torch.mps.recommended_max_memory()))
  record['transport_self_test']=transport_test(torch);baseline=torch.mps.driver_allocated_memory();require(baseline<256*1024**2,'unexpected initial allocator baseline')
  spec=importlib.util.spec_from_file_location('independent_mps_official',HF/'joint_schema_model.py');official=importlib.util.module_from_spec(spec);sys.modules[spec.name]=official;spec.loader.exec_module(official)
  processor=transformers.AutoProcessor.from_pretrained(HF,local_files_only=True)
  cases=prepare_cases(official,processor,json.loads(a.native_encoded.read_text()),a.mode)
  record['inputs']=[{'id':x['id'],'request_sha256':x['request_sha256'],'record':x['record_json']} for x in cases];write(path,record)
  raw=json.loads((HF/'config.json').read_text())['text_config'];require(raw['num_hidden_layers']==64 and raw['hidden_size']==5120 and raw['intermediate_size']==17408,'architecture differs')
  config=modeling.Qwen3_5TextConfig.from_dict(raw);config._attn_implementation='eager';config.use_cache=False;config.dtype=torch.float32
  require(sha(CPU/'full-1/results/receipt.json')=='4bc4e7bc7732bdbb732889d5376dab1c887782a3434ec86620cbc0a666bc8fe5','saved CPU reference changed')
  reader=StreamingGguf();old=json.loads((CPU/'full-1/results/receipt.json').read_text());require(reader.state==old['gguf_file_state'],'GGUF differs from saved CPU reference');record['gguf_state']=reader.state;record['fixtures']=fixture_checks()
  adapter,record['negative_a_adapter'],adapter_source=make_negative_a_adapter(modeling);(a.output_dir/'negative_a_forward.py').write_text(adapter_source)
  states={x['id']:torch.from_numpy(reader.rows('token_embd.weight',list(x['record'].input_ids))).unsqueeze(0) for x in cases}
  record.update(status='running',started_at_unix=time.time());write(path,record)
  for index in range(64):
   reader.check_state();began=time.monotonic();layer,metrics=loaded_layer(torch,modeling,config,index,reader,adapter);row={'layer':index,**metrics,'requests':[]}
   for case in cases:
    started=time.monotonic();hidden,identity,auxiliary_hashes=forward_one(torch,modeling,config,layer,index,states[case['id']]);states[case['id']]=hidden
    evidence={'id':case['id'],'tokens':len(case['record'].input_ids),'hidden_sha256':identity,'seconds':time.monotonic()-started,'finite':True,'input_unchanged':True,'auxiliary_unchanged':True,'auxiliary_sha256':auxiliary_hashes}
    if a.mode=='pilot':
     require(sha(CPU/'full-1/results'/f'layer-{index:02}.safetensors')==old['layers'][index]['file_sha256'],'saved CPU layer changed')
     reference=load_file(str(CPU/'full-1/results'/f'layer-{index:02}.safetensors'),device='cpu')['hidden'];evidence['cpu_rows']=row_errors(hidden,reference);del reference
     save_file({'hidden':hidden.contiguous()},str(a.output_dir/f'layer-{index:02}.safetensors'))
    row['requests'].append(evidence)
    require(torch.mps.driver_allocated_memory()<=8*GIB,'MPS allocation crossed8GiB bound')
   require(layer_hashes(torch,layer,'mps')==metrics['parameter_hashes'],'layer reuse changed parameter bytes')
   del layer;gc.collect();torch.mps.synchronize();torch.mps.empty_cache();torch.mps.synchronize()
   row['driver_after_cleanup']=torch.mps.driver_allocated_memory();row['current_after_cleanup']=torch.mps.current_allocated_memory();require(row['driver_after_cleanup']<=baseline+256*1024**2,'MPS allocations did not return to bounded baseline')
   row['seconds']=time.monotonic()-began;row['payload_read_bytes']=reader.payload_read_bytes;record['layers'].append(row);reader.check_state();write(path,record);print(json.dumps({'layer':index,'seconds':row['seconds'],'driver_after_cleanup':row['driver_after_cleanup']}),flush=True)
  head=official.JointSchemaHead(**json.loads((HF/'joint_head_config.json').read_text()));head.load_state_dict(load_file(str(HF/'joint_head.safetensors'),device='cpu'),strict=True);head=head.to(device='cpu',dtype=torch.float32).eval()
  for case in cases:
   hidden,result=finish_head(torch,official,reader,config,case,states[case['id']],head);save_file({'hidden':hidden.contiguous()},str(a.output_dir/(case['id']+'-final.safetensors')))
   result.update(id=case['id'],request_sha256=case['request_sha256'],input_tokens=len(case['record'].input_ids))
   if a.mode=='pilot':
    cpu_hidden=load_file(str(CPU/'full-1/results/final-hidden.safetensors'),device='cpu')['hidden'];require(digest(cpu_bytes(torch,cpu_hidden))==old['hidden_sha256'],'saved CPU final rows changed');result['cpu_rows']=row_errors(hidden,cpu_hidden);result['cpu_logits']=old['logits'];result['cpu_comparison']=compare_logits(result['logits'],old['logits']);del cpu_hidden
    r=result['cpu_rows'];c=result['cpu_comparison'];result['acceleration_screen_passed']=r['relative_l2'] is not None and r['relative_l2']<=1e-4 and c['max_abs_logit']<=1e-3 and c['max_abs_probability']<=1e-4 and c['all_tops_match']
   result['native_stage6_comparisons']=native_comparisons(case,result['logits'],old['logits'] if a.mode=='pilot' else None)
   record['cases'].append(result);write(path,record)
  reader.check_state()
  if a.mode=='pilot':require(record['cases'][0]['acceleration_screen_passed'],'CPU full-runtime acceleration screen failed')
  record.update(status='complete',completed=True,execution_completed=True,head_device='cpu',payload_read_bytes=reader.payload_read_bytes)
  if a.mode=='full':
   record['runtime_acceptance']=full_acceptance(record['cases']);write(path,record)
   require(record['runtime_acceptance']['passed'],'frozen same-Q4 full runtime acceptance failed')
 except BaseException as error:record.update(status='failed',error_type=type(error).__name__,error=str(error));raise
 finally:
  if reader is not None:reader.close()
  write(path,record)

if __name__=='__main__':main()
