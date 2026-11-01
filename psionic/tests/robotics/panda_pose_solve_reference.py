#!/usr/bin/env python3
"""NEW whole-pose reference: carried matrix/tangent step, new trajectories only.

Freeze inputs before generating expected trajectories. Never invokes Coax, SDK,
NN, the old oracle main, or its 54/4200 corpus. Each physical goal is integrated
once; all budget/sign queries select prefixes of that cached trajectory.
"""
from pathlib import Path
import argparse,hashlib,json,math,struct,time
import panda_pose_ik_reference as kernel

ROOT=Path(__file__).resolve().parents[3]
FIXTURE=ROOT/'psionic/fixtures/robotics/panda-pose-solve'
KERNEL_SHA='0ac5a7b2d674c699af7c851401322e4b62bf93ac59f834b0a9ee61afd3698f04'
MATRIX_SHA='c0cc39cbce0e23b1087bb89061d8f95447202d04173cb95b81cecdbdcfd2177f'
PROFILE_SHA='0852bb3ee8d55f64bf7f1d72b902ee3496dd2ed02b5be5236a5f3d5a9a264aa3'
MODEL_SHA='4e8f3aca4884775f5d6a9a5d7c186de70ee3798fa46ed72032d657ac60e6a027'
POSE=struct.Struct('<17d2q')
QUERY=struct.Struct('<14dq')
ANSWER=struct.Struct('<q17d2q')
ANCHORS=[[.17,.23,-.14,-2.31,.11,2.73,.62],[-.43,-.17,.52,-1.87,-.61,1.43,-.82],[1.13,-.83,.71,-.93,-.48,2.17,1.26]]
NEAR=[[.012,-.018,.009,.021,-.016,.014,.011],[-.017,.013,-.022,.015,.019,-.011,-.014]]

def require(ok,message):
    if not ok:raise RuntimeError(message)
def sha(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()
def identity(path):return dict(bytes=Path(path).stat().st_size,sha256=sha(path))
def finite(values,n,label):
    require(len(values)==n and all(math.isfinite(x)for x in values),label+' ALL finite and complete');return list(values)
def write_json(path,value):path.write_text(json.dumps(value,indent=2)+'\n')
def frozen_sources():
    result={Path(__file__).resolve():sha(__file__),ROOT/'psionic/tests/robotics/panda_pose_ik_reference.py':KERNEL_SHA,ROOT/'psionic/tests/robotics/panda_inverse_dynamics_matrix.py':MATRIX_SHA,ROOT/'psionic/fixtures/robotics/panda-fk/source-profile.json':PROFILE_SHA,ROOT/'psionic/fixtures/robotics/panda-fk/sdk-model.json':MODEL_SHA}
    require(all(sha(p)==h for p,h in result.items()),'exact carried source/model identities, no old gate');return result
def load_geometry():
    profile=json.loads((ROOT/'psionic/fixtures/robotics/panda-fk/source-profile.json').read_text());model=json.loads((ROOT/'psionic/fixtures/robotics/panda-fk/sdk-model.json').read_text());nodes=profile['nodes'];limits=[j['range']for j in model['arm_joints']]
    require(len(nodes)==12 and len(limits)==7 and model['site']['body_name']==nodes[11]['name']=='gripper0_eef' and model['site']['position']==[0,0,0] and model['site']['quaternion_wxyz']==[1,0,0,0],'fixed grip SITE/model interpretation');return nodes,limits
def rotation_left_z(r,angle):
    c,s=math.cos(angle),math.sin(angle);delta=[[c,-s,0.],[s,c,0.],[0.,0.,1.]];return [[math.fsum(delta[i][k]*r[k][j]for k in range(3))for j in range(3)]for i in range(3)]
def freeze():
    sources=frozen_sources();nodes,limits=load_geometry();FIXTURE.mkdir(parents=True,exist_ok=True);require(not(FIXTURE/'experiment.json').exists(),'never overwrite a frozen experiment');contexts=[]
    def add(name,seed,target_q,budgets,kind,xyz=None,attitude=None,prescribed_status=None):
        require(all(lo<=q<=hi for q,(lo,hi)in zip(seed,limits)),'source-valid seed before freeze')
        if target_q is not None:
            require(all(lo<=q<=hi for q,(lo,hi)in zip(target_q,limits)),'known target q inside source limits');p,r,_=kernel.site_state(nodes,target_q);xyz=p;attitude=kernel.matrix_quaternion(r)
        row=finite(list(seed)+list(xyz)+list(attitude),14,'new explicit pose input');require(all(abs(x)<=10 for x in row[7:10]) and all(abs(x)<=2 for x in row[10:]) and abs(math.fsum(x*x for x in row[10:])-1)<=2e-12,'new target numerical admission')
        contexts.append(dict(index=len(contexts),name=name,kind=kind,seed_radians=list(seed),known_target_radians=target_q,target_world_metres=list(xyz),target_world_from_grip_wxyz=list(attitude),budgets=budgets,input_f64=row,prescribed_status=prescribed_status))
    for i,a in enumerate(ANCHORS):add('known-zero-'+str(i),a,a,[0,96],'known-zero')
    for i,a in enumerate(ANCHORS):
        for j,delta in enumerate(NEAR):add('near-'+str(i)+'-'+str(j),a,[x+y for x,y in zip(a,delta)],[0,1,96],'known-reachable-near')
    add('moderate-B',ANCHORS[1],[x+y for x,y in zip(ANCHORS[1],[.28,.19,-.24,-.31,.22,.16,-.27])],[4,32,96],'known-reachable-moderate')
    add('moderate-C',ANCHORS[2],[x+y for x,y in zip(ANCHORS[2],[-.37,.21,.32,-.41,-.26,.34,.29])],[4,32,96],'known-reachable-moderate')
    for i,xyz in enumerate([[3.,2.,1.],[-3.,-2.,2.]]):
        _,r,_=kernel.site_state(nodes,ANCHORS[i]);add('finite-far-'+str(i),ANCHORS[i],None,[96],'finite-far',xyz,kernel.matrix_quaternion(r))
    upper=[x[1]for x in limits];add('active-upper-inward005',[x-.005 for x in upper],upper,[1,96],'known-reachable-active-limit')
    p,r,_=kernel.site_state(nodes,ANCHORS[0]);guard_specs=[('position-only',2e-6,0.,7),('angle-only',0.,2e-6,7),('both-below',5e-7,5e-7,0),('both-above',2e-6,2e-6,7)]
    for name,position,angle,status in guard_specs:add('AND-'+name,ANCHORS[0],None,[0],'AND-guard',[p[0]+position,p[1],p[2]],kernel.matrix_quaternion(rotation_left_z(r,angle)),status)
    require(len(contexts)==18 and [c['index']for c in contexts if c['kind']=='AND-guard']==list(range(14,18)),'14physical plus4guard contexts')
    queries=[]
    for c in contexts:
        for budget in c['budgets']:
            for sign in [1,-1]:queries.append(dict(index=len(queries),context=c['index'],sign=sign,max_updates=budget,kind='guard'if c['kind']=='AND-guard'else'base'))
    require(sum(q['kind']=='base'for q in queries)==68 and sum(q['kind']=='guard'for q in queries)==8,'68base and8independent AND queries')
    context_bytes=struct.pack('<252d',*[x for c in contexts for x in c['input_f64']]);(FIXTURE/'contexts.f64').write_bytes(context_bytes)
    query_bytes=b''.join(QUERY.pack(*[x if j<10 else q['sign']*x for j,x in enumerate(contexts[q['context']]['input_f64'])],q['max_updates'])for q in queries);(FIXTURE/'base-queries.bin').write_bytes(query_bytes)
    e=dict(schema='psionic.panda-pose-solve.experiment.v1',frozen_before_expected_generation_and_product_comparison=True,profile='fixed-Panda-geometric-pose-solve-f64-v1',numeric_profile='Darwin arm64 binary64 scalar-libm,FPCR0,no fastmath/contraction; no cross-target determinism',scope='New local geometric whole-pose solver and atomic exhausted-budget refusal; no command,trajectory,collision,global convergence,nullspace,RRT,clock,#bounded,R14 or hardware claim.',target_site='gripper0_grip_site',target_frame='PfkWorld',attitude='Hamilton wxyz active world-from-grip; raw numerical boundary',joint_limits_radians=limits,target_position_box_metres=10,quaternion_component_bound=2,quaternion_norm_squared_atol=2e-12,position_tolerance_metres=1e-6,angle_tolerance_radians=1e-6,convergence='BOTH inclusive criteria on fresh initial and post-update metrics',maximum_updates=96,angular_weight_metres_per_radian=.25,diagonal_regularizer_metres_squared=.0025,maximum_step_radians=.1,pose_initial_relative_abs_w_floor=0.,carried_kernel='Unchanged pose_step function; its configurable reference-only initial-|w| guard is0 for the new trajectory context. No arithmetic/profile change.',principal_cut_abs_w=2e-14,minimum_normalized_stopping_decision_margin=.001,budgets=dict(joint_radians_atol=2e-10,position_metres_atol=2e-10,physical_rotation_matrix_max_entry_atol=2e-10,position_error_metres_atol=2e-10,angle_error_radians_atol=2e-10,weighted_error_metres_atol=2e-10,rtol=0,updates='exact',projected_coordinates='exact',status='exact',mode_bytes='exact',target_sign_pairs='exact'),physical_contexts=14,guard_contexts=4,base_queries=68,guard_queries=8,contexts=contexts,queries=queries,derived_query_rule='For every nonzero-success physical trajectory, append both signs at n-1 and n only if(context,sign,budget) not already in base; derive from cached trajectory before ANY product comparison.',trajectory_rule='Exactly one independent integration per physical goal until first BOTH-criteria success or96updates; zero-work and AND guards inspect initial metrics only. Per-step payload updates1/projections that step; successful final payload uses cumulative counts.',contexts_file=identity(FIXTURE/'contexts.f64'),base_queries_file=identity(FIXTURE/'base-queries.bin'),source_sha256={str(p.relative_to(ROOT)):h for p,h in sources.items()},old_corpora_repeated=False,SDK_calls=0,Coax_calls=0,NN_calls=0)
    write_json(FIXTURE/'experiment.json',e);require(all(sha(p)==h for p,h in sources.items()),'freeze source stability');receipt=dict(schema='psionic.panda-pose-solve.input-freeze.v1',passed=True,experiment=identity(FIXTURE/'experiment.json'),contexts=identity(FIXTURE/'contexts.f64'),queries=identity(FIXTURE/'base-queries.bin'),queries_before_generation=76,expected_generated=False,product_comparisons=0,SDK_calls=0,Coax_calls=0,NN_calls=0);write_json(FIXTURE/'input-freeze-v1.json',receipt);print(json.dumps(receipt))
def state(nodes,q,target,e,updates,projected):
    p,r,_=kernel.site_state(nodes,q);_,angle,_,relative=kernel.rotation_error(r,kernel.quat_matrix(target[3:]));distance=math.hypot(*[target[i]-p[i]for i in range(3)]);weighted=math.hypot(distance,e['angular_weight_metres_per_radian']*angle);values=finite(list(q)+p+kernel.matrix_quaternion(r)+[distance,angle,weighted],17,'fresh full pose state');normalized=max(distance/e['position_tolerance_metres'],angle/e['angle_tolerance_radians']);return dict(values=values,updates=updates,projected=projected,converged=normalized<=1,normalized_decision_margin=abs(normalized-1),relative_abs_w=abs(relative[0]))
def generate(out):
    require(not out.exists(),'preserve every prior or failed new reference');out.mkdir(parents=True);record=dict(schema='psionic.panda-pose-solve.independent-reference.v1',passed=False,SDK_calls=0,Coax_calls=0,NN_calls=0,old_corpora_repeated=False,old_reference_regenerated=False);began=time.monotonic()
    try:
        sources=frozen_sources();e=json.loads((FIXTURE/'experiment.json').read_text());freeze_receipt=json.loads((FIXTURE/'input-freeze-v1.json').read_text());require(freeze_receipt['passed'] and freeze_receipt['experiment']==identity(FIXTURE/'experiment.json')and e['source_sha256']=={str(p.relative_to(ROOT)):h for p,h in sources.items()},'exact pre-generation input/source freeze');require(e['contexts_file']==identity(FIXTURE/'contexts.f64')and e['base_queries_file']==identity(FIXTURE/'base-queries.bin'),'frozen numeric transport');nodes,limits=load_geometry();contexts=e['contexts'];require(len(contexts)==18 and e['physical_contexts']==14 and len(e['queries'])==76,'exact new context/query extents');raw=(FIXTURE/'contexts.f64').read_bytes();require(raw==struct.pack('<252d',*[x for c in contexts for x in c['input_f64']]),'context JSON/binary exact bits');finite(struct.unpack('<252d',raw),252,'ALL frozen context words');traces=[];steps=bytearray();step_index=[];diagnostics=[]
        for c in contexts:
            row=c['input_f64'];target=row[7:];current=state(nodes,row[:7],target,e,0,0);frames=[current];details=[];offset=len(steps)//POSE.size
            if c['kind']!='AND-guard':
                while not current['converged']and current['updates']<96:
                    values,counts,detail=kernel.pose_step(nodes,current['values'][:7],target[:3],target[3:],limits,e);require(counts[0]==1 and 0<=counts[1]<=7,'one carried step count');steps+=POSE.pack(*values,*counts);current=state(nodes,values[:7],target,e,current['updates']+1,current['projected']+counts[1]);require(kernel.maximum(current['values'],values)<=2e-12,'same independent fresh-state arithmetic');frames.append(current);details.append(detail)
            if c['kind']=='AND-guard':require((0 if current['converged'] else 7)==c['prescribed_status'],'predeclared independent AND outcome')
            require(all(f['normalized_decision_margin']>e['minimum_normalized_stopping_decision_margin']for f in frames),'predeclared stopping-count decision margin, before product comparison');traces.append(frames);step_index.append(dict(context=c['index'],name=c['name'],offset_rows=offset,rows=len(details),terminal_converged=current['converged'],terminal_updates=current['updates'],terminal_projected=current['projected'],initial_values=frames[0]['values'],terminal_values=current['values'],minimum_normalized_stopping_decision_margin=min(f['normalized_decision_margin']for f in frames),minimum_relative_abs_w=min(f['relative_abs_w']for f in frames),minimum_projection_boundary_margin=min((d['nearest_projection_boundary']for d in details),default=None)));diagnostics.append(dict(context=c['index'],name=c['name'],frames=frames,steps=details))
        queries=list(e['queries']);present={(q['context'],q['sign'],q['max_updates'])for q in queries}
        for index,frames in enumerate(traces[:14]):
            terminal=frames[-1]
            if terminal['converged']and terminal['updates']>0:
                for budget in [terminal['updates']-1,terminal['updates']]:
                    for sign in [1,-1]:
                        key=(index,sign,budget)
                        if key not in present:queries.append(dict(index=len(queries),context=index,sign=sign,max_updates=budget,kind='derived-stopping-boundary'));present.add(key)
        query_bytes=bytearray();answers=bytearray();outcomes=[]
        for query in queries:
            c=contexts[query['context']];row=c['input_f64'];query_bytes+=QUERY.pack(*[x if i<10 else query['sign']*x for i,x in enumerate(row)],query['max_updates']);last=traces[query['context']][-1];ok=last['converged']and last['updates']<=query['max_updates'];status=0 if ok else 7;values=last['values']if ok else[0.]*17;counts=[last['updates'],last['projected']]if ok else[0,0];answers+=ANSWER.pack(status,*values,*counts);outcomes.append(dict(**query,status=status,successful_updates=last['updates']if ok else None,successful_projected=last['projected']if ok else None,reference_first_converged_update=last['updates']if last['converged']else None,reference_terminal_updates=last['updates'],atomic_failure_payload='canonical transport zero; native must separately assert original output unchanged'if not ok else None))
        require(len(steps)%152==0 and len(query_bytes)==len(queries)*120 and len(answers)==len(queries)*160,'complete new expected transports')
        for index in range(len(queries)):finite(QUERY.unpack_from(query_bytes,index*120)[:14],14,'ALL query input floats');finite(ANSWER.unpack_from(answers,index*160)[1:18],17,'ALL expected answer floats')
        for index in range(len(steps)//152):finite(POSE.unpack_from(steps,index*152)[:17],17,'ALL expected trajectory floats')
        (out/'queries.bin').write_bytes(query_bytes);(out/'query-expected.bin').write_bytes(answers);(out/'trajectory-expected.bin').write_bytes(steps);write_json(out/'queries.json',outcomes);write_json(out/'trajectory-index.json',step_index);write_json(out/'trajectory-diagnostics.json',diagnostics)
        require(all(sha(p)==h for p,h in sources.items()),'carried kernel/new generator unchanged after new integration');physical=step_index[:14];record.update(passed=True,experiment=identity(FIXTURE/'experiment.json'),input_freeze=identity(FIXTURE/'input-freeze-v1.json'),source_sha256=e['source_sha256'],budgets=e['budgets'],physical_contexts=14,guard_contexts=4,base_queries=68,guard_queries=8,derived_queries=len(queries)-76,total_queries=len(queries),successful_queries=sum(x['status']==0 for x in outcomes),not_converged_queries=sum(x['status']==7 for x in outcomes),independent_trajectory_integrations=14,trajectory_steps=len(steps)//152,physical_successes=sum(x['terminal_converged']for x in physical),physical_nonconvergences=sum(not x['terminal_converged']for x in physical),terminal_updates=[x['terminal_updates']for x in physical],terminal_projections=[x['terminal_projected']for x in physical],minimum_normalized_stopping_decision_margin=min(x['minimum_normalized_stopping_decision_margin']for x in step_index),minimum_relative_abs_w=min(x['minimum_relative_abs_w']for x in step_index),minimum_projection_boundary_margin=min(x['minimum_projection_boundary_margin']for x in physical if x['minimum_projection_boundary_margin']is not None),maximum_Gaussian_residual=max(d['linear_residual']for c in diagnostics for d in c['steps']),query_record='input14F64+budgetI64=120B; answer statusI64+17F64+2I64=160B',trajectory_record='17F64+updatesI64(1)+per-step-projectedI64=152B; index includes initial and terminal metadata',outputs={name:identity(out/name)for name in ['queries.bin','query-expected.bin','queries.json','trajectory-expected.bin','trajectory-index.json','trajectory-diagnostics.json']})
    except BaseException as error:record['failure']=repr(error);raise
    finally:record['seconds']=time.monotonic()-began;write_json(out/'reference.json',record)
    print(json.dumps(dict(passed=True,reference=str(out/'reference.json'),physical_successes=record['physical_successes'],physical_nonconvergences=record['physical_nonconvergences'],queries=record['total_queries'],trajectory_steps=record['trajectory_steps'])))
def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--freeze',action='store_true');parser.add_argument('--output',type=Path);args=parser.parse_args();require(args.freeze!=bool(args.output),'choose exactly freeze or independent generation');freeze()if args.freeze else generate(args.output.resolve())
if __name__=='__main__':main()
