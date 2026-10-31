#!/usr/bin/env python3
"""New Panda FK proof: stdlib homogeneous matrices + retained/new SDK bytes.
No downloaded source imports, live SDK, model runtime or old gate is invoked.
"""
from pathlib import Path
import argparse,hashlib,json,math,os,platform,random,struct,subprocess,sys,time
ROOT=Path(__file__).resolve().parents[3]
FIXTURE=ROOT/'psionic/fixtures/robotics/panda-fk'
LIB=ROOT/'psionic/lib/robotics'
ATOL=2e-12
ROWS=(0,1,2,3,64,128,232)
SEED=0x50464b3132

def require(value,message):
    if not value:raise RuntimeError(message)
def sha(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()
def doubles(path):
    raw=Path(path).read_bytes();require(len(raw)%8==0,'F64 extent');return struct.unpack('<'+'d'*(len(raw)//8),raw)
def verify_sdk_receipt(receipt,profile,files):
    require(receipt.get('schema')=='psionic.panda-fk.sdk-reference.v1' and receipt.get('passed') is True,'SDK receipt status/schema')
    require(receipt.get('sdk_version')=='3.3.1','pinned SDK version')
    require(receipt.get('source_original_xml_sha256')==profile['source']['original_xml_sha256'] and receipt.get('source_rewritten_xml_sha256')==profile['source']['rewritten_xml_sha256'],'SDK/source scene identity')
    require(receipt.get('forward_calls')==7 and receipt.get('integration_steps')==0,'new forward-only scope')
    require(receipt.get('case_shape')==[7,7] and receipt.get('pose_shape')==[7,204],'SDK shape identity')
    require(receipt.get('absolute_tolerance_frozen_before_capture')==ATOL and receipt.get('relative_tolerance')==0,'frozen numerical budget')
    require(receipt.get('body_names')==[n['name'] for n in profile['nodes']] and receipt.get('site_name')=='gripper0_grip_site','SDK nominal order')
    names={'sdk-profile.json','sdk-model.json','sdk-cases.f64','sdk-poses.f64'}
    require(set(receipt.get('fixtures',{}))==names and set(files)==names,'SDK finite file inventory')
    for name in sorted(names):
        entry=receipt['fixtures'][name];require(len(files[name])==entry['bytes'] and hashlib.sha256(files[name]).hexdigest()==entry['sha256'],'SDK fixture byte identity: '+name)
    selected=json.loads(files['sdk-profile.json']);rows=selected['cases_radians'];require(len(rows)==7 and all(len(row)==7 for row in rows),'SDK frozen request coordinates')
    require(b''.join(struct.pack('<7d',*row) for row in rows)==files['sdk-cases.f64'],'SDK coordinate/profile binding')

def chunks(values,n):
    require(len(values)%n==0,'row shape');return [values[i:i+n] for i in range(0,len(values),n)]
def mm(a,b):return [[math.fsum(a[i][k]*b[k][j] for k in range(4)) for j in range(4)] for i in range(4)]
def identity():return [[float(i==j) for j in range(4)] for i in range(4)]
def fixed(node):
    w,x,y,z=node['fixed_quaternion_wxyz_raw'];scale=math.sqrt(math.fsum(v*v for v in (w,x,y,z)));w,x,y,z=[v/scale for v in (w,x,y,z)]
    # Full squared quaternion diagonals differ from the product's 1-2sum form.
    r=[[w*w+x*x-y*y-z*z,2*(x*y-w*z),2*(x*z+w*y)],
       [2*(x*y+w*z),w*w-x*x+y*y-z*z,2*(y*z-w*x)],
       [2*(x*z-w*y),2*(y*z+w*x),w*w-x*x-y*y+z*z]]
    out=identity()
    for i in range(3):out[i][:3]=r[i];out[i][3]=node['offset_metres'][i]
    return out

def matrix_reference(nodes,q):
    poses=[]
    for node in nodes:
        parent=identity() if node['parent']<0 else poses[node['parent']]
        pose=mm(parent,fixed(node))
        if node['joint']>=0:
            theta=q[node['joint']];c,s=math.cos(theta),math.sin(theta)
            hinge=[[c,-s,0.,0.],[s,c,0.,0.],[0.,0.,1.,0.],[0.,0.,0.,1.]]
            pose=mm(pose,hinge)
        poses.append(pose)
    return poses

def errors(actual,expected):
    require(len(actual)==len(expected),'comparison shape')
    require(all(math.isfinite(v) for v in actual),'nonfinite published FK output')
    worst=max((abs(a-b) for a,b in zip(actual,expected)),default=0.)
    require(worst<=ATOL,f'unchanged {ATOL:g} budget exceeded: {worst:.17g}')
    return {'values':len(actual),'max_abs_error':worst,'atol':ATOL,'max_budget_fraction':worst/ATOL,'passed':True}

def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--compiler',type=Path,required=True);p.add_argument('--retained',type=Path,required=True);p.add_argument('--output',type=Path,required=True);p.add_argument('--source-refusals',type=Path);a=p.parse_args()
    compiler=a.compiler.resolve();out=a.output.resolve();require(sys.platform=='darwin' and platform.machine()=='arm64','qualified Darwin-arm64 scalar-libm profile');require(sys.byteorder=='little','owned binary transport is little endian');require(not out.exists(),'output exists; preserve prior proof');out.mkdir(parents=True)
    profile=json.loads((FIXTURE/'source-profile.json').read_text());nodes=profile['nodes'];require(len(nodes)==12,'fixed12 profile')
    verify_sdk_receipt(json.loads((FIXTURE/'sdk-reference.json').read_text()),profile,{name:(FIXTURE/name).read_bytes() for name in ('sdk-profile.json','sdk-model.json','sdk-cases.f64','sdk-poses.f64')})
    sdk_model=json.loads((FIXTURE/'sdk-model.json').read_text());sdk_q=chunks(doubles(FIXTURE/'sdk-cases.f64'),7);sdk_expected=chunks(doubles(FIXTURE/'sdk-poses.f64'),204);require(len(sdk_q)==len(sdk_expected)==7,'new SDK seven cases')
    metadata=json.loads((ROOT/'psionic/tests/robotics/panda-control-qualification.json').read_text())['native_reference']['inputs']['source_samples']
    require(len(metadata)==len(ROWS) and len({m['row'] for m in metadata})==len(ROWS) and {m['row'] for m in metadata}==set(ROWS),'retained seven unique source rows')
    by_row={m['row']:m for m in metadata};retained=[];retained_receipts=[]
    for row in ROWS:
        path=a.retained/('row'+str(row)+'.pc-sample.f64');expected=by_row[row];require(path.stat().st_size==expected['bytes']==1152 and sha(path)==expected['sha256'],'retained PcSample identity')
        words=doubles(path);retained.append(words);retained_receipts.append({'row':row,'bytes':1152,'sha256':sha(path),'semantics':'Fresh reset+forward sample, not row-number-matched HDF5 post-action observation.'})
    cases=list(sdk_q)+[w[12:19] for w in retained];rng=random.Random(SEED)
    limits=[j['range'] for j in sdk_model['arm_joints']]
    for _ in range(128):cases.append(tuple(rng.uniform(lo,hi) for lo,hi in limits))
    cases.append((0.,)*7)
    for axis in range(7):
        for sign in (-1.,1.):
            q=[0.]*7;q[axis]=100*sign;cases.append(tuple(q))
    q=[0.]*7;q[3]=-0.;cases.append(tuple(q));require(len(cases)==158,'fixed corpus size')
    case_bytes=struct.pack('<II',0x314b4650,len(cases))+b''.join(struct.pack('<7d',*q) for q in cases);(out/'cases.bin').write_bytes(case_bytes)
    sources=[LIB/('panda-fk-'+name+'.coax') for name in ('types','profile','math','chain','check-fixtures')]+[Path(__file__).resolve(),ROOT/'psionic/tests/robotics/panda_fk_native.c',ROOT/'psionic/tests/robotics/panda_fk_bad_angle.coax',ROOT/'psionic/tests/robotics/panda_fk_bad_world.coax',FIXTURE/'source-profile.json',FIXTURE/'sdk-model.json',FIXTURE/'sdk-cases.f64',FIXTURE/'sdk-poses.f64',FIXTURE/'sdk-reference.json']
    frozen={str(p.relative_to(ROOT)):sha(p) for p in sources}
    record={'schema':'psionic-panda-fk-qualification-v1','passed':False,'platform':platform.platform(),'compiler_sha256':sha(compiler),'source_sha256':frozen,'corpus':{'seed':SEED,'cases':len(cases),'new_sdk_cases':7,'retained_fresh_cases':7,'independent_bounded_random_cases':128,'analytic_zero_case':1,'inclusive_coordinate_boundary_cases':14,'signed_zero_case':1,'case_sha256':sha(out/'cases.bin')},'retained_endpoints':retained_receipts,'modes':[],'negative_source_fixtures':[],'compiled_source_model_constants_scope':'Independent normalization of the byte-exact raw source table against compiled SDK constants; Coax normalization is separately covered by algebra and full FK outputs.','atol':ATOL,'rtol':0,'host_numeric_profile':'Darwin-arm64 F64, scalar libm sin/cos with -fno-builtin-sin -fno-builtin-cos, -fno-fast-math -ffp-contract=off; no cross-target deterministic math claim.','scope':'Fixed12-body/seven-hinge Coax FK only; M3 world positions and radian quantity source boundary, explicit raw attitude; no fingers/Jacobian/importer/dynamics/general typed transforms/bounded-control/backend parity.'}
    def command(name,args):
        r=subprocess.run([str(v) for v in args],cwd=ROOT,capture_output=True,timeout=180);(out/(name+'.stdout')).write_bytes(r.stdout);(out/(name+'.stderr')).write_bytes(r.stderr)
        require(r.returncode==0,(name,r.stderr.decode(errors='replace')));return r.stdout
    def save():(out/'record.json').write_text(json.dumps(record,indent=2)+'\n')
    began=time.monotonic();save()
    try:
        if a.source_refusals:
            proof=json.loads(a.source_refusals.read_text());require(proof['schema']=='psionic-panda-fk-source-refusals-v1' and proof['compiler_sha256']==record['compiler_sha256'],'reused source-refusal compiler identity');require(len(proof['checks'])==2,'two source refusals')
            for item,(name,code) in zip(proof['checks'],(('angle','E0317'),('world','E0327'))):
                path=ROOT/('psionic/tests/robotics/panda_fk_bad_'+name+'.coax');require(item['name']==name and item['diagnostic']==code and item['refused'] and item['source_sha256']==sha(path),'reused source-refusal source identity')
            record['negative_source_fixtures']=proof['checks'];record['negative_source_reused']=True
        else:
            for name,code in (('angle','E0317'),('world','E0327')):
                path=ROOT/('psionic/tests/robotics/panda_fk_bad_'+name+'.coax');r=subprocess.run([str(compiler),'check',str(path)],cwd=ROOT,capture_output=True,timeout=180)
                (out/('refusal-'+name+'.stderr')).write_bytes(r.stderr);require(r.returncode!=0 and ('error['+code+']').encode() in r.stderr,'source unit/frame refusal');record['negative_source_fixtures'].append({'name':name,'source_sha256':sha(path),'refused':True,'diagnostic':code,'stderr_sha256':hashlib.sha256(r.stderr).hexdigest()})
        outputs=[];cc=os.environ.get('CC','cc')
        for mode,extra,flags in [('debug',[],['-O0','-g']),('release',['--release'],['-O2']),('sanitized',[],['-O1','-g','-fsanitize=address,undefined','-fno-sanitize-recover=all','-fno-omit-frame-pointer'])]:
            d=out/mode;d.mkdir();header=d/'panda-fk.h';source=d/'panda-fk.c';source.write_bytes(command('emit-'+mode,[compiler,'c','psionic/lib/robotics/panda-fk-chain.coax','--library',*extra,'--header',header]))
            binary=d/'native';command('compile-'+mode,[cc,'-std=c11',*flags,'-fno-fast-math','-ffp-contract=off','-fno-builtin-sin','-fno-builtin-cos','-I',d,source,ROOT/'psionic/tests/robotics/panda_fk_native.c','-lm','-o',binary])
            native=json.loads(command('run-'+mode,[binary,out/'cases.bin',d/'poses.f64',d/'profile.bin']))
            require(native['passed'] and native['cases']==len(cases),'native case coverage');rows=chunks(doubles(d/'poses.f64'),208);require(len(rows)==len(cases),'native output coverage');outputs.append((d/'poses.f64').read_bytes())
            actual_profile=(d/'profile.bin').read_bytes();require(len(actual_profile)==profile['table_bytes'] and hashlib.sha256(actual_profile).hexdigest()==profile['table_sha256'],'exact immutable source table')
            actual=[];expected=[]
            for row,q in zip(rows,cases):
                matrices=matrix_reference(nodes,q)
                for i,pose in enumerate(matrices+[matrices[11]]):
                    actual.extend(row[i*16:i*16+3]);expected.extend(pose[k][3] for k in range(3));actual.extend(row[i*16+7:i*16+16]);expected.extend(pose[k][j] for k in range(3) for j in range(3))
            independent=errors(actual,expected)
            sdk_actual=[];sdk_gold=[]
            for i,gold in enumerate(sdk_expected):sdk_actual.extend(rows[i][:195]+rows[i][199:208]);sdk_gold.extend(gold)
            sdk=errors(sdk_actual,sdk_gold)
            old_actual=[];old_gold=[]
            for i,gold in enumerate(retained):old_actual.extend(rows[7+i][192:195]+rows[7+i][199:208]);old_gold.extend(gold[:12])
            old=errors(old_actual,old_gold)
            compiled=sdk_model['bodies'];require(len(compiled)==12,'compiled body coverage');model_actual=[];model_gold=[]
            for i,node in enumerate(nodes):
                body=compiled[i];require(body['name']==node['name'] and body['parent_name']==node['parent_name'],'compiled nominal tree');require(body['joint_count']==int(node['joint']>=0),'compiled hinge count');model_actual.extend(node['offset_metres']);model_gold.extend(body['position']);q=node['fixed_quaternion_wxyz_raw'];norm=math.sqrt(sum(v*v for v in q));model_actual.extend(v/norm for v in q);model_gold.extend(body['quaternion_wxyz'])
            model=errors(model_actual,model_gold)
            require(all(j['type_name']=='hinge' and j['axis']==[0,0,1] and j['position']==[0,0,0] and j['qpos0_reference']==0 and j['qpos_adr']==i and j['dof_adr']==i for i,j in enumerate(sdk_model['arm_joints'])),'zero-anchor/local-Z/reference profile')
            record['modes'].append({'mode':mode,'native':native,'native_c_sha256':sha(source),'native_h_sha256':sha(header),'binary_sha256':sha(binary),'output_sha256':sha(d/'poses.f64'),'independent_homogeneous_matrices':independent,'new_sdk_all_link_position_quaternion_matrix':sdk,'retained_fresh_grip_site_position_matrix':old,'compiled_source_model_constants':model});save()
        require(outputs[0]==outputs[1]==outputs[2],'debug/release/sanitized output identity')
        require({str(p.relative_to(ROOT)):sha(p) for p in sources}==frozen,'source changed during qualification');require(sha(compiler)==record['compiler_sha256'],'compiler changed during qualification');record['native_modes_byte_identical']=True;record['passed']=True
    finally:record['duration_seconds']=time.monotonic()-began;save()
    print(json.dumps({'passed':True,'cases':len(cases),'record':str(out/'record.json'),'seconds':record['duration_seconds']}))
if __name__=='__main__':main()
