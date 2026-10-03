#!/usr/bin/env python3
"""Independent descriptive recomputation; imports no experiment/report modules."""
from pathlib import Path
from decimal import Decimal
from collections import Counter,defaultdict
import json,hashlib,math
ROOT=Path(__file__).resolve().parent
DATA=ROOT/'collected'
FILES={}
def read(path,jsonl=False):
 raw=path.read_bytes(); FILES[str(path.relative_to(ROOT))]=hashlib.sha256(raw).hexdigest()
 return [json.loads(x) for x in raw.splitlines() if x.strip()] if jsonl else json.loads(raw)
def digest(path):
 return hashlib.sha256(path.read_bytes()).hexdigest()
def eq(a,b):
 assert math.isclose(float(a),float(b),rel_tol=1e-11,abs_tol=1e-10),(a,b)
PRICE={'claude-sonnet-5-5':{'input':2,'output':10,'cache_write_5m':2.5,'cache_write_1h':4,'cache_read':.2},'claude-haiku-4-5':{'input':1,'output':5,'cache_write_5m':1.25,'cache_write_1h':2,'cache_read':.1},'claude-haiku-4-5-20251001':{'input':1,'output':5,'cache_write_5m':1.25,'cache_write_1h':2,'cache_read':.1}}
FIELDS={'input_tokens':'input','output_tokens':'output','cache_write_5m_tokens':'cache_write_5m','cache_write_1h_tokens':'cache_write_1h','cache_read_input_tokens':'cache_read'}
def ledger(directory):
 records=read(directory/'provider-calls.jsonl',True); config=read(directory/'provider-config.json')
 by=defaultdict(dict); phases=Counter()
 for rec in records:
  phase=rec['phase'];phases[phase]+=1
  assert rec['call_id'] not in by or phase not in by[rec['call_id']], 'duplicate phase/call'
  by[rec['call_id']][phase]=rec
 assert phases['refused']==0
 total=Decimal(0);use=Counter();served=Counter();individual=[]
 for cid,rs in by.items():
  assert set(rs)=={'admitted','finished'},set(rs)
  a,f=rs['admitted'],rs['finished']; assert f['status']=='complete'
  if f['usage_status']=='token_count_only':
   assert f['cost_basis']=='non_generation_endpoint' and f['path'].split('?')[0]=='/v1/messages/count_tokens' and f['cost_usd']==0 and f['usage']=={}
   individual.append({'call_id':cid,'model':None,'cost_usd':0,'non_generation':True});continue
  assert f['usage_status']=='reported'
  assert a['model']==f['requested_model']==f['served_model']
  model=f['served_model'];rates=PRICE[model]
  assert config['models'][model]['usd_per_million']==rates
  assert f.get('unknown_reservations_usd',0)==0
  assert not any(f.get('server_tool_use',{}).values())
  u=f['usage'];assert u['cache_creation_input_tokens']==u['cache_write_1h_tokens']+u['cache_write_5m_tokens']
  assert all(type(v)is int and v>=0 for v in u.values())
  cost=sum(Decimal(u[k])*Decimal(str(rates[v])) for k,v in FIELDS.items())/Decimal(1000000)
  eq(cost,f['cost_usd']);total+=cost;use.update(u);served[model]+=1
  individual.append({'call_id':cid,'model':model,'cost_usd':float(cost)})
 return {'cost_usd':float(total),'usage':dict(use),'served_models':dict(served),'calls':len(by),'record_count':len(records),'individual':individual}
plan=read(DATA/'plan/plan.json');panel=read(DATA/'panel/panel.json');report=read(ROOT/'report/report.json')
assert panel['status']=='complete' and panel['plan_sha256']==digest(DATA/'plan/plan.json')
assert len(plan['schedule'])==len(panel['attempts'])==12
assert len({x['run_id'] for x in plan['schedule']})==12
assert {x.name for x in (DATA/'runs').iterdir() if x.is_dir()}=={x['run_id'] for x in plan['schedule']}
expected=[('alternative-beta',1,'bare'),('alternative-beta',1,'deterministic'),('alternative-beta',1,'jev'),('alternative-gamma',1,'jev'),('alternative-gamma',1,'bare'),('alternative-gamma',1,'deterministic'),('alternative-beta',2,'deterministic'),('alternative-beta',2,'jev'),('alternative-beta',2,'bare'),('alternative-gamma',2,'bare'),('alternative-gamma',2,'deterministic'),('alternative-gamma',2,'jev')]
rows=[]
for i,(e,attempt) in enumerate(zip(plan['schedule'],panel['attempts'])):
 assert e['position']==i+1 and (e['task_id'],e['repetition'],e['arm'])==expected[i]
 assert all(e[k]==attempt[k] for k in ['position','run_id','task_id','arm','repetition'])
 d=DATA/'runs'/e['run_id'];p=read(d/'pilot.json');n=read(d/'native/result.json');c=read(d/'acceptance/checks.json');cfg=read(DATA/'plan'/f"{e['run_id']}.json");prep=read(d/'preparation/preparation.json')
 assert digest(d/'pilot.json')==attempt['receipt_sha256']
 assert digest(DATA/'plan'/f"{e['run_id']}.json")==e['config']['sha256']==p['config_sha256']
 for name,r in p['artifacts'].items(): assert digest(d/r['path'])==r['sha256']
 assert c['candidate_manifest_sha256']==p['candidate_manifest_sha256']==n['candidate_manifest_sha256']
 assert c['source_commit']==n['source_commit']==p['source_commit']
 assert c['source_archive_sha256']==n['source_archive_sha256']==cfg['native_template']['source_archive_sha256']
 assert c['checker_sha256']==cfg['acceptance_template']['checker']['sha256']
 assert c['target_seed_manifest_sha256']==n['target_seed_manifest_sha256']==cfg['native_template']['target_seed_manifest_sha256']
 assert c['snapshot_commit']==n['snapshot_commit']
 assert n['cli_sha256']==n['cli_hash_after']==cfg['native_template']['binary_sha256']
 assert n['model']==cfg['native_template']['model']=='claude-sonnet-5-5' and n['effort']==cfg['native_template']['effort']=='medium'
 assert p['execution_closed'] and c['execution_closed'] and n['execution_closed'] and p['safe_to_continue']
 assert all(x['removed'] for x in p['cleanup']) and p['native_scratch_release']['status']=='complete'
 l=ledger(d/'native');eq(l['cost_usd'],n['provider_cost_usd']);eq(l['cost_usd'],p['native_cost_lower_usd']);eq(l['cost_usd'],p['native_cost_upper_usd']);eq(l['cost_usd'],n['cost_usd'])
 j=0.0;gateway=None
 if e['arm']=='jev':
  g=read(d/'preparation/gateway-call/receipt.json');response=read(d/'preparation/gateway-call/response.json')
  assert g['outcome']=='answered' and g['answers_valid'] and g['attempts']==1 and g['cost_status']=='gateway_reported'
  assert g['request_sha256']==digest(d/'preparation/gateway-call/request.json') and g['response_sha256']==digest(d/'preparation/gateway-call/response.json')
  j=float(Decimal(response['provider_metadata']['gateway']['cost']));eq(j,g['cost_usd']);eq(j,prep['cost_usd'])
  gateway={'calls':g['attempts'],'cost_usd':j,'wall_s':g['wall_s'],'internal_attempts':g['gateway_metadata']['routing']['totalProviderAttemptCount']}
 else: assert prep['cost_usd']==0
 cost=l['cost_usd']+j;eq(cost,p['cost_lower_usd']);eq(cost,p['cost_upper_usd'])
 accepted=c['completed'] and c['execution_closed'] and all(c[k]['passed'] is True for k in ['scope','format','ordinary','independent']) and p['accounting_complete'] and p['execution_closed']
 assert accepted==p['accepted']==c['accepted']
 wall=p['endpoint_wall_s'];checks=p['checks_endpoint_wall_s'];assert wall>=checks
 phases={k:v['wall_s'] for k,v in p['phases'].items()};phases['native_scratch_release']=p['native_scratch_release']['wall_s']
 row={k:e[k] for k in ['position','run_id','task_id','repetition','arm']}
 row.update(cost_usd=cost,native_cost_usd=l['cost_usd'],jev_cost_usd=j,endpoint_wall_s=wall,checks_endpoint_wall_s=checks,accepted=accepted,model_completed=n['model_completed'],checks={k:c[k]['passed'] for k in ['scope','format','ordinary','independent']},usage=l['usage'],provider_calls=l['calls'],served_models=l['served_models'],gateway=gateway,phases=phases,endpoint_minus_phase_sum_s=wall-math.fsum(phases.values()))
 rr=next(r for r in report['rows'] if r['run_id']==e['run_id']);eq(cost,rr['cost_usd']);eq(wall,rr['endpoint_wall_s']);assert rr['accepted']==accepted
 rows.append(row)
def aggregate(rs):
 return {'attempts':len(rs),'accepted':sum(r['accepted'] for r in rs),'model_completed':sum(r['model_completed'] for r in rs),'cost_usd':math.fsum(r['cost_usd'] for r in rs),'native_cost_usd':math.fsum(r['native_cost_usd'] for r in rs),'jev_cost_usd':math.fsum(r['jev_cost_usd'] for r in rs),'endpoint_total_s':math.fsum(r['endpoint_wall_s'] for r in rs),'mean_endpoint_s':math.fsum(r['endpoint_wall_s'] for r in rs)/len(rs),'checks_endpoint_total_s':math.fsum(r['checks_endpoint_wall_s'] for r in rs),'provider_calls':sum(r['provider_calls'] for r in rs),'usage':dict(sum((Counter(r['usage']) for r in rs),Counter()))}
arms={a:aggregate([r for r in rows if r['arm']==a]) for a in ['bare','deterministic','jev']}
per_task={task:{a:aggregate([r for r in rows if r['task_id']==task and r['arm']==a]) for a in arms} for task in sorted({r['task_id'] for r in rows})}
comparisons={}
for a,b in [('jev','bare'),('jev','deterministic'),('deterministic','bare')]:
 pairs=[]
 for task in per_task:
  for rep in [1,2]:
   x=next(r for r in rows if (r['arm'],r['task_id'],r['repetition'])==(a,task,rep));y=next(r for r in rows if (r['arm'],r['task_id'],r['repetition'])==(b,task,rep))
   pairs.append({'task':task,'repetition':rep,'cost_difference_usd':x['cost_usd']-y['cost_usd'],'time_difference_s':x['endpoint_wall_s']-y['endpoint_wall_s'],'cost_ratio':x['cost_usd']/y['cost_usd'],'time_ratio':x['endpoint_wall_s']/y['endpoint_wall_s'],'both_lower':x['cost_usd']<y['cost_usd'] and x['endpoint_wall_s']<y['endpoint_wall_s']})
 value={'ratio_summed_task_mean_cost':sum(v[a]['cost_usd']/v[a]['attempts'] for v in per_task.values())/sum(v[b]['cost_usd']/v[b]['attempts'] for v in per_task.values()),'ratio_summed_task_mean_time':sum(v[a]['mean_endpoint_s'] for v in per_task.values())/sum(v[b]['mean_endpoint_s'] for v in per_task.values()),'pairs':pairs,'joint_wins':sum(r['both_lower'] for r in pairs)}
 reported=report['comparisons'][a+'/'+b];eq(value['ratio_summed_task_mean_cost'],reported['aggregate_ratio_of_summed_task_means']['cost']);eq(value['ratio_summed_task_mean_time'],reported['aggregate_ratio_of_summed_task_means']['time']);assert value['joint_wins']==reported['lower_cost_and_time_pairs']
 comparisons[a+'/'+b]=value
for a in arms:
 eq(arms[a]['cost_usd'],report['arms'][a]['cost_usd']);eq(arms[a]['mean_endpoint_s'],report['arms'][a]['mean_endpoint_wall_s']);assert arms[a]['accepted']==report['arms'][a]['accepted']
gates={'all_12_valid':True,'jev_all_four':arms['jev']['accepted']==4,'jev_no_fewer':all(arms['jev']['accepted']>=arms[a]['accepted'] for a in ['bare','deterministic'])}
for b in ['bare','deterministic']:
 comp=comparisons['jev/'+b];gates['cost_10pct_vs_'+b]=comp['ratio_summed_task_mean_cost']<=.9;gates['time_10pct_vs_'+b]=comp['ratio_summed_task_mean_time']<=.9;gates['joint3_vs_'+b]=comp['joint_wins']>=3
assert all(gates.values())==report['directional_pilot_win']
probe=ledger(DATA/'probe');pn=read(DATA/'probe/result.json');eq(probe['cost_usd'],pn['provider_cost_usd'])
seeds={p.parent.name:read(p) for p in (DATA/'seeds').glob('*/result.json')}
result={'schema':'openagents.jev-native-independent-recompute.v1','method':'stdlib only; unique admitted/finished call IDs, Decimal token-price recomputation and one gateway cost per Jev call; no reporter imports','report_sha256':digest(ROOT/'report/report.json'),'plan_sha256':digest(DATA/'plan/plan.json'),'panel_sha256':digest(DATA/'panel/panel.json'),'rows':rows,'arms':arms,'per_task':per_task,'comparisons':comparisons,'gates':gates,'directional_win':all(gates.values()),'totals':aggregate(rows),'separate_setup':{'probe_cost_usd':probe['cost_usd'],'probe_retained_wall_s':pn['total_retained_wall_s'],'seed_total_wall_s':{k:v['total_wall_s'] for k,v in seeds.items()},'seed_build_wall_s':{k:v['build_wall_s'] for k,v in seeds.items()},'other_exclusions':['Initial index construction, archive provisioning, tool installation, engineering/machine costs and separate advisory post hoc calls excluded from panel comparisons. Not all these have monetary receipts here.']},'limits':['Quality here means original frozen checks only, not exhaustive correctness or post hoc diagnostics.','Two exposed tasks, two repetitions each; no generalization or causal quality estimate.','Time uses retained coordinator monotonic receipts, checked against bound source boundaries; no reconstruction of an independent hardware clock.','Provider-priced token cost is not a subscription invoice. Final receipt serialization and serial driver overhead excluded from primary endpoint.'],'input_sha256':FILES}
(ROOT/'independent-numeric-audit.json').write_text(json.dumps(result,indent=2,sort_keys=True)+'\n')
print(json.dumps({k:result[k] for k in ['report_sha256','arms','comparisons','gates','directional_win','totals','separate_setup']},indent=2))
