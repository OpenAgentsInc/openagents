#!/usr/bin/env python3
"""Independent NEW trajectory controller audit and separately reported goals.

Reads one completed SDK capture; no SDK/model/runtime calls, old gate mains or
trajectory regeneration. Pure matrix potential-gradient definitions are reused.
"""
from pathlib import Path
import argparse,hashlib,json,math,struct,time
import panda_joint_pd_reference as pd
ROOT=Path(__file__).resolve().parents[3]
F=ROOT/'psionic/fixtures/robotics/panda-joint-pd-tracking'
HEADER=struct.Struct('<4I');SDK=struct.Struct('<120d10q');OWN=struct.Struct('<120d10q18d4q')
SDK_MAGIC=0x31525450;OWN_MAGIC=0x314f5450
FROZEN='cfe805220004838440a4ea92c258880597ca5de42b7360ca7102281e242b3612'
def require(ok,message):
    if not ok:raise RuntimeError(message)
def sha(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()
def identity(path):return dict(bytes=Path(path).stat().st_size,sha256=sha(path))
def finite(values,n,label):
    require(len(values)==n and all(math.isfinite(x)for x in values),label+' ALL finite/complete');return list(values)
def bits(values):return struct.pack('<'+str(len(values))+'d',*values)
def write(path,value):path.write_text(json.dumps(value,indent=2)+'\n')
def decode(raw,own=False):
    row=OWN if own else SDK;require(len(raw)==HEADER.size+3000*row.size,'exact NEW4×750record bytes');require(HEADER.unpack_from(raw)==(OWN_MAGIC if own else SDK_MAGIC,4,750,row.size),'PTR1/PTO1 record-width case/tick inventory');result=[]
    for index in range(3000):
        v=row.unpack_from(raw,HEADER.size+index*row.size);finite(v[:120],120,'ALL SDK/own record fields before any metrics');c,k,stage,code,clips,rt,n,pulse,cc,fc=v[120:130];require((c,k)==divmod(index,750),'case-major increasing complete tick inventory');require(stage==code==0,'actual successful complete controller/step status');require(0<=clips<=7 and 0<=cc<=9 and 0<=fc<=2 and pulse in[0,1]and 0<=rt<=n<=1000000,'all counter domains');require(pulse==int(c==2 and 200<=k<250),'exact pulse interval')
        if own:finite(v[130:148],18,'ALL recorded-command shadow states');require(all(0<=v[148+j]<=([9,2,9,2][j]) for j in range(4)),'all four PeU clipping counts')
        result.append(v)
    return result
def decoder_guards():
    good=bytearray(HEADER.pack(SDK_MAGIC,4,750,SDK.size));q=[0.]*120
    for c in range(4):
        for k in range(750):good+=SDK.pack(*q,c,k,0,0,0,0,0,int(c==2 and 200<=k<250),0,0)
    decode(good);poisons=[bytes(good[:-1]),bytes(good)+b'\0']
    for z in[float('nan'),float('inf'),-float('inf')]:
        b=bytearray(good);struct.pack_into('<d',b,HEADER.size+2999*SDK.size+119*8,z);poisons.append(bytes(b))
    for lane,z in[(0,2),(1,748),(2,3),(3,1),(4,8),(5,1),(6,-1),(7,1),(8,10),(9,3)]:
        b=bytearray(good);struct.pack_into('<q',b,HEADER.size+2999*SDK.size+120*8+lane*8,z);poisons.append(bytes(b))
    for b in poisons:
        try:decode(b)
        except RuntimeError:pass
        else:raise RuntimeError('late record poison admitted')
    return dict(passed=True,refused_cases=len(poisons),SDK_calls=0,neural_calls=0)
def compare(a,b,atol,label,rtol=0):
    finite(a,len(b),label+' actual');finite(b,len(a),label+' expected');require(a,label+' nonempty');errors=[abs(x-y)for x,y in zip(a,b)];ratios=[e/(atol+rtol*abs(y))for e,y in zip(errors,b)];m=max(ratios);return dict(passed=m<=1,values=len(a),max_abs_error=max(errors),maximum_fraction_of_frozen_budget=m,atol=atol,rtol=rtol)
def clipping(value,lo,hi):return(lo,1)if value<lo else(hi,1)if value>hi else(value,0)
def actuator(q,v,controls):
    result=[];cc=fc=0
    for j in range(9):
        lo,hi=(-80.,80.)if j<5 else(-12.,12.)if j<7 else(0.,.04)if j==7 else(-.04,0.)
        u,n=clipping(controls[j],lo,hi);cc+=n
        if j<7:result.append(u)
        else:
            # Fixed-gain product plus affine bias, independently sourced from
            # the frozen source coefficient metadata. Do not factor u-q.
            bias=(0.+(-1000.)*q[j])+0.*v[j];force=1000.*u+bias;x,n=clipping(force,-20.,20.);result.append(x);fc+=n
    return finite(result,9,'independent scalar actuator'),cc,fc
def rms(values):return math.sqrt(math.fsum(x*x for x in values)/len(values))
def trajectory_metrics(rows,references,c,limits=None):
    q0=c['initial_q9'];errors=[];rate_errors=[];drift=[];fingers=[];finger_rates=[];baseline=[]
    for k,row in enumerate(rows):
        target=references[(k+1)*21:(k+2)*21];post=row[101:110];v=row[110:119];errors.append([post[j]-target[j]for j in range(7)]);rate_errors.append([v[j]-target[7+j]for j in range(7)]);drift.append([post[j]-q0[j]for j in range(7)]);fingers.append([post[7+j]-q0[7+j]for j in range(2)]);finger_rates.append(v[7:9]);baseline.append([q0[j]-target[j]for j in range(7)])
    flatten=lambda x:[z for row in x for z in row]
    if limits is None:
        require(sha(F/'experiment.json')==FROZEN,'non-gating diagnostic source ranges remain frozen');limits=json.loads((F/'experiment.json').read_text())['joint_limits_radians']
    require(len(limits)==7 and all(len(pair)==2 and all(math.isfinite(x)for x in pair)and pair[0]<pair[1]for pair in limits),'seven admitted diagnostic soft ranges')
    effective=[[ (row[110+j]-row[9+j])/.002 for j in range(7)]for row in rows];finite(flatten(effective),5250,'ALL newly observed effective accelerations before maxima')
    peak_speed=[max(abs(rows[0][9+j]),max(abs(row[110+j])for row in rows))for j in range(7)];peak_accel=[max(abs(value[j])for value in effective)for j in range(7)]
    excursions=[[max(0.,limits[j][0]-row[101+j],row[101+j]-limits[j][1])for j in range(7)]for row in rows];finite(flatten(excursions),5250,'ALL poststate soft-limit excursions')
    plant_diagnostics=dict(gating=False,speed_state_ticks=[0,750],perjoint_peak_abs_hinge_speed_rad_per_second=peak_speed,perjoint_peak_abs_effective_step_acceleration_rad_per_second_squared=peak_accel,effective_acceleration_recipe='(post_v-pre_v)/0.002; effective implicit-step acceleration, not SDK qacc',poststate_soft_limit_grid=[1,750],perjoint_peak_soft_limit_excursion_rad=[max(e[j]for e in excursions)for j in range(7)],perjoint_soft_limit_excursion_states=[sum(e[j]>0 for e in excursions)for j in range(7)],perjoint_first_soft_limit_excursion_state=[next((i+1 for i,e in enumerate(excursions)if e[j]>0),None)for j in range(7)],reference_velocity_limits_rad_per_second=c['velocity_limits7'],reference_acceleration_limits_rad_per_second_squared=c['acceleration_limits7'],perjoint_speed_exceeds_reference_velocity_limit=[peak_speed[j]>c['velocity_limits7'][j]for j in range(7)],perjoint_effective_acceleration_exceeds_reference_acceleration_limit=[peak_accel[j]>c['acceleration_limits7'][j]for j in range(7)],note='Reference kinematic limits do not bound the actual feedback-controlled plant. No continuous-between-samples, collision, nonlinear-stability or hardware certificate.')
    allerror=finite(flatten(errors),5250,'ALL poststate aligned errors');allrate=finite(flatten(rate_errors),5250,'ALL poststate aligned rate errors');baseline_flat=finite(flatten(baseline),5250,'ALL ideal stationary comparator errors');start=250 if c['index']==2 else 1;entry=None
    for n in range(start,727):
        if all(max(abs(x)for x in errors[m-1])<=.01 and max(abs(x)for x in rate_errors[m-1])<=.05 for m in range(n,n+25)):entry=n;break
    recovery=dict(applicable=c['index']in[2,3],eligible_first_state_tick=start,consecutive_states=25,recovered=entry is not None,entry_state_tick=entry,confirmation_state_tick=entry+24 if entry is not None else None,entry_seconds=entry/500 if entry is not None else None,confirmation_seconds=(entry+24)/500 if entry is not None else None,entry_seconds_after_pulse_end_or_brake_start=(entry-(250 if c['index']==2 else 0))/500 if entry is not None else None,missing='not recovered'if entry is None else None)
    return dict(case=c['index'],name=c['name'],actual_plant_diagnostics=plant_diagnostics,poststate_ticks=[1,750],samples=750,global_position_RMS_rad=rms(allerror),perjoint_position_RMS_rad=[rms([e[j]for e in errors])for j in range(7)],perjoint_position_IAE_rad_seconds=[.002*math.fsum(abs(e[j])for e in errors)for j in range(7)],peak_position_error_rad=max(abs(x)for x in allerror),peak_rate_error_rad_per_second=max(abs(x)for x in allrate),final_position_error7_rad=errors[-1],final_max_position_error_rad=max(abs(x)for x in errors[-1]),final_rate_error7_rad_per_second=rate_errors[-1],final_max_rate_error_rad_per_second=max(abs(x)for x in rate_errors[-1]),final_window=dict(state_ticks=[626,750],samples=125,global_position_RMS_rad=rms(flatten(errors[625:])),peak_position_error_rad=max(abs(x)for x in flatten(errors[625:])),peak_rate_error_rad_per_second=max(abs(x)for x in flatten(rate_errors[625:]))),peak_arm_drift_from_initial_rad=max(abs(x)for x in flatten(drift)),peak_finger_drift_metres=max(abs(x)for x in flatten(fingers)),peak_finger_rate_metres_per_second=max(abs(x)for x in flatten(finger_rates)),ideal_stationary_position_baseline=dict(executed=False,global_position_RMS_rad=rms(baseline_flat),better_than_baseline=rms(allerror)<rms(baseline_flat)),Pjp_clipped_ticks=sum(row[124]>0 for row in rows),Pjp_clipped_coordinate_sum=sum(row[124]for row in rows),Pa_control_clipped_ticks=sum(row[128]>0 for row in rows),Pa_force_clipped_ticks=sum(row[129]>0 for row in rows),initial_Pjp_clipped_coordinates=rows[0][124],recovery=recovery)
def goals(metrics):
    require([m['case']for m in metrics]==list(range(4)),'all four quality cases');return dict(hold_peak_arm_drift=metrics[0]['peak_arm_drift_from_initial_rad']<=.005,nominal_final_position_error=metrics[1]['final_max_position_error_rad']<=.01,nominal_RMS_beats_ideal_stationary=metrics[1]['ideal_stationary_position_baseline']['better_than_baseline'],pulse_final_position_error=metrics[2]['final_max_position_error_rad']<=.02,pulse_recovery_observed=metrics[2]['recovery']['recovered'],brake_initial_Pjp_saturation=metrics[3]['initial_Pjp_clipped_coordinates']>0,brake_recovery_observed=metrics[3]['recovery']['recovered'])
def audit(sdk_dir,out,own_paths):
    require(not out.exists(),'preserve first/failed audit');out.mkdir(parents=True);r=dict(schema='psionic.panda-joint-pd-tracking.independent-audit.v1',passed=False,SDK_calls=0,Coax_calls=0,NN_calls=0,simulation_steps=0,old_gates_repeated=False);began=time.monotonic()
    try:
        require(sha(F/'experiment.json')==FROZEN,'immutable precapture experiment');e=json.loads((F/'experiment.json').read_text());require(identity(F/'cases.f64')==e['input_file'],'all2048 frozen input bits');ref=json.loads((F/'analytic-reference.json').read_text());require(ref['passed']and ref['experiment']==identity(F/'experiment.json'),'original once-only reference execution');require(all(sha(ROOT/name)==digest for name,digest in e['source_sha256'].items()),'every original source/metadata remains byte-bound')
        for name,item in ref['outputs'].items():require(identity(F/name)==item,'original analytic output '+name)
        references=finite(struct.unpack('<63084d',(F/'reference-states.f64').read_bytes()),4*751*21,'ALL cached Decimal80 target fields');external=finite(struct.unpack('<27000d',(F/'external-forces.f64').read_bytes()),4*750*9,'ALL frozen external forces');plans=json.loads((F/'reference-plans.json').read_text());require([p['steps']for p in plans]==[0,270,270,0],'all four timing profiles')
        receipt=json.loads((sdk_dir/'record.json').read_text());require(receipt['passed']and receipt['experiment_sha256']==FROZEN,'completed single SDK acquisition bound to inputfreeze');trace=sdk_dir/'sdk-trace.bin';require(receipt['trace']==identity(trace),'SDK trace identity');rows=decode(trace.read_bytes());deps={ROOT/name:digest for name,digest in e['source_sha256'].items()};deps[Path(__file__).resolve()]=sha(__file__);deps[F/'experiment.json']=FROZEN;deps[F/'analytic-reference.json']=sha(F/'analytic-reference.json');deps[F/'reference-states.f64']=sha(F/'reference-states.f64');deps[F/'external-forces.f64']=sha(F/'external-forces.f64')
        require(isinstance(receipt['source'],dict)and all((ROOT/name).exists()and identity(ROOT/name)==item for name,item in receipt['source'].items()),'all actual SDK source identities unchanged');arm=json.loads((ROOT/'psionic/fixtures/robotics/panda-fk/source-profile.json').read_text());branch=json.loads((ROOT/'psionic/fixtures/robotics/panda-fingers/source-profile.json').read_text());model=json.loads((ROOT/'psionic/fixtures/robotics/panda-fingers/sdk-model.json').read_text());nodes=arm['nodes']+branch['nodes'];require(branch['arm_profile']['sha256']==sha(ROOT/branch['arm_profile']['path'])and len(nodes)==len(model['bodies'])==16 and [n['name']for n in nodes]==[b['name']for b in model['bodies']],'all16body/source inertia association');require(all(j['qpos0_reference']==0 for j in model['robot_joints']),'source zero coordinate reference')
        expected=[];actual=[];target_actual=[];target_expected=[];act_actual=[];act_expected=[];clips=[];controlbits=[];minimum_requested_margin=math.inf;exact_capped_transports=0;exact_Pa_control_boundaries=0;ambiguous=[]
        for index,row in enumerate(rows):
            c,k=divmod(index,750);case=e['cases'][c];q=row[:9];v=row[9:18];curve=references[c*751*21:(c+1)*751*21];reference=curve[k*21:(k+1)*21];target_actual+=row[18:39];target_expected+=reference;require(row[125:128]==(min(k,plans[c]['steps']),plans[c]['steps'],int(c==2 and 200<=k<250)),'exact reference tick/N/pulse');require(bits(row[83:92])==bits(external[index*9:(index+1)*9]),'exact accepted external force order');require(abs(row[119]-(k+1)/500)<=e['parity']['clock_atol_seconds'],'physical SDK clock')
            if k==0:require(bits(q+v)==bits(case['initial_q9']+case['initial_v9']),'each fresh compiled-default robot override')
            else:require(bits(q+v)==bits(rows[index-1][101:119]),'fresh current state exactly previouspost')
            g=pd.potential_gradient(nodes,model,q);scalar_case={pd.KEYS[0]:q[:7],pd.KEYS[2]:v[:7],pd.KEYS[3]:reference[:7],pd.KEYS[4]:reference[7:14],pd.KEYS[5]:case['kp7'],pd.KEYS[6]:case['kd7']};want=pd.scalar(scalar_case,g);expected.extend(x for field in pd.FIELDS for x in want[field]);actual+=row[39:74];require(row[124]==want['clipped'],'every independent Pjp clip count exact');require(bits(row[74:81])==bits(row[67:74])and bits(row[81:83])==bits(case['finger_targets2']),'controls exactly published Pjp captransport and fixed slide targets');act,cc,fc=actuator(q,v,row[74:83]);act_actual+=row[92:101];act_expected+=act;require(row[128:130]==(cc,fc),'all independent actuator clipping stages exact');clips.append([want['clipped'],cc,fc]);controlbits+=row[74:83]
            for j in range(7):
                cap=80 if j<5 else 12;margin=abs(abs(want['requested'][j])-cap);minimum_requested_margin=min(minimum_requested_margin,margin)
                if margin<=e['parity']['clip_distance_diagnostic']:ambiguous.append(dict(case=c,tick=k,joint=j,independent_requested=want['requested'][j],actual_requested=row[60+j],margin=margin))
                if abs(row[74+j])==cap:
                    exact_Pa_control_boundaries+=1
                    if abs(row[60+j])>cap:exact_capped_transports+=1
        finite(expected,105000,'ALL NEW independent35controller targets before maxima');finite(act_expected,27000,'ALL NEW independent actuator targets before maxima');force=compare(actual,expected,2e-8,'all105000 controller fields');actuation=compare(act_actual,act_expected,2e-8,'all27000 actuator fields');angles=compare([target_actual[21*i+j]for i in range(3000)for j in range(7)],[target_expected[21*i+j]for i in range(3000)for j in range(7)],2e-12,'all target angles');derivatives=compare([target_actual[21*i+j]for i in range(3000)for j in range(7,21)],[target_expected[21*i+j]for i in range(3000)for j in range(7,21)],2e-10,'all target derivatives',2e-12);numerical=dict(controller=force,actuator=actuation,target_angles=angles,target_derivatives=derivatives);require(all(x['passed']for x in numerical.values()),'frozen independent numerical budgets')
        (out/'expected-controller.f64').write_bytes(bits(expected));(out/'expected-actuator.f64').write_bytes(bits(act_expected));sdk_metrics=[trajectory_metrics(rows[c*750:(c+1)*750],references[c*751*21:(c+1)*751*21],e['cases'][c])for c in range(4)];sdk_goals=goals(sdk_metrics);own_metrics=[]
        for path in own_paths:
            own=decode(path.read_bytes(),True);metrics=[trajectory_metrics(own[c*750:(c+1)*750],references[c*751*21:(c+1)*751*21],e['cases'][c])for c in range(4)];own_metrics.append(dict(path=str(path),identity=identity(path),metrics=metrics,quality_hypotheses=goals(metrics),note='Independent quality observation only; native mode/parity qualification is a separate record.'))
        require(all(sha(path)==digest for path,digest in deps.items()),'executed sources/targets frozen at completion');r.update(passed=True,experiment_sha256=FROZEN,capture_receipt=identity(sdk_dir/'record.json'),capture_trace=identity(trace),source_sha256={str(p.relative_to(ROOT))if p.is_relative_to(ROOT)else str(p):d for p,d in deps.items()},decoder_guards=decoder_guards(),all_actual_fields_finite=360000,all_reference_fields_finite=63084,all_external_fields_finite=27000,all_independent_PD_fields_finite=105000,all_independent_actuator_fields_finite=27000,numerical_gates=numerical,independent_clip_counts_exact=True,clip_diagnostics=dict(minimum_independent_prePjp_requested_cap_distance=minimum_requested_margin,ambiguous_requested_crossings=ambiguous,structurally_exact_Pjp_capped_transports=exact_capped_transports,exact_inclusive_Pa_control_boundaries=exact_Pa_control_boundaries,cap_transport_is_not_an_ambiguous_requested_crossing=True),SDK_quality_metrics=sdk_metrics,SDK_quality_hypotheses=sdk_goals,all_quality_hypotheses_met=all(sdk_goals.values()),quality_failures_do_not_invalidate_capture=True,own_quality_metrics=own_metrics,outputs={name:identity(out/name)for name in['expected-controller.f64','expected-actuator.f64']},scope=e['scope'])
    except BaseException as error:r['failure']=repr(error);raise
    finally:r['seconds']=time.monotonic()-began;write(out/'record.json',r)
    print(json.dumps(dict(passed=True,record=str(out/'record.json'),numerical=numerical,quality=sdk_goals)))
def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--sdk',type=Path,required=True);p.add_argument('--output',type=Path,required=True);p.add_argument('--own-trace',type=Path,action='append',default=[]);a=p.parse_args();audit(a.sdk.resolve(),a.output.resolve(),[x.resolve()for x in a.own_trace])
if __name__=='__main__':main()
