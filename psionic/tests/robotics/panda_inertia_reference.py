#!/usr/bin/env python3
"""NEW M9+gravity only: retained SDK facts and independent matrix-FK differences.
No SDK, downloaded repository code, renderer, integration or neural runtime.
"""
from pathlib import Path
import argparse,ast,hashlib,json,math,os,platform,random,struct,subprocess,sys,time
import panda_fingers_reference as fingers
ROOT=Path(__file__).resolve().parents[3]
FIXTURE=ROOT/'psionic/fixtures/robotics/panda-inertia'
SOURCE=ROOT/'psionic/fixtures/robotics/panda-fingers'
LIB=ROOT/'psionic/lib/robotics'
def require(ok,message):
    if not ok:raise RuntimeError(message)
def sha(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()
def f64(path):
    data=Path(path).read_bytes();require(len(data)%8==0,'F64 extent');return struct.unpack('<'+'d'*(len(data)//8),data)
def rows(values,n):
    require(len(values)%n==0,'row shape');return [values[i:i+n] for i in range(0,len(values),n)]
def require_finite_actual(actual):
    require(len(actual)==57 and all(len(row)==90 and all(math.isfinite(v) for v in row) for row in actual),'all57x90 decoded actual words must be finite before SDK/FD maxima')
def difference(actual,expected,budget,label):
    require(len(actual)==len(expected)>0 and all(math.isfinite(v) for v in actual) and all(math.isfinite(v) for v in expected),label+' complete finite values')
    error=max(abs(a-b) for a,b in zip(actual,expected));require(error<=budget,f'{label}: {error:.17g} exceeds frozen {budget:g}')
    return {'values':len(actual),'max_abs_error':error,'atol':budget,'rtol':0,'max_budget_fraction':error/budget,'passed':True}
def left_log(plus,minus):
    d=[[math.fsum(plus[i][k]*minus[j][k] for k in range(3)) for j in range(3)] for i in range(3)]
    v=[.5*(d[2][1]-d[1][2]),.5*(d[0][2]-d[2][0]),.5*(d[1][0]-d[0][1])];s=math.sqrt(math.fsum(x*x for x in v));c=.5*(math.fsum(d[i][i] for i in range(3))-1.)
    if s==0:return [0.,0.,0.]
    return [x*math.atan2(s,c)/s for x in v]
def matrix_bodies(nodes,branches,q):
    poses=fingers.matrix_reference(nodes,branches,q)
    return poses[:12]+poses[13:]
def com(pose,offset):return [pose[i][3]+math.fsum(pose[i][j]*offset[j] for j in range(3)) for i in range(3)]
def potential(poses,bodies,gravity):
    return -math.fsum(b['mass']*math.fsum(gravity[k]*x for k,x in enumerate(com(p,b['inertial_position']))) for p,b in zip(poses,bodies))
def kinetic_reference(nodes,branches,model,q,h):
    # Derivatives of independent full-angle matrices, not product quaternion/cross
    # arithmetic. Kinetic polarization uses principal-frame angular velocities;
    # it never constructs the product's world inertia tensor.
    bodies=model['bodies'];base=matrix_bodies(nodes,branches,q);velocities=[];omegas=[];g=[]
    for j in range(9):
        plus=list(q);minus=list(q);plus[j]+=h;minus[j]-=h;p=matrix_bodies(nodes,branches,plus);m=matrix_bodies(nodes,branches,minus)
        velocities.append([[(x-y)/(2*h) for x,y in zip(com(pp,b['inertial_position']),com(mm,b['inertial_position']))] for pp,mm,b in zip(p,m,bodies)])
        omegas.append([[x/(2*h) for x in left_log(pp,mm)] for pp,mm in zip(p,m)])
        g.append((potential(p,bodies,model['gravity'])-potential(m,bodies,model['gravity']))/(2*h))
    inertial_rotations=[]
    for pose,b in zip(base,bodies):
        local=fingers.arm.fixed({'fixed_quaternion_wxyz_raw':b['inertial_quaternion_wxyz'],'offset_metres':[0.,0.,0.]})
        inertial_rotations.append(fingers.arm.mm(pose,local))
    # Each column is generated from +/- coordinate motion; principal coordinates
    # are world-to-inertial transpose products, with independent fsum reductions.
    principal_omega=[[[math.fsum(r[k][axis]*w[k] for k in range(3)) for axis in range(3)] for r,w in zip(inertial_rotations,column)] for column in omegas]
    metric=[]
    for i in range(9):
        for j in range(9):
            body_energy=[]
            for n,b in enumerate(bodies):
                translation=b['mass']*math.fsum(x*y for x,y in zip(velocities[i][n],velocities[j][n]))
                rotation=math.fsum(b['principal_inertia'][k]*principal_omega[i][n][k]*principal_omega[j][n][k] for k in range(3))
                body_energy.append(translation+rotation)
            metric.append(math.fsum(body_energy)+(model['robot_joints'][i]['armature'] if i==j else 0.))
    return metric+g
def expected_table(experiment):
    p=experiment['inertial_source'];out=bytearray()
    for i in range(16):out+=struct.pack('<11dq',p['mass'][i],*p['com'][3*i:3*i+3],*p['inertial'][4*i:4*i+4],*p['principal'][3*i:3*i+3],p['ancestors'][i])
    out+=struct.pack('<12d',*p['armature'],*p['gravity']);require(len(out)==1632,'canonical fixed inertial table');return bytes(out)
def verify_arm_profile(branch_profile):
    item=branch_profile['arm_profile'];path=ROOT/item['path'];require(item['path']=='psionic/fixtures/robotics/panda-fk/source-profile.json','exact independent arm-profile path');require(sha(path)==item['sha256'],'independent matrix arm-profile identity');return path
def verify_model(experiment,model):
    p=experiment['inertial_source'];bodies=model['bodies'];joints=model['robot_joints'];require(len(bodies)==16 and len(joints)==9,'complete compiled body/joint inventory')
    require([b['name'] for b in bodies]==experiment['body_names'],'nominal body order')
    require(model['model']['nv']==21 and model['model']['nq']==23 and model['model']['nplugin']==0,'pinned compiled scene dimensions')
    ids={b['id']:i for i,b in enumerate(bodies)}
    for i,b in enumerate(bodies):
        require(b['mass']==p['mass'][i] and b['inertial_position']==p['com'][3*i:3*i+3] and b['principal_inertia']==p['principal'][3*i:3*i+3],'exact source mass/COM/principal constants')
        raw=p['inertial'][4*i:4*i+4];length=math.sqrt(math.fsum(x*x for x in raw));normalized=[x/length for x in raw]
        require(normalized==b['inertial_quaternion_wxyz'],'independently normalized source inertial frame')
        ancestry=set();node=i
        while True:
            ancestry.add(bodies[node]['id']);parent=bodies[node]['parent_id']
            if parent not in ids:break
            node=ids[parent]
        mask=sum(1<<j for j,joint in enumerate(joints) if joint['body_id'] in ancestry)
        require(mask==p['ancestors'][i],'independent compiled-tree ancestor mask')
    for j,item in enumerate(joints):
        require(item['qpos_adr']==j and item['dof_adr']==j and item['qpos0_reference']==0 and item['position']==[0,0,0],'compiled address/reference/anchor')
        require(item['type_name']==('hinge' if j<7 else 'slide') and item['axis']==([0,0,1] if j<7 else [0,1,0]),'compiled unit axis/type')
        require(item['armature']==p['armature'][j],'exact armature coefficient')
    require(model['gravity']==p['gravity']==[0,0,-9.81],'compiled gravity')
def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--compiler',type=Path,required=True);parser.add_argument('--output',type=Path,required=True);parser.add_argument('--prepared',type=Path,help='reuse hash-bound FD/source-refusal/emission facts after a native-harness-only repair');args=parser.parse_args();compiler=args.compiler.resolve();out=args.output.resolve()
    require(sys.platform=='darwin' and platform.machine()=='arm64' and sys.byteorder=='little','qualified Darwin-arm64 little-endian target');require(not out.exists(),'preserve earlier proof');out.mkdir(parents=True)
    experiment=json.loads((FIXTURE/'experiment.json').read_text());require(experiment['frozen_before_comparison'] and experiment['SDK_M_atol']==2e-11 and experiment['SDK_g_atol']==2e-10 and experiment['FD_primary_h']==1e-5 and experiment['FD_kinetic_atol']==3e-8 and experiment['FD_potential_atol']==2e-7 and experiment['FD_h_sweep']==[1e-3,1e-4,1e-5,1e-6,1e-7],'unchanged frozen budgets')
    for name,h in experiment['frozen_dependencies'].items():require(sha(ROOT/name)==h,'frozen source changed '+name)
    for name,info in experiment['SDK_fixtures'].items():require((SOURCE/name).stat().st_size==info['bytes'] and sha(SOURCE/name)==info['sha256'],'retained SDK fixture '+name)
    sdk_receipt=json.loads((SOURCE/'sdk-dynamics.json').read_text());require(sdk_receipt['schema']=='psionic.panda-fingers.sdk-dynamics-reference.v1' and sdk_receipt['captured'] and sdk_receipt['qualified_owned_dynamics'] is False and sdk_receipt['shape']==[5,90] and sdk_receipt['additional_forward_calls']==0 and sdk_receipt['integration_steps']==0,'retained future-dynamics scope')
    model=json.loads((SOURCE/'sdk-model.json').read_text());verify_model(experiment,model);branch_profile=json.loads((SOURCE/'source-profile.json').read_text());arm_path=verify_arm_profile(branch_profile);arm_profile=json.loads(arm_path.read_text());nodes=arm_profile['nodes'];branches=branch_profile['nodes'];require(len(nodes)==12 and len(branches)==4,'fixed16 matrix input profile')
    cases=list(rows(f64(SOURCE/'sdk-cases.f64'),9));sdk_gold=rows(f64(SOURCE/'sdk-dynamics.f64'),90);require(len(cases)==len(sdk_gold)==5 and all(math.isfinite(v) for r in sdk_gold for v in r),'all450 retained finite reference words');rng=random.Random(experiment['case_seed'])
    for _ in range(32):cases.append(tuple(rng.uniform(j['range'][0]+.1*(j['range'][1]-j['range'][0]),j['range'][1]-.1*(j['range'][1]-j['range'][0])) for j in model['robot_joints']))
    cases.append((0.,)*9);require(len(cases)==38,'FD interior corpus')
    for j in range(9):
        for sign in (-1.,1.):q=[0.]*9;q[j]=sign*(100. if j<7 else 1.);cases.append(tuple(q))
    q=[0.]*9;q[0]=q[8]=-0.;cases.append(tuple(q));require(len(cases)==57 and len({struct.pack('<9d',*q) for q in cases})==57,'complete unique corpus')
    (out/'cases.bin').write_bytes(struct.pack('<II',0x314d4950,len(cases))+b''.join(struct.pack('<9d',*q) for q in cases));table=expected_table(experiment)
    sources=list(sorted(LIB.glob('panda-inertia*.coax')))+[Path(__file__).resolve(),ROOT/'psionic/tests/robotics/panda_inertia_native.c',FIXTURE/'experiment.json',ROOT/'psionic/tests/robotics/panda_fingers_reference.py',ROOT/'psionic/tests/robotics/panda_fk_reference.py',arm_path]+[ROOT/('psionic/tests/robotics/panda_inertia_bad_'+name+'.coax') for name in ('angle','slide','mass','gravity')]
    frozen={str(p.relative_to(ROOT)):sha(p) for p in sources};record={'schema':'psionic.panda-inertia.qualification.v1','passed':False,'compiler_sha256':sha(compiler),'platform':platform.platform(),'source_sha256':frozen,'experiment_sha256':sha(FIXTURE/'experiment.json'),'case_count':57,'FD_case_count':38,'case_sha256':sha(out/'cases.bin'),'source_table_sha256':hashlib.sha256(table).hexdigest(),'source_table_bytes':len(table),'numeric_profile':experiment['numeric_profile'],'scope':experiment['scope'],'SDK_work':'None:405M+45zero-rate bias words reused from the same five frozen original forwards. No SDK/link/import/load/forward/step/render/NN.','compiled_model_source_metadata':{'body_count':16,'joint_count':9,'mass_COM_principal_exact':True,'inertial_quaternions_independently_normalized_exact':True,'ancestor_masks_independent_compiled_tree_exact':True,'addresses_references_axes_armature_gravity_exact':True},'modes':[],'commands':[],'source_refusals':[]}
    def save():(out/'record.json').write_text(json.dumps(record,indent=2)+'\n')
    def command(name,argv):
        began=time.monotonic();r=subprocess.run([str(x) for x in argv],cwd=ROOT,capture_output=True,timeout=180);(out/(name+'.stdout')).write_bytes(r.stdout);(out/(name+'.stderr')).write_bytes(r.stderr);record['commands'].append({'name':name,'argv':[str(x) for x in argv],'exit':r.returncode,'seconds':time.monotonic()-began,'stderr_bytes':len(r.stderr),'stdout_sha256':hashlib.sha256(r.stdout).hexdigest(),'stderr_sha256':hashlib.sha256(r.stderr).hexdigest()});save();require(r.returncode==0 and not r.stderr,(name,r.stdout.decode(errors='replace'),r.stderr.decode(errors='replace')));return r.stdout
    prior=None
    if args.prepared:
        prior=json.loads((args.prepared/'record.json').read_text());require(prior['compiler_sha256']==sha(compiler) and prior['platform']==platform.platform() and prior['experiment_sha256']==sha(FIXTURE/'experiment.json') and prior['case_sha256']==sha(out/'cases.bin'),'prepared compiler/platform/experiment/case identity')
        require((args.prepared/'cases.bin').read_bytes()==(out/'cases.bin').read_bytes() and prior['source_table_sha256']==hashlib.sha256(table).hexdigest(),'prepared exact inputs')
        for name,h in prior['source_sha256'].items():
            if name not in (str(Path(__file__).resolve().relative_to(ROOT)),'psionic/tests/robotics/panda_inertia_native.c'):require(sha(ROOT/name)==h,'prepared source changed '+name)
        old=args.prepared/'oracle-source.py';require(sha(old)==prior['source_sha256'][str(Path(__file__).resolve().relative_to(ROOT))],'prepared executed oracle source')
        def recipe(path):return {n.name:ast.dump(n,include_attributes=False) for n in ast.parse(Path(path).read_text()).body if isinstance(n,ast.FunctionDef) and n.name in ('left_log','matrix_bodies','com','potential','kinetic_reference','expected_table','verify_model')}
        require(recipe(old)==recipe(__file__),'prepared independent numeric recipe changed');record['prepared_reuse']={'record_sha256':sha(args.prepared/'record.json'),'prior_passed':prior['passed'],'numeric_recipe_AST_exact':True,'experiment_cases_dependencies_exact':True,'repair':'Native refusal helper s+j-7 formed an out-of-bounds intermediate C pointer; changed only to s+(j-7). Failed sanitized receipt preserved. All changed-harness modes are newly qualified; earlier FD/refusal/emission facts reused without repeating them.'}
    began=time.monotonic();save()
    try:
        if prior:
            require(sha(args.prepared/'independent-fd.json')==prior['FD_table_sha256'],'prepared FD table identity');cached=json.loads((args.prepared/'independent-fd.json').read_text());require(cached['case_count']==38 and cached['primary_h']==experiment['FD_primary_h'],'prepared FD shape/profile');fd=cached['steps'];require(set(fd)=={str(h) for h in experiment['FD_h_sweep']},'prepared full step sweep')
        else:fd={str(h):[kinetic_reference(nodes,branches,model,q,h) for q in cases[:38]] for h in experiment['FD_h_sweep']}
        require(all(len(rr)==38 and all(len(r)==90 and all(math.isfinite(v) for v in r) for r in rr) for rr in fd.values()),'complete finite independent FD table')
        (out/'independent-fd.json').write_text(json.dumps({'schema':'psionic.panda-inertia.independent-fd.v1','case_count':38,'primary_h':experiment['FD_primary_h'],'steps':fd},indent=2)+'\n');record['FD_table_sha256']=sha(out/'independent-fd.json');save()
        for name,code in [('angle','E0317'),('slide','E0317'),('mass','E0330'),('gravity','E0330')]:
            path=ROOT/('psionic/tests/robotics/panda_inertia_bad_'+name+'.coax')
            if prior:
                entries=[r for r in prior['source_refusals'] if r['name']==name];require(len(entries)==1,'prepared refusal coverage');item=entries[0];require(item['source_sha256']==sha(path) and item['diagnostic']==code and item['exit']!=0 and sha(args.prepared/('refusal-'+name+'.stderr'))==item['stderr_sha256'],'prepared source refusal exact');record['source_refusals'].append({**item,'reused_not_repeated':True});continue
            r=subprocess.run([str(compiler),'check',str(path)],cwd=ROOT,capture_output=True,timeout=180);(out/('refusal-'+name+'.stdout')).write_bytes(r.stdout);(out/('refusal-'+name+'.stderr')).write_bytes(r.stderr);record['source_refusals'].append({'name':name,'source_sha256':sha(path),'exit':r.returncode,'diagnostic':code,'stderr_sha256':hashlib.sha256(r.stderr).hexdigest()});save();require(r.returncode!=0 and ('error['+code+']').encode() in r.stderr,('unit refusal',name,r.stderr.decode(errors='replace')))
        blobs=[];cc=os.environ.get('CC','cc')
        for mode,extra,flags in [('debug',[],['-O0','-g']),('release',['--release'],['-O2']),('sanitized',[],['-O1','-g','-fsanitize=address,undefined','-fno-sanitize-recover=all','-fno-omit-frame-pointer'])]:
            d=out/mode;d.mkdir();header=d/'panda-inertia.h';source=d/'panda-inertia.c';binary=d/'native'
            if prior:
                for target in (source,header):target.write_bytes((args.prepared/mode/target.name).read_bytes())
                emitted=[r for r in prior['commands'] if r['name']=='emit-'+mode];require(len(emitted)==1 and emitted[0]['exit']==0 and emitted[0]['stderr_bytes']==0 and emitted[0]['stdout_sha256']==sha(source),'prepared emitted C exact')
                require(sha(header)==prior['modes'][0]['header_sha256'],'prepared header exact');record.setdefault('reused_emissions',[]).append({'mode':mode,'C_sha256':sha(source),'header_sha256':sha(header),'executed_command':emitted[0]})
            else:source.write_bytes(command('emit-'+mode,[compiler,'c',LIB/'panda-inertia.coax','--library',*extra,'--header',header]))
            require('pm_sum_owned(' not in header.read_text(),'no public caller-POD provenance API')
            command('compile-'+mode,[cc,'-std=c11',*flags,'-fno-builtin-sin','-fno-builtin-cos','-fno-fast-math','-ffp-contract=off','-I'+str(d),source,ROOT/'psionic/tests/robotics/panda_inertia_native.c','-lm','-o',binary]);native=json.loads(command('run-'+mode,[binary,out/'cases.bin',d/'mass-gravity.f64',d/'profile.bin']));require(native['passed'] and native['cases']==57 and native['words_per_case']==90,'native coverage');blob=(d/'mass-gravity.f64').read_bytes();require(len(blob)==57*90*8 and (d/'profile.bin').read_bytes()==table,'output/source-table exact extents and every source word');blobs.append(blob);actual=rows(f64(d/'mass-gravity.f64'),90);require_finite_actual(actual)
            mass=difference([v for r in actual[:5] for v in r[:81]],[v for r in sdk_gold for v in r[:81]],experiment['SDK_M_atol'],'all405 retained SDK mass coefficients');gravity=difference([v for r in actual[:5] for v in r[81:]],[v for r in sdk_gold for v in r[81:]],experiment['SDK_g_atol'],'all45 retained SDK zero-rate bias coefficients');sweep={}
            for h in experiment['FD_h_sweep']:
                gold=fd[str(h)];me=max(abs(actual[i][j]-gold[i][j]) for i in range(38) for j in range(81));ge=max(abs(actual[i][j]-gold[i][j]) for i in range(38) for j in range(81,90));sweep[str(h)]={'cases':38,'M_values':3078,'g_values':342,'M_max_abs_error':me,'g_max_abs_error':ge,'within_primary_M_budget':me<=experiment['FD_kinetic_atol'],'within_primary_g_budget':ge<=experiment['FD_potential_atol']}
            primary=sweep[str(experiment['FD_primary_h'])];require(primary['within_primary_M_budget'] and primary['within_primary_g_budget'],'frozen primary FD comparison failed; do not select a best step');record['modes'].append({'mode':mode,'native':native,'C_sha256':sha(source),'header_sha256':sha(header),'binary_sha256':sha(binary),'output_sha256':sha(d/'mass-gravity.f64'),'source_table_exact':True,'retained_SDK_M':mass,'retained_SDK_g0':gravity,'independent_FD_sweep':sweep,'primary_h':experiment['FD_primary_h']});save()
        require(blobs[0]==blobs[1]==blobs[2],'qualified native modes byte identity');require({str(p.relative_to(ROOT)):sha(p) for p in sources}==frozen and sha(compiler)==record['compiler_sha256'],'new source/compiler changed');record['native_modes_byte_identical']=True;record['passed']=True
    except BaseException as error:record['failure']=repr(error);raise
    finally:record['seconds']=time.monotonic()-began;save()
    print(json.dumps({'passed':True,'cases':57,'words_per_mode':5130,'record':str(out/'record.json'),'seconds':record['seconds']}))
if __name__=='__main__':main()
