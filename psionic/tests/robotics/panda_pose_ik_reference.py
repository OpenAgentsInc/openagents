#!/usr/bin/env python3
"""NEW independent principal-rotation and one geometric pose-DLS reference.

Full 4x4 matrix tangents, matrix-relative dominant-diagonal quaternion and
math.atan2, plus pivoted Gaussian6. No Coax/product quaternion recursion or
quarter-atan polynomial, SDK, NN, old harness/main or numerical gate is run.
"""
from pathlib import Path
import argparse,hashlib,json,math,struct,time
import panda_inverse_dynamics_matrix as matrix
ROOT=Path(__file__).resolve().parents[3]
FIXTURE=ROOT/'psionic/fixtures/robotics/panda-pose-ik'
EXPERIMENT_SHA='4ff40ce11ebcbfa7be54f3a02aded7d35ac3b56101541717dd365b40d7c47d88'
ADDENDUM_SHA='2acc58ffe264cbafe1b2014a1bb5f0fe738e01d7271cce51875937b6c03d2e64'
POSE_SHA='aef336dd7417caf91edcaa1ee4e27ae752b6a5fd9a845a42859c08b72721ea14'
POSE=struct.Struct('<17d2q')
def require(ok,message):
    if not ok:raise RuntimeError(message)
def sha(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()
def finite(values,count,label):
    require(len(values)==count and all(math.isfinite(x)for x in values),label+' ALL finite and complete');return list(values)
def words(path,count,label):
    raw=Path(path).read_bytes();require(len(raw)==8*count,label+' exact byte extent');return finite(struct.unpack('<'+str(count)+'d',raw),count,label)
def maximum(actual,expected):
    finite(actual,len(expected),'comparison actual');finite(expected,len(actual),'comparison expected');require(actual,'nonempty comparison');return max(abs(a-b)for a,b in zip(actual,expected))
def flat(r):return [v for row in r for v in row]
def canonical(q):
    n=math.hypot(*q);require(n>0 and math.isfinite(n),'nonzero finite quaternion');q=[x/n for x in q]
    sign=-1 if q[0]<0 else 1
    if q[0]==0:
        for x in q[1:]:
            if x!=0:sign=-1 if x<0 else 1;break
    return [0. if x==0 else sign*x for x in q]
def quat_matrix(q):
    w,x,y,z=canonical(q)
    return [[w*w+x*x-y*y-z*z,2*(x*y-w*z),2*(x*z+w*y)],
            [2*(x*y+w*z),w*w-x*x+y*y-z*z,2*(y*z-w*x)],
            [2*(x*z-w*y),2*(y*z+w*x),w*w-x*x-y*y+z*z]]
def matrix_quaternion(r):
    """Choose largest squared component; no product quaternion multiplication."""
    diagonal=[1+r[0][0]+r[1][1]+r[2][2],1+r[0][0]-r[1][1]-r[2][2],1-r[0][0]+r[1][1]-r[2][2],1-r[0][0]-r[1][1]+r[2][2]]
    k=max(range(4),key=lambda i:diagonal[i]);s=2*math.sqrt(max(0.,diagonal[k]));require(s>0,'dominant matrix-quaternion denominator')
    if k==0:q=[s/4,(r[2][1]-r[1][2])/s,(r[0][2]-r[2][0])/s,(r[1][0]-r[0][1])/s]
    elif k==1:q=[(r[2][1]-r[1][2])/s,s/4,(r[0][1]+r[1][0])/s,(r[0][2]+r[2][0])/s]
    elif k==2:q=[(r[0][2]-r[2][0])/s,(r[0][1]+r[1][0])/s,s/4,(r[1][2]+r[2][1])/s]
    else:q=[(r[1][0]-r[0][1])/s,(r[0][2]+r[2][0])/s,(r[1][2]+r[2][1])/s,s/4]
    return canonical(q)
def relative_rotation(current,target):return [[math.fsum(target[i][k]*current[j][k]for k in range(3))for j in range(3)]for i in range(3)]
def rotation_error(current,target):
    r=relative_rotation(current,target);q=matrix_quaternion(r);s=math.hypot(*q[1:]);angle=2*math.atan2(s,q[0]);vector=[angle*x/s for x in q[1:]]if s else[0.,0.,0.]
    finite(vector+[angle]+flat(r),13,'matrix principal rotation');return vector,angle,r,q
def vector_matrix(vector,angle=None):
    n=math.hypot(*vector)
    if n==0:return [[float(i==j)for j in range(3)]for i in range(3)]
    theta=n if angle is None else angle;s=math.sin(theta/2);return quat_matrix([math.cos(theta/2)]+[s*x/n for x in vector])
def gaussian6(a,b):
    finite(flat(a)+list(b),42,'Gaussian6 operands');rows=[list(a[i])+[b[i]]for i in range(6)]
    for k in range(6):
        p=max(range(k,6),key=lambda i:abs(rows[i][k]));require(abs(rows[p][k])>1e-15,'Gaussian6 nonsingular regularized pivot');rows[k],rows[p]=rows[p],rows[k];pivot=rows[k][k]
        for j in range(k,7):rows[k][j]/=pivot
        for i in range(6):
            if i==k:continue
            factor=rows[i][k]
            for j in range(k,7):rows[i][j]-=factor*rows[k][j]
    result=[r[6]for r in rows];finite(result,6,'Gaussian6 result');residual=max(abs(math.fsum(a[i][j]*result[j]for j in range(6))-b[i])for i in range(6));require(residual<1e-11,'Gaussian6 residual');return result,residual
def site_state(nodes,q):
    poses=matrix.body_tangents(nodes,q);require(len(poses)==12,'fixed12-body tangent chain');pose,derivatives=poses[11];r=[row[:3]for row in pose[:3]];p=[pose[i][3]for i in range(3)];j=[[0.]*7 for _ in range(6)]
    for k in range(7):
        d=derivatives[k];w=[[math.fsum(d[i][a]*r[n][a]for a in range(3))for n in range(3)]for i in range(3)]
        for i in range(3):j[i][k]=d[i][3]
        for i,value in enumerate([.5*(w[2][1]-w[1][2]),.5*(w[0][2]-w[2][0]),.5*(w[1][0]-w[0][1])]):j[i+3][k]=value
    finite(p+flat(r)+flat(j),54,'fresh independent grip pose/WORLD Jacobian');return p,r,j
def pose_step(nodes,seed,xyz,attitude,limits,e):
    p,r,j=site_state(nodes,seed);target=quat_matrix(attitude);vector,angle,_,relative=rotation_error(r,target);require(abs(relative[0])>=e['pose_initial_relative_abs_w_floor'],'frozen initial pose error away from principal cut');weight=e['angular_weight_metres_per_radian'];j6=[row[:]for row in j[:3]]+[[weight*x for x in row]for row in j[3:]];residual=[xyz[i]-p[i]for i in range(3)]+[weight*x for x in vector];normal=[[math.fsum(j6[i][k]*j6[n][k]for k in range(7))+(e['diagonal_regularizer_metres_squared']if i==n else 0.)for n in range(6)]for i in range(6)];solution,linear=gaussian6(normal,residual);delta=[math.fsum(j6[i][k]*solution[i]for i in range(6))for k in range(7)];largest=max(abs(x)for x in delta)
    if largest>e['maximum_step_radians']:delta=[x*(e['maximum_step_radians']/largest)for x in delta]
    unprojected=[seed[k]+delta[k]for k in range(7)];projected=sum(x<lo or x>hi for x,(lo,hi)in zip(unprojected,limits));q=[min(hi,max(lo,x))for x,(lo,hi)in zip(unprojected,limits)];position,fresh_r,_=site_state(nodes,q);_,orientation,_,_=rotation_error(fresh_r,target);distance=math.hypot(*[xyz[i]-position[i]for i in range(3)]);weighted=math.hypot(distance,weight*orientation);quat=matrix_quaternion(fresh_r);values=finite(q+position+quat+[distance,orientation,weighted],17,'complete one-step pose target');margin=min(abs(x-y)for x,pair in zip(unprojected,limits)for y in pair);return values,[1,projected],dict(linear_residual=linear,unscaled_delta_infinity_norm=largest,projected=projected,nearest_projection_boundary=margin,initial_angle=angle)
def literal_checks():
    identity=[[float(i==j)for j in range(3)]for i in range(3)];halfturn=[[1.,0.,0.],[0.,-1.,0.],[0.,0.,-1.]];v,a,r,_=rotation_error(identity,halfturn);require(maximum(v,[math.pi,0.,0.])<1e-15 and a==math.pi and maximum(flat(vector_matrix(v,a)),flat(r))<1e-15,'literal exact-pi x tie');require(rotation_error(identity,identity)[0]==[0.,0.,0.],'literal identity log')
    diag=[[float(i==j)*(i+1)for j in range(6)]for i in range(6)];b=[(i+1)/8 for i in range(6)];result,_=gaussian6(diag,b);require(result==[.125]*6,'literal independent Gaussian diagonal');return dict(passed=True,checks=3,Coax_calls=0,SDK_calls=0)
def load_corpus():
    require(sha(FIXTURE/'experiment.json')==EXPERIMENT_SHA and sha(FIXTURE/'precomparison-input-addendum-v1.json')==ADDENDUM_SHA,'frozen original/addendum identities');e=json.loads((FIXTURE/'experiment.json').read_text());a=json.loads((FIXTURE/'precomparison-input-addendum-v1.json').read_text());require(e['frozen_before_product_arithmetic'] and e['pose_case_count']==54 and e['rotation_case_count']==4200 and e['principal_cut']['abs_relative_w_at_most']==2e-14 and e['budgets']['rtol']==0,'frozen corpus/branch/budgets');require(a['schema']=='psionic.panda-pose-ik.precomparison-input-addendum.v1' and a['frozen_before_expected_generation_and_product_comparison'] and a['original_experiment_sha256']==EXPERIMENT_SHA and a['replacement_file']=='pose-cases-v2.f64' and a['replacement_pose_cases_sha256']==POSE_SHA,'additive precomparison source revision')
    dependencies={ROOT/name:digest for name,digest in e['source_dependencies'].items()};require(len(dependencies)==15 and all(sha(p)==digest for p,digest in dependencies.items()),'all15 independently frozen source/data dependencies');original_path=FIXTURE/'pose-cases.f64';effective_path=FIXTURE/'pose-cases-v2.f64';require(sha(original_path)==e['pose_cases_sha256']==a['original_pose_cases_sha256'] and sha(effective_path)==POSE_SHA,'original/effective pose input identities');original=words(original_path,756,'original54poses');effective=words(effective_path,756,'effective54poses');require(original_path.read_bytes()==struct.pack('<756d',*[x for c in e['pose_cases']for key in ['seed_radians','target_world_metres','target_world_from_grip_wxyz']for x in c[key]]),'original explicit JSON pose bits');require([c['case_index']for c in a['changes']]==[22,23,24,25],'exact four approved revision rows')
    for index in range(54):
        old=original[index*14:(index+1)*14];new=effective[index*14:(index+1)*14]
        if index not in [22,23,24,25]:require(struct.pack('<14d',*old)==struct.pack('<14d',*new),'allother50rows unchanged');continue
        change=a['changes'][index-22];require(change['case_name']==e['pose_cases'][index]['name'] and old[:7]==change['old_seed_radians'] and new[:7]==change['new_seed_radians'] and list(struct.unpack('<7Q',struct.pack('<7d',*old[:7])))==change['old_seed_u64'] and list(struct.unpack('<7Q',struct.pack('<7d',*new[:7])))==change['new_seed_u64'],'revision explicit old/new bits');require(new[:7]==[x+(.01 if index<24 else -.01)for x in old[:7]] and struct.pack('<7d',*old[7:])==struct.pack('<7d',*new[7:]) and change['target_bytes_unchanged'],'only approved inward seeds, no target tuning')
    require(sha(FIXTURE/'rotation-cases.f64')==e['rotation_cases_sha256'] and sha(FIXTURE/'rotation-flags.u8')==e['rotation_flags_sha256'],'unchanged frozen SO3 corpus/mask');rotations=words(FIXTURE/'rotation-cases.f64',33600,'all4200 current/target quaternion pairs');flags=(FIXTURE/'rotation-flags.u8').read_bytes();require(len(flags)==4200 and all(x in [0,1,3]for x in flags) and sum(bool(x&1)for x in flags)==1008 and sum(bool(x&2)for x in flags)==112,'exact cut/literal-pi branch inventory');profile=json.loads((ROOT/'psionic/fixtures/robotics/panda-fk/source-profile.json').read_text());model=json.loads((ROOT/'psionic/fixtures/robotics/panda-fk/sdk-model.json').read_text());nodes=profile['nodes'];require(len(nodes)==12 and model['site']['body_name']==nodes[11]['name']=='gripper0_eef' and model['site']['position']==[0,0,0] and model['site']['quaternion_wxyz']==[1,0,0,0],'fixed coincident grip SITE, not hand BODY');require(e['joint_limits_radians']==[j['range']for j in model['arm_joints']] and all(j['qpos0_reference']==0 and j['axis']==[0,0,1] and j['position']==[0,0,0]for j in model['arm_joints']),'source limits/zero anchors/references/axes');dependencies.update({FIXTURE/name:sha(FIXTURE/name)for name in ['experiment.json','precomparison-input-addendum-v1.json','pose-cases.f64','pose-cases-v2.f64','rotation-cases.f64','rotation-flags.u8']});dependencies[Path(__file__).resolve()]=sha(__file__);return e,effective,rotations,flags,nodes,dependencies
def verify_targets(e,poses,nodes):
    limits=e['joint_limits_radians'];geometry=[];fk=ROOT/'psionic/fixtures/robotics/panda-fk';sdk_q=words(fk/'sdk-cases.f64',49,'all7 source-valid SDK seeds');sdk_pose=words(fk/'sdk-poses.f64',1428,'all7 original SDK all-link pose facts');require([sdk_q[i*7+3]for i in range(7)]==e['selected_original_SDK_q4'],'actual selected source-valid q4 identities')
    for index,c in enumerate(e['pose_cases']):
        row=poses[index*14:(index+1)*14];seed=row[:7];xyz=row[7:10];quat=row[10:];require(all(lo<=x<=hi for x,(lo,hi)in zip(seed,limits)) and all(abs(x)<=10 for x in xyz) and max(abs(x)for x in quat)<=2 and abs(math.fsum(x*x for x in quat)-1)<=2e-12,'every effective pose admitted before comparison');source=c['source'];kind=source['kind'];observed=None
        if 'path'in source:
            values=words(ROOT/source['path'],144,'original PcSample');require(struct.pack('<3d',*xyz)==struct.pack('<3d',*values[:3]) and source['matching_q_radians']==values[12:19],'original fresh SITE position/q provenance');observed=[values[3+3*i:6+3*i]for i in range(3)];require(source['observed_rotation']==observed,'original fresh SITE matrix provenance')
        elif 'row'in source:
            j=source['row'];require(0<=j<7 and source['matching_q_radians']==sdk_q[j*7:(j+1)*7] and xyz==sdk_pose[j*204+192:j*204+195],'original cached SDK SITE position/q provenance');observed=[sdk_pose[j*204+195+3*i:j*204+198+3*i]for i in range(3)];require(flat(observed)==source['observed_rotation'],'original cached SDK SITE matrix provenance')
        if 'matching_q_radians'in source:
            target_q=source['matching_q_radians'];require(all(lo<=x<=hi for x,(lo,hi)in zip(target_q,limits)),'target-source q uses legal source limits');p,r,_=site_state(nodes,target_q);error=maximum(p,xyz);require(error<=2e-12 and maximum(flat(quat_matrix(quat)),flat(r))<=2e-12,'NEW target/knownq matrix consistency');geometry.append(error)
        if observed is not None:require(maximum(flat(quat_matrix(quat)),flat(observed))<=2e-12,'target attitude reconstructs original SITE rotation')
    return dict(passed=True,cases=54,original_PcSample_target_records=14,original_SDK_forward_target_records=14,new_matrix_targets=24,finite_far_targets=2,max_known_q_target_position_error=max(geometry),SDK_calls=0,old_FK_gate_repeated=False)
def generate(out):
    require(not out.exists(),'preserve earlier/failed independent reference');out.mkdir(parents=True);record=dict(schema='psionic.panda-pose-ik.independent-reference.v1',passed=False,SDK_calls=0,Coax_calls=0,NN_calls=0,old_gates_repeated=False);began=time.monotonic()
    def save():(out/'reference.json').write_text(json.dumps(record,indent=2)+'\n')
    save()
    try:
        e,poses,rotations,flags,nodes,dependencies=load_corpus();record.update(experiment_sha256=EXPERIMENT_SHA,addendum_sha256=ADDENDUM_SHA,effective_pose_cases_sha256=POSE_SHA,budgets=e['budgets'],source_sha256={str(p.relative_to(ROOT)):h for p,h in sorted(dependencies.items())},literal_checks=literal_checks());record['source_target_checks']=verify_targets(e,poses,nodes);rotation_output=[];grid=e['rotation_grid'];threshold=e['principal_cut']['abs_relative_w_at_most']
        for index in range(4200):
            pair=rotations[index*8:(index+1)*8];require(all(abs(x)<=2 for x in pair) and abs(math.fsum(x*x for x in pair[:4])-1)<=2e-12 and abs(math.fsum(x*x for x in pair[4:])-1)<=2e-12,'all standalone inputs admitted');current=quat_matrix(pair[:4]);target=quat_matrix(pair[4:]);vector,angle,r,q=rotation_error(current,target);flag=flags[index];angle_index=(index//2)%25;axis_index=(index//50)%28;current_index=index//1400;theta=grid['angles_radians'][angle_index];expected_cut=abs(math.cos(theta/2))<=threshold;expected_pi=current_index==0 and abs(theta)==math.pi;require(bool(flag&1)==expected_cut and bool(flag&2)==expected_pi and bool(flag&1)==(abs(q[0])<=threshold),'frozen source and independent matrix cut classification')
            if flag&2:
                axis=canonical([0.]+grid['axes'][axis_index])[1:];wanted=[math.pi*x for x in axis];require(maximum(vector,wanted)<=2e-12 and abs(angle-math.pi)<=2e-12,'literal exact-pi canonical-axis tie')
            require(maximum(flat(vector_matrix(vector,angle)),flat(r))<=2e-12,'independent log reconstructs physical rotation');rotation_output.extend(vector+[angle]+flat(r))
        finite(rotation_output,4200*13,'ALL standalone expected fields');pose_output=bytearray();diagnostics=[]
        for index in range(54):
            row=poses[index*14:(index+1)*14];values,counts,detail=pose_step(nodes,row[:7],row[7:10],row[10:],e['joint_limits_radians'],e);pose_output+=POSE.pack(*values,*counts);diagnostics.append(dict(case=index,**detail))
        require(len(pose_output)==54*152,'complete pose target/counter extent');(out/'rotation-expected.f64').write_bytes(struct.pack('<54600d',*rotation_output));(out/'rotation-flags.u8').write_bytes(flags);(out/'pose-expected.bin').write_bytes(pose_output);(out/'pose-diagnostics.json').write_text(json.dumps(diagnostics,indent=2)+'\n');record.update(passed=True,rotation_cases=4200,rotation_expected_shape=[4200,13],rotation_fields='WORLDvec3,angle,relativeR9 rowmajor',ordinary_vector_cases=sum(not(x&1)for x in flags),cut_cases=sum(bool(x&1)for x in flags),exact_pi_cases=sum(bool(x&2)for x in flags),pose_cases=54,pose_expected_record='17F64,updatesInt64,projectedInt64',projection_counts=[x['projected']for x in diagnostics],minimum_projection_boundary_margin=min(x['nearest_projection_boundary']for x in diagnostics),maximum_Gaussian_residual=max(x['linear_residual']for x in diagnostics),outputs={name:dict(bytes=(out/name).stat().st_size,sha256=sha(out/name))for name in ['rotation-expected.f64','rotation-flags.u8','pose-expected.bin','pose-diagnostics.json']},profile='independent matrix/tangent WORLD geometry, dominant-diagonal relative quaternion +math.atan2; pivoted Gaussian6; no product quaternion recursion/quarter-atan')
        require(all(sha(p)==h for p,h in dependencies.items()),'all frozen source/corpus identities at completion')
    except BaseException as error:record['passed']=False;record['failure']=repr(error);raise
    finally:record['seconds']=time.monotonic()-began;save()
    print(json.dumps(dict(passed=True,reference=str(out/'reference.json'),rotations=4200,poses=54,outputs=record['outputs'])))
def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--output',type=Path,required=True);args=parser.parse_args();generate(args.output.resolve())
if __name__=='__main__':main()
