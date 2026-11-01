#!/usr/bin/env python3
"""New local posture projection proof; cached Gram/Gaussian oracle only.

No applied candidate, old SDK/FK/pose/rotor gate, or cofactor target generator.
"""
from pathlib import Path
from fractions import Fraction
import argparse,hashlib,json,math,os,platform,struct,time
from panda_pose_ik_native_reference import Run,identity,sha,graph,finite,require,metric
ROOT=Path(__file__).resolve().parents[3];FIXTURE=ROOT/'psionic/fixtures/robotics/panda-nullspace';SOURCE=ROOT/'psionic/tests/robotics/panda-nullspace-native.coax';NATIVE=ROOT/'psionic/tests/robotics/panda_nullspace_native.c'
EXPERIMENT_SHA='76c0e1cf3621e7990cc91d1f7dbbc986017c81d22c4b360fc9e4f1945f35a8d6'
ROW=struct.Struct('<q27d')

def frozen(oracle):
    require(sha(FIXTURE/'experiment.json')==EXPERIMENT_SHA,'frozen new local nullspace experiment');e=json.loads((FIXTURE/'experiment.json').read_text());r=json.loads((oracle/'reference.json').read_text())
    require(r['passed'] and not r['cofactor_oracle'] and r['decimal_precision']==80 and r['experiment']==identity(FIXTURE/'experiment.json') and r['budgets']==e['budgets'] and r['SDK_calls']==r['Coax_calls']==0,'independent Gram/Gaussian reference')
    require(all(identity(oracle/name)==value for name,value in r['outputs'].items()) and all(sha(ROOT/name)==digest for name,digest in r['source_sha256'].items()),'exact independent source/target bytes')
    physical=(FIXTURE/'physical-inputs.f64').read_bytes();synthetic=(FIXTURE/'synthetic-inputs.f64').read_bytes();require(len(physical)==48*112 and len(synthetic)==24*392 and identity(FIXTURE/'physical-inputs.f64')==e['physical_file'] and identity(FIXTURE/'synthetic-inputs.f64')==e['synthetic_file'],'exact48/24 input extents')
    require(physical==b''.join(struct.pack('<14d',*x['input_f64'])for x in e['physical']) and synthetic==b''.join(struct.pack('<49d',*x['input_f64'])for x in e['synthetic']),'original JSON/input bit identity')
    finite(struct.unpack('<672d',physical),672,'all physical input words');finite(struct.unpack('<1176d',synthetic),1176,'all synthetic input words');cases=json.loads((oracle/'cases.json').read_text());require(len(cases)==72,'all reference cases')
    for i,c in enumerate(cases):
        determinant=Fraction(c['gram_determinant_fraction']);require((determinant>Fraction(1,2**48))==(c['status']==0),'independent exact squared-volume gate '+str(i))
    for group,S in enumerate([1.,2.,.5]):
        for side in range(3):
            i=15+group*3+side;row=e['synthetic'][i]['input_f64'];a=row[:42];product=Fraction(1);row_max_product=Fraction(1)
            for j in range(6):
                require(all(a[j*7+k]==(1. if j==1 and k==0 else 0.)for k in range(7)if k!=j),'exact near-parallel triangular threshold fixture');product*=Fraction.from_float(a[j*7+j]);row_max_product*=Fraction.from_float(max(abs(x)for x in a[j*7:(j+1)*7]))
            wanted=[math.nextafter(2.**-24,0),2.**-24,math.nextafter(2.**-24,math.inf)][side]
            require(a[0]==S and all(a[j*7+j]==1. for j in range(2,6))and row_max_product==Fraction.from_float(S)and max(abs(x)for x in a)==max(1.,S)and product==Fraction.from_float(wanted) and cases[48+i]['status']==(0 if side==2 else 5),'near-parallel restored strict/IEEE-neighbor fixture before comparison')
    require(e['squared_norm_error_limit']==2e-12 and e['null_residual_limit']==e['posture_residual_limit']==2e-10,'final squarednorm/physicalleak contract')
    require(cases[22]['status']==cases[49]['status']==0,'dense public/math alias sources admitted by independent reference')
    return e,r,cases,physical,synthetic

def decode(raw,count):
    require(len(raw)==count*224,'exact complete224-byte rows');rows=[ROW.unpack_from(raw,i*224)for i in range(count)];finite([v for row in rows for v in row[1:]],count*27,'ALL actual/expected serialized fields before maxima')
    for i,row in enumerate(rows):
        require(row[0]in[0,5,6],'query status domain')
        if row[0]:require(raw[i*224+8:(i+1)*224]==bytes(216),'unavailablezero only after C full actual-output atomicity assertions')
    return rows

def expected(oracle):
    p=(oracle/'physical-expected.bin').read_bytes();s=(oracle/'synthetic-expected.bin').read_bytes();rows=decode(p,48)+decode(s,24);raw=(oracle/'projector-expected.f64').read_bytes();mat=(oracle/'physical-matrices.f64').read_bytes();require(len(raw)==72*49*8 and len(mat)==48*49*8,'complete independent matrix/projector extents');projectors=finite(struct.unpack('<3528d',raw),3528,'ALL independent projector fields');physical_matrices=finite(struct.unpack('<2352d',mat),2352,'ALL independent physical A/d fields');return rows,[projectors[i*49:(i+1)*49]for i in range(72)],[physical_matrices[i*49:(i+1)*49]for i in range(48)]

def compare(actual_physical,actual_synthetic,wanted,projectors,physical_matrices,e):
    rows=decode(actual_physical,48)+decode(actual_synthetic,24);errors={k:[]for k in['projector','delta','reported_null','reported_leak','unit_squarednorm','computed_null','computed_leak','projector_symmetry','projector_idempotence','projector_trace','delta_from_actual_projector']};volumes=[];maxleakfraction=0.;successful=refused=0
    finite([v for row in wanted for v in row[1:]],1944,'ALL expected result words before maxima')
    for i,(a,b)in enumerate(zip(rows,wanted)):
        require(a[0]==b[0],'exact local query status '+str(i))
        if a[0]:refused+=1;continue
        successful+=1;v=a[1:];n=list(v[:7]);delta=list(v[7:14]);ad=(physical_matrices[i]if i<48 else e['synthetic'][i-48]['input_f64']);A=ad[:42];d=ad[42:49]
        finite(n+delta+A+d,63,'independent property inputs');dnorm=math.sqrt(math.fsum(x*x for x in d));normerror=abs(math.fsum(x*x for x in n)-1);require(normerror<=2e-12,'squared unit norm');errors['unit_squarednorm'].append(normerror)
        largest=max(abs(x)for x in n);pivot=next(j for j,x in enumerate(n)if abs(x)==largest);require(n[pivot]>0,'canonical first-largest direction')
        for x in n+delta:
            if x==0:require(struct.pack('<d',x)==bytes(8),'canonical positivezero n/projectedcoefficients')
        P=[n[j]*n[k]for j in range(7)for k in range(7)];errors['projector'].extend(abs(x-y)for x,y in zip(P,projectors[i]));errors['delta'].extend(abs(x-y)for x,y in zip(delta,b[8:15]))
        errors['projector_symmetry'].extend(abs(P[j*7+k]-P[k*7+j])for j in range(7)for k in range(7));errors['projector_trace'].append(abs(math.fsum(P[j*7+j]for j in range(7))-1));errors['projector_idempotence'].extend(abs(math.fsum(P[j*7+l]*P[l*7+k]for l in range(7))-P[j*7+k])for j in range(7)for k in range(7));errors['delta_from_actual_projector'].extend(abs(math.fsum(P[j*7+k]*d[k]for k in range(7))-delta[j])/max(1,dnorm)for j in range(7))
        An=[math.fsum(A[row*7+j]*n[j]for j in range(7))for row in range(6)];Ap=[math.fsum(A[row*7+j]*delta[j]for j in range(7))for row in range(6)];finite(An+Ap,12,'independent task/null properties');cap=2e-10*max(1,dnorm)
        require(max(abs(x)for x in An)<=2e-10 and max(abs(x)for x in Ap)<=cap,'independent null and physical task leakage gates');maxleakfraction=max(maxleakfraction,max(abs(x)for x in Ap)/cap)
        errors['computed_null'].extend(abs(x)for x in An);errors['computed_leak'].extend(abs(x)/max(1,dnorm)for x in Ap);errors['reported_null'].extend(abs(x-y)for x,y in zip(v[14:20],An));errors['reported_leak'].extend(abs(x-y)/max(1,dnorm)for x,y in zip(v[20:26],Ap))
        require(max(abs(x)for x in v[14:20])<=2e-10 and max(abs(x)for x in v[20:26])<=cap,'all actual reported gate fields')
        lim=e['budgets']['volume_atol']+e['budgets']['volume_rtol']*abs(b[27]);error=abs(v[26]-b[27]);require(v[26]>2.**-24 and math.isfinite(error)and error<=lim,'restored normalized volume');volumes.append((error,lim))
    for i in[17,20,23]:require(rows[48+i][0]==0 and struct.pack('<d',rows[48+i][27])==struct.pack('<d',math.nextafter(2.**-24,math.inf)),'exact restored above-threshold volume bits')
    identity=rows[48];require(identity[0]==0 and identity[1:8]==(0.,0.,0.,0.,0.,0.,1.) and identity[27]==1.,'exact null-axis/canonical basis')
    tie=rows[58];require(tie[0]==0 and tie[1]>0 and tie[2]<0 and tie[7]<0 and abs(tie[1])==abs(tie[2])==abs(tie[7]),'exact first-largest synthetic tie sign/magnitudes')
    for j in range(3,7):require(struct.pack('<d',tie[j])==bytes(8),'canonical tie zero direction')
    budgets=e['budgets'];metrics={k:metric(values,budgets['projector_atol']if k=='projector'else budgets['projected_delta_radians_atol']if k=='delta'else 2e-12 if k=='unit_squarednorm'else budgets['properties_atol'],k)for k,values in errors.items()}
    return dict(passed=True,successful_payloads=successful,unavailable_payloads=refused,all_actual_serialized_finite_words=1944,all_expected_serialized_finite_words=1944,metrics=metrics,volume_max_abs_error=max(x for x,_ in volumes),volume_maximum_budget_fraction=max(x/y for x,y in volumes),maximum_independent_scaled_leak_fraction=maxleakfraction,exact_threshold_S_restore_and_canonical_ties=True,applied_candidate=False)

def reader_guards(p,s,wp,ws,projectors,matrices):
    count=0
    for raw,n in[(p,48),(s,24),(wp,48),(ws,24)]:
        bad=[raw[:-1],raw+b'\0']
        for v in[math.nan,math.inf,-math.inf]:
            x=bytearray(raw);struct.pack_into('<d',x,len(x)-8,v);bad.append(bytes(x))
        for x in bad:
            try:decode(x,n)
            except RuntimeError:count+=1
            else:raise RuntimeError('reader admitted actual/expected extent/late IEEE poison')
    for values,n in[(projectors,3528),(matrices,2352)]:
        for v in[math.nan,math.inf,-math.inf]:
            bad=values[:];bad[-1]=v
            try:finite(bad,n,'late independent property table')
            except RuntimeError:count+=1
            else:raise RuntimeError('reader admitted nonfinite independent property table')
    require(count==26,'all26 actual/expected/property reader poisons');return dict(passed=True,cases=26,actual_and_expected_and_property_tables=True)

def native(args):
    require(platform.system()=='Darwin'and platform.machine()=='arm64'and os.sys.byteorder=='little','qualified Darwin-arm64 scalar-F64 profile');oracle=args.oracle.resolve();e,reference,cases,physical,synthetic=frozen(oracle);wanted,projectors,matrices=expected(oracle);out=args.output.resolve();require(not out.exists(),'preserve every prior/partial native attempt');out.mkdir(parents=True);compiler=args.compiler.resolve();record=dict(schema='psionic.panda-nullspace.native.v1',passed=False,SDK_calls=0,NN_calls=0,old_gates_repeated=False,oracle_regenerated=False,applied_candidate=False,experiment=identity(FIXTURE/'experiment.json'),reference=identity(oracle/'reference.json'),compiler=identity(compiler),commands=[],budgets=e['budgets'])
    runner=Run(out,record);runner.save();began=time.monotonic()
    try:
        dependencies=graph(SOURCE)
        for p in[NATIVE,Path(__file__).resolve(),ROOT/'psionic/tests/robotics/panda_pose_ik_native_reference.py']+[p for directory in[FIXTURE,oracle]for p in directory.iterdir()if p.is_file()]:dependencies[p]=identity(p)
        record['frozen_inputs']={str(p.relative_to(ROOT))if p.is_relative_to(ROOT)else str(p):value for p,value in sorted(dependencies.items())};record['preflight']=dict(physical=48,synthetic=24,exact_Fraction_squared_volume=True,threshold_first_row_scales=[1,2,.5],threshold_global_normalizers=[1,2,1],native_residual_probe_queries=53,no_candidate_or_component_clamp=True);runner.save();(out/'input.bin').write_bytes(struct.pack('<4I',0x31534e50,48,24,1)+physical+synthetic+struct.pack('<14d',*[x[0]for x in e['joint_limits_radians']],*[x[1]for x in e['joint_limits_radians']])+struct.pack('<2q',22,1));record['input']=identity(out/'input.bin');emissions={}
        for mode,extra in[('debug',[]),('release',['--release'])]:
            d=out/mode;d.mkdir();c=d/'panda-nullspace.c';h=d/'panda-nullspace.h';c.write_bytes(runner.command('emit-'+mode,[compiler,'c',SOURCE.relative_to(ROOT),'--library',*extra,'--header',h]));emissions[mode]=(c,h)
        record['emissions']={m:dict(C=identity(c),header=identity(h))for m,(c,h)in emissions.items()};runner.save();outputs=[];common=['-std=c11','-Wall','-Wextra','-Werror','-Wno-unused-function','-Wno-misleading-indentation','-fno-fast-math','-ffp-contract=off','-fno-builtin-sin','-fno-builtin-cos']
        for mode,source_mode,flags in[('debug','debug',['-O0','-g']),('release','release',['-O2']),('sanitized','debug',['-O1','-g','-fsanitize=address,undefined','-fno-sanitize-recover=all','-fno-omit-frame-pointer'])]:
            d=out/('native-'+mode);d.mkdir();c,h=emissions[source_mode];binary=d/'native';runner.command('compile-Coax-'+mode,[os.environ.get('CC','cc'),*common,'-Wno-unused-parameter',*flags,'-c',c,'-o',d/'coax.o']);runner.command('compile-effects-'+mode,[os.environ.get('CC','cc'),*common,*flags,'-I'+str(h.parent),NATIVE,d/'coax.o','-lm','-o',binary]);report=json.loads(runner.command('run-'+mode,[binary,out/'input.bin',d/'physical.bin',d/'synthetic.bin']))
            require(report['passed']and report['SDK_calls']==0 and report['physical_cases']==48 and report['synthetic_cases']==24 and report['record_bytes']==224 and report['successful_payloads']==62 and report['unavailable_payloads']==10 and report['alias_cases']==13 and report['public_q_rest_endpoints']==28 and report['math_endpoints']==4 and report['residual_probe_queries']==53 and report['public_IEEE_patterns']==70 and report['math_IEEE_patterns']==245 and report['fpcr']==0,'all exact new native inventory')
            p=(d/'physical.bin').read_bytes();s=(d/'synthetic.bin').read_bytes();record.setdefault('modes',{})[mode]=dict(native=report,comparison=compare(p,s,wanted,projectors,matrices,e),binary=identity(binary),physical=identity(d/'physical.bin'),synthetic=identity(d/'synthetic.bin'));outputs.append((p,s));runner.save()
        require(outputs[0]==outputs[1]==outputs[2],'all complete public/math mode files bit-identical');record['new_reader_guards']=reader_guards(*outputs[0],(oracle/'physical-expected.bin').read_bytes(),(oracle/'synthetic-expected.bin').read_bytes(),[v for row in projectors for v in row],[v for row in matrices for v in row]);require(all(identity(p)==v for p,v in dependencies.items())and identity(compiler)==record['compiler'],'all source/reference/compiler identities unchanged');record.update(passed=True,modes_byte_identical=True,physical_sha256=hashlib.sha256(outputs[0][0]).hexdigest(),synthetic_sha256=hashlib.sha256(outputs[0][1]).hexdigest())
    except BaseException as error:record['failure']=repr(error);raise
    finally:record['seconds']=time.monotonic()-began;record['partial_or_full_outputs']={str(p.relative_to(out)):identity(p)for pattern in['native-*/physical.bin','native-*/synthetic.bin']for p in out.glob(pattern)};runner.save()
    print(json.dumps(dict(passed=True,record=str(out/'record.json'),assertions={m:v['native']['assertions']for m,v in record['modes'].items()})))

def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--compiler',type=Path,required=True);p.add_argument('--output',type=Path,required=True);p.add_argument('--oracle',type=Path,default=FIXTURE);native(p.parse_args())
if __name__=='__main__':main()
