#!/usr/bin/env python3
"""Compare saved CPU/native layer rows and replay the complete official CPU head."""
import argparse,importlib.util,json,os,sys
from pathlib import Path
HERE=Path(__file__).resolve().parent;RUN=HERE.parent
sys.path.insert(0,str(RUN));import mps_reference as ref

def main():
 p=argparse.ArgumentParser();p.add_argument('--output-dir',type=Path,required=True);a=p.parse_args()
 leases=set(os.environ.get('OPENAGENTS_LEASES','').split(','));ref.require({'memory','disk'}<=leases and not {'gpu','build','quiet'}&leases and os.environ.get('CLEF_LAYER_COMPARE_GUARDED')=='1','CPU-only guarded comparison required')
 ref.require(not a.output_dir.exists(),'preserve previous output');a.output_dir.mkdir(parents=True)
 result={'schema':'clef.text029-layer-comparison.v1','status':'running','gpu_executed':False,'trunk_executed':False,'layers':[],'head_variants':[],'acceptance_scope':'Diagnostic localization only. The frozen full40/104 gate is unchanged.','observation_scope':'Native per-layer observer introduces waits. Full-memory official CPU head replay checks its final rows against the prior normal-request native raw logits, with the existing1e-3 raw-logit bound and exact top agreement.'};path=a.output_dir/'receipt.json';ref.write(path,result);reader=None
 try:
  import numpy as np,torch,transformers
  from safetensors.torch import load_file
  torch.set_num_threads(2);torch.set_num_interop_threads(1);torch.set_default_device('cpu')
  ref.require(str(torch.__version__)=='2.9.1' and transformers.__version__=='5.10.2','runtime versions differ')
  cdir=HERE/'cpu-1/results';ndir=HERE/'native-1/results';cpu=json.loads((cdir/'receipt.json').read_text());native=json.loads((ndir/'receipt.json').read_text())
  ref.require(cpu['completed'] and len(cpu['layers'])==64 and native['completed'] and len(native['files'])==65,'capture coverage differs')
  files={row['name']:row for row in native['files']}
  def native_rows(name):
   p=ndir/'rows'/name;ref.require(ref.sha(p)==files[name]['sha256'],'native row changed');values=torch.from_numpy(np.fromfile(p,dtype='<f4').reshape(1,359,5120));ref.require(torch.isfinite(values).all(),'nonfinite native rows');return values
  spans={'question':[192,194],'true_option':[205,223],'false_option':[229,247]}
  for index in range(64):
   row=cpu['layers'][index];ref.require(row['layer']==index,'CPU layers reordered');f=cdir/row['file'];ref.require(ref.sha(f)==row['file_sha256'],'CPU row file changed')
   c=load_file(str(f),device='cpu')['hidden'];n=native_rows(f'l_out-{index}.f32');ref.require(ref.digest(ref.cpu_bytes(torch,c))==row['hidden_sha256'],'CPU raw hidden changed');ref.require(tuple(c.shape)==(1,359,5120) and c.dtype==torch.float32 and bool(torch.isfinite(c).all()),'CPU layer shape/finiteness differs')
   entry={'layer':index,'type':'full_attention' if index%4==3 else 'delta','all_rows':ref.row_errors(n,c),'spans':{name:ref.row_errors(n[:,first:end],c[:,first:end]) for name,(first,end) in spans.items()}}
   result['layers'].append(entry);ref.write(path,result);del c,n
  full=json.loads((RUN/'mps-full-1/results/receipt.json').read_text());saved=next(row for row in full['cases'] if row['id']=='text-029');prior=next(row for row in full['inputs'] if row['id']=='text-029')
  ch=load_file(str(cdir/'final-hidden.safetensors'),device='cpu')['hidden'];nh=native_rows('result_norm.f32');mh=load_file(str(RUN/'mps-full-1/results/text-029-final.safetensors'),device='cpu')['hidden']
  ref.require(ref.digest(ref.cpu_bytes(torch,ch))==cpu['hidden_sha256'] and ref.digest(ref.cpu_bytes(torch,mh))==saved['hidden_sha256'],'final hidden identity differs')
  result['final_rows']={'native_vs_cpu':ref.row_errors(nh,ch),'native_vs_mps':ref.row_errors(nh,mh),'cpu_vs_mps':ref.row_errors(ch,mh)}
  ref.require(ref.sha(ref.HF/'joint_schema_model.py')==ref.SOURCE_SHA and ref.sha(ref.HF/'joint_head.safetensors')==ref.HEAD_SHA,'official head changed')
  spec=importlib.util.spec_from_file_location('layer_compare_official',ref.HF/'joint_schema_model.py');official=importlib.util.module_from_spec(spec);sys.modules[spec.name]=official;spec.loader.exec_module(official)
  processor=transformers.AutoProcessor.from_pretrained(ref.HF,local_files_only=True);raw=(ref.NATIVE/'text-029.request.json').read_bytes();record=official.encode_record(processor.tokenizer,json.loads(raw),processor=processor)
  ref.require(ref.digest(raw)==prior['request_sha256'] and json.loads(json.dumps(ref.asdict(record)))==prior['record'],'exact record differs')
  ids=list(record.input_ids);ref.require(len(ids)==359 and record.questions[1].question_id=='日本1' and record.questions[1].option_ids[1]=='false','diagnostic coordinates differ')
  q=record.questions[1];ref.require(spans=={'question':list(q.question_span),'true_option':list(q.option_spans[q.option_ids.index('true')]),'false_option':list(q.option_spans[q.option_ids.index('false')])},'hardcoded diagnostic spans differ from exact official record')
  reader=ref.StreamingGguf();ref.require(reader.state==full['gguf_state'],'same-GGUF file changed');tokens=sorted({token for q in record.questions for first,end in q.option_spans for token in ids[first:end]});lexical=torch.from_numpy(reader.rows('output.weight',tokens));ref.require(ref.digest(ref.cpu_bytes(torch,lexical))==saved['lexical_sha256']==cpu['lexical_sha256'],'lexical rows changed')
  mapping={token:i for i,token in enumerate(tokens)};remapped=[mapping.get(token,0) for token in ids]
  for q in record.questions:
   for first,end in q.option_spans:ref.require([tokens[i] for i in remapped[first:end]]==ids[first:end],'lexical remapping differs')
  hr=official.EncodedRecord(input_ids=tuple(remapped),questions=record.questions,record_id=record.record_id);hids=torch.tensor([remapped],dtype=torch.long);mask=torch.ones((1,len(ids)),dtype=torch.long)
  head=official.JointSchemaHead(**json.loads((ref.HF/'joint_head_config.json').read_text()));head.load_state_dict(load_file(str(ref.HF/'joint_head.safetensors'),device='cpu'),strict=True);head=head.float().eval()
  captures={}
  for mode in ('staged','fused-fields'):
   rows=[json.loads(x) for x in (ref.NATIVE/mode/'rows.jsonl').read_text().splitlines() if x];rows=sorted((r for r in rows if r['request_sha256']=='sha256:'+prior['request_sha256']),key=lambda r:r['question_index']);ref.require(len(rows)==3,'native captured question count differs');captures[mode]=rows
  for name,hidden,expected in [('cpu',ch,cpu['logits']),('mps',mh,saved['logits']),('native_observer',nh,None)]:
   before={k:ref.cpu_bytes(torch,v) for k,v in [('hidden',hidden),('lexical',lexical),('ids',hids),('mask',mask)]}
   # Each saved array already includes the backbone output RMSNorm. Do not apply it again.
   with torch.inference_mode():outputs=head(hidden,hids,mask,[hr],lexical)[0];normalized=head.hidden_norm(hidden)
   values=[v.tolist() for v in outputs]
   exact=all(torch.equal(v,torch.tensor(w,dtype=torch.float32)) for v,w in zip(outputs,expected)) if expected is not None else None
   if expected is not None:ref.require(exact,'unchanged complete CPU head replay differs')
   pools=[]
   for q in record.questions:
    with torch.inference_mode():pools.append(torch.stack([normalized[0,-1],head._mean_span(normalized[0],q.question_span),*(head._mean_span(normalized[0],span) for span in q.option_spans)]))
   comparisons={};pool_comparisons={}
   for mode,rows in captures.items():
    ref.native_response({'id':'text-029','record':record},mode,[r['logits'] for r in rows])
    comparisons[mode]=ref.compare_logits(values,[r['logits'] for r in rows]);pc=[]
    for q,row,pool in zip(record.questions,rows,pools):
     ref.require(row['question']==q.question_id and row['rows'][2:]==['option:'+v for v in q.option_ids],'native pool ordering differs')
     with (ref.NATIVE/mode/'rows.f16').open('rb') as f:f.seek(row['offset']);raw_pool=f.read(len(row['rows'])*5120*2)
     saved_pool=torch.from_numpy(np.frombuffer(raw_pool,dtype='<f2').astype(np.float32).copy().reshape(-1,5120));pc.append({'question':q.question_id,'metrics':ref.row_errors(pool,saved_pool),'f16_rounded_bytes_equal':pool.half().numpy().astype('<f2').tobytes()==raw_pool})
    pool_comparisons[mode]=pc
   ref.require(all(ref.cpu_bytes(torch,v)==before[k] for k,v in [('hidden',hidden),('lexical',lexical),('ids',hids),('mask',mask)]),'head replay changed inputs')
   entry={'name':name,'logits':values,'probabilities':ref.distributions(values),'saved_reference_replay_exact':exact,'normal_request_comparisons':comparisons,'pooled_normal_request_comparisons':pool_comparisons};result['head_variants'].append(entry);ref.write(path,result)
  observer=result['head_variants'][-1]['normal_request_comparisons'];result['observer_consistency_screen']={'raw_logit_limit':1e-3,'all_tops_required':True,'passed':all(x['max_abs_logit']<=1e-3 and x['all_tops_match'] for x in observer.values()),'modes':observer}
  reader.check_state();result.update(status='complete',completed=True,all64layers=True,unchanged_reference_heads_exact=True)
 except BaseException as error:result.update(status='failed',error_type=type(error).__name__,error=str(error));raise
 finally:
  if reader is not None:reader.close()
  ref.write(path,result)

if __name__=='__main__':main()
