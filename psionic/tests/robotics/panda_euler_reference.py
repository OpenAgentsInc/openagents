#!/usr/bin/env python3
"""NEW offline Euler consumer: cached algebra plus original SDK step facts.

This never acquires SDK observations or re-runs the independent target solver.
The reused Pa scalar helper supplies clipping counts, not physics outputs.
"""
from pathlib import Path
import argparse,hashlib,json,math,os,platform,struct,subprocess,sys,time
from panda_actuation_reference import scalar as actuation_scalar
ROOT=Path(__file__).resolve().parents[3]
FIXTURE=ROOT/'psionic/fixtures/robotics/panda-euler'
SOURCE=ROOT/'psionic/lib/robotics/panda-euler.coax'
NATIVE=ROOT/'psionic/tests/robotics/panda_euler_native.c'
EXPERIMENT_SHA='9f4eb555739fdc6ac2d61f02d68a1d432599078ce77eb792eba2ee68b1572262'
CACHED_TARGET_SHA='a6ae2fa733d6a8e56b3fb3607c65c0a42206a188355b7e292e5854c6cb6296ee'
ROW=struct.Struct('<q27d2q')
def require(ok,message):
    if not ok:raise RuntimeError(message)
def sha(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()
def finite(values,count,label):
    require(len(values)==count and all(math.isfinite(x) for x in values),label+' ALL finite and complete');return list(values)
def words(raw,count,label):
    require(len(raw)==count*8,label+' exact bytes');return finite(struct.unpack('<'+str(count)+'d',raw),count,label)
def compare(actual,expected,atol,label):
    finite(actual,len(expected),label+' actual');finite(expected,len(actual),label+' expected');require(actual,label+' nonempty');error=max(abs(x-y)for x,y in zip(actual,expected));require(error<=atol,f'{label} {error:.17g} exceeds frozen {atol:g}');return dict(passed=True,values=len(actual),max_abs_error=error,atol=atol,rtol=0)
def decode(raw):
    require(len(raw)==6*ROW.size,'six exact240-byte native records');rows=[]
    for i in range(6):
        row=ROW.unpack_from(raw,i*ROW.size);require(row[0]==0,'every frozen context admits');finite(row[1:28],27,'every actual q/v/effective word');require(0<=row[28]<=9 and 0<=row[29]<=2,'actual clipping counts');rows.append(row)
    return rows
def decoder_guards():
    good=ROW.pack(0,*([0.]*27),0,0)*6;decode(good);bad=[good[:-1],good+ROW.pack(0,*([0.]*27),0,0)]
    for x in [float('nan'),float('inf'),-float('inf')]:
        b=bytearray(good);struct.pack_into('<d',b,5*ROW.size+8+26*8,x);bad.append(bytes(b))
    b=bytearray(good);struct.pack_into('<q',b,5*ROW.size,3);bad.append(bytes(b));b=bytearray(good);struct.pack_into('<q',b,len(b)-8,3);bad.append(bytes(b))
    for value in bad:
        try:decode(value)
        except RuntimeError:pass
        else:raise RuntimeError('decoder admitted late poison/extent/status/count')
    return dict(passed=True,refused_cases=len(bad),SDK_calls=0)
def reference_paths(cache,sdk):
    return ((cache/'record.json',cache/'targets.f64')if cache else(FIXTURE/'cached-reference.json',FIXTURE/'cached-targets.f64'))+((sdk/'record.json',sdk/'values.f64')if sdk else(FIXTURE/'sdk-reference.json',FIXTURE/'sdk-values.f64'))
def load_reference(cache,sdk):
    paths=reference_paths(cache,sdk);cp,ct,sp,st=paths
    require(sha(FIXTURE/'experiment.json')==EXPERIMENT_SHA,'frozen six-case experiment');e=json.loads((FIXTURE/'experiment.json').read_text());require(e['frozen_before_implementation_and_comparison'] and e['step_seconds']==.002 and e['budgets']=={'q_absolute':2e-12,'v_absolute':2e-10,'effective_acceleration_absolute':1e-8,'relative':0},'unchanged frozen budgets/step')
    raw=(FIXTURE/'cases.f64').read_bytes();require(sha(FIXTURE/'cases.f64')==e['cases_f64_sha256'],'frozen input digest');cases=words(raw,216,'all frozen inputs');require(len(e['cases'])==6 and raw==struct.pack('<216d',*[x for c in e['cases']for k in ['q','v','ctrl','external']for x in c[k]]),'six explicit JSON input rows bit-identical')
    c=json.loads(cp.read_text());require(c['schema']=='psionic.panda-euler.cached-algebraic-reference.v1' and c['passed'] and c['reference_is_observed_SDK_step'] is False and c['SDK_calls']==c['Coax_calls']==0,'independent retained-operand algebraic origin');require(sha(ct)==c['targets_sha256']==CACHED_TARGET_SHA,'immutable independent Gaussian targets');cached=words(ct.read_bytes(),81,'all cached algebraic targets');require(len(c['cases'])==3 and [v['case']for v in c['cases']]==[0,1,2] and [v['source_Pa_row']for v in c['cases']]==[12,13,14] and all(v['source_PID_mass_bias_row']==0 for v in c['cases']),'cached exact source row coverage')
    require(ct.read_bytes()==struct.pack('<81d',*[x for row in c['cases']for k in ['q','v','effective_acceleration']for x in row[k]]),'all cached record/binary output bits')
    cached_inputs={'psionic/fixtures/robotics/panda-euler/experiment.json','psionic/fixtures/robotics/panda-euler/cases.f64','psionic/fixtures/robotics/panda-inverse-dynamics/cases.f64','psionic/fixtures/robotics/panda-inverse-dynamics/sdk-mass-bias.f64','psionic/fixtures/robotics/panda-actuation/experiment.json','psionic/fixtures/robotics/panda-actuation/sdk-values.f64','psionic/tests/robotics/panda_euler_cached_reference.py'};require(set(c['dependencies'])==cached_inputs,'exact cached independent source inventory')
    for path,digest in c['dependencies'].items():
        if path.endswith('panda_euler_cached_reference.py'):continue
        require(sha(ROOT/path)==digest,'immutable cached DATA dependency '+path)
    followup=json.loads((FIXTURE/'cached-guard-qualification.json').read_text());require(followup['passed'] and followup['original_execution_record_sha256']==sha(cp) and followup['targets_sha256']==sha(ct) and followup['Gaussian_reexecuted'] is False and followup['source_M_and_Pa_artifacts_match_original_capture_hashes'] is True,'historical algebraic execution plus changed finite/provenance guard')
    require(followup['current_guard_source_sha256']==sha(ROOT/'psionic/tests/robotics/panda_euler_cached_reference.py'),'current cached guard source identity')
    require(set(followup['source_receipts'])=={'psionic/fixtures/robotics/panda-inverse-dynamics/sdk-reference.json','psionic/fixtures/robotics/panda-actuation/sdk-reference.json'},'both original source receipt identities')
    for path,digest in followup['source_receipts'].items():require(sha(ROOT/path)==digest,'original cached source receipt '+path)
    s=json.loads(sp.read_text());require(s['schema']=='psionic.panda-euler.sdk-reference.v1' and s['passed'] and s['selected_experiment_rows']==[3,4,5],'three genuine source SDK step contexts');require(s['values']['bytes']==3*154*8 and s['values']['sha256']==sha(st),'SDK observation exact extent/digest');sdk_values=words(st.read_bytes(),462,'ALL original SDK observed fields')
    report=s['native'];require(report['passed'] and report['rows']==3 and report['words_per_row']==154 and report['explicit_forwards']==report['steps']==3 and report['post_step_forwards']==report['NN_calls']==report['render_calls']==0,'exact SDK work scope');require(s['explicit_forwards_started']==s['explicit_forwards_completed']==s['steps_started']==s['steps_completed']==3,'exact completed SDK stage ledger');require(s['support_admission']['passed'] and s['support_admission']['original_disableflags']==0 and s['support_admission']['modified_disableflags']==17 and s['support_admission']['EULERDAMP_enabled'] is True,'admitted implicit damping/constraint-off profile');require(s['source_XML']['sha256']==e['scene_sha256'] and s['SDK_library']['sha256']==e['SDK_sha256'],'same pinned SDK and source scene')
    sdk_inputs={'psionic/tests/robotics/'+n for n in ['panda_euler_sdk_collect.c','panda_euler_sdk_reference.py','panda_actuation_sdk_collect.c','panda_actuation_sdk_reference.py']}|{'psionic/fixtures/robotics/panda-actuation/experiment.json'}|{'psionic/fixtures/robotics/panda-euler/'+n for n in ['experiment.json','cases.f64','sdk-capture-profile.json','sdk-source-addendum-v1.json']};require(set(s['source'])==sdk_inputs,'exact original SDK source inventory')
    for path,identity in s['source'].items():require((ROOT/path).stat().st_size==identity['bytes'] and sha(ROOT/path)==identity['sha256'],'SDK source context '+path)
    sdk_model=sdk/'model.json'if sdk else FIXTURE/'sdk-model.json';sdk_input=sdk/'input.bin'if sdk else FIXTURE/'sdk-input.bin';require(sha(sdk_model)==s['metadata']['sha256'] and sdk_model.stat().st_size==s['metadata']['bytes'],'actual captured model identity');require(sha(sdk_input)==s['input']['sha256'] and sdk_input.stat().st_size==s['input']['bytes'],'actual captured input identity');require(sdk_input.read_bytes()==struct.pack('<II',0x31554550,3)+raw[3*36*8:],'SDK actually consumed exact frozen rows3..5');paths=paths+(sdk_model,sdk_input)
    expected=[]
    for i,case in enumerate(e['cases']):
        if i<3:target=cached[i*27:(i+1)*27]
        else:
            row=sdk_values[(i-3)*154:(i-2)*154];require(row[153]==.002,'source time one fixed increment');target=row[126:153];derived=[(row[135+j]-case['v'][j])/.002 for j in range(9)];require(struct.pack('<9d',*derived)==struct.pack('<9d',*row[144:153]),'effective acceleration explicitly derived from velocity increment, not d.qacc')
        finite(target,27,'complete expected context');scalar=actuation_scalar(case);expected.append(dict(q=target[:9],v=target[9:18],effective=target[18:],control_clipped=scalar[2],force_clipped=scalar[3],reference='retained-operand algebraic'if i<3 else'new original SDK one-step observation'))
    return e,cases,expected,paths,{'cached_original_executed_producer_sha256':c['dependencies']['psionic/tests/robotics/panda_euler_cached_reference.py'],'cached_record_sha256':sha(cp),'cached_guard_followup_sha256':sha(FIXTURE/'cached-guard-qualification.json'),'SDK_record_sha256':sha(sp),'cached_steps_observed':False,'SDK_steps_observed':3}
def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--compiler',type=Path,default=ROOT/'coax/bin/coax');parser.add_argument('--output',type=Path,required=True);parser.add_argument('--cache','--cached',dest='cache',type=Path);parser.add_argument('--sdk',type=Path);args=parser.parse_args();out=args.output.resolve();require(not out.exists(),'preserve earlier/failed proof');out.mkdir(parents=True);compiler=args.compiler.resolve();record=dict(schema='psionic.panda-euler.native-qualification.v1',passed=False,SDK_calls=0,NN_calls=0,integration_effects=0,render_calls=0,commands=[],modes=[]);began=time.monotonic()
    def save():(out/'record.json').write_text(json.dumps(record,indent=2)+'\n')
    def command(name,argv):
        start=time.monotonic();result=subprocess.run([str(x)for x in argv],cwd=ROOT,capture_output=True,timeout=240);(out/(name+'.stdout')).write_bytes(result.stdout);(out/(name+'.stderr')).write_bytes(result.stderr);record['commands'].append(dict(name=name,arguments=[str(x)for x in argv],exit=result.returncode,seconds=time.monotonic()-start,stdout_sha256=hashlib.sha256(result.stdout).hexdigest(),stderr_sha256=hashlib.sha256(result.stderr).hexdigest(),stderr_bytes=len(result.stderr)));save();require(result.returncode==0 and not result.stderr,(name,result.stdout.decode(errors='replace'),result.stderr.decode(errors='replace')));return result.stdout
    save()
    try:
        require(sys.platform=='darwin'and platform.machine()=='arm64'and sys.byteorder=='little','qualified Darwin-arm64 scalar-F64 profile');e,inputs,expected,paths,origins=load_reference(args.cache,args.sdk);binary_sha=sha(compiler);record.update(experiment_sha256=EXPERIMENT_SHA,compiler_sha256=binary_sha,compiler_input_fingerprint='580cc4b7c058db8b828df2d13b43eb4feb38af53f4d692430943d51cca81e036'if binary_sha=='a429ad59a39f1b77adee37dd09895d8f7fd36b3149bc3fdf6eed4131a2d4f818'else None,decoder_guards=decoder_guards(),origins=origins,cached_algebraic_cases=3,original_SDK_steps=3,numeric_profile='Darwin-arm64 scalar F64 libm, no fast math/contraction or cross-target claim')
        deps=json.loads(command('dependencies',[compiler,'deps',SOURCE.relative_to(ROOT)]));dependencies={Path(p):sha(p)for p in deps['sources']};dependencies.update({Path(__file__).resolve():sha(__file__),NATIVE:sha(NATIVE),Path(__file__).with_name('panda_actuation_reference.py'):sha(Path(__file__).with_name('panda_actuation_reference.py'))})
        for p in list(paths)+[FIXTURE/'experiment.json',FIXTURE/'cases.f64',FIXTURE/'checks-qualification.json',FIXTURE/'example-qualification.json',FIXTURE/'cached-guard-qualification.json',ROOT/'psionic/tests/robotics/panda_euler_cached_reference.py']:dependencies[p]=sha(p)
        for path in json.loads((FIXTURE/'cached-guard-qualification.json').read_text())['source_receipts']:dependencies[ROOT/path]=sha(ROOT/path)
        for path in json.loads(paths[2].read_text())['source']:dependencies[ROOT/path]=sha(ROOT/path)
        source_refusals=json.loads((FIXTURE/'checks-qualification.json').read_text());require(source_refusals['passed'] and len(source_refusals['unit_refusals'])==4,'four reused historical unit refusals')
        for item in source_refusals['unit_refusals']:
            require(item['diagnostic']=='E0317'and sha(ROOT/item['source'])==item['source_sha256']and sha(ROOT/item['log'])==item['log_sha256'],'reused unit proof identity');dependencies[ROOT/item['source']]=item['source_sha256'];dependencies[ROOT/item['log']]=item['log_sha256']
        record['reused_unit_refusals']=source_refusals['unit_refusals'];record['unit_refusal_historical_compiler_sha256']=source_refusals['compiler']['sha256'];record['unit_refusal_calls_repeated']=False;record['source_sha256']={str(p.relative_to(ROOT))if p.is_relative_to(ROOT)else str(p):s for p,s in sorted(dependencies.items())};(out/'input.bin').write_bytes(struct.pack('<II',0x31554550,6)+struct.pack('<216d',*inputs));(out/'expected.json').write_text(json.dumps(expected,indent=2)+'\n');record['expected_sha256']=sha(out/'expected.json');emitted={}
        for mode,flags in [('debug',[]),('release',['--release'])]:
            d=out/mode;d.mkdir();header=d/'panda-euler.h';source=d/'panda-euler.c';source.write_bytes(command('emit-'+mode,[compiler,'c',SOURCE.relative_to(ROOT),'--library',*flags,'--header',header]));emitted[mode]=(source,header)
        payloads=[]
        for mode,source_mode,flags in [('debug','debug',['-O0','-g']),('release','release',['-O2']),('sanitized','debug',['-O1','-g','-fsanitize=address,undefined','-fno-sanitize-recover=all','-fno-omit-frame-pointer'])]:
            d=out/mode;d.mkdir(exist_ok=True);source,header=emitted[source_mode];binary=d/'native';command('compile-'+mode,[os.environ.get('CC','cc'),'-std=c11',*flags,'-fno-fast-math','-ffp-contract=off','-fno-builtin-sin','-fno-builtin-cos','-I'+str(header.parent),source,NATIVE,'-lm','-o',binary]);native=json.loads(command('run-'+mode,[binary,out/'input.bin',d/'outputs.bin']));require(native['passed']and native['cases']==6 and native['record_bytes']==240 and native['coordinate_rate_endpoints']==36 and native['control_endpoints']==native['external_endpoints']==18 and native['alias_cases']==4 and native['nonfinite_lane_cases']==180 and native['late_position_cases']==18 and native['late_rate_cases']==2,'complete NEW native coverage');payload=(d/'outputs.bin').read_bytes();rows=decode(payload);payloads.append(payload);metrics={}
            for first,last,label in [(0,3,'cached_algebraic'),(3,6,'original_SDK_steps'),(0,6,'all')]:
                for key,offset,budget in [('q',1,2e-12),('v',10,2e-10),('effective',19,1e-8)]:
                    metrics[label+'.'+key]=compare([rows[i][offset+j]for i in range(first,last)for j in range(9)],[expected[i][key][j]for i in range(first,last)for j in range(9)],budget,label+' '+key)
                    metrics[label+'.'+key]['meaning']='maximum scalar coefficient error; hinge/slide physical units remain distinct'
                    for kind,indices in [('hinge',range(7)),('slide',range(7,9))]:metrics[label+'.'+key+'.'+kind]=compare([rows[i][offset+j]for i in range(first,last)for j in indices],[expected[i][key][j]for i in range(first,last)for j in indices],budget,label+' '+key+' '+kind)
            for row,want in zip(rows,expected):require(row[28:]==(want['control_clipped'],want['force_clipped']),'independent clipping counts on actual controls')
            record['modes'].append(dict(mode=mode,native=native,metrics=metrics,all162_actual_fields_finite=True,clipping_counts_exact=True,C_sha256=sha(source),header_sha256=sha(header),binary_sha256=sha(binary),output_sha256=sha(d/'outputs.bin')));save()
        require(payloads[0]==payloads[1]==payloads[2],'complete own status/state/count payload bit identity across profiles');require(all(sha(p)==s for p,s in dependencies.items())and sha(compiler)==binary_sha,'frozen source/reference/compiler at completion');record.update(passed=True,native_modes_byte_identical=True,cases=6,output_shape='6×(int64status,27F64,2int64counts)',old_component_gates_repeated=False)
    except BaseException as error:record['failure']=repr(error);raise
    finally:record['seconds']=time.monotonic()-began;save()
    print(json.dumps(dict(passed=True,record=str(out/'record.json'),modes=[x['native']for x in record['modes']])))
if __name__=='__main__':main()
