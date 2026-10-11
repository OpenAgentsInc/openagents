#!/usr/bin/env python3
"""Check saved HTTP metadata and the frozen aggregate gate without models."""
import ast
import copy
import hashlib
import json
import math
import struct
from pathlib import Path
from types import SimpleNamespace

HERE=Path(__file__).resolve().parent
NATIVE=HERE.parent/'27b-head-stage6-combined'
HEAD_SHA='a010ac04f078e699988e4049cbea5e62c962393f59fec366640b64e8d69a4953'
def require(value,message):
 if not value:raise ValueError(message)
def sha(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()
def distributions(logits):
 result=[]
 for row in logits:
  require(row and all(math.isfinite(x) for x in row),'invalid logits')
  exp=[math.exp(x-max(row)) for x in row];total=sum(exp);result.append([x/total for x in exp])
 return result
source=ast.parse((HERE/'mps_reference.py').read_text())
selected=[x for x in source.body if isinstance(x,ast.FunctionDef) and x.name in ('native_response','full_acceptance')]
exec(compile(ast.Module(body=selected,type_ignores=[]),'mps_reference.py','exec'))
records=[]
for index in range(40):
 name=f'text-{index:03}';raw=(NATIVE/(name+'.request.json')).read_bytes();request=json.loads(raw)
 questions=[SimpleNamespace(question_id=k,question_type={'noul':0,'choice':1,'score':2}[v['type']],option_ids=(['true','false'] if v['type']=='noul' else sorted(v['criteria']) if v['type']=='choice' else [str(i) for i in range(len(v['criteria']))])) for k,v in request['questions'].items()]
 # Captured prompt length is checked independently against the real encoder later.
 tokens=json.loads((NATIVE/('staged-'+name+'.response.json')).read_bytes())['usage']['input_tokens']
 case={'id':name,'record':SimpleNamespace(input_ids=[0]*tokens,questions=questions)}
 evidence={'id':name,'native_stage6_comparisons':{}}
 for mode in ('staged','fused-fields'):
  rows=[json.loads(x) for x in (NATIVE/mode/'rows.jsonl').read_text().splitlines()];rows=[x for x in rows if x['request_sha256']=='sha256:'+hashlib.sha256(raw).hexdigest()];rows.sort(key=lambda x:x['question_index'])
  require(len(rows)==len(questions),'capture count differs');logits=[x['logits'] for x in rows]
  http=native_response(case,mode,logits)
  evidence['native_stage6_comparisons'][mode]={'http':http,'mps_vs_native':{'questions':[{'max_abs_probability':0.,'same_top':True} for _ in questions]}}
 records.append(evidence)
require(full_acceptance(records)['passed'],'identical test gate failed')
wrong=copy.deepcopy(records);wrong[0]['native_stage6_comparisons']['staged']['mps_vs_native']['questions'][0]['same_top']=False
require(not full_acceptance(wrong)['passed'],'different top incorrectly passed')
wrong=copy.deepcopy(records);wrong[-1]['native_stage6_comparisons']['fused-fields']['mps_vs_native']['questions'][-1]['max_abs_probability']=.020000001
require(not full_acceptance(wrong)['passed'],'excess probability error incorrectly passed')
refused=0
for variant in (records[:-1],records[::-1]):
 try:full_acceptance(variant)
 except ValueError:refused+=1
require(refused==2,'incomplete/reordered coverage accepted')
print(json.dumps({'status':'passed','saved_http_responses':80,'questions_per_mode':104,'aggregate_failure_checks':4,'models_or_gpu_executed':False,'limits':'HTTP token counts are capture-derived here; pilot requires independent native encoder IDs/counts. Synthetic zero errors only test gate logic.','source_sha256':sha(HERE/'mps_reference.py')},indent=2))
