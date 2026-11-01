#!/usr/bin/env python3
"""Package already-audited continuation observations; no oracle/product calls."""
import argparse
import csv
import hashlib
import json
import math
from pathlib import Path
import struct

ROOT=Path(__file__).resolve().parents[3]
OLD=ROOT/'psionic/fixtures/robotics/locomotion-lqr'
X=ROOT/'psionic/fixtures/robotics/locomotion-lqr-longer'
SOURCE=Path(__file__).resolve()

def need(ok,message):
    if not ok:raise ValueError(message)

def identity(path):
    b=Path(path).read_bytes();return {'bytes':len(b),'sha256':hashlib.sha256(b).hexdigest()}

def load(path):return json.loads(Path(path).read_text())
def write(path,value):Path(path).write_text(json.dumps(value,indent=2,allow_nan=False)+'\n')

def publish(run,output):
    need(not output.exists(),'fresh immutable publication target')
    e=load(X/'experiment.json');top=load(run/'record.json');old_summary=load(OLD/'independent-learning/summary.json');old_curves=load(OLD/'independent-learning/curves.json')
    need(top['schema']=='psionic.locomotion-lqr-longer.learning.v1' and top['passed'] is True and top['lifecycle_state']=='complete','completed FIRST continuation')
    need(tuple(len(top[k]) for k in ('imports','chunks','replays','evaluations','completed_actions'))==(3,72,3,3,81),'exact81 frozen actions')
    for name,h in e['metadata_inputs'].items():need(identity(ROOT/name)==h,'carried metadata unchanged '+name)
    output.mkdir(parents=True);(output/'audits').mkdir()
    for name in ('curves.json','curves.csv','summary.json'):(output/('original-'+name)).write_bytes((OLD/'independent-learning'/name).read_bytes())
    audits=[];curves=list(old_curves['rows']);results={};lineage=[];maxima={}
    counts={'import_actions':0,'canonical_plant_steps':0,'canonical_AdamW_updates':0,'canonical_coefficient_observations':0,'replay_plant_steps':0,'replay_AdamW_updates':0,'replay_coefficient_observations':0,'final_deterministic_eval_steps':0,'projected_updates':0,'ppo_clipped_samples':0,'old_prefix_plant_steps_repeated':0,'baseline_eval_steps_repeated':0}
    for group in ('imports','chunks','replays','evaluations'):
        for item in top[group]:
            ap=run/item['audit']['directory']/'record.json';need(identity(ap)==item['audit']['record'],'immutable FIRST independent action receipt')
            a=load(ap);need(a['schema']=='psionic.locomotion-lqr-longer.learning-audit.v1' and a['passed'] is True and a['inputs']['descriptor']==top['descriptor'],'same PASSED continuation audit context');r=a['result'];seed=item['seed']
            if group=='imports':
                label=f'import-seed-{seed}';need(r['seed']==seed and r['import_only'] is True and r['noncounter_bytes_exact'] is True and r['plant_steps']==0 and r['AdamW_updates']==0,'zero-step counter-only import');counts['import_actions']+=1
            elif group=='chunks':
                first,count=item['first_batch'],item['batches'];label=f'canonical-seed-{seed}-batch-{first:03d}'
                need((r['seed'],r['first_batch'],r['batches'])==(seed,first,count),'canonical labels')
                for key in ('plant_steps','AdamW_updates','coefficient_observations'):counts['canonical_'+key]+=r[key]
                counts['projected_updates']+=r['projected_updates'];counts['ppo_clipped_samples']+=r['ppo_clipped_samples']
                for key,value in r['maximum_absolute_errors'].items():need(math.isfinite(value),'finite cached error');maxima[key]=max(maxima.get(key,0.),value)
                path=run/item['directory']/'iterations.bin';need(identity(path)==item['artifacts']['iterations.bin']==a['inputs']['iterations'],'captured curve source identity')
                raw=path.read_bytes();need(len(raw)==48+count*62696 and struct.unpack_from('<4I',raw)==(0x3149584c,1,62696,16) and raw[16:48]==bytes.fromhex(top['descriptor']['sha256']),'captured curve wire/context')
                for j in range(count):
                    at=48+j*62696;Kold=struct.unpack_from('<d',raw,at+24)[0];K,m,v,step,skip=struct.unpack_from('<3d2q',raw,at+62256);epochs=[struct.unpack_from('<3d2q3d4q',raw,at+61872+i*96) for i in range(4)]
                    need(all(math.isfinite(x) for x in [Kold,K,m,v,epochs[-1][5],epochs[-1][6]]),'ALL cached curve fields finite')
                    curves.append({'seed':seed,'completed_batch':first+j+1,'rollout_K':Kold,'K_after':K,'moment_m_after':m,'moment_v_after':v,'optimizer_step':step,'skipped':skip,'last_epoch_loss_before_update':epochs[-1][5],'last_epoch_gradient_before_update':epochs[-1][6],'projected_updates':sum(z[8] for z in epochs),'ppo_clipped_samples':sum(z[11] for z in epochs)})
                lineage.append({'directory':item['directory'],'iterations':item['artifacts']['iterations.bin'],'coefficient_observations':item['artifacts']['coefficients.bin'],'independent_audit':item['audit']['record']})
            elif group=='replays':
                label=f'replay-seed-{seed}';need(r['seed']==seed and r['iteration_and_coefficients_byte_exact'] is True,'exact declared replay')
                for key in ('plant_steps','AdamW_updates','coefficient_observations'):counts['replay_'+key]+=r[key]
            else:
                label=f'evaluation-seed-{seed}';need(r['seed']==seed and not r['baseline'] and r['steps']==768,'final six128-step evaluations')
                counts['final_deterministic_eval_steps']+=r['steps'];real=r['real'];gaps=list(map(float,real['relative_gaps']));need(len(gaps)==6 and all(math.isfinite(x) for x in gaps),'ALL captured real-gap diagnostics finite')
                results[str(seed)]={'K':r['K'],'nominal_optimal_K':float(real['Kstar']),'gain_error':float(real['gain_error']),'maximum_nominal_relative_cost_gap':max(gaps),'source_gain_goal':bool(r['source_gain_ok']),'source_cost_goal':bool(r['source_gap_ok']),'real_goals_supported':r['real_goals_supported'],'all_frozen_quality_goals_met':r['promoted'],'real_gain_margin':real['gain_margin'],'real_cost_margin':real['cost_margin'],'nominal_costs':real['nominal_costs'],'deterministic_evaluation_max_absolute_errors':r['evaluation_max_absolute']}
            target=output/'audits'/(label+'.json');target.write_bytes(ap.read_bytes());audits.append({'label':label,'path':'audits/'+target.name,'identity':identity(target)})
    need(sorted(results)==['0','1','7'],'ALL prescribed outcomes')
    need((counts['import_actions'],counts['canonical_plant_steps'],counts['canonical_AdamW_updates'],counts['canonical_coefficient_observations'],counts['replay_plant_steps'],counts['replay_AdamW_updates'],counts['replay_coefficient_observations'],counts['final_deterministic_eval_steps'])==(3,589824,4608,2359296,1536,12,6144,2304),'separate frozen new workload')
    curves.sort(key=lambda r:(r['seed'],r['completed_batch']))
    for seed in (0,1,7):
        need([r['completed_batch'] for r in curves if r['seed']==seed]==list(range(1,513)),'complete carried+new curve512')
        need([r for r in curves if r['seed']==seed and r['completed_batch']<=128]==[r for r in old_curves['rows'] if r['seed']==seed],'carried first128 objects unchanged')
    write(output/'curves.json',{'schema':'psionic.locomotion-lqr-longer.captured-curves.v1','initial_K':.25,'scope':'First128 rows/seed carried from original frozen curves;129..512 are new captured fields. No math-oracle, optimizer or training invocation.','rows':curves})
    with (output/'curves.csv').open('w',newline='') as f:
        w=csv.DictWriter(f,fieldnames=list(curves[0]));w.writeheader();w.writerows(curves)
    passed=all(r['all_frozen_quality_goals_met'] for r in results.values())
    summary={'schema':'psionic.locomotion-lqr-longer.independent-learning-summary.v1','numerical_qualification_passed':True,'all_prescribed_quality_goals_met':passed,'promoted_seed_count':3 if passed else 0,'original_128_batch_outcomes_preserved':old_summary['seeds'],'baseline_carried_verbatim':old_summary['baseline'],'original_summary':identity(OLD/'independent-learning/summary.json'),'original_curves':identity(OLD/'independent-learning/curves.json'),'first_original_learning_record':identity(run/'record.json'),'descriptor':top['descriptor'],'seeds':results,'new_counts':counts,'carried_counts':old_summary['counts'],'captured_new_numeric_error_maxima':maxima,'audits':audits,'canonical_lineage':lineage,'scope':{'profile':'Post-outcome separately frozen longer continuation; same optimizer/law/sigma/nominal critic; only RNG reservation and total batch bound changed.','quality':'Same all-seed gain and nominal cost goals; no tuning/early stop/best-seed selection.','evaluation':'New2304 deterministic mean-policy transitions and nominal analytic goals; original baseline carried, not simulated again.','counters':'Source-bound recomputed coefficient observations, not invented internal instrumentation.','limits':'Finite-top53 sampler and nominal continuous-Gaussian critic; no learned critic/neural actor/quadruped/GPU/hardware or universal convergence claim.','publication':'Only cached captured fields/receipts read, no product/oracle/training/RNG/optimizer/SDK calls.'}}
    write(output/'summary.json',summary);files={str(p.relative_to(output)):identity(p) for p in output.rglob('*') if p.is_file()}
    write(output/'publication.json',{'schema':'psionic.locomotion-lqr-longer.independent-learning-publication.v1','passed':True,'source':identity(SOURCE),'original_learning':identity(run/'record.json'),'original_descriptor':identity(run/'descriptor.json'),'all81_audit_receipts_byte_exact':True,'full_first128_curves_copied_byte_exact':True,'new_oracle_product_training_RNG_SDK_calls':0,'files':files})
    print(json.dumps({'summary':identity(output/'summary.json'),'publication':identity(output/'publication.json'),'goals':passed,'seeds':results}))

if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('--run',required=True,type=Path);p.add_argument('--output',required=True,type=Path);a=p.parse_args();publish(a.run,a.output)
