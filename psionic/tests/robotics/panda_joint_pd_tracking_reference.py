#!/usr/bin/env python3
"""Freeze a NEW four-case tracking experiment; generate analytic references.

No SDK, Coax runtime, earlier harness main or old corpus is executed. Imports
only authored independent quintic and matrix-controller function definitions.
"""
from pathlib import Path
from decimal import localcontext
import argparse,hashlib,json,math,re,struct,time
import panda_quintic_reference as quintic
import panda_joint_pd_reference as pd

ROOT=Path(__file__).resolve().parents[3]
FIXTURE=ROOT/'psionic/fixtures/robotics/panda-joint-pd-tracking'
Q=[-.31,.22,-.14,-1.91,.36,1.47,-.52]
SLIDES=[.012,-.026]
OFFSET=[.03,-.025,.02,-.02,.015,.02,-.025]
KP=[60.,60.,60.,60.,40.,20.,10.]
KD=[8.,8.,8.,8.,5.,3.,2.]
SOURCE_PATHS=['psionic/tests/robotics/panda_quintic_reference.py','psionic/tests/robotics/panda_joint_pd_reference.py','psionic/tests/robotics/panda_inverse_dynamics_matrix.py','psionic/fixtures/robotics/panda-fk/source-profile.json','psionic/fixtures/robotics/panda-fk/sdk-model.json','psionic/fixtures/robotics/panda-fingers/source-profile.json','psionic/fixtures/robotics/panda-fingers/sdk-model.json','psionic/fixtures/robotics/panda-actuation/experiment.json','psionic/fixtures/robotics/panda-actuation/sdk-model.json']
def require(ok,message):
    if not ok:raise RuntimeError(message)
def sha(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()
def identity(path):return dict(bytes=Path(path).stat().st_size,sha256=sha(path))
def finite(values,n,label):
    require(len(values)==n and all(math.isfinite(x)for x in values),label+' ALL finite/complete');return list(values)
def write(path,value):path.write_text(json.dumps(value,indent=2)+'\n')
def dependency_graph(path,result):
    path=path.resolve();name=str(path.relative_to(ROOT))
    if name in result:return
    result[name]=sha(path)
    for child in re.findall(r'^\s*#load\s+"([^"]+)"',path.read_text(),re.M):dependency_graph(path.parent/child,result)
def source_identity():
    result={name:sha(ROOT/name)for name in SOURCE_PATHS};result[str(Path(__file__).resolve().relative_to(ROOT))]=sha(__file__)
    for name in ['panda-joint-pd.coax','panda-euler.coax','panda-quintic.coax']:dependency_graph(ROOT/'psionic/lib/robotics'/name,result)
    return result
def model_mapping():
    path=ROOT/'psionic/fixtures/robotics/panda-actuation/sdk-model.json';m=json.loads(path.read_text());a=m['actuators'];j=m['joints'];require(len(a)==len(j)==9,'nine source actuators/joints')
    for i,(act,joint)in enumerate(zip(a,j)):
        require(act['transmission_type']==0 and act['transmission_ids']==[joint['id'],-1]and act['gear']==[1,0,0,0,0,0]and joint['dof_adr']==joint['qpos_adr']==i,'pinned one-to-one positive-unit joint transmission')
        require(act['dynamic_type']==0 and act['activation_count']==0 and act['gain_type']==0 and act['gain_params'][0]==(1 if i<7 else 1000),'source stateless fixed actuator gain')
    return dict(source_metadata=identity(path),joint_dof_qpos_addresses=list(range(9)),gear=[1,0,0,0,0,0],transmission_type=0,SDK_scalar_to_Pa_generalized='Only after fresh metadata and runtime sparse moment mapping each actuator to namedDOF+1 is verified; SDK qfrc_actuator copied independently per tick.',source_model=m['model'],qpos0=m['qpos0'])
def freeze():
    FIXTURE.mkdir(parents=True,exist_ok=True);require(not(FIXTURE/'experiment.json').exists(),'never rewrite a frozen experiment');source=source_identity();mapping=model_mapping();limits=json.loads((ROOT/'psionic/fixtures/robotics/panda-fk/sdk-model.json').read_text())['arm_joints'];limits=[x['range']for x in limits];cases=[]
    for index,name in enumerate(['gravity-hold','quintic-track','quintic-track-pulse','saturated-brake-hold']):
        q=Q+SLIDES;v=[0.]*9;end=[x+y for x,y in zip(Q,OFFSET)]if index in[1,2]else Q[:];kd=KD[:]
        if index==3:v[6]=2.;kd[6]=20.
        external=[0.]*9
        if index==2:external[1]=3.
        values=finite(q+v+end+[.12]*7+[.6]*7+KP+kd+SLIDES+external,64,'four-case input');require(all(lo<=x<=hi for row in[Q,end]for x,(lo,hi)in zip(row,limits)),'all endpoint source joint limits');cases.append(dict(index=index,name=name,initial_q9=q,initial_v9=v,end_q7=end,velocity_limits7=[.12]*7,acceleration_limits7=[.6]*7,kp7=KP,kd7=kd,finger_targets2=SLIDES,pulse_external9=external,input_f64=values))
    (FIXTURE/'cases.f64').write_bytes(struct.pack('<256d',*[x for c in cases for x in c['input_f64']]))
    parity=dict(hinge_position_atol_radians=2e-8,slide_position_atol_metres=2e-9,hinge_rate_atol_radians_per_second=2e-7,slide_rate_atol_metres_per_second=2e-8,controller_and_actuator_force_atol=2e-8,target_angle_atol=2e-12,target_derivative_atol=2e-10,target_derivative_rtol=2e-12,clock_atol_seconds=2.**-26,rtol=0,status='exact',clipping_counts='exact',mode_bytes='exact',clip_distance_diagnostic=2e-8)
    quality=dict(separate_from_numerical_gates=True,miss_does_not_discard_completed_valid_capture=True,hold_peak_arm_drift_goal_rad=.005,nominal_final_max_position_error_goal_rad=.01,nominal_global_position_RMS_less_than_ideal_stationary_baseline=True,pulse_final_max_position_error_goal_rad=.02,recovery=dict(position_max_atol_rad=.01,reference_rate_max_atol_rad_per_second=.05,consecutive_states=25,pulse_first_eligible_state_tick=250,brake_first_eligible_state_tick=1,report_entry_and_confirmation_ticks=True,missing='not recovered'),saturation=dict(case=3,initial_joint_index=6,expected_initial_Pjp_clip_positive=True),baseline='Ideal stationary-position q(n)=q0 analytically; not an executed holding controller.')
    e=dict(schema='psionic.panda-joint-pd-tracking.experiment.v1',frozen_before_new_trajectory_or_SDK_calls=True,scope='Four short fixed-source unconstrained nonlinear holding/tracking measurements. No NN,contact,collision,hardware,global/nonlinear stability,task success or real-time claim.',cases=cases,case_count=4,ticks_per_case=750,total_new_SDK_steps=3000,physics_step_seconds=.002,controller_hertz=500,pulse=dict(case=2,joint_index=1,amplitude_J_per_radian=3.,step_start_inclusive=200,step_end_exclusive=250),reference=dict(schema='PqtSample21F64',frames_per_case=751,control='prestate q_k/v_k compared with reference r_k/rate_k; use min(k,N)',quality='poststate q_(k+1)/v_(k+1) compared with analytic r_(k+1)/rate_(k+1)',same_tracking_curve_cases=[1,2],hold_cases=[0,3],no_acceleration_feedforward=True),reset=dict(profile='new-compiled-default-reset-named-robot9-overrides-v1',operation='fresh mj_resetData, zero full-scene qvel/applied/xfrc, set named robot9 q/v; one initial forward per case',HDF_init_state_used=False,not_LIBERO_task_reset_distribution=True,source_XML_sha256='95a4049c1454ffd77b2185f0a21966832c5be55af67f64c2b110cc32e617d02f',original_XML_sha256='13e900d1086ecee8e16df2876104e6b6c95e06c46a14b5860ccebb16ee1af7e5',sdk_library_sha256='f8c9735ed948d6830d1d3d31d50216ec14ca6a6e95c7346a4dc8300e1b8ffed4',sdk_version=331,disableflags=17,EULERDAMP_disabled=False,density=1.2,viscosity=.00002,wind=[0,0,0],gravity=[0,0,-9.81],NN=0,render=0,explicit_initial_forwards=4,post_step_forwards=0,other_explicit_forwards=0),mapping=mapping,metric_conventions=dict(post_states=list(range(1,751)),RMS='sqrt(sum(error_n^2)/750) perjoint; global=sqrt(sum_all7joint750errors^2/(7*750))',IAE='0.002*sum(abs(error_n)), right rectangle on poststate grid; rad*s',peak='maxabs over poststate ticks1..750',final='state tick750',final_window=dict(first_state_tick=626,last_state_tick=750,states=125),reference_rate_error=True,fingers='report slide drift and peak slide rates; no exact nineDOF equilibrium promise',recovery='first eligible entry n such that states n..n+24 satisfy BOTH maximum joint position/rate errors; confirmation=n+24; timestamp relative to pulse end250 or brake start0'),parity=parity,quality_hypotheses=quality,ABI=dict(status_bytes=16,initial_bytes=144,command_bytes=712,step_bytes=944,SDK_record='120F64+10I64=1040B;preq/v18,reference21,PD35,ctrl9,external9,SDKactuator_force9,postq/v18,time;case,tick,stage,code,PJPclip,ref_tick,ref_N,pulse,PActrlclip,PAforceclip',own_record='SDKshape with Pa generalized actuator prediction replacing SDKscalarforce, then shadowq/v18F64+four PeuclipI64=1216B',case_record='64F64:initialq9,v9,end7,vlim7,alim7,Kp7,Kd7,finger_targets2,pulse_external9=512B'),input_file=identity(FIXTURE/'cases.f64'),joint_limits_radians=limits,source_sha256=source)
    write(FIXTURE/'experiment.json',e);record=dict(schema='psionic.panda-joint-pd-tracking.input-freeze.v1',passed=True,experiment=identity(FIXTURE/'experiment.json'),inputs=identity(FIXTURE/'cases.f64'),source=source,SDK_calls=0,Coax_calls=0,analytic_references_generated=False,acquisition_authorized=False);write(FIXTURE/'input-freeze-v1.json',record);print(json.dumps(dict(passed=True,experiment=record['experiment'],inputs=record['inputs'])))
def generate(out):
    require(not out.exists(),'preserve every original/failed reference');out.mkdir(parents=True);record=dict(schema='psionic.panda-joint-pd-tracking.analytic-reference.v1',passed=False,SDK_calls=0,Coax_calls=0,NN_calls=0,old_gates_repeated=False);began=time.monotonic()
    try:
        e=json.loads((FIXTURE/'experiment.json').read_text());freeze=json.loads((FIXTURE/'input-freeze-v1.json').read_text());require(freeze['passed']and freeze['experiment']==identity(FIXTURE/'experiment.json')and e['source_sha256']==source_identity(),'frozen source/experiment');require(identity(FIXTURE/'cases.f64')==e['input_file'],'exact64F64 input cases');targets=bytearray();external=bytearray();plans=[];cache={}
        with localcontext()as context:
            context.prec=80
            for c in e['cases']:
                row=c['initial_q9'][:7]+c['end_q7']+c['velocity_limits7']+c['acceleration_limits7'];key=struct.pack('<28d',*row)
                if key not in cache:
                    plan=quintic.policy(row);require(plan['status']==0 and plan['steps']<750,'new reference finishes within measured horizon');curve=[quintic.analytic_sample(row,plan['steps'],min(n,plan['steps']))for n in range(751)];cache[key]=(plan,curve)
                plan,curve=cache[key];plans.append(dict(case=c['index'],name=c['name'],**plan))
                for values in curve:targets+=struct.pack('<21d',*finite(values,21,'ALL analytic reference fields'))
                for k in range(750):values=c['pulse_external9']if c['index']==2 and 200<=k<250 else[0.]*9;external+=struct.pack('<9d',*values)
        require(len(cache)==2 and len(targets)==4*751*168 and len(external)==4*750*72,'two new unique reference curves and all4case frames')
        arm=json.loads((ROOT/'psionic/fixtures/robotics/panda-fk/source-profile.json').read_text());branch=json.loads((ROOT/'psionic/fixtures/robotics/panda-fingers/source-profile.json').read_text());require(branch['arm_profile']['sha256']==sha(ROOT/branch['arm_profile']['path']),'arm/branch source identity');nodes=arm['nodes']+branch['nodes'];model=json.loads((ROOT/'psionic/fixtures/robotics/panda-fingers/sdk-model.json').read_text());c=e['cases'][3];g=pd.potential_gradient(nodes,model,c['initial_q9']);case={pd.KEYS[0]:c['initial_q9'][:7],pd.KEYS[2]:c['initial_v9'][:7],pd.KEYS[3]:c['end_q7'],pd.KEYS[4]:[0.]*7,pd.KEYS[5]:c['kp7'],pd.KEYS[6]:c['kd7']};initial=pd.scalar(case,g);require(initial['clipped']>0 and initial['requested'][6]<-12 and initial['command'][6]==-12,'independent initial brake saturation BEFORE SDK acquisition');write(out/'initial-brake-proof.json',dict(passed=True,case=3,gravity7=g,**initial,SDK_calls=0,Coax_calls=0))
        (out/'reference-states.f64').write_bytes(targets);(out/'external-forces.f64').write_bytes(external);write(out/'reference-plans.json',plans);require(source_identity()==e['source_sha256'],'executed source stable');record.update(passed=True,experiment=identity(FIXTURE/'experiment.json'),source_sha256=e['source_sha256'],unique_analytic_curves=2,reference_shape=[4,751,21],external_shape=[4,750,9],all_reference_finite_words=4*751*21,all_external_finite_words=4*750*9,outputs={name:identity(out/name)for name in['reference-states.f64','external-forces.f64','reference-plans.json','initial-brake-proof.json']})
    except BaseException as error:record['failure']=repr(error);raise
    finally:record['seconds']=time.monotonic()-began;write(out/'reference.json',record)
    print(json.dumps(dict(passed=True,record=str(out/'reference.json'),plans=plans)))
def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--freeze',action='store_true');parser.add_argument('--output',type=Path);args=parser.parse_args();require(args.freeze!=bool(args.output),'choose exactly inputfreeze or referencegeneration');freeze()if args.freeze else generate(args.output.resolve())
if __name__=='__main__':main()
