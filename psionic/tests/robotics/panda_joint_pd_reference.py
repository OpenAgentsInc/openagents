#!/usr/bin/env python3
"""NEW offline joint PD proof: original gravity facts and analytic matrix dU/dq.

Imports authored matrix-function definitions only. No old harness/main, SDK,
step, renderer, neural inference or downloaded repository code is executed.
"""
from pathlib import Path
import argparse,hashlib,json,math,os,platform,struct,subprocess,sys,time
import panda_inverse_dynamics_matrix as matrix
ROOT=Path(__file__).resolve().parents[3]
FIXTURE=ROOT/'psionic/fixtures/robotics/panda-joint-pd'
SOURCE=ROOT/'psionic/lib/robotics/panda-joint-pd.coax'
NATIVE=ROOT/'psionic/tests/robotics/panda_joint_pd_native.c'
EXPERIMENT_SHA='5c5a44371db1dd48695dfb08953409fa425914b46f0cd1b2ffad24b5653a0143'
KEYS=['q_radians','slides_metres','rate_radians_per_second','target_radians','target_rate_radians_per_second','kp_J_per_radian_squared','kd_J_second_per_radian_squared']
FIELDS=['proportional','derivative','gravity','requested','command']
ROW=struct.Struct('<q35dq')
def require(ok,message):
    if not ok:raise RuntimeError(message)
def sha(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()
def finite(values,count,label):
    require(len(values)==count and all(math.isfinite(v)for v in values),label+' ALL finite and complete');return list(values)
def words(path,count,label):
    raw=Path(path).read_bytes();require(len(raw)==8*count,label+' exact byte extent');return finite(struct.unpack('<'+str(count)+'d',raw),count,label)
def compare(actual,expected,label):
    finite(actual,len(expected),label+' actual');finite(expected,len(actual),label+' expected');require(actual,label+' nonempty');error=max(abs(a-b)for a,b in zip(actual,expected));require(error<=5e-10,f'{label} error {error:.17g} exceeds frozen5e-10');return dict(passed=True,values=len(actual),max_abs_error=error,atol=5e-10,rtol=0,unit='Joule/Radian; per-one-radian generator normalization')
def decode(raw):
    require(len(raw)==44*ROW.size,'exact44 native296-byte records');out=[]
    for i in range(44):
        row=ROW.unpack_from(raw,i*ROW.size);require(row[0]==0,'every frozen query admits');finite(row[1:36],35,'all actual fields');require(0<=row[36]<=7,'clip count domain');out.append(row)
    return out
def decoder_guards():
    good=ROW.pack(0,*([0.]*35),0)*44;decode(good);bad=[good[:-1],good+ROW.pack(0,*([0.]*35),0)]
    for value in [float('nan'),float('inf'),-float('inf')]:
        b=bytearray(good);struct.pack_into('<d',b,43*ROW.size+8+34*8,value);bad.append(bytes(b))
    b=bytearray(good);struct.pack_into('<q',b,43*ROW.size,4);bad.append(bytes(b));b=bytearray(good);struct.pack_into('<q',b,len(b)-8,8);bad.append(bytes(b))
    for raw in bad:
        try:decode(raw)
        except RuntimeError:pass
        else:raise RuntimeError('late poison/status/count/extent admitted')
    return dict(passed=True,refused_cases=len(bad),SDK_calls=0)
def potential_gradient(nodes,model,q):
    """Analytic derivative of -sum(m*g dot COM) via full-angle matrix products.

    No owned quaternion recursion, hinge cross product or metric is used.
    The derivative is per one-radian coordinate, not an implicit angle erasure.
    """
    poses=matrix.body_tangents(nodes,q);require(len(poses)==len(model['bodies'])==16,'all16 source bodies')
    result=[]
    for j in range(7):
        terms=[]
        for (_,partials),body in zip(poses,model['bodies']):
            d=partials[j];velocity=[d[k][3]+math.fsum(d[k][a]*body['inertial_position'][a]for a in range(3))for k in range(3)]
            terms.append(-body['mass']*math.fsum(model['gravity'][k]*velocity[k]for k in range(3)))
        result.append(math.fsum(terms))
    return finite(result,7,'independent analytic potential gradient')
def scalar(case,gravity):
    q,v,t,tv,kp,kd=(case[key]for key in [KEYS[0],KEYS[2],KEYS[3],KEYS[4],KEYS[5],KEYS[6]])
    p=[kp[i]*(t[i]-q[i])for i in range(7)];d=[kd[i]*(tv[i]-v[i])for i in range(7)];requested=[(p[i]+d[i])+gravity[i]for i in range(7)];command=[];clipped=0
    for i,value in enumerate(requested):
        limit=80 if i<5 else 12
        if value< -limit:command.append(-float(limit));clipped+=1
        elif value>limit:command.append(float(limit));clipped+=1
        else:command.append(value)
    result=dict(proportional=p,derivative=d,gravity=gravity,requested=requested,command=command,clipped=clipped)
    finite([x for key in FIELDS for x in result[key]],35,'complete independent PD target');return result
def prepare_reference():
    require(sha(FIXTURE/'experiment.json')==EXPERIMENT_SHA,'frozen pre-implementation44-case experiment');e=json.loads((FIXTURE/'experiment.json').read_text());require(e['frozen_before_product_implementation'] and e['case_count']==44 and e['retained_based_positive_cases']==20 and e['independent_matrix_cases']==24 and e['budgets']=={'force_fields_atol':5e-10,'rtol':0,'clipping_counts':'exact','mode_bytes':'exact'},'frozen corpus/scope/budgets')
    dependencies={ROOT/name:digest for name,digest in e['dependencies'].items()};require(len(dependencies)==9 and all(sha(p)==h for p,h in dependencies.items()),'all nine original data/matrix dependencies')
    cases=e['cases'];require(len(cases)==44 and len({x['name']for x in cases})==44,'complete unique explicit case inventory');require([c['name']for c in cases[20:]]==['matrix'+str(i).zfill(2)for i in range(24)],'all24 new matrix cases')
    raw=(FIXTURE/'cases.f64').read_bytes();require(sha(FIXTURE/'cases.f64')==e['cases_sha256'] and raw==struct.pack('<1936d',*[x for c in cases for k in KEYS for x in c[k]]),'all1936 frozen JSON/binary input bits');inputs=words(FIXTURE/'cases.f64',1936,'all input values')
    fingers=ROOT/'psionic/fixtures/robotics/panda-fingers';arm=ROOT/'psionic/fixtures/robotics/panda-fk';sdk_profile=json.loads((fingers/'sdk-profile.json').read_text());sdk_receipt=json.loads((fingers/'sdk-dynamics.json').read_text());model=json.loads((fingers/'sdk-model.json').read_text());sdk_cases=words(fingers/'sdk-cases.f64',45,'original fiveq9');sdk_values=words(fingers/'sdk-dynamics.f64',450,'ALL original source dynamics fields')
    require(sdk_receipt['schema']=='psionic.panda-fingers.sdk-dynamics-reference.v1' and sdk_receipt['captured'] and sdk_receipt['shape']==[5,90] and sdk_receipt['additional_forward_calls']==sdk_receipt['integration_steps']==0,'original five zero-rate facts; no new SDK work');require(sdk_receipt['case_sha256']==sha(fingers/'sdk-cases.f64') and sdk_receipt['dynamics_sha256']==sha(fingers/'sdk-dynamics.f64') and sdk_receipt['model_metadata_sha256']==sha(fingers/'sdk-model.json') and sdk_receipt['pose_profile_sha256']==sha(fingers/'sdk-profile.json'),'original SDK artifact associations')
    require(sdk_receipt['source_original_xml_sha256']==sdk_profile['source_original_xml_sha256'] and sdk_receipt['source_rewritten_xml_sha256']==sdk_profile['source_rewritten_xml_sha256'] and sdk_receipt['sdk_library_sha256']=='f8c9735ed948d6830d1d3d31d50216ec14ca6a6e95c7346a4dc8300e1b8ffed4','source model/SDK association');require(model['gravity']==sdk_receipt['gravity']==[0,0,-9.81] and len(model['bodies'])==16 and len(model['robot_joints'])==9,'fixed16-body gravity profile')
    arm_model=json.loads((arm/'sdk-model.json').read_text());require(e['joint_limits_radians']==[j['range']for j in arm_model['arm_joints']],'exact seven source soft limits');branch=json.loads((fingers/'source-profile.json').read_text());require(branch['arm_profile']['path']=='psionic/fixtures/robotics/panda-fk/source-profile.json' and branch['arm_profile']['sha256']==sha(arm/'source-profile.json'),'independent arm/branch profile association');nodes=json.loads((arm/'source-profile.json').read_text())['nodes']+branch['nodes'];require(len(nodes)==16 and [n['name']for n in nodes]==[b['name']for b in model['bodies']],'all16 physical matrix nodes in compiled model order');require(all(j['qpos0_reference']==0 for j in model['robot_joints']),'zero reference coordinates for analytic hinge generators')
    matrix_cache={};expected=[];retained_actual=[];retained_matrix=[]
    for i,case in enumerate(cases):
        values=[x for key in KEYS for x in case[key]];finite(values,44,'case input');q9=case[KEYS[0]]+case[KEYS[1]];packed=struct.pack('<9d',*q9)
        if packed not in matrix_cache:matrix_cache[packed]=potential_gradient(nodes,model,q9)
        independent=matrix_cache[packed]
        if i<20:
            row=i//4;witness=case['gravity_reference'];require(witness['kind']=='retained_sdk_zero_rate_gravity' and witness['row']==row and struct.pack('<9d',*q9)==struct.pack('<9d',*sdk_cases[row*9:(row+1)*9]) and q9==witness['input_q_radians_then_metres'],'every retained regime matches same actual SDK configuration');require(q9[:7]==sdk_profile['arm_radians'],'home-arm source, not older zero-angle set');gravity=sdk_values[row*90+81:row*90+88]
            if i%4==0:retained_actual+=gravity;retained_matrix+=independent
        else:require(case['gravity_reference']['kind']=='independent_matrix_potential_gradient','new matrix target origin');gravity=independent
        target=scalar(case,gravity);target['reference']='retained SDK zero-rate gravity + literal scalarPD'if i<20 else'independent analytic matrix potential gradient + literal scalarPD';expected.append(target)
    require(len(expected)==44 and len(matrix_cache)==29,'five original +24new unique matrix contexts');matrix_sdk=compare(retained_matrix,retained_actual,'NEW independent matrix derivative vs35 retained home-arm gravity values')
    return e,inputs,expected,dependencies,dict(matrix_vs_retained_SDK=matrix_sdk,unique_gravity_contexts=29,retained_SDK_states=5,retained_based_PD_queries=20,new_matrix_states=24,SDK_calls=0,old_numeric_gates_repeated=False)
def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--compiler',type=Path,default=ROOT/'coax/bin/coax');parser.add_argument('--output',type=Path,required=True);parser.add_argument('--source-refusals',type=Path,default=FIXTURE/'unit-refusals.json');args=parser.parse_args();out=args.output.resolve();require(not out.exists(),'preserve earlier/failed proof');out.mkdir(parents=True);compiler=args.compiler.resolve();record=dict(schema='psionic.panda-joint-pd.native-qualification.v1',passed=False,SDK_calls=0,NN_calls=0,integration_steps=0,render_calls=0,commands=[],modes=[]);began=time.monotonic()
    def save():(out/'record.json').write_text(json.dumps(record,indent=2)+'\n')
    def command(name,argv):
        start=time.monotonic();run=subprocess.run([str(x)for x in argv],cwd=ROOT,capture_output=True,timeout=240);(out/(name+'.stdout')).write_bytes(run.stdout);(out/(name+'.stderr')).write_bytes(run.stderr);record['commands'].append(dict(name=name,arguments=[str(x)for x in argv],exit=run.returncode,seconds=time.monotonic()-start,stdout_sha256=hashlib.sha256(run.stdout).hexdigest(),stderr_sha256=hashlib.sha256(run.stderr).hexdigest(),stderr_bytes=len(run.stderr)));save();require(run.returncode==0 and not run.stderr,(name,run.stdout.decode(errors='replace'),run.stderr.decode(errors='replace')));return run.stdout
    save()
    try:
        require(sys.platform=='darwin' and platform.machine()=='arm64' and sys.byteorder=='little','qualified Darwin-arm64 scalar-libm profile');e,inputs,expected,dependencies,oracle=prepare_reference();binary_sha=sha(compiler);record.update(experiment_sha256=EXPERIMENT_SHA,compiler_sha256=binary_sha,compiler_input_fingerprint='580cc4b7c058db8b828df2d13b43eb4feb38af53f4d692430943d51cca81e036'if binary_sha=='a429ad59a39f1b77adee37dd09895d8f7fd36b3149bc3fdf6eed4131a2d4f818'else None,independent_oracle=oracle,decoder_guards=decoder_guards(),scope=e['scope']);(out/'expected.json').write_text(json.dumps(expected,indent=2)+'\n');record['expected_sha256']=sha(out/'expected.json');(out/'input.bin').write_bytes(struct.pack('<II',0x31504a50,44)+struct.pack('<1936d',*inputs))
        deps=json.loads(command('dependencies',[compiler,'deps',SOURCE.relative_to(ROOT)]));require(not deps['descriptors'],'no unbound protocol dependencies');dependencies.update({Path(p):sha(p)for p in deps['sources']});dependencies.update({Path(__file__).resolve():sha(__file__),NATIVE:sha(NATIVE),FIXTURE/'experiment.json':EXPERIMENT_SHA,FIXTURE/'cases.f64':sha(FIXTURE/'cases.f64'),FIXTURE/'checks-qualification.json':sha(FIXTURE/'checks-qualification.json'),args.source_refusals.resolve():sha(args.source_refusals)})
        proof=json.loads(args.source_refusals.read_text());require(proof['passed'] and len(proof['checks'])==4 and {x['name']for x in proof['checks']}=={'angle','rate','kp','kd'},'four precise historical unit witnesses')
        for item in proof['checks']:
            path=ROOT/('psionic/tests/robotics/panda_joint_pd_bad_'+item['name']+'.coax');log=FIXTURE/('unit-'+item['name']+'.log');require(sha(path)==item['source_sha256'] and item['exit']!=0 and item['diagnostic']=='E0317' and item['refused'] is True and sha(log)==item['stderr_sha256'],'historical unit refusal identity');dependencies[path]=sha(path);dependencies[log]=sha(log)
        record['historical_unit_refusals']=dict(receipt_sha256=sha(args.source_refusals),compiler_sha256=proof['compiler_sha256'],calls_repeated=False);record['source_sha256']={str(p.relative_to(ROOT))if p.is_relative_to(ROOT)else str(p):h for p,h in sorted(dependencies.items())};emitted={}
        for mode,flags in [('debug',[]),('release',['--release'])]:
            d=out/mode;d.mkdir();header=d/'panda-joint-pd.h';source=d/'panda-joint-pd.c';source.write_bytes(command('emit-'+mode,[compiler,'c',SOURCE.relative_to(ROOT),'--library',*flags,'--header',header]));emitted[mode]=(source,header)
        payloads=[]
        for mode,source_mode,flags in [('debug','debug',['-O0','-g']),('release','release',['-O2']),('sanitized','debug',['-O1','-g','-fsanitize=address,undefined','-fno-sanitize-recover=all','-fno-omit-frame-pointer'])]:
            d=out/mode;d.mkdir(exist_ok=True);source,header=emitted[source_mode];binary=d/'native';command('compile-'+mode,[os.environ.get('CC','cc'),'-std=c11',*flags,'-fno-fast-math','-ffp-contract=off','-fno-builtin-sin','-fno-builtin-cos','-I'+str(header.parent),source,NATIVE,'-lm','-o',binary]);native=json.loads(command('run-'+mode,[binary,out/'input.bin',d/'outputs.bin']));require(native['passed'] and native['cases']==44 and native['record_bytes']==296 and native['nonfinite_cases']==220 and native['boundary_cases']==88 and native['adjacent_precedence_cases']==86 and native['alias_cases']==4,'complete NEW native proof');payload=(d/'outputs.bin').read_bytes();actual=decode(payload);payloads.append(payload);metrics={}
            for start,end,label in [(0,20,'retained_SDK_based'),(20,44,'new_matrix'),(0,44,'all')]:
                for k,key in enumerate(FIELDS):metrics[label+'.'+key]=compare([actual[i][1+7*k+j]for i in range(start,end)for j in range(7)],[expected[i][key][j]for i in range(start,end)for j in range(7)],label+' '+key)
            require(all(row[36]==want['clipped']for row,want in zip(actual,expected)),'all44 independent strict clipping counts exact');record['modes'].append(dict(mode=mode,native=native,metrics=metrics,all1540_actual_fields_finite=True,clipping_counts_exact=True,C_sha256=sha(source),header_sha256=sha(header),binary_sha256=sha(binary),output_sha256=sha(d/'outputs.bin')));save()
        require(payloads[0]==payloads[1]==payloads[2],'complete owned-mode query/count bit identity');require(all(sha(p)==h for p,h in dependencies.items()) and sha(compiler)==binary_sha,'source/reference/compiler frozen at completion');record.update(passed=True,native_modes_byte_identical=True,cases=44,output_shape='44×(int64status,35F64,int64clipped)')
    except BaseException as error:record['failure']=repr(error);raise
    finally:record['seconds']=time.monotonic()-began;save()
    print(json.dumps(dict(passed=True,record=str(out/'record.json'),modes=[m['native']for m in record['modes']])))
if __name__=='__main__':main()
