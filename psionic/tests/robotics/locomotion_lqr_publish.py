#!/usr/bin/env python3
"""Publish cached LQR observations only; no product or reference math calls."""
import argparse
import csv
import hashlib
import json
import math
from pathlib import Path
import struct

ROOT=Path(__file__).resolve().parents[3]
SOURCE=Path(__file__).resolve()


def need(ok,text):
    if not ok:raise ValueError(text)


def identity(path):
    raw=Path(path).read_bytes();return {'bytes':len(raw),'sha256':hashlib.sha256(raw).hexdigest()}


def load(path):return json.loads(Path(path).read_text())
def write(path,obj):Path(path).write_text(json.dumps(obj,indent=2,allow_nan=False)+'\n')


def publish(run,output):
    need(not output.exists(),'immutable publication target must be fresh')
    top=load(run/'record.json');need(top['schema']=='psionic.locomotion-lqr.learning.v1' and top['passed'] is True and top['lifecycle_state']=='complete','completed FIRST frozen study')
    need((len(top['chunks']),len(top['replays']),len(top['evaluations']),len(top['completed_actions']))==(24,3,4,31),'exact frozen action counts')
    output.mkdir(parents=True);(output/'audits').mkdir()
    audits=[];curves=[];seed_results={};baseline=None;totals={'canonical_plant_steps':0,'canonical_AdamW_updates':0,'canonical_coefficient_observations':0,'replay_plant_steps':0,'replay_AdamW_updates':0,'replay_coefficient_observations':0,'deterministic_eval_steps':0,'projected_updates':0,'ppo_clipped_samples':0}
    numeric_max={};lineage=[]
    for group in ['chunks','replays','evaluations']:
        for item in top[group]:
            audit_path=run/item['audit']['directory']/'record.json'
            need(identity(audit_path)==item['audit']['record'],'original independent receipt identity')
            audit=load(audit_path);need(audit['passed'] is True,'completed independent action audit');result=audit['result']
            need(audit['inputs']['descriptor']==top['descriptor'],'same sealed execution context')
            if group=='chunks':
                seed,first,count=item['seed'],item['first_batch'],item['batches'];label=f'canonical-seed-{seed}-batch-{first:03d}'
                need((result['seed'],result['first_batch'],result['batches'])==(seed,first,count),'canonical receipt labels')
                need(result['plant_steps']==count*512 and result['AdamW_updates']==count*4,'canonical semantic counts')
                for key in ['plant_steps','AdamW_updates','coefficient_observations']:totals['canonical_'+key]+=result[key]
                totals['projected_updates']+=result['projected_updates'];totals['ppo_clipped_samples']+=result['ppo_clipped_samples']
                for key,value in result['maximum_absolute_errors'].items():
                    need(math.isfinite(value),'finite cached error metric');numeric_max[key]=max(numeric_max.get(key,0.),value)
                path=run/item['directory']/'iterations.bin';need(identity(path)==item['artifacts']['iterations.bin']==audit['inputs']['iterations'],'immutable captured curve source')
                raw=path.read_bytes();need(len(raw)==48+count*62696,'captured curve extent')
                for j in range(count):
                    at=48+j*62696;rollout_K=struct.unpack_from('<d',raw,at+24)[0];K,m,v,step,skip=struct.unpack_from('<3d2q',raw,at+62256)
                    epochs=[struct.unpack_from('<3d2q3d4q',raw,at+61872+e*96) for e in range(4)]
                    values=[rollout_K,K,m,v,epochs[-1][5],epochs[-1][6]];need(all(math.isfinite(x) for x in values),'finite cached curve fields')
                    curves.append({'seed':seed,'completed_batch':first+j+1,'rollout_K':rollout_K,'K_after':K,'moment_m_after':m,'moment_v_after':v,'optimizer_step':step,'skipped':skip,'last_epoch_loss_before_update':epochs[-1][5],'last_epoch_gradient_before_update':epochs[-1][6],'projected_updates':sum(e[8] for e in epochs),'ppo_clipped_samples':sum(e[11] for e in epochs)})
                lineage.append({'directory':item['directory'],'iterations':item['artifacts']['iterations.bin'],'coefficient_observations':item['artifacts']['coefficients.bin'],'independent_audit':item['audit']['record']})
            elif group=='replays':
                seed=item['seed'];label=f'replay-seed-{seed}'
                need(result['iteration_and_coefficients_byte_exact'] is True and result['seed']==seed,'exact replay evidence')
                for key in ['plant_steps','AdamW_updates','coefficient_observations']:totals['replay_'+key]+=result[key]
            else:
                label='evaluation-'+item['label'];need(result['steps']==768,'six128-step deterministic checks')
                totals['deterministic_eval_steps']+=result['steps']
                real=result['real'];gaps=list(map(float,real['relative_gaps']));need(len(gaps)==6 and all(math.isfinite(x) for x in gaps),'finite cached real quality gaps')
                value={'K':result['K'],'nominal_optimal_K':float(real['Kstar']),'gain_error':float(real['gain_error']),'maximum_nominal_relative_cost_gap':max(gaps),'source_gain_goal':bool(result['source_gain_ok']),'source_cost_goal':bool(result['source_gap_ok']),'real_goals_supported':result['real_goals_supported'],'all_frozen_quality_goals_met':result['promoted'],'real_gain_margin':real['gain_margin'],'real_cost_margin':real['cost_margin'],'nominal_costs':real['nominal_costs'],'deterministic_evaluation_max_absolute_errors':result['evaluation_max_absolute']}
                if result['baseline']:baseline=value
                else:seed_results[str(result['seed'])]=value
            destination=output/'audits'/(label+'.json');destination.write_bytes(audit_path.read_bytes())
            audits.append({'label':label,'path':'audits/'+destination.name,'identity':identity(destination)})
    need(sorted(seed_results)==['0','1','7'] and baseline is not None,'all prescribed seed outcomes and baseline retained')
    need(totals['canonical_plant_steps']==196608 and totals['canonical_AdamW_updates']==1536 and totals['canonical_coefficient_observations']==786432,'canonical counts')
    need(totals['replay_plant_steps']==1536 and totals['replay_AdamW_updates']==12 and totals['replay_coefficient_observations']==6144 and totals['deterministic_eval_steps']==3072,'separate replay/evaluation counts')
    for seed in (0,1,7):need([x['completed_batch'] for x in curves if x['seed']==seed]==list(range(1,129)),'exact full captured curve')
    write(output/'curves.json',{'schema':'psionic.locomotion-lqr.captured-curves.v1','initial_K':.25,'scope':'Captured source fields only; last-epoch loss/gradient precede that update. No performance-oracle recomputation or extra training.','rows':curves})
    with (output/'curves.csv').open('w',newline='') as stream:
        writer=csv.DictWriter(stream,fieldnames=list(curves[0]));writer.writeheader();writer.writerows(curves)
    summary={'schema':'psionic.locomotion-lqr.independent-learning-summary.v1','numerical_qualification_passed':True,'all_prescribed_quality_goals_met':all(x['all_frozen_quality_goals_met'] for x in seed_results.values()),'first_original_learning_record':identity(run/'record.json'),'descriptor':top['descriptor'],'baseline':baseline,'seeds':seed_results,'counts':totals,'captured_numeric_error_maxima':numeric_max,'audits':audits,'canonical_lineage':lineage,'scope':{'learner':'One projected scalar-K PPO actor, fixed sigma .25 and explicitly nominal continuous-Gaussian analytic critic; finite-top53 sampler.','clipping':'Report observed active-clipping count separately; math fixtures exercise clipping even if this learning run does not.','quality':'Frozen gain≤.05 and all-six nominal gap≤1% goals stay unchanged; no best-seed/checkpoint selection or post-result tuning.','evaluation':'3072 deterministic mean-policy transitions and independent nominal analytic costs; not stochastic held-out episodes.','limits':'No learned critic, neural actor, quadruped/contact/device training, hardware or full compilerR14/R16 proof.','publication':'Only immutable captured fields and existing audit/quality records read; no product, math-oracle, training, optimizer or SDK call.'}}
    write(output/'summary.json',summary)
    files={str(p.relative_to(output)):identity(p) for p in output.rglob('*') if p.is_file()}
    write(output/'publication.json',{'schema':'psionic.locomotion-lqr.independent-learning-publication.v1','passed':True,'source':identity(SOURCE),'original_learning':identity(run/'record.json'),'original_descriptor':identity(run/'descriptor.json'),'all31_audit_receipts_byte_exact':True,'new_oracle_product_model_SDK_calls':0,'files':files})
    print(json.dumps({'summary':identity(output/'summary.json'),'publication':identity(output/'publication.json'),'goals':summary['all_prescribed_quality_goals_met'],'seeds':seed_results}))


if __name__=='__main__':
    parser=argparse.ArgumentParser();parser.add_argument('--run',required=True,type=Path);parser.add_argument('--output',required=True,type=Path);args=parser.parse_args();publish(args.run,args.output)
