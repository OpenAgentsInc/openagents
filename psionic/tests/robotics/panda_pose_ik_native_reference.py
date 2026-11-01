#!/usr/bin/env python3
"""NEW principal-rotation/one-pose-step native qualification, no SDK calls.

The independent oracle is prepared separately. This consumer never rebuilds
its expected tables or runs old kinematics/position-IK gates.
"""
from pathlib import Path
import argparse, hashlib, json, math, os, platform, re, struct, subprocess, time
ROOT=Path(__file__).resolve().parents[3]
FIXTURE=ROOT/'psionic/fixtures/robotics/panda-pose-ik'
SOURCE=ROOT/'psionic/lib/robotics/panda-pose-ik.coax'
NATIVE=ROOT/'psionic/tests/robotics/panda_pose_ik_native.c'
EXPERIMENT_SHA='4ff40ce11ebcbfa7be54f3a02aded7d35ac3b56101541717dd365b40d7c47d88'
ADDENDUM_SHA='2acc58ffe264cbafe1b2014a1bb5f0fe738e01d7271cce51875937b6c03d2e64'
ROTATION=struct.Struct('<q4d');POSE=struct.Struct('<q17d2q');POSE_EXPECTED=struct.Struct('<17d2q')

def require(ok,message):
    if not ok:raise RuntimeError(message)
def sha(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()
def identity(path):return dict(bytes=Path(path).stat().st_size,sha256=sha(path))
def finite(values,n,label):
    require(len(values)==n and all(math.isfinite(v)for v in values),label+' ALL finite and exact extent');return list(values)
def graph(path,seen=None):
    if seen is None:seen={}
    p=Path(path).resolve()
    if p in seen:return seen
    seen[p]=identity(p)
    for name in re.findall(r'^\s*#load\s+"([^\"]+)"',p.read_text(),re.M):graph(p.parent/name,seen)
    return seen

def frozen():
    require(sha(FIXTURE/'experiment.json')==EXPERIMENT_SHA,'original frozen experiment')
    e=json.loads((FIXTURE/'experiment.json').read_text())
    require(sha(FIXTURE/'precomparison-input-addendum-v1.json')==ADDENDUM_SHA,'precomparison corpus correction')
    addendum=json.loads((FIXTURE/'precomparison-input-addendum-v1.json').read_text())
    require(addendum['original_experiment_sha256']==EXPERIMENT_SHA and addendum['budgets']==e['budgets'] and addendum['algorithm_or_budget_changes'] is False and addendum['frozen_before_expected_generation_and_product_comparison'],'unchanged frozen math/budgets')
    original=(FIXTURE/'pose-cases.f64').read_bytes();effective=(FIXTURE/addendum['replacement_file']).read_bytes();rotation=(FIXTURE/'rotation-cases.f64').read_bytes();flags=(FIXTURE/'rotation-flags.u8').read_bytes()
    require(len(original)==len(effective)==54*14*8 and sha(FIXTURE/'pose-cases.f64')==e['pose_cases_sha256']==addendum['original_pose_cases_sha256'],'original54 contexts retained')
    require(sha(FIXTURE/addendum['replacement_file'])==addendum['replacement_pose_cases_sha256'],'exact effective54 context bytes')
    require(original==b''.join(struct.pack('<14d',*c['seed_radians'],*c['target_world_metres'],*c['target_world_from_grip_wxyz'])for c in e['pose_cases']),'original JSON/binary bits')
    require(len(rotation)==4200*8*8 and sha(FIXTURE/'rotation-cases.f64')==e['rotation_cases_sha256']==addendum['rotation_cases_sha256'],'unchanged4200 rotation inputs')
    require(len(flags)==4200 and set(flags)=={0,1,3} and sha(FIXTURE/'rotation-flags.u8')==e['rotation_flags_sha256']==addendum['rotation_flags_sha256'],'unchanged preclassified cut flags')
    require(sum(bool(x&1)for x in flags)==1008 and sum(bool(x&2)for x in flags)==112,'predeclared cut/exactpi inventory')
    changed={x['case_index']:x for x in addendum['changes']};require(set(changed)=={22,23,24,25},'only four precomparison seed changes')
    cases=[]
    for i in range(54):
        old=original[i*112:(i+1)*112];new=effective[i*112:(i+1)*112];row=list(struct.unpack('<14d',new));finite(row,14,'effective pose case')
        if i in changed:
            c=changed[i];require(old[56:]==new[56:] and old[:56]==struct.pack('<7Q',*c['old_seed_u64']) and new[:56]==struct.pack('<7Q',*c['new_seed_u64']),'exact four changed seeds and unchanged target bytes')
        else:require(old==new,'other50 contexts byte-identical')
        require(all(lo<=q<=hi for q,(lo,hi)in zip(row[:7],e['joint_limits_radians'])) and all(abs(v)<=10 for v in row[7:10]),'source bounds/admitted targets')
        cases.append(row)
    finite(struct.unpack('<33600d',rotation),33600,'all rotation inputs')
    for name,digest in e['source_dependencies'].items():require(sha(ROOT/name)==digest,'unchanged original dependency '+name)
    return e,cases,rotation,flags

def decode_rotation(raw):
    require(len(raw)==4200*ROTATION.size,'exact4200×40 rotation actual records');rows=[ROTATION.unpack_from(raw,i*ROTATION.size)for i in range(4200)]
    finite([v for r in rows for v in r[1:]],16800,'all actual rotation fields');require(all(r[0]==0 for r in rows),'all rotation statuses exactlyOK')
    return rows

def decode_pose(raw):
    require(len(raw)==54*POSE.size,'exact54×160 pose actual records');rows=[POSE.unpack_from(raw,i*POSE.size)for i in range(54)]
    finite([v for r in rows for v in r[1:18]],918,'all actual pose fields');require(all(r[0]==0 and r[18]==1 and 0<=r[19]<=7 for r in rows),'exact pose status/update/projected domains')
    return rows

def decode_expected_rotation(raw):
    require(len(raw)==4200*13*8,'complete independent rotation extent')
    values=finite(struct.unpack('<54600d',raw),54600,'ALL independent rotation fields')
    return [values[i*13:(i+1)*13]for i in range(4200)]

def decode_expected_pose(raw):
    require(len(raw)==54*POSE_EXPECTED.size,'complete independent pose extent')
    rows=[POSE_EXPECTED.unpack_from(raw,i*POSE_EXPECTED.size)for i in range(54)]
    finite([v for row in rows for v in row[:17]],918,'ALL independent pose fields')
    require(all(r[17]==1 and 0<=r[18]<=7 for r in rows),'independent counter inventory')
    return rows

def expected_tables(directory):
    flags=(directory/'rotation-flags.u8').read_bytes();require(len(flags)==4200,'independent flags exact extent')
    return decode_expected_rotation((directory/'rotation-expected.f64').read_bytes()),decode_expected_pose((directory/'pose-expected.bin').read_bytes()),flags

def quaternion_matrix(q):
    finite(q,4,'physical quaternion');w,x,y,z=q
    require(abs(sum(t*t for t in q)-1)<=2e-12,'unit output quaternion')
    return [1-2*(y*y+z*z),2*(x*y-z*w),2*(x*z+y*w),2*(x*y+z*w),1-2*(x*x+z*z),2*(y*z-x*w),2*(x*z-y*w),2*(y*z+x*w),1-2*(x*x+y*y)]

def vector_matrix(v):
    finite(v,3,'rotation-vector reconstruction');x,y,z=v;n=math.hypot(x,y,z)
    if n==0:return [1.,0.,0.,0.,1.,0.,0.,0.,1.]
    a=math.sin(n)/n;b=2*(math.sin(n/2)/n)**2
    k=[[0.,-z,y],[z,0.,-x],[-y,x,0.]]
    return [float(i==j)+a*k[i][j]+b*math.fsum(k[i][t]*k[t][j]for t in range(3))for i in range(3)for j in range(3)]

def metric(differences,budget,label):
    finite(differences,len(differences),label+' finite differences');require(differences,label+' nonempty');maximum=max(differences);require(maximum<=budget,f'{label}: {maximum:.17g} exceeds frozen {budget:g}')
    return dict(passed=True,values=len(differences),max_abs_error=maximum,atol=budget,rtol=0)

def compare_rotation(actual,expected,flags,e):
    rows=decode_rotation(actual);require(len(expected)==4200 and len(flags)==4200,'complete angular comparison inventory')
    finite([x for row in expected for x in row],54600,'all expected angular words before maxima')
    outside=[];angle=[];physical=[];ties=[]
    for i,(a,b,f)in enumerate(zip(rows,expected,flags)):
        angle.append(abs(a[4]-b[3]));require(a[4]>=0 and a[4]<=math.pi+2e-12,'principal angle range')
        require(abs(math.hypot(*a[1:4])-a[4])<=2e-12,'vector magnitude matches angle')
        if f&1:physical.extend(abs(x-y)for x,y in zip(vector_matrix(a[1:4]),b[4:13]))
        else:outside.extend(abs(x-y)for x,y in zip(a[1:4],b[:3]))
        if f&2:
            ties.extend(abs(x-y)for x,y in zip(a[1:4],b[:3]));first=next((x for x in a[1:4]if x!=0),None);require(first is not None and first>0,'literal exactpi canonical axis')
        if i%2:require(actual[i*40:(i+1)*40]==actual[(i-1)*40:i*40],'paired target-sign full canonical record bytes')
    b=e['budgets']
    return dict(outside_cut_vector=metric(outside,b['angular_vector_radians_atol'],'outside-cut vector'),all_angles=metric(angle,b['angular_magnitude_radians_atol'],'all principal magnitudes'),inside_cut_physical_rotation=metric(physical,b['physical_rotation_matrix_max_entry_atol'],'inside-cut physical matrices'),exact_pi_vectors=metric(ties,b['angular_vector_radians_atol'],'literal exactpi axis'),cut_cases=1008,exact_pi_cases=112,modeled_cut_band_unchanged=True,all_actual_finite_words=16800,all_expected_finite_words=54600)

def compare_pose(actual,expected,e,cases):
    rows=decode_pose(actual);require(len(expected)==54,'complete independent pose inventory');finite([v for row in expected for v in row[:17]],918,'all expected pose words before maxima')
    q=[];position=[];rotation=[];diagnostics=[]
    for i,(a,b)in enumerate(zip(rows,expected)):
        require(a[18:]==b[17:],'exact update/projection counts case'+str(i))
        q.extend(abs(x-y)for x,y in zip(a[1:8],b[:7]));position.extend(abs(x-y)for x,y in zip(a[8:11],b[7:10]));rotation.extend(abs(x-y)for x,y in zip(quaternion_matrix(a[11:15]),quaternion_matrix(b[10:14])));diagnostics.extend(abs(x-y)for x,y in zip(a[15:18],b[14:17]))
        require(all(lo<=x<=hi for x,(lo,hi)in zip(a[1:8],e['joint_limits_radians'])) and all(abs(x-y)<=.1+2e-15 for x,y in zip(a[1:8],cases[i][:7])),'source limits and global step cap')
        pair=e['pose_cases'][i].get('sign_pair')
        if i%2 and pair is not None:
            require(pair==e['pose_cases'][i-1].get('sign_pair') and actual[i*160:(i+1)*160]==actual[(i-1)*160:i*160],'actual paired target-sign whole pose bytes')
    b=e['budgets']
    return dict(joints_radians=metric(q,b['joint_radians_atol'],'pose joint coordinates'),position_metres=metric(position,b['position_metres_atol'],'fresh pose position'),physical_rotation=metric(rotation,b['physical_rotation_matrix_max_entry_atol'],'fresh pose quaternion physical rotation'),diagnostics=metric(diagnostics,b['pose_error_diagnostics_atol'],'fresh named-unit diagnostics'),all_actual_finite_words=918,all_expected_finite_words=918,all_status_and_counters_exact=True,whole_pose_convergence_claimed=False)

def reader_guards(actual_rotation,actual_pose,expected_rotation,expected_pose):
    negatives=0
    for raw,decoder,late in [(actual_rotation,decode_rotation,len(actual_rotation)-8),(actual_pose,decode_pose,len(actual_pose)-24),(expected_rotation,decode_expected_rotation,len(expected_rotation)-8),(expected_pose,decode_expected_pose,len(expected_pose)-24)]:
        for bad in [raw[:-1],raw+b'\0']:
            try:decoder(bad)
            except RuntimeError:negatives+=1
            else:raise RuntimeError('reader admitted wrong actual/expected extent')
        for number in [math.nan,math.inf,-math.inf]:
            bad=bytearray(raw);struct.pack_into('<d',bad,late,number)
            try:decoder(bytes(bad))
            except RuntimeError:negatives+=1
            else:raise RuntimeError('reader admitted late actual/expected IEEE')
    for raw,decoder,offset,value in [(actual_rotation,decode_rotation,4199*40,1),(actual_pose,decode_pose,53*160,1),(actual_pose,decode_pose,len(actual_pose)-16,0),(actual_pose,decode_pose,len(actual_pose)-8,8),(expected_pose,decode_expected_pose,len(expected_pose)-16,0),(expected_pose,decode_expected_pose,len(expected_pose)-8,8)]:
        bad=bytearray(raw);struct.pack_into('<q',bad,offset,value)
        try:decoder(bytes(bad))
        except RuntimeError:negatives+=1
        else:raise RuntimeError('reader admitted wrong actual/expected status/counter')
    require(negatives==26,'all new reader poison cases refused')
    return dict(passed=True,new_negative_cases=negatives,expected_and_actual=True,no_oracle_or_product_regeneration=True)

class Run:
    def __init__(self,out,record):self.out=out;self.record=record
    def save(self):(self.out/'record.json').write_text(json.dumps(self.record,indent=2)+'\n')
    def command(self,name,argv):
        began=time.monotonic();entry=dict(name=name,argv=[str(x)for x in argv]);self.record['commands'].append(entry);self.save()
        try:
            p=subprocess.run(entry['argv'],cwd=ROOT,capture_output=True,timeout=180,env={**os.environ,'ASAN_OPTIONS':'detect_leaks=0:halt_on_error=1','UBSAN_OPTIONS':'halt_on_error=1:print_stacktrace=1'});stdout,stderr=p.stdout,p.stderr;entry['exit']=p.returncode
        except subprocess.TimeoutExpired as error:
            stdout,stderr=error.stdout or b'',error.stderr or b'';entry.update(timed_out=True,exit=None);raise
        finally:
            for kind in ['stdout','stderr']:(self.out/(name+'.'+kind)).write_bytes(locals().get(kind,b''))
            entry.update(seconds=time.monotonic()-began,stdout=identity(self.out/(name+'.stdout')),stderr=identity(self.out/(name+'.stderr')));self.save()
        require(p.returncode==0 and not p.stderr,(name,p.stdout.decode(errors='replace'),p.stderr.decode(errors='replace')));return p.stdout

def native(args):
    require(platform.system()=='Darwin' and platform.machine()=='arm64' and os.sys.byteorder=='little','frozen Darwin-arm64 binary64/FPCR0 profile')
    e,cases,rotation,flags=frozen();oracle=args.oracle.resolve();expected_rotation,expected_pose,oracle_flags=expected_tables(oracle)
    require(oracle_flags==flags,'independent targets use exact predeclared cut classification')
    reference=json.loads((oracle/'reference.json').read_text());require(reference['schema']=='psionic.panda-pose-ik.independent-reference.v1' and reference['passed'] and reference['experiment_sha256']==EXPERIMENT_SHA and reference['addendum_sha256']==ADDENDUM_SHA and reference['budgets']==e['budgets'] and reference['rotation_cases']==4200 and reference['pose_cases']==54 and reference['SDK_calls']==reference['Coax_calls']==0,'previously prepared exact independent reference')
    require(all(identity(oracle/name)==value for name,value in reference['outputs'].items()),'all original independent expected artifact bytes')
    require(all(sha(ROOT/name)==digest for name,digest in reference['source_sha256'].items()),'every original independent-source byte restored/unchanged')
    out=args.output.resolve();require(not out.exists(),'preserve every earlier or failed component attempt');out.mkdir(parents=True)
    compiler=args.compiler.resolve();record=dict(schema='psionic.panda-pose-ik.native.v1',passed=False,SDK_calls=0,NN_calls=0,simulation_steps=0,old_gates_repeated=False,oracle_regenerated=False,experiment_sha256=EXPERIMENT_SHA,input_addendum_sha256=ADDENDUM_SHA,compiler=identity(compiler),commands=[],budgets=e['budgets'],source_sha256={str(p.relative_to(ROOT)):v['sha256']for p,v in graph(SOURCE).items()})
    run=Run(out,record);run.save();began=time.monotonic()
    try:
        dependencies=graph(SOURCE);dependencies[NATIVE]=identity(NATIVE);dependencies[Path(__file__).resolve()]=identity(__file__)
        for p in FIXTURE.iterdir():
            if p.is_file():dependencies[p]=identity(p)
        for p in oracle.iterdir():
            if p.is_file():dependencies[p]=identity(p)
        record['frozen_inputs']={str(p.relative_to(ROOT))if p.is_relative_to(ROOT)else str(p):v for p,v in sorted(dependencies.items())};run.save()
        lower=[x[0]for x in e['joint_limits_radians']];upper=[x[1]for x in e['joint_limits_radians']];guard_seed=cases[0][:7]
        body=struct.pack('<4I',0x314f5050,4200,54,1)+rotation+(FIXTURE/'pose-cases-v2.f64').read_bytes()+struct.pack('<21d',*lower,*upper,*guard_seed);(out/'input.bin').write_bytes(body);record['input']=identity(out/'input.bin');record['guard_seed_source']='effective pose case0; no nominal controller home assumption'
        emissions={}
        if args.reuse_first:
            previous=args.reuse_first.resolve();old=json.loads((previous/'failed-record-v1.json').read_text())
            require(not old['passed'] and old['compiler']==record['compiler'],'exact failed first native context')
            allowed={str(NATIVE.relative_to(ROOT)),str(Path(__file__).resolve().relative_to(ROOT))}
            for name,value in old['frozen_inputs'].items():
                path=ROOT/name if not Path(name).is_absolute()else Path(name)
                require(name in allowed or identity(path)==value,'all other frozen first-run inputs unchanged')
            for mode in ['debug','release']:
                c=previous/mode/'panda-pose-ik.c';h=previous/mode/'panda-pose-ik.h';require(old['emissions'][mode]==dict(C=identity(c),header=identity(h)),'exact first emitted C/header bytes');emissions[mode]=(c,h)
            record['tooling_followup']=dict(first_failed_record=identity(previous/'failed-record-v1.json'),first_native_source=identity(previous/'executed-panda_pose_ik_native.c'),first_driver=identity(previous/'executed-panda_pose_ik_native_reference.py'),addendum=identity(FIXTURE/'native-guard-followup-v1.json'),Coax_emissions_repeated=False,debug_corpus_repeated=False,all_other_frozen_inputs_exact=True)
        else:
            for mode,extra in [('debug',[]),('release',['--release'])]:
                directory=out/mode;directory.mkdir();c=directory/'panda-pose-ik.c';h=directory/'panda-pose-ik.h';c.write_bytes(run.command('emit-'+mode,[compiler,'c',SOURCE.relative_to(ROOT),'--library',*extra,'--header',h]));emissions[mode]=(c,h)
        record['emissions']={m:dict(C=identity(c),header=identity(h))for m,(c,h)in emissions.items()};run.save();all_bytes=[]
        common=['-std=c11','-Wall','-Wextra','-Werror','-Wno-unused-function','-Wno-misleading-indentation','-fno-fast-math','-ffp-contract=off','-fno-builtin-sin','-fno-builtin-cos']
        for mode,source_mode,extra in [('debug','debug',['-O0','-g']),('release','release',['-O2']),('sanitized','debug',['-O1','-g','-fsanitize=address,undefined','-fno-sanitize-recover=all','-fno-omit-frame-pointer'])]:
            directory=out/('native-'+mode);directory.mkdir();c,h=emissions[source_mode];binary=directory/'native'
            if args.reuse_first and mode=='debug':
                rpath=previous/'native-debug/rotation.bin';ppath=previous/'native-debug/pose.bin';rbytes=rpath.read_bytes();pbytes=ppath.read_bytes()
                metrics=dict(rotation=compare_rotation(rbytes,expected_rotation,flags,e),pose=compare_pose(pbytes,expected_pose,e,cases))
                run.command('compile-effects-debug-guards',[os.environ.get('CC','cc'),*common,*extra,'-I'+str(h.parent),NATIVE,previous/'native-debug/coax.o','-lm','-o',binary])
                report=json.loads(run.command('run-debug-guards',[binary,out/'input.bin','--guards-only']))
                require(report['passed'] and report['guards_only'] and report['executed_rotation_corpus']==report['executed_pose_corpus']==0 and report['SDK_calls']==0 and report['fpcr']==0,'new guard-only continuation, zero repeated corpus')
                require(report['alias_cases']==8 and report['seed_endpoints']==14 and report['XYZ_endpoints']==6 and report['norm_shell_contexts']==12 and report['exact_literals']==8 and report['nonfinite_lane_patterns']==110,'all corrected guard contexts')
                record.setdefault('modes',{})[mode]=dict(original_native_failed_after_complete_corpus=True,original_failed_record=identity(previous/'failed-record-v1.json'),original_native_binary=identity(previous/'native-debug/native'),guard_continuation=report,guard_binary=identity(binary),rotation=identity(rpath),pose=identity(ppath),metrics=metrics,full_debug_assertion_total_not_reconstructed=True)
            else:
                run.command('compile-Coax-'+mode,[os.environ.get('CC','cc'),*common,'-Wno-unused-parameter',*extra,'-c',c,'-o',directory/'coax.o'])
                run.command('compile-effects-'+mode,[os.environ.get('CC','cc'),*common,*extra,'-I'+str(h.parent),NATIVE,directory/'coax.o','-lm','-o',binary])
                report=json.loads(run.command('run-'+mode,[binary,out/'input.bin',directory/'rotation.bin',directory/'pose.bin']))
                require(report['passed'] and report['SDK_calls']==0 and not report['guards_only'] and report['executed_rotation_corpus']==4200 and report['executed_pose_corpus']==54 and report['rotation_cases']==4200 and report['pose_cases']==54 and report['rotation_record_bytes']==40 and report['pose_record_bytes']==160 and report['fpcr']==0,'exact new native inventory')
                require(report['full_byte_sign_checks']==12654 and report['alias_cases']==8 and report['seed_endpoints']==14 and report['XYZ_endpoints']==6 and report['norm_shell_contexts']==12 and report['exact_literals']==8 and report['nonfinite_lane_patterns']==110,'all frozen native contract guards')
                rbytes=(directory/'rotation.bin').read_bytes();pbytes=(directory/'pose.bin').read_bytes();metrics=dict(rotation=compare_rotation(rbytes,expected_rotation,flags,e),pose=compare_pose(pbytes,expected_pose,e,cases))
                record.setdefault('modes',{})[mode]=dict(native=report,binary=identity(binary),rotation=identity(directory/'rotation.bin'),pose=identity(directory/'pose.bin'),metrics=metrics)
            all_bytes.append((rbytes,pbytes));run.save()
        require(all_bytes[0]==all_bytes[1]==all_bytes[2],'whole rotation and pose files byte-identical across3profiles')
        record['new_reader_guards']=reader_guards(*all_bytes[0],(oracle/'rotation-expected.f64').read_bytes(),(oracle/'pose-expected.bin').read_bytes())
        require(all(identity(p)==v for p,v in dependencies.items()) and identity(compiler)==record['compiler'],'all executed source/compiler/reference bytes unchanged')
        record.update(passed=True,whole_mode_bytes_exact=True,rotation_sha256=hashlib.sha256(all_bytes[0][0]).hexdigest(),pose_sha256=hashlib.sha256(all_bytes[0][1]).hexdigest())
    except BaseException as error:record['failure']=repr(error);raise
    finally:
        record['seconds']=time.monotonic()-began;record['committed_output_files']={str(p.relative_to(out)):identity(p)for pattern in ['native-*/rotation.bin','native-*/pose.bin']for p in out.glob(pattern)};run.save()
    print(json.dumps(dict(passed=True,record=str(out/'record.json'),seconds=record['seconds'],assertions_by_context={m+('_guard_continuation'if 'guard_continuation'in x else ''):x.get('native',x.get('guard_continuation'))['assertions']for m,x in record['modes'].items()})))

def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--compiler',type=Path,required=True);parser.add_argument('--output',type=Path,required=True);parser.add_argument('--oracle',type=Path,default=FIXTURE);parser.add_argument('--reuse-first',type=Path);native(parser.parse_args())
if __name__=='__main__':main()
