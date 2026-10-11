#!/usr/bin/env python3
"""Require complete fixed coverage before reporting paired GPU command times."""
import argparse
from collections import Counter
import json,math,statistics
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('run',type=Path);a=p.parse_args()
receipt=json.loads((a.run/'receipt.json').read_text())
rows=[json.loads(x) for x in (a.run/'stdout.jsonl').read_text().splitlines() if x.strip()]
bench=receipt.get('mode')=='bench';errors=[]
checks=[x for x in rows if x.get('kind') in {'small_correctness','weighted_correctness'}]
complete=[x for x in rows if x.get('kind')=='complete']
expectations=[x for x in rows if x.get('kind')=='expectations_checked']
timings=[x for x in rows if x.get('kind')=='timing']
scopes=['gate_up','activation','down_accumulate','full_ffn_accumulate']
small={(n,f,h,acc) for n,f,h in [(1,1,1),(3,7,127),(7,129,257),(63,1,4096),(64,128,65),(65,129,257),(257,1,12288),(1,33,4096),(33,1,12288)] for acc in [False,True]}
weighted={(65,False),(65,True),(512,True),(1024,True),(2048,True)}
expected_keys={(n,r,s,c) for n in [512,1024,2048] for r in range(5) for s in range(4) for c in scopes} if bench else set()
def finite(v):return isinstance(v,(int,float)) and math.isfinite(v)
def unique(actual,want):return len(actual)==len(want) and set(actual)==want
def comparison(v):
 common=v.get('passed') is True and v.get('criterion_version')==2 and v.get('criterion_violations')==0 and finite(v.get('max_abs')) and v['max_abs']>=0 and v.get('values',0)>0 and not v.get('first_failures')
 return common and ((v.get('half') is True and v.get('half_max_ulp',-1)>=0 and v.get('half_above_one_ulp',-1)>=0) or (v.get('half') is False and v.get('above_0_001')==0 and v['max_abs']<=.001))
def oracle(v,count,all_outputs):
 return v.get('passed') is True and v.get('all_outputs') is all_outputs and v.get('sample_count')==count and all(finite(v.get(k)) and 0<=v[k]<=.001 for k in ['baseline_max_abs','candidate_max_abs'])
def gate(x,all_outputs):
 if set(x.get('comparisons',{}))!={'gate','up','activation','down'} or not all(comparison(v) for v in x['comparisons'].values()):return False
 if set(x.get('f64_oracles',{}))!={'gate','up','down'}:return False
 for f,v in x['f64_oracles'].items():
  count=x['n']*(x['k'] if f=='down' else x['m']) if all_outputs else 96
  if not oracle(v,count,all_outputs):return False
 return True
if receipt.get('mode') not in {'check','bench'} or receipt.get('status')!='complete' or receipt.get('exit')!=0 or receipt.get('memory',{}).get('owned_group_stopped') is not True:errors.append('owned runtime did not finish cleanly')
if len(complete)!=1 or complete[0].get('passed') is not True or complete[0].get('timing_included') is not bench or complete[0].get('commands')!=(292 if bench else 46) or complete[0].get('dispatches')!=(602 if bench else 161):errors.append('completion/count mismatch')
if not unique([(x['n'],x['m'],x['k'],x['accumulate']) for x in checks if x['kind']=='small_correctness'],small):errors.append('small coverage mismatch')
if not unique([(x['n'],x['accumulate']) for x in checks if x['kind']=='weighted_correctness'],weighted):errors.append('weighted coverage mismatch')
for x in checks:
 if x.get('passed') is not True or x.get('guards_and_inputs_unchanged') is not True or x.get('padding_checked') is not True or not gate(x,x['kind']=='small_correctness'):errors.append('initial numerical/oracle/padding check failed')
if not unique([x['n'] for x in expectations],{512,1024,2048} if bench else set()):errors.append('expectation coverage mismatch')
for x in expectations:
 if x.get('passed') is not True or not gate(x,False) or len(x.get('baseline_sha256',[]))!=3 or len(x.get('candidate_sha256',[]))!=3:errors.append('expectation gate failed')
if not unique([(x['n'],x['round'],x['slot'],x['scope']) for x in timings],expected_keys):errors.append('timing coverage mismatch')
for x in timings:
 expected={'gate_up':{'projections'},'activation':{'activation','source_projections'},'down_accumulate':{'down'},'full_ffn_accumulate':{'projections','activation','down'}}[x['scope']]
 if x.get('variant')!=['baseline','interleaved','interleaved','baseline'][x['slot']] or x.get('output_gate_passed') is not True or set(x.get('repeats',{}))!=expected or not all(v.get('passed') is True and v.get('bitwise_equal') is True and type(v.get('bytes')) is int and v['bytes']>0 for v in x['repeats'].values()) or not all(finite(x.get(k)) and x[k]>0 for k in ['gpu_ms','wall_ms']):errors.append('timing repeat, ordering or timestamp failed')
ulp=[x for x in rows if x.get('kind')=='half_ulp_self_test']
if len(ulp)!=1 or ulp[0].get('passed') is not True or ulp[0].get('cases')!=19 or ulp[0].get('criterion_version')!=2:errors.append('half ULP check missing')
packing=[x for x in rows if x.get('kind')=='packing']
if len(packing)!=10 or not all(x.get('passed') is True and x.get('validated_half_values')==x['packed_rows']*x['h'] and x.get('zero_rows')==x['packed_rows']-2*x['f'] for x in packing):errors.append('weight packing evidence incomplete')
if len(complete)==1 and (complete[0].get('explicit_peak_bytes',2**64)>1024**3 or complete[0].get('combined_driver_and_expected_peak_bytes',2**64)>1536*1024**2):errors.append('allocation cap exceeded')
result={'coverage_and_correctness_passed':not errors,'validation_errors':sorted(set(errors)),'checks':len(checks),'timing_rows':len(timings),'scope':'fixed real Flash weights with synthetic inputs; shared-machine microbenchmark, not model latency','conditions':'five ABBA rounds; temporal drift and cache effects remain possible','by_tokens':[]}
if not errors and bench:
 lookup={(x['n'],x['round'],x['slot'],x['scope']):x['gpu_ms'] for x in timings}
 for n in [512,1024,2048]:
  item={'n':n,'scopes':{}}
  for scope in scopes+['projection_sum']:
   def val(r,s):return sum(lookup[n,r,s,c] for c in ['gate_up','down_accumulate']) if scope=='projection_sum' else lookup[n,r,s,scope]
   ratios=[val(r,b)/val(r,a) for r in range(5) for a,b in [(0,1),(3,2)]]
   item['scopes'][scope]={'baseline_median_gpu_ms':statistics.median(val(r,s) for r in range(5) for s in [0,3]),'interleaved_median_gpu_ms':statistics.median(val(r,s) for r in range(5) for s in [1,2]),'paired_interleaved_over_baseline_median':statistics.median(ratios),'paired_ratios':ratios}
  result['by_tokens'].append(item)
 result['material_full_ffn_gain_all_sizes']=all(all(x['scopes'][s]['paired_interleaved_over_baseline_median']<=.90 for s in ['projection_sum','full_ffn_accumulate']) for x in result['by_tokens'])
(a.run/'summary.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result,indent=2))
raise SystemExit(0 if not errors else 1)
