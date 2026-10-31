#!/usr/bin/env python3
"""New finger FK only: independent matrices, analytic branch motion, frozen SDK.
Python is test tooling; neither an SDK nor downloaded source is executed here.
"""
from pathlib import Path
import argparse,hashlib,json,math,os,platform,random,struct,subprocess,sys,time
import panda_fk_reference as arm
ROOT=Path(__file__).resolve().parents[3]
FIXTURE=ROOT/'psionic/fixtures/robotics/panda-fingers'
LIB=ROOT/'psionic/lib/robotics'
ATOL=2e-12
SEED=0x5046463136

def require(value,message):
    if not value:raise RuntimeError(message)
def sha(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()
def f64(path):
    data=Path(path).read_bytes();require(len(data)%8==0,'F64 extent');return struct.unpack('<'+'d'*(len(data)//8),data)
def rows(values,n):
    require(len(values)%n==0,'row extent');return [values[i:i+n] for i in range(0,len(values),n)]
def errors(actual,expected):
    require(len(actual)==len(expected) and len(actual)>0,'comparison extent')
    require(all(math.isfinite(v) for v in actual) and all(math.isfinite(v) for v in expected),'nonfinite comparison word')
    worst=max(abs(a-b) for a,b in zip(actual,expected));require(worst<=ATOL,f'unchanged {ATOL:g} budget exceeded: {worst:.17g}')
    return {'values':len(actual),'max_abs_error':worst,'atol':ATOL,'rtol':0,'max_budget_fraction':worst/ATOL,'passed':True}

def matrix_reference(arm_nodes,branches,q):
    # Independent full-angle homogeneous arm algebra from the frozen FK oracle.
    chain=arm.matrix_reference(arm_nodes,q[:7])
    for node in branches:
        parent=chain[node['parent']]
        child=arm.mm(parent,arm.fixed(node))
        if node['joint']>=0:
            translate=arm.identity();delta=q[7+node['joint']]-node['reference_metres']
            for i in range(3):translate[i][3]=node['axis'][i]*delta
            child=arm.mm(child,translate)
        chain.append(child)
    return chain[:12]+[chain[11]]+chain[12:]

def independent_metrics(output,cases,arm_nodes,branches):
    actual=[];gold=[];relative_actual=[];relative_gold=[];attitudes=[];attitude_gold=[]
    require(len(output)==len(cases),'case coverage')
    for row,q in zip(output,cases):
        require(len(row)==272 and all(math.isfinite(v) for v in row),'complete finite output')
        poses=matrix_reference(arm_nodes,branches,q)
        for i,p in enumerate(poses):
            actual.extend(row[i*16:i*16+3]);gold.extend(p[k][3] for k in range(3))
            actual.extend(row[i*16+7:i*16+16]);gold.extend(p[k][j] for k in range(3) for j in range(3))
        # Separate literal geometry: rotate world delta back into gripper BODY.
        # No quaternion multiplication or source branch evaluation is reused.
        parent=row[10*16:11*16];r=parent[7:16]
        local=[[-q[7],0.,.0524],[-q[7]-.0085,0.,.1084],[-q[8],0.,.0524],[-q[8]+.0085,0.,.1084]]
        fixed_rotation=[[0.,-1.,0.],[1.,0.,0.],[0.,0.,1.]]
        for i in range(4):
            pose=row[(13+i)*16:(14+i)*16];delta=[pose[j]-parent[j] for j in range(3)]
            relative_actual.extend(math.fsum(r[k*3+j]*delta[k] for k in range(3)) for j in range(3));relative_gold.extend(local[i])
            attitudes.extend(pose[7:16]);attitude_gold.extend(math.fsum(r[k*3+n]*fixed_rotation[n][j] for n in range(3)) for k in range(3) for j in range(3))
    return {'independent_homogeneous_matrices':errors(actual,gold),'analytic_parent_relative_slide_and_tip_positions':errors(relative_actual,relative_gold),'analytic_slide_independent_body_attitudes':errors(attitudes,attitude_gold)}

def verify_sdk_order(receipt,names):
    require(receipt.get('body_names')==names and receipt.get('site_name')=='gripper0_grip_site','SDK nominal pose order')

def verify_sdk(profile):
    receipt=json.loads((FIXTURE/'sdk-reference.json').read_text())
    require(receipt.get('schema')=='psionic.panda-fingers.sdk-reference.v1' and receipt.get('passed') is True,'SDK receipt schema/status')
    require(receipt.get('sdk_version')=='3.3.1' and receipt.get('forward_calls')==5 and receipt.get('integration_steps')==0,'NEW five forward-only scope')
    require(receipt.get('source_original_xml_sha256')==profile['source']['original_xml_sha256'] and receipt.get('source_rewritten_xml_sha256')==profile['source']['rewritten_xml_sha256'],'SDK scene identity')
    require(receipt.get('case_shape')==[5,9] and receipt.get('pose_shape')==[5,268],'SDK shapes')
    require(receipt.get('absolute_tolerance_frozen_before_capture')==ATOL and receipt.get('relative_tolerance')==0,'frozen pose budget')
    arm_nodes=json.loads((ROOT/profile['arm_profile']['path']).read_text())['nodes']
    verify_sdk_order(receipt,[n['name'] for n in arm_nodes+profile['nodes']])
    files=receipt.get('fixtures',{})
    required={'sdk-profile.json','sdk-model.json','sdk-cases.f64','sdk-poses.f64'}
    require(set(files)==required,'finite pose fixture inventory')
    for name,item in files.items():
        path=FIXTURE/name;require(path.stat().st_size==item['bytes'] and sha(path)==item['sha256'],'SDK fixture binding '+name)
    cases=[tuple(profile['new_sdk_frozen_arm_radians']+s) for s in profile['new_sdk_frozen_slide_pairs_metres']]
    require((FIXTURE/'sdk-cases.f64').read_bytes()==b''.join(struct.pack('<9d',*q) for q in cases),'SDK/frozen request input identity')
    gold=rows(f64(FIXTURE/'sdk-poses.f64'),268);require(len(gold)==5 and all(math.isfinite(v) for row in gold for v in row),'SDK complete finite poses')
    return receipt,cases,gold

def compiled_metrics(arm_nodes,branches):
    model=json.loads((FIXTURE/'sdk-model.json').read_text());compiled=model['bodies'];nodes=arm_nodes+branches
    require(len(compiled)==len(nodes)==16,'compiled body coverage');actual=[];gold=[]
    for node,body in zip(nodes,compiled):
        require(body['name']==node['name'] and body['parent_name']==node['parent_name'],'compiled nominal body tree')
        require(body['joint_count']==int(node['joint']>=0),'compiled joint count')
        actual.extend(node['offset_metres']);gold.extend(body['position']);q=node['fixed_quaternion_wxyz_raw'];norm=math.sqrt(math.fsum(x*x for x in q))
        actual.extend(x/norm for x in q);gold.extend(body['quaternion_wxyz'])
    joints=model['robot_joints'];require(len(joints)==9,'compiled nine joints')
    for i,joint in enumerate(joints):
        want='robot0_joint'+str(i+1) if i<7 else 'gripper0_finger_joint'+str(i-6)
        require(joint['name']==want and joint['type_name']==('hinge' if i<7 else 'slide'),'compiled joint nominal/type identity')
        require(joint['axis']==([0,0,1] if i<7 else [0,1,0]) and joint['position']==[0,0,0] and joint['qpos0_reference']==0,'compiled axes/anchor/reference')
        require(joint['qpos_adr']==i and joint['dof_adr']==i,'compiled coordinate/velocity address')
    return errors(actual,gold)

def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--compiler',type=Path,required=True);p.add_argument('--output',type=Path,required=True);a=p.parse_args()
    compiler=a.compiler.resolve();out=a.output.resolve();require(sys.platform=='darwin' and platform.machine()=='arm64' and sys.byteorder=='little','qualified Darwin-arm64 little-endian host')
    require(not out.exists(),'output exists; preserve previous proof');out.mkdir(parents=True)
    profile=json.loads((FIXTURE/'source-profile.json').read_text());arm_path=ROOT/profile['arm_profile']['path'];require(sha(arm_path)==profile['arm_profile']['sha256'],'frozen arm source profile')
    arm_profile=json.loads(arm_path.read_text());nodes=arm_profile['nodes'];branches=profile['nodes'];require(len(nodes)==12 and len(branches)==4,'fixed16 tree')
    receipt,cases,sdk_gold=verify_sdk(profile);cases=list(cases);rng=random.Random(SEED)
    limits=[j['range'] for j in json.loads((ROOT/'psionic/fixtures/robotics/panda-fk/sdk-model.json').read_text())['arm_joints']]
    for _ in range(128):cases.append(tuple(rng.uniform(lo,hi) for lo,hi in limits)+tuple(rng.uniform(-1,1) for _ in range(2)))
    cases.append((0.,)*9)
    for axis in range(9):
        for sign in (-1.,1.):
            q=[0.]*9;q[axis]=sign*(100 if axis<7 else 1);cases.append(tuple(q))
    q=[0.]*9;q[6]=q[7]=-0.;cases.append(tuple(q));require(len(cases)==153,'frozen corpus size')
    case_data=struct.pack('<II',0x31464650,len(cases))+b''.join(struct.pack('<9d',*q) for q in cases)
    require(len({struct.pack('<9d',*q) for q in cases})==len(cases),'unique complete case inventory');(out/'cases.bin').write_bytes(case_data)
    source_paths=[LIB/('panda-fingers-'+n+'.coax') for n in ('types','profile','chain','check-fixtures')]+[LIB/('panda-fk-'+n+'.coax') for n in ('types','profile','math','chain','check-fixtures')]+[Path(__file__).resolve(),Path(arm.__file__),ROOT/'psionic/tests/robotics/panda_fingers_native.c',ROOT/'psionic/tests/robotics/panda_fingers_bad_angle.coax',ROOT/'psionic/tests/robotics/panda_fingers_bad_slide.coax',FIXTURE/'source-profile.json',arm_path]+[FIXTURE/name for name in sorted(receipt['fixtures'])]+[FIXTURE/'sdk-reference.json']
    frozen={str(path.relative_to(ROOT)):sha(path) for path in source_paths}
    record={'schema':'psionic.panda-fingers.qualification.v1','passed':False,'compiler_sha256':sha(compiler),'platform':platform.platform(),'source_sha256':frozen,'atol':ATOL,'rtol':0,'corpus':{'cases':153,'seed':SEED,'new_sdk_cases':5,'independent_random_cases':128,'analytic_zero_cases':1,'arm_boundary_cases':14,'slide_boundary_cases':4,'signed_zero_cases':1,'input_sha256':sha(out/'cases.bin')},'host_numeric_profile':'Darwin-arm64 scalar libm F64 sin/cos, -fno-builtin-sin -fno-builtin-cos -fno-fast-math -ffp-contract=off. No default-fused-libm, R15 or cross-target deterministic-math claim.','scope':'Separate272F64 arm+four-finger FK output; radian/metre inputs and world Point3 positions, raw attitude boundary. No command/soft-limit/control/dynamics/importer/fullR12 transform claim.','modes':[],'negative_source_fixtures':[],'sdk_pose_receipt_sha256':sha(FIXTURE/'sdk-reference.json'),'sdk_future_dynamics_reference_qualified_here':False}
    def save():(out/'record.json').write_text(json.dumps(record,indent=2)+'\n')
    def command(name,args):
        r=subprocess.run([str(x) for x in args],cwd=ROOT,capture_output=True,timeout=180);(out/(name+'.stdout')).write_bytes(r.stdout);(out/(name+'.stderr')).write_bytes(r.stderr)
        require(r.returncode==0 and not r.stderr,(name,r.stderr.decode(errors='replace')));return r.stdout
    began=time.monotonic();save()
    try:
        for name in ('angle','slide'):
            path=ROOT/('psionic/tests/robotics/panda_fingers_bad_'+name+'.coax');r=subprocess.run([str(compiler),'check',str(path)],cwd=ROOT,capture_output=True,timeout=180)
            (out/('refusal-'+name+'.stderr')).write_bytes(r.stderr);require(r.returncode!=0 and b'error[E0317]' in r.stderr,'new source dimension refusal')
            record['negative_source_fixtures'].append({'name':name,'source_sha256':sha(path),'diagnostic':'E0317','refused':True,'stderr_sha256':hashlib.sha256(r.stderr).hexdigest()})
        payloads=[];cc=os.environ.get('CC','cc')
        for mode,extra,flags in [('debug',[],['-O0','-g']),('release',['--release'],['-O2']),('sanitized',[],['-O1','-g','-fsanitize=address,undefined','-fno-sanitize-recover=all','-fno-omit-frame-pointer'])]:
            d=out/mode;d.mkdir();source=d/'panda-fingers.c';header=d/'panda-fingers.h';source.write_bytes(command('emit-'+mode,[compiler,'c','psionic/lib/robotics/panda-fingers-chain.coax','--library',*extra,'--header',header]))
            binary=d/'native';command('compile-'+mode,[cc,'-std=c11',*flags,'-fno-fast-math','-ffp-contract=off','-fno-builtin-sin','-fno-builtin-cos','-I',d,source,ROOT/'psionic/tests/robotics/panda_fingers_native.c','-lm','-o',binary])
            native=json.loads(command('run-'+mode,[binary,out/'cases.bin',d/'poses.f64',d/'profile.bin']));require(native['passed'] and native['cases']==153 and native['output_words_per_case']==272,'native coverage')
            require((d/'poses.f64').stat().st_size==153*272*8,'native complete output bytes');payloads.append((d/'poses.f64').read_bytes())
            require((d/'profile.bin').stat().st_size==profile['table_bytes']==416 and sha(d/'profile.bin')==profile['table_sha256'],'every immutable branch table word')
            record['modes'].append({'mode':mode,'native':native,'native_c_sha256':sha(source),'native_h_sha256':sha(header),'binary_sha256':sha(binary),'output_sha256':sha(d/'poses.f64')});save()
        require(payloads[0]==payloads[1]==payloads[2],'scalar-libm debug/release/ASan byte identity');record['native_modes_byte_identical']=True
        output=rows(struct.unpack('<'+'d'*(len(payloads[0])//8),payloads[0]),272);record.update(independent_metrics(output,cases,nodes,branches))
        actual=[];gold=[]
        for row,expected in zip(output[:5],sdk_gold):actual.extend(row[:192]+row[208:272]+row[192:195]+row[199:208]);gold.extend(expected)
        record['new_sdk_all16body_and_grip_poses']=errors(actual,gold);record['compiled_source_local_poses']=compiled_metrics(nodes,branches)
        require({str(path.relative_to(ROOT)):sha(path) for path in source_paths}==frozen and sha(compiler)==record['compiler_sha256'],'source/compiler changed during qualification');record['passed']=True
    finally:record['duration_seconds']=time.monotonic()-began;save()
    print(json.dumps({'passed':True,'cases':153,'record':str(out/'record.json'),'seconds':record['duration_seconds']}))
if __name__=='__main__':main()
