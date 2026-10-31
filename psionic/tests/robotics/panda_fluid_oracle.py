"""Independent NEW offline fluid expected values; never invokes Coax or SDK.
Only authored 4x4 matrix/product-rule helper definitions are imported.
"""
from pathlib import Path
import argparse,copy,hashlib,json,math,struct,time
from panda_inverse_dynamics_matrix import body_tangents,fixed,multiply
ROOT=Path(__file__).resolve().parents[3]
FIXTURE=ROOT/'psionic/fixtures/robotics/panda-fluid'
EXPERIMENT_SHA='165382c310c8ed28025d378e413efa1fdfa190426be5f5f6d7e2e50da533f297'
ADDENDUM_SHA='773c3a35e3cb0e2a8073653ca163295081fd5bbd03a50d2b07e48b0a59c225ef'
FORCE_ATOL=5e-10
POWER_ATOL=5e-9
MINVAL=1e-15
def require(value,message):
    if not value:raise RuntimeError(message)
def sha(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()
def finite(values,count,label):
    require(len(values)==count and all(math.isfinite(x) for x in values),label+' complete finite extent')
    return list(values)
def facts():
    arm=json.loads((ROOT/'psionic/fixtures/robotics/panda-fk/source-profile.json').read_text())['nodes']
    fingers=json.loads((ROOT/'psionic/fixtures/robotics/panda-fingers/source-profile.json').read_text())['nodes']
    return arm+fingers
def compare(actual,expected,atol=FORCE_ATOL,label='force'):
    finite(actual,len(expected),label+' actual');finite(expected,len(actual),label+' expected');require(actual,label+' nonempty')
    error=max(abs(x-y) for x,y in zip(actual,expected));require(error<=atol,f'{label} {error:.17g} exceeds frozen {atol:g}')
    return {'passed':True,'values':len(actual),'max_abs_error':error,'atol':atol,'rtol':0}
def validate_source(model,experiment=None):
    experiment=experiment or json.loads((FIXTURE/'experiment.json').read_text())
    addendum_path=FIXTURE/'support-addendum-v1.json';require(sha(addendum_path)==ADDENDUM_SHA,'frozen support addendum');addendum=json.loads(addendum_path.read_text());require(addendum['schema']=='psionic.panda-fluid.support-addendum.v1' and addendum['frozen_before_numerical_comparison'] and addendum['original_experiment_sha256']==EXPERIMENT_SHA and addendum['cases_sha256']==experiment['case_sha256'] and addendum['budgets']==experiment['budgets'] and not addendum['numerical_recipe_or_case_changes'],'unchanged numerical contract across support addendum')
    require(sha(ROOT/'psionic/fixtures/robotics/panda-actuation/sdk-model.json')==addendum['model_sha256'],'addendum original model binding')
    require(model['schema']=='psionic.panda-actuation.sdk-model.v1' and model['sdk_version_number']==331,'cached source schema')
    require(model['model']['nplugin']==model['model']['ntendon']==model['model']['nflex']==0,'zero other passive systems')
    require(model['options']['density']==1.2 and model['options']['viscosity']==2e-5 and model['options']['wind']==[0,0,0],'exact fluid options')
    require(model['options']['disableflags']==model['options']['enableflags']==0,'exact source flags')
    require(set(model['callbacks'])=={'control','passive','sensor','time','contactfilter','act_dyn','act_gain','act_bias'} and all(v is True for v in model['callbacks'].values()),'source callbacks absent')
    require(all(j['stiffness']==0 and j['actuator_gravcomp']==0 for j in model['all_joint_facts']) and len(model['all_joint_facts'])==11,'zero springs/actuator gravcomp')
    require(len(model['bodies'])==26 and all(b['gravcomp']==0 for b in model['bodies']),'zero body gravcomp')
    nodes=facts();require(len(nodes)==16 and all(-1<=n['parent']<i for i,n in enumerate(nodes)),'sixteen ordered source nodes')
    prior=json.loads((ROOT/'psionic/fixtures/robotics/panda-inverse-dynamics/sdk-model.json').read_text());require(len(prior['bodies'])==16,'prior independent sixteen-body metadata')
    geoms=[]
    for i,(node,old) in enumerate(zip(nodes,prior['bodies'])):
        b=model['bodies'][i+2];require(b['id']==i+2 and b['name']==node['name']==old['name'] and b['root_id']==2,'robot identity/root')
        require(b['parent_id']==old['parent_id'] and b['mass']==old['mass'] and b['inertia']==old['principal_inertia'] and b['inertial_position']==old['inertial_position'] and b['inertial_quaternion_wxyz']==old['inertial_quaternion_wxyz'],'unchanged PID source inertials')
        finite(b['inertia'],3,'principal inertias');finite(b['inertial_position'],3,'COM offset');finite(b['inertial_quaternion_wxyz'],4,'principal quaternion');require(math.isfinite(b['mass']) and b['mass']>=0,'finite source mass')
        pose=fixed(node['offset_metres'],node['fixed_quaternion_wxyz_raw']);compiled=fixed(old['position'],old['quaternion_wxyz'])
        compare([x for row in pose for x in row],[x for row in compiled for x in row],2e-15,'source attachment matrix')
        selected=[g for g in model['geoms'] if g['body_id']==b['id']];require(len(selected)==b['geom_count'] and all(len(g['fluid12'])==12 and all(math.isfinite(x) for x in g['fluid12']) and g['fluid12'][0]==0 for g in selected),'zero robot ellipsoid selection')
        geoms += [g['id'] for g in selected]
    require(geoms==list(range(21,87)),'complete66robot geoms')
    omitted=[];omitted_chains=[]
    for index,b in enumerate(model['bodies']):
        require(b['id']==index,'all source body ids ordered')
        if 2<=index<18:continue
        omitted.append(index);at=index;seen=set();chain=[]
        while True:
            require(0<=at<26 and at not in seen,'omitted ancestor shape/cycle');seen.add(at);ancestor=model['bodies'][at];adr=ancestor['dof_adr'];count=ancestor['dof_count'];chain.append({k:ancestor[k] for k in ['id','parent_id','dof_adr','dof_count']})
            require(0<=count<=21 and (count==0 or 9<=adr<=21-count),'omitted body must have no robotDOF ancestry')
            if at==0:break
            at=ancestor['parent_id']
        omitted_chains.append({'body_id':index,'root_id':b['root_id'],'ancestor_chain_to_world':chain})
    require(omitted_chains==addendum['omitted_body_ancestry'],'complete independent ancestry/addendum equality')
    require([b['id'] for b in model['bodies'][18:23]]==list(range(18,23)) and all(b['root_id']==2 and b['dof_count']==0 for b in model['bodies'][18:23]),'fixed mount source support')
    damping=[.1]*5+[.01]*2+[100.]*2
    for i,j in enumerate(model['joints']):require(j['id']==i and j['qpos_adr']==j['dof_adr']==i and j['damping']==damping[i] and j['stiffness']==0 and j['actuator_gravcomp']==0,'matching passive joint facts')
    require(len(model['joints'])==9 and len(prior['robot_joints'])==9,'joint metadata extents')
    for i,j in enumerate(prior['robot_joints']):require(j['qpos_adr']==j['dof_adr']==i and j['position']==[0,0,0] and j['axis']==([0,0,1] if i<7 else [0,1,0]) and j['qpos0_reference']==0,'matrix tangent joint assumptions')
    old=json.loads((ROOT/'psionic/fixtures/robotics/panda-inertia/qualification.json').read_text());path='psionic/lib/robotics/panda-inertia-profile.coax';require(old['passed'] and old['source_sha256'][path]==sha(ROOT/path),'unchanged qualified PmProfile source identity')
    return {'passed':True,'robot_bodies':16,'SDK_body_ids':list(range(2,18)),'root':2,'robot_geom_ids':geoms,'joint_damping':damping,'PmProfile_body_source_carried':True,'principal_frame':'inertial COM and normalized body*inertial rotation','omitted_bodies_have_no_robotDOF_ancestry':omitted,'fixed_mounts_sharing_root2':list(range(18,23))}
def box_sides(mass,inertia):
    finite(inertia,3,'box principal inertias');require(math.isfinite(mass) and mass>=0,'finite box mass')
    if mass<MINVAL:return None
    return [math.sqrt(max(MINVAL,math.fsum(inertia)-2*inertia[i])/mass*6.) for i in range(3)]
def resistance(box,omega,linear,density=1.2,viscosity=2e-5):
    """Independent diagonal linear/quadratic resistance in principal axes."""
    finite(box,3,'box');finite(omega,3,'angular rate');finite(linear,3,'linear rate');require(all(x>0 for x in box) and density>=0 and viscosity>=0 and math.isfinite(density) and math.isfinite(viscosity),'drag coefficients')
    diameter=math.fsum(box)/3.;force=[];moment=[]
    for i in range(3):
        j,k=[axis for axis in range(3) if axis!=i]
        linear_coefficient=3.*math.pi*diameter*viscosity
        area_coefficient=.5*density*box[j]*box[k]
        angular_coefficient=math.pi*diameter**3*viscosity
        rotational_coefficient=density*box[i]*(box[j]**4+box[k]**4)/64.
        force.append(-math.fsum([linear_coefficient*linear[i],area_coefficient*abs(linear[i])*linear[i]]))
        moment.append(-math.fsum([angular_coefficient*omega[i],rotational_coefficient*abs(omega[i])*omega[i]]))
    return finite(force,3,'local force'),finite(moment,3,'local moment')
def body_oracle(pose,partials,body,rates,density=1.2,viscosity=2e-5):
    com=[pose[i][3]+math.fsum(pose[i][k]*body['inertial_position'][k] for k in range(3)) for i in range(3)]
    principal=multiply(pose,fixed([0.,0.,0.],body['inertial_quaternion_wxyz']))
    jv=[];jw=[]
    for derivative in partials:
        jv.append([derivative[i][3]+math.fsum(derivative[i][k]*body['inertial_position'][k] for k in range(3)) for i in range(3)])
        generator=[[math.fsum(derivative[i][k]*pose[n][k] for k in range(3)) for n in range(3)] for i in range(3)]
        jw.append([.5*(generator[2][1]-generator[1][2]),.5*(generator[0][2]-generator[2][0]),.5*(generator[1][0]-generator[0][1])])
    world_v=[math.fsum(jv[j][i]*rates[j] for j in range(9)) for i in range(3)];world_w=[math.fsum(jw[j][i]*rates[j] for j in range(9)) for i in range(3)]
    local_v=[math.fsum(principal[k][i]*world_v[k] for k in range(3)) for i in range(3)];local_w=[math.fsum(principal[k][i]*world_w[k] for k in range(3)) for i in range(3)]
    box=box_sides(body['mass'],body['inertia'])
    force,moment=([0.]*3,[0.]*3) if box is None else resistance(box,local_w,local_v,density,viscosity)
    world_force=[math.fsum(principal[i][k]*force[k] for k in range(3)) for i in range(3)];world_moment=[math.fsum(principal[i][k]*moment[k] for k in range(3)) for i in range(3)]
    projected=[math.fsum(jv[j][i]*world_force[i]+jw[j][i]*world_moment[i] for i in range(3)) for j in range(9)]
    local_power=math.fsum(force[i]*local_v[i]+moment[i]*local_w[i] for i in range(3));world_power=math.fsum(world_force[i]*world_v[i]+world_moment[i]*world_w[i] for i in range(3));projected_power=math.fsum(projected[j]*rates[j] for j in range(9))
    finite(projected,9,'body generalized force');finite([local_power,world_power,projected_power],3,'body power')
    return {'COM':com,'world_velocity':world_v,'world_angular_velocity':world_w,'local_velocity':local_v,'local_angular_velocity':local_w,'world_force':world_force,'world_moment':world_moment,'generalized':projected,'local_power':local_power,'world_power':world_power,'projected_power':projected_power,'virtual_work_error':max(abs(local_power-world_power),abs(local_power-projected_power)),'mass_skipped':box is None}
def oracle_case(q,v,model):
    finite(q,9,'coordinate input');finite(v,9,'rate input')
    require(all(abs(x)<=(100 if i<7 else 1) for i,x in enumerate(q)) and all(abs(x)<=(20 if i<7 else 2) for i,x in enumerate(v)),'oracle admitted coordinate/rate subset')
    poses=body_tangents(facts(),q);bodies=[body_oracle(pose,partials,model['bodies'][i+2],v) for i,(pose,partials) in enumerate(poses)]
    fluid=[math.fsum(b['generalized'][i] for b in bodies) for i in range(9)]
    damping=[model['joints'][i]['damping'] for i in range(9)];joint=[-damping[i]*v[i] for i in range(9)];passive=[math.fsum([fluid[i],joint[i]]) for i in range(9)]
    body_power=math.fsum(b['local_power'] for b in bodies);projected_power=math.fsum(fluid[i]*v[i] for i in range(9));passive_power=math.fsum(passive[i]*v[i] for i in range(9));values=finite(fluid+joint+passive,27,'oracle full output')
    return {'fluid':fluid,'joint_passive':joint,'passive':passive,'values':values,'body_power':body_power,'projected_power':projected_power,'passive_power':passive_power,'virtual_work_error':max(abs(body_power-projected_power),*[b['virtual_work_error'] for b in bodies]),'bodies':bodies}
def validate_pm_profile(raw,model):
    """NEW caller-supplied metadata witness only, not an oracle input source."""
    require(len(raw)==16*96,'sixteen PM body POD extents');nodes=facts();worst=0.;masks=[]
    for i,node in enumerate(nodes):
        joint=node['joint'] if i<12 or node['joint']<0 else node['joint']+7;mask=0 if node['parent']<0 else masks[node['parent']]
        if joint>=0:mask|=1<<joint
        masks.append(mask);values=struct.unpack_from('<11dq',raw,i*96);finite(values[:11],11,'actual PM coefficient metadata');b=model['bodies'][i+2]
        require(values[0]==b['mass'] and list(values[1:4])==b['inertial_position'] and list(values[8:11])==b['inertia'] and values[11]==mask,'PM source mass/COM/inertia/ancestor metadata')
        norm=math.sqrt(math.fsum(x*x for x in values[4:8]));require(norm>0,'PM raw quaternion');err=max(abs(x/norm-y) for x,y in zip(values[4:8],b['inertial_quaternion_wxyz']));require(err<=2e-15,'normalized PM inertial quaternion');worst=max(worst,err)
    return {'passed':True,'bodies':16,'raw_bytes':len(raw),'normalized_quaternion_max_abs':worst,'atol':2e-15,'not_used_to_generate_expected_forces':True}
def synthetic_checks():
    density=resistance([2,4,6],[1,-2,3],[-1,2,3],2,0);viscous=resistance([2,4,6],[1,-2,3],[-1,2,3],0,1/math.pi)
    metrics={'density_literal':compare(density[0]+density[1],[24,-48,-72,-97,656,-459],label='density literal'),'viscosity_literal':compare(viscous[0]+viscous[1],[12,-24,-36,-64,128,-192],label='viscosity literal')}
    node={'parent':-1,'joint':0,'offset_metres':[4,5,6],'fixed_quaternion_wxyz_raw':[1,0,0,0]};pose,partials=body_tangents([node],[math.pi/2]+[0.]*8)[0]
    body={'mass':12.,'inertia':[52.,40.,20.],'inertial_position':[1.,2.,3.],'inertial_quaternion_wxyz':[math.sqrt(.5),math.sqrt(.5),0.,0.]};got=body_oracle(pose,partials,body,[1.]+[0.]*8,2,0)
    metrics['rotated_COM_literal']=compare(got['COM']+got['world_velocity']+got['local_velocity']+got['local_angular_velocity']+got['world_force']+got['world_moment']+got['generalized'],[2,6,9]+[-1,-2,0]+[-2,0,-1]+[0,1,0]+[8,96,0]+[0,0,-164]+[-364]+[0]*8,label='COM/principal/dual literal')
    metrics['rotated_power_literal']=compare([got['local_power'],got['world_power'],got['projected_power']],[-364]*3,POWER_ATOL,'literal power')
    require(box_sides(0,[0,0,0]) is None and box_sides(math.nextafter(MINVAL,0),[1,1,1]) is None,'mass below threshold skips')
    compare(box_sides(MINVAL,[MINVAL]*3),[math.sqrt(6)]*3,label='mass endpoint active')
    compare(box_sides(6,[5,1,1]),[math.sqrt(MINVAL),math.sqrt(5),math.sqrt(5)],label='triangle floor')
    return {'passed':True,'metrics':metrics,'mass_threshold_and_triangle_floor':True}
def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--output',type=Path,required=True);a=parser.parse_args();out=a.output.resolve();require(not out.exists(),'preserve earlier/failed independent proof');out.mkdir(parents=True)
    record={'schema':'psionic.panda-fluid.independent-oracle-qualification.v1','passed':False,'SDK_calls':0,'Coax_product_calls':0,'NN_calls':0,'steps':0,'render_calls':0};began=time.monotonic()
    def save():(out/'record.json').write_text(json.dumps(record,indent=2)+'\n')
    save()
    try:
        ep=FIXTURE/'experiment.json';require(sha(ep)==EXPERIMENT_SHA,'frozen42-case experiment');e=json.loads(ep.read_text());profile=json.loads((FIXTURE/'oracle-profile.json').read_text());require(profile['frozen_before_new_oracle_comparison'] and profile['experiment_sha256']==EXPERIMENT_SHA and profile['case_shape']==[42,18] and profile['force_atol']==FORCE_ATOL and profile['power_atol_W']==POWER_ATOL,'frozen independent recipe')
        bindings=dict(e['reference_dependencies']);bindings.update(profile['frozen_definition_and_profile_hashes']);bindings.update({str(Path(__file__).resolve().relative_to(ROOT)):sha(__file__),'psionic/fixtures/robotics/panda-fluid/experiment.json':sha(ep),'psionic/fixtures/robotics/panda-fluid/support-addendum-v1.json':ADDENDUM_SHA,'psionic/fixtures/robotics/panda-fluid/oracle-profile.json':sha(FIXTURE/'oracle-profile.json'),'psionic/fixtures/robotics/panda-fluid/cases.f64':sha(FIXTURE/'cases.f64')});require(all(sha(ROOT/path)==digest for path,digest in bindings.items()),'unchanged source/reference/helper facts')
        model=json.loads((ROOT/'psionic/fixtures/robotics/panda-actuation/sdk-model.json').read_text());record['source_admission']=validate_source(model,e);record['synthetic_checks']=synthetic_checks()
        raw=(FIXTURE/'cases.f64').read_bytes();require(len(raw)==42*18*8 and sha(FIXTURE/'cases.f64')==e['case_sha256'],'42 complete frozen case rows');require(raw==b''.join(struct.pack('<18d',*c['q'],*c['v']) for c in e['cases']),'explicit case bit binding')
        old=json.loads((ROOT/'psionic/fixtures/robotics/panda-actuation/experiment.json').read_text())['cases'];require(all(struct.pack('<18d',*e['cases'][i]['q'],*e['cases'][i]['v'])==struct.pack('<18d',*old[i]['q'],*old[i]['v']) for i in range(18)),'same cached18 observation inputs')
        unique={(tuple(c['q']),tuple(c['v'])) for c in e['cases'][:18]};require(len(unique)==7 and sum(any(x!=0 for x in v) for q,v in unique)==2,'cached corpus redundancy disclosed')
        sdkraw=(ROOT/'psionic/fixtures/robotics/panda-actuation/sdk-values.f64').read_bytes();require(len(sdkraw)==18*162*8,'cached SDK extent');sdk=finite(struct.unpack('<2916d',sdkraw),2916,'ALL cached SDK facts');rows=[];diagnostics=[];maximum_work=0.;maximum_fluid_power=-math.inf;maximum_passive_power=-math.inf;odd_actual=[];odd_expected=[]
        for c in e['cases']:
            r=oracle_case(c['q'],c['v'],model);require(r['virtual_work_error']<=POWER_ATOL and r['projected_power']<=POWER_ATOL and r['passive_power']<=POWER_ATOL,'virtual-work/dissipation frozen power budget');maximum_work=max(maximum_work,r['virtual_work_error']);maximum_fluid_power=max(maximum_fluid_power,r['projected_power']);maximum_passive_power=max(maximum_passive_power,r['passive_power']);rows.append(r['values']);diagnostics.append({k:r[k] for k in ['body_power','projected_power','passive_power','virtual_work_error']});opposite=oracle_case(c['q'],[-x for x in c['v']],model);odd_actual+=opposite['values'];odd_expected +=[-x for x in r['values']]
        record['oddness']=compare(odd_actual,odd_expected,label='all42 mirrored rates');record['power']={'passed':True,'max_virtual_work_error_W':maximum_work,'max_fluid_power_W':maximum_fluid_power,'max_passive_power_W':maximum_passive_power,'atol_W':POWER_ATOL}
        actual={k:[] for k in ['fluid','joint_passive','passive']};gold={k:[] for k in actual}
        for i in range(18):
            for at,k in enumerate(actual):actual[k]+=rows[i][at*9:(at+1)*9]
            sr=sdk[i*162:(i+1)*162];gold['fluid']+=sr[45:54];gold['joint_passive'] +=[sr[18+j]+sr[27+j] for j in range(9)];gold['passive']+=sr[54:63]
        record['cached_SDK_comparisons']={k:compare(actual[k],gold[k],label='independent matrix '+k) for k in actual}
        zero=oracle_case(e['cases'][18]['q'],[0.]*9,model);record['zero_rate']=compare(zero['values'],[0.]*27,0,'zero rate')
        payload=b''.join(struct.pack('<27d',*r) for r in rows);(out/'expected.f64').write_bytes(payload);(out/'diagnostics.json').write_text(json.dumps(diagnostics,indent=2)+'\n');require(all(sha(ROOT/path)==digest for path,digest in bindings.items()),'frozen independent inputs at completion');record.update(passed=True,cases=42,cached_SDK_contexts=18,cached_unique_qv=7,cached_unique_moving=2,new_owned_contexts=24,output_shape=[42,27],output_sha256=sha(out/'expected.f64'),diagnostics_sha256=sha(out/'diagnostics.json'),source_sha256=bindings)
    except BaseException as error:record['failure']=repr(error);raise
    finally:record['seconds']=time.monotonic()-began;save()
    print(json.dumps({'passed':True,'record':str(out/'record.json'),'expected_sha256':record['output_sha256'],'SDK':record['cached_SDK_comparisons'],'power':record['power']}))
if __name__=='__main__':main()
