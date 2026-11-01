#!/usr/bin/env python3
"""NEW nullspace oracle: matrix tangents, exact Gram volume, Decimal projector.

No signed cofactor computation is used here. No old oracle main, SDK, NN,
product or earlier numerical corpus is executed. Inputs freeze separately.
"""
from pathlib import Path
from fractions import Fraction
from decimal import Decimal,localcontext
import argparse,hashlib,json,math,struct,time
import panda_pose_ik_reference as geometry

ROOT=Path(__file__).resolve().parents[3]
FIXTURE=ROOT/'psionic/fixtures/robotics/panda-nullspace'
TAU=2.**-24
ANSWER=struct.Struct('<q27d')
SOURCE={'psionic/tests/robotics/panda_pose_ik_reference.py':'0ac5a7b2d674c699af7c851401322e4b62bf93ac59f834b0a9ee61afd3698f04','psionic/tests/robotics/panda_inverse_dynamics_matrix.py':'c0cc39cbce0e23b1087bb89061d8f95447202d04173cb95b81cecdbdcfd2177f','psionic/fixtures/robotics/panda-fk/source-profile.json':'0852bb3ee8d55f64bf7f1d72b902ee3496dd2ed02b5be5236a5f3d5a9a264aa3','psionic/fixtures/robotics/panda-fk/sdk-model.json':'4e8f3aca4884775f5d6a9a5d7c186de70ee3798fa46ed72032d657ac60e6a027','psionic/fixtures/robotics/panda-fk/sdk-cases.f64':'482fb40c4c7bac35ff733a1cdb9de071f534959d552a69d89d94dd551787186b'}
def require(ok,message):
    if not ok:raise RuntimeError(message)
def sha(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()
def identity(path):return dict(bytes=Path(path).stat().st_size,sha256=sha(path))
def finite(values,n,label):
    require(len(values)==n and all(math.isfinite(x)for x in values),label+' ALL finite/complete');return list(values)
def write(path,value):path.write_text(json.dumps(value,indent=2)+'\n')
def flat(rows):return[x for row in rows for x in row]
def dec(value):return Decimal(value.numerator)/Decimal(value.denominator)
def sources():
    result=dict(SOURCE);result[str(Path(__file__).resolve().relative_to(ROOT))]=sha(__file__);require(all(sha(ROOT/p)==h for p,h in result.items()),'fixed original geometry/source identities');return result
def rng(seed):
    while True:seed=(1664525*seed+1013904223)&0xffffffff;yield seed
def freeze():
    bound=sources();FIXTURE.mkdir(parents=True,exist_ok=True);require(not(FIXTURE/'experiment.json').exists(),'preserve frozen experiment');model=json.loads((ROOT/'psionic/fixtures/robotics/panda-fk/sdk-model.json').read_text());limits=[j['range']for j in model['arm_joints']];home=list(struct.unpack('<7d',(ROOT/'psionic/fixtures/robotics/panda-fk/sdk-cases.f64').read_bytes()[:56]));qs=[('home',home),('home-joint6-zero',home[:5]+[0.,home[6]]),('axis-aligned-a',[0.,0.,0.,-math.pi/2,0.,0.,0.]),('axis-aligned-b',[0.,0.,0.,-2.,0.,0.,0.]),('axis-aligned-c',[.2,0.,.3,-math.pi/2,-.4,0.,.5]),('upper-inward005',[hi-.005 for lo,hi in limits]),('lower-inward005',[lo+.005 for lo,hi in limits])];random=rng(0x504e5331)
    for i in range(9):qs.append(('seeded-'+str(i),[lo+(hi-lo)*(.15+.70*((next(random)+.5)/2**32))for lo,hi in limits]))
    rest_random=rng(0x52455354);physical=[]
    for index,(name,q)in enumerate(qs):
        rest_home=home if q!=home else[lo+.25*(hi-lo)for lo,hi in limits];rests=[('self',q),('home-or-distinct-quarter',rest_home),('seeded-dense',[lo+(hi-lo)*(.20+.60*((next(rest_random)+.5)/2**32))for lo,hi in limits])]
        for rest_name,rest in rests:
            require(all(lo<=value<=hi for values in[q,rest]for value,(lo,hi)in zip(values,limits)),'physical q and rest source limits');physical.append(dict(index=len(physical),configuration=index,name=name+'/'+rest_name,q=q,rest=rest,input_f64=q+rest,presumed_singular=False))
    require(len(qs)==16 and len(physical)==48,'16configurations and48new queries');synthetic=[];d=[1.,-2.,.5,-.25,3.,-1.5,2.25]
    def add(name,a,expected):
        values=finite(flat(a)+d,49,'synthetic coefficient input');require(max(abs(x)for x in values[:42])<=4 and max(abs(x)for x in values[42:])<=6,'fixed math coefficient profile');synthetic.append(dict(index=len(synthetic),name=name,A=flat(a),d=d,input_f64=values,prescribed_status=expected))
    ident=[[float(i==j)for j in range(7)]for i in range(6)];add('identity-zero-column',ident,0)
    diagonal=[1.,.5,1.5,.25,.125,.25];last=[.2,-.3,.1,.07,-.02,.04];add('diagonal-dense-null',[[diagonal[i]*float(i==j)for j in range(6)]+[last[i]]for i in range(6)],0)
    dense=[];random=rng(0x44454e53)
    for k in range(2):
        a=[[(next(random)%9-4)/64+float(i==j)for j in range(7)]for i in range(6)];dense.append(a);add('dense-dyadic-'+str(k),a,0)
    for shift in[1,3,6]:add('column-permutation-'+str(shift),[[row[(j+shift)%7]for j in range(7)]for row in dense[0]],0)
    for shift in[1,4]:add('row-permutation-'+str(shift),[dense[0][(i+shift)%6]for i in range(6)],0)
    factors=[2.,.5,1.,2.,.25,.5];add('power-two-row-scaling',[[factors[i]*x for x in row]for i,row in enumerate(dense[0])],0)
    tie=[row[:]for row in ident];tie[0][6]=1.;tie[1][6]=-1.;add('canonical-first-largest-tie',tie,0)
    add('rank-zero',[[0.]*7 for _ in range(6)],5);a=[row[:]for row in ident];a[5]=[0.]*7;add('rank-five-zero-row',a,5);a=[row[:]for row in ident];a[5]=a[4][:];add('rank-five-duplicate-row',a,5);a=[row[:]for row in dense[0]];a[5]=[x+y for x,y in zip(a[0],a[1])];add('rank-five-dense-dependent',a,5)
    for scale in[1.,2.,.5]:
        center=TAU/scale
        for side,t in[('below',math.nextafter(center,-math.inf)),('equal',center),('above',math.nextafter(center,math.inf))]:
            a=[row[:]for row in ident];a[0]=[scale,0.,0.,0.,0.,0.,0.];a[1]=[1.,t,0.,0.,0.,0.,0.];add('restored-threshold-S'+str(scale)+'-'+side,a,0 if side=='above' else 5)
    require(len(synthetic)==24,'24predeclared synthetic matrices');(FIXTURE/'physical-inputs.f64').write_bytes(struct.pack('<672d',*[x for c in physical for x in c['input_f64']]));(FIXTURE/'synthetic-inputs.f64').write_bytes(struct.pack('<1176d',*[x for c in synthetic for x in c['input_f64']]))
    e=dict(schema='psionic.panda-nullspace.experiment.v1',frozen_before_expected_generation_and_product_comparison=True,scope='Source-bound local nullspace/posture DELTA query only. No applied candidate,clamp,finite pose preservation,global objective,collision,RRT,time,dynamics,SVD or hardware permission.',source_frame='PfkWorld',source_site='gripper0_grip_site',joint_limits_radians=limits,normalization='J columns times1rad; linear rows metres,angular rows times.25m/rad; all rows divide1m to dimensionlessA. d=(rest-q)/(1rad).',angular_weight_metres_per_radian=.25,volume_floor=TAU,volume_gate='gamma>2^-24 strict, restored unscaled physical normalized volume; product of singular values,not sigma_min',squared_norm_error_limit=2e-12,null_residual_limit=2e-10,posture_residual_limit=2e-10,posture_residual_scale='max(1,EuclideanNorm(d))',canonical_direction='largest absolute component positive;first index on exact tie',math_bounds=dict(A_absolute=4,d_absolute=6),result_fields='direction7 UnitOne,projected_delta7 Radian,null_residual6 UnitOne,weighted_physical_task_leak6 Metre,normalized_volume1 UnitOne',result_bytes=216,expected_record='I64status+27F64=224B;rank failures use unavailable zero transport ONLY after native atomicity checks',projector_record='49F64 rowmajor,unavailable zero for rank refusal',oracle='Independent homogeneous-matrix tangents; exactFraction Gram determinant for gamma^2/rank;Decimal80 Gaussian inverse ofGram forN=I-A^T inverse(Gram) A;NOcofactororacle.',budgets=dict(projector_atol=2e-8,projected_delta_radians_atol=2e-8,properties_atol=2e-10,volume_atol=2e-12,volume_rtol=2e-10,status='exact',synthetic_thresholds='exact',mode_bytes='exact'),physical=physical,synthetic=synthetic,physical_count=48,synthetic_count=24,physical_file=identity(FIXTURE/'physical-inputs.f64'),synthetic_file=identity(FIXTURE/'synthetic-inputs.f64'),source_sha256=bound,SDK_calls=0,Coax_calls=0,NN_calls=0,old_gates_repeated=False)
    write(FIXTURE/'experiment.json',e);receipt=dict(schema='psionic.panda-nullspace.input-freeze.v1',passed=True,experiment=identity(FIXTURE/'experiment.json'),physical=e['physical_file'],synthetic=e['synthetic_file'],expected_generated=False,product_comparisons=0);write(FIXTURE/'input-freeze-v1.json',receipt);print(json.dumps(receipt))
def determinant(matrix):
    a=[row[:]for row in matrix];result=Fraction(1)
    for k in range(6):
        pivot=max(range(k,6),key=lambda i:abs(a[i][k]))
        if not a[pivot][k]:return Fraction(0)
        if pivot!=k:a[k],a[pivot]=a[pivot],a[k];result=-result
        value=a[k][k];result*=value
        for i in range(k+1,6):
            factor=a[i][k]/value
            for j in range(k+1,6):a[i][j]-=factor*a[k][j]
            a[i][k]=Fraction(0)
    return result
def inverse_decimal(gram):
    a=[[dec(x)for x in row]+[Decimal(i==j)for j in range(6)]for i,row in enumerate(gram)]
    for k in range(6):
        pivot=max(range(k,6),key=lambda i:abs(a[i][k]));require(a[pivot][k]!=0,'accepted Gram nonsingular');a[k],a[pivot]=a[pivot],a[k];value=a[k][k];a[k]=[x/value for x in a[k]]
        for i in range(6):
            if i==k:continue
            factor=a[i][k];a[i]=[x-factor*y for x,y in zip(a[i],a[k])]
    return[row[6:]for row in a]
def prepare(a):
    f=[[Fraction.from_float(x)for x in row]for row in a];g=[[sum((f[i][k]*f[j][k]for k in range(7)),Fraction(0))for j in range(6)]for i in range(6)];det=determinant(g);require(det>=0,'exact Gram determinant nonnegative');gamma=dec(det).sqrt();admit=det>Fraction.from_float(TAU)**2;info=dict(gram_determinant_fraction=str(det),volume=float(gamma),rank_admitted=admit,volume_distance_to_floor=float(gamma-Decimal.from_float(TAU)))
    if not admit:return info,None,None,None
    inv=inverse_decimal(g);da=[[Decimal.from_float(x)for x in row]for row in a];ga=[[sum((inv[i][k]*da[k][j]for k in range(6)),Decimal(0))for j in range(7)]for i in range(6)];nmat=[[Decimal(i==j)-sum((da[k][i]*ga[k][j]for k in range(6)),Decimal(0))for j in range(7)]for i in range(7)];pivot=max(range(7),key=lambda i:nmat[i][i]);require(nmat[pivot][pivot]>0,'rank-one projector pivot');length=nmat[pivot][pivot].sqrt();n=[nmat[i][pivot]/length for i in range(7)];length=sum((x*x for x in n),Decimal(0)).sqrt();n=[x/length for x in n];require(abs(sum((x*x for x in n),Decimal(0))-1)<Decimal('1e-50'),'independent unit direction');require(max(abs(nmat[i][j]-n[i]*n[j])for i in range(7)for j in range(7))<Decimal('1e-50'),'independent rank-one projector');return info,da,nmat,n
def evaluate(prepared,d):
    info,a,nmat,n=prepared
    if not info['rank_admitted']:return 5,[0.]*27,[0.]*49,info
    dd=[Decimal.from_float(x)for x in d];p=[sum((nmat[i][j]*dd[j]for j in range(7)),Decimal(0))for i in range(7)];residual=[sum((a[i][j]*n[j]for j in range(7)),Decimal(0))for i in range(6)];leak=[sum((a[i][j]*p[j]for j in range(7)),Decimal(0))for i in range(6)];require(max(map(abs,residual))<Decimal('1e-50')and max(map(abs,leak))<Decimal('1e-49'),'independent linear nullspace/leak properties');values=finite(list(map(float,n+p+residual+leak))+[info['volume']],27,'ALL independent nullspace fields');projector=finite(list(map(float,flat(nmat))),49,'ALL independent projector fields');return 0,values,projector,info
def generate(out):
    require(not out.exists(),'preserve every original/failed reference');out.mkdir(parents=True);record=dict(schema='psionic.panda-nullspace.independent-reference.v1',passed=False,SDK_calls=0,Coax_calls=0,NN_calls=0,old_gates_repeated=False,cofactor_oracle=False);began=time.monotonic()
    try:
        bound=sources();e=json.loads((FIXTURE/'experiment.json').read_text());freeze=json.loads((FIXTURE/'input-freeze-v1.json').read_text());require(freeze['passed']and freeze['experiment']==identity(FIXTURE/'experiment.json')and e['source_sha256']==bound,'frozen source/input identities');require(identity(FIXTURE/'physical-inputs.f64')==e['physical_file']and identity(FIXTURE/'synthetic-inputs.f64')==e['synthetic_file'],'all frozen input bits');require((FIXTURE/'physical-inputs.f64').read_bytes()==struct.pack('<672d',*[x for c in e['physical']for x in c['input_f64']])and(FIXTURE/'synthetic-inputs.f64').read_bytes()==struct.pack('<1176d',*[x for c in e['synthetic']for x in c['input_f64']]),'JSON/binary input identity');nodes=json.loads((ROOT/'psionic/fixtures/robotics/panda-fk/source-profile.json').read_text())['nodes'];physical=bytearray();synthetic=bytearray();projectors=bytearray();matrices=bytearray();metadata=[];cache={}
        with localcontext()as ctx:
            ctx.prec=80
            for c in e['physical']:
                key=tuple(c['q'])
                if key not in cache:
                    _,_,j=geometry.site_state(nodes,c['q']);a=j[:3]+[[.25*x for x in row]for row in j[3:]];cache[key]=(a,prepare(a))
                a,prepared=cache[key];d=[r-q for r,q in zip(c['rest'],c['q'])];finite(flat(a)+d,49,'physical normalized coefficients');status,values,p,detail=evaluate(prepared,d);physical+=ANSWER.pack(status,*values);projectors+=struct.pack('<49d',*p);matrices+=struct.pack('<49d',*flat(a),*d);metadata.append(dict(kind='physical',index=c['index'],name=c['name'],status=status,**detail))
            for c in e['synthetic']:
                a=[c['A'][i*7:(i+1)*7]for i in range(6)];prepared=prepare(a);status,values,p,detail=evaluate(prepared,c['d']);require(status==c['prescribed_status'],'predeclared exact synthetic rank/threshold classification');synthetic+=ANSWER.pack(status,*values);projectors+=struct.pack('<49d',*p);metadata.append(dict(kind='synthetic',index=c['index'],name=c['name'],status=status,**detail))
        require(len(cache)==16 and len(physical)==48*224 and len(synthetic)==24*224 and len(projectors)==72*392 and len(matrices)==48*392,'all new reference extents')
        for raw in [physical,synthetic]:
            for offset in range(0,len(raw),224):finite(ANSWER.unpack_from(raw,offset)[1:],27,'ALL accepted/unavailable expected fields')
        finite(struct.unpack('<3528d',projectors),3528,'ALL accepted/unavailable projector fields');finite(struct.unpack('<2352d',matrices),2352,'ALL cached physical A/d fields')
        (out/'physical-expected.bin').write_bytes(physical);(out/'synthetic-expected.bin').write_bytes(synthetic);(out/'projector-expected.f64').write_bytes(projectors);(out/'physical-matrices.f64').write_bytes(matrices);write(out/'cases.json',metadata);require(sources()==bound,'all original executed source bytes unchanged');record.update(passed=True,experiment=identity(FIXTURE/'experiment.json'),input_freeze=identity(FIXTURE/'input-freeze-v1.json'),source_sha256=bound,budgets=e['budgets'],decimal_precision=80,unique_physical_J_evaluations=16,physical_cases=48,synthetic_cases=24,physical_ok=sum(c['status']==0 for c in metadata[:48]),physical_rank_refusals=sum(c['status']==5 for c in metadata[:48]),synthetic_ok=sum(c['status']==0 for c in metadata[48:]),synthetic_rank_refusals=sum(c['status']==5 for c in metadata[48:]),all_expected_output_finite_words=72*27,all_expected_projector_finite_words=72*49,outputs={name:identity(out/name)for name in['physical-expected.bin','synthetic-expected.bin','projector-expected.f64','physical-matrices.f64','cases.json']})
    except BaseException as error:record['failure']=repr(error);raise
    finally:record['seconds']=time.monotonic()-began;write(out/'reference.json',record)
    print(json.dumps(dict(passed=True,reference=str(out/'reference.json'),physical_ok=record['physical_ok'],physical_rank=record['physical_rank_refusals'],synthetic_ok=record['synthetic_ok'],synthetic_rank=record['synthetic_rank_refusals'])))
def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--freeze',action='store_true');parser.add_argument('--output',type=Path);args=parser.parse_args();require(args.freeze!=bool(args.output),'choose exactly freeze or generation');freeze()if args.freeze else generate(args.output.resolve())
if __name__=='__main__':main()
