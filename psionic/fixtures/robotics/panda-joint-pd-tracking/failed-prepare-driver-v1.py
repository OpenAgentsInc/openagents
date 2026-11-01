#!/usr/bin/env python3
"""NEW four-case tracking acquisition; preparation never runs the SDK binary.

--prepare emits/builds once. --admit-metadata-prepared loads metadata only; --capture-prepared then runs
3000steps once, authorized separately. Offline replay is a different driver.
"""
from pathlib import Path
import argparse,json,math,os,platform,struct,time
from panda_joint_pd_prefix_reference import Run,identity,sha,assets,validate_step_model
from panda_pose_ik_native_reference import graph
ROOT=Path(__file__).resolve().parents[3];FIXTURE=ROOT/'psionic/fixtures/robotics/panda-joint-pd-tracking';TESTS=ROOT/'psionic/tests/robotics';SOURCE=TESTS/'panda-joint-pd-tracking.coax';COLLECTOR=TESTS/'panda_joint_pd_tracking_sdk_collect.c';FORMAT=TESTS/'panda_joint_pd_tracking_format.h';TRANSPORT=FIXTURE/'transport-profile-v1.json'
EXPERIMENT_SHA='cfe805220004838440a4ea92c258880597ca5de42b7360ca7102281e242b3612';COMPILER_SHA='a429ad59a39f1b77adee37dd09895d8f7fd36b3149bc3fdf6eed4131a2d4f818';COMPILER=Path('/tmp/coax-spatial-fixture-fixes-reconstruction/coax');ROW=struct.Struct('<120d10q');OWN=struct.Struct('<120d10q18d4q');HEAD=struct.Struct('<4I')
def require(ok,message):
    if not ok:raise RuntimeError(message)
def finite(values,n,label):
    require(len(values)==n and all(math.isfinite(x)for x in values),label+' all finite/exactextent');return list(values)
def frozen():
    require(sha(FIXTURE/'experiment.json')==EXPERIMENT_SHA,'frozen NEW tracking experiment');e=json.loads((FIXTURE/'experiment.json').read_text());cases=(FIXTURE/'cases.f64').read_bytes();require(len(cases)==4*64*8 and identity(FIXTURE/'cases.f64')==e['input_file']and cases==b''.join(struct.pack('<64d',*c['input_f64'])for c in e['cases']),'exact4x64 casebits');finite(struct.unpack('<256d',cases),256,'case inputs');require(e['case_count']==4 and e['ticks_per_case']==750 and e['total_new_SDK_steps']==3000 and e['physics_step_seconds']==.002,'fixed newworkscope');require(all(sha(ROOT/p)==v for p,v in e['source_sha256'].items()),'unchanged source definitions');r=json.loads((FIXTURE/'analytic-reference.json').read_text());require(r['passed']and r['experiment']==identity(FIXTURE/'experiment.json')and r['SDK_calls']==r['Coax_calls']==r['NN_calls']==0,'independent analytic freeze');require(all(identity(FIXTURE/p)==v for p,v in r['outputs'].items()),'exact independent target bytes');finite(struct.unpack('<63084d',(FIXTURE/'reference-states.f64').read_bytes()),63084,'ALL independent751frame targets');finite(struct.unpack('<27000d',(FIXTURE/'external-forces.f64').read_bytes()),27000,'ALL independent externalforces');initial=b''.join(struct.pack('<18d',*c['initial_q9'],*c['initial_v9'])for c in e['cases']);t=json.loads(TRANSPORT.read_text());require(t['experiment_sha256']==EXPERIMENT_SHA and t['SDK_record_bytes']==1040 and t['own_record_bytes']==1216,'versioned transport');return e,HEAD.pack(0x31495450,4,750,1)+initial

def state(q,v,label):
    finite(list(q)+list(v),18,label)
    require(all(abs(x)<=(100 if i<7 else 1)for i,x in enumerate(q))and all(abs(x)<=(20 if i<7 else 2)for i,x in enumerate(v)),label+' fixedquery boxes')
def checked(row,index,e):
    case,tick=divmod(index,750);m=row[120:130];require(m[0:4]==(case,tick,0,0),'orderedcase/tick/success stage+code');require(0<=m[4]<=7 and 0<=m[5]<=m[6]<=1000000 and m[5]==min(tick,m[6])and m[7]in[0,1]and 0<=m[8]<=9 and 0<=m[9]<=2,'clipping/reference metadata');require(abs(row[119]-(tick+1)/500.)<=e['parity']['clock_atol_seconds'],'logicalstepclock');state(row[:9],row[9:18],'prestate');state(row[101:110],row[110:119],'poststate');limits=[80.]*5+[12.]*2;requested=row[60:67];command=row[67:74];require(all(c==min(L,max(-L,r))for r,c,L in zip(requested,command,limits))and m[4]==sum(r< -L or r>L for r,L in zip(requested,limits)),'recordedPjpclamp/count');require(row[74:81]==command and list(row[81:83])==e['cases'][case]['finger_targets2'],'controls useclippedcommand/fixedfingers');pulse=case==2 and 200<=tick<250;expected=[0.]*9;expected[1]=3. if pulse else 0.;require(list(row[83:92])==expected and m[7]==int(pulse),'Coaxfixedgeneralizedpulse')
def decode_sdk(raw,e):
    require(len(raw)==16+3000*1040 and HEAD.unpack_from(raw)==(0x31525450,4,750,1040),'completePTR1 extent/header');rows=[ROW.unpack_from(raw,16+i*1040)for i in range(3000)];finite([v for r in rows for v in r[:120]],360000,'ALL SDK observations BEFOREmax')
    for i,row in enumerate(rows):checked(row,i,e)
    for case in range(4):
        first=rows[case*750];require(struct.pack('<18d',*first[:18])==struct.pack('<18d',*e['cases'][case]['initial_q9'],*e['cases'][case]['initial_v9']),'exact firstrawstate')
        for tick in range(1,750):require(rows[case*750+tick][:18]==rows[case*750+tick-1][101:119],'rawSDK prestate ispreviouspost,no cachedgeometry substitution')
    return rows

def decode_own(raw,e):
    require(len(raw)==16+3000*1216 and HEAD.unpack_from(raw)==(0x314f5450,4,750,1216),'completePTO1 extent/header');rows=[OWN.unpack_from(raw,16+i*1216)for i in range(3000)];finite([v for r in rows for v in r[:120]+r[130:148]],414000,'ALL own/shadow observations BEFOREmax')
    for i,row in enumerate(rows):
        checked(row,i,e);state(row[130:139],row[139:148],'shadowstate');require(0<=row[148]<=9 and 0<=row[149]<=2 and 0<=row[150]<=9 and 0<=row[151]<=2 and row[148:150]==row[128:130],'Peuclipmetadata agreesfreshPa')
    for case in range(4):
        for tick in range(1,750):require(rows[case*750+tick][:18]==rows[case*750+tick-1][101:119],'ownfreshprestate ispreviousownpost')
    return rows

def progress(path,strict=False):
    events=[];bad=0
    if path.exists():
        for line in path.read_bytes().splitlines():
            try:events.append(json.loads(line))
            except(ValueError,UnicodeError):bad+=1
    if strict:
        want=[]
        for case in range(4):
            want.extend(dict(case=case,tick=-1,event=k,stage=0,code=0)for k in['before_initial_forward','after_initial_forward'])
            for tick in range(750):want.extend(dict(case=case,tick=tick,event=k,stage=0,code=0)for k in['before_step','after_step'])
        require(not bad and events==want,'exact4forward/3000step sourcephaseledger')
    return dict(explicit_forwards_started=sum(x.get('event')=='before_initial_forward'for x in events),explicit_forwards_completed=sum(x.get('event')=='after_initial_forward'for x in events),steps_started=sum(x.get('event')=='before_step'for x in events),steps_completed=sum(x.get('event')=='after_step'for x in events),malformed_progress_lines=bad)

def verify_sources(source):require(all(identity(ROOT/name)==v for name,v in source.items()),'every frozen executed source unchanged')
def source_inventory():
    paths=set(graph(SOURCE));paths.update([COLLECTOR,FORMAT,Path(__file__).resolve(),TESTS/'panda_actuation_sdk_collect.c',TESTS/'panda_joint_pd_prefix_reference.py',TESTS/'panda_euler_sdk_reference.py',TESTS/'panda_actuation_sdk_reference.py',TESTS/'panda_actuation_reference.py',TESTS/'panda_pose_ik_native_reference.py',TRANSPORT,FIXTURE/'experiment.json',FIXTURE/'cases.f64',FIXTURE/'analytic-reference.json',FIXTURE/'reference-states.f64',FIXTURE/'external-forces.f64',ROOT/'psionic/fixtures/robotics/panda-joint-pd-prefix/sdk-reference.json']);return {str(p.relative_to(ROOT)):identity(p)for p in sorted(paths)}
def flags():return ['-std=c11','-Wall','-Wextra','-Werror','-Wno-unused-function','-Wno-misleading-indentation','-fno-fast-math','-ffp-contract=off','-fno-builtin-sin','-fno-builtin-cos']
def validate_model(model,e):
    admission=validate_step_model(model);require(model['qpos0']==e['mapping']['qpos0'],'exactcompileddefaultqpos0');require(model['model']==e['mapping']['source_model'],'exactsource modelcounts');return admission

def context(args,e):
    compiler=args.coax.resolve();scene=args.scene.resolve();sdk=args.sdk.resolve();library=sdk/'libmujoco.3.3.1.dylib';require(sha(compiler)==COMPILER_SHA and sha(scene)==e['reset']['source_XML_sha256']and sha(library)==e['reset']['sdk_library_sha256'],'exact original compiler/XML/SDK');declarations,unique=assets(scene);prior=json.loads((ROOT/'psionic/fixtures/robotics/panda-joint-pd-prefix/sdk-reference.json').read_text());headers={str(p.relative_to(sdk)):identity(p)for p in sorted((sdk/'include/mujoco').glob('*.h'))};require(prior['passed']and prior['source_XML']==identity(scene)and prior['SDK_library']==identity(library)and prior['SDK_headers']==headers and prior['assets']==unique and prior['asset_declarations']==declarations,'reusepinned85asset/headeridentities,no oldgate');return compiler,scene,sdk,library,declarations,unique,headers

def prepare(args,e,input_bytes):
    out=args.output.resolve();require(not out.exists(),'preserve allpreparations');out.mkdir(parents=True);compiler,scene,sdk,library,declarations,unique,headers=context(args,e);record=dict(schema='psionic.panda-joint-pd-tracking.sdk-reference.v1',passed=False,prepared=False,capture_started=False,SDK_loads=0,experiment_sha256=EXPERIMENT_SHA,commands=[],source=source_inventory(),compiler=identity(compiler),compiler_input_fingerprint='580cc4b7c058db8b828df2d13b43eb4feb38af53f4d692430943d51cca81e036',SDK_library=identity(library),SDK_headers=headers,source_XML=identity(scene),assets=unique,asset_declarations=declarations,transport=identity(TRANSPORT),NN_calls=0,render_calls=0,post_step_forwards=0,SDK_runtime_uninstrumented=True,collector_and_Coax_ASan_UBSan_fail_fast=True);run=Run(out,record);run.save()
    try:
        record['emissions']={}
        for mode,extra in[('debug',[]),('release',['--release'])]:
            d=out/mode;d.mkdir();c=d/'panda-joint-pd-tracking.c';h=d/'panda-joint-pd-tracking.h';c.write_bytes(run.command('emit-'+mode,[compiler,'c',SOURCE.relative_to(ROOT),'--library',*extra,'--header',h]));record['emissions'][mode]=dict(source=identity(c),header=identity(h));run.save()
        (out/'input.bin').write_bytes(input_bytes);record['input']=identity(out/'input.bin');numeric=['-O1','-g','-fsanitize=address,undefined','-fno-sanitize-recover=all','-fno-omit-frame-pointer'];run.command('compile-sdk-Coax',[os.environ.get('CC','cc'),*flags(),'-Wno-unused-parameter',*numeric,'-c',out/'debug/panda-joint-pd-tracking.c','-o',out/'sdk-coax.o']);run.command('compile-sdk-effects',[os.environ.get('CC','cc'),*flags(),*numeric,'-I'+str(out/'debug'),'-I'+str(sdk/'include'),COLLECTOR,out/'sdk-coax.o',library,'-Wl,-rpath,'+str(sdk),'-lm','-o',out/'collect']);verify_sources(record['source']);native_source=TESTS/'panda_joint_pd_tracking_native.c';record['new_ABI_guard_source']=identity(native_source);run.command('compile-new-ABI-guards',[os.environ.get('CC','cc'),*flags(),*numeric,'-I'+str(out/'debug'),native_source,out/'sdk-coax.o','-lm','-o',out/'guard-native']);guard_env=dict(os.environ,ASAN_OPTIONS='detect_leaks=0:halt_on_error=1',UBSAN_OPTIONS='halt_on_error=1');record['new_ABI_guards']=json.loads(run.command('new-ABI-guards-once',[out/'guard-native','--guards-only'],guard_env));require(record['new_ABI_guards']['passed']and record['new_ABI_guards']['SDK_calls']==0 and record['new_ABI_guards']['alias_cases']==4,'new guarded ABI phase once');record['guard_binary']=identity(out/'guard-native');record.update(prepared=True,binary=identity(out/'collect'),SDK_loads=0,steps_started=0,steps_completed=0,explicit_forwards_started=0,explicit_forwards_completed=0)
    except BaseException as error:record['failure']=repr(error);raise
    finally:run.save()
    print(json.dumps(dict(prepared=True,SDK_loads=0,steps=0,record=str(out/'record.json'))))

def prepared_context(args,e,input_bytes):
    out=args.output.resolve();record=json.loads((out/'record.json').read_text());require(record['prepared']and not record['capture_started']and not record['passed'],'immutable prepared context');compiler,scene,sdk,library,declarations,unique,headers=context(args,e);verify_sources(record['source']);require(record['compiler']==identity(compiler)and record['binary']==identity(out/'collect')and record['input']==identity(out/'input.bin')and(out/'input.bin').read_bytes()==input_bytes,'immutablepreparedbinary/input');require(all(record[k]==v for k,v in dict(SDK_library=identity(library),SDK_headers=headers,source_XML=identity(scene),assets=unique,asset_declarations=declarations).items()),'unchanged completeeffectcontext');require(all(v==dict(source=identity(out/m/'panda-joint-pd-tracking.c'),header=identity(out/m/'panda-joint-pd-tracking.h'))for m,v in record['emissions'].items()),'onceemittedC/H unchanged')
    require(record['new_ABI_guard_source']==identity(TESTS/'panda_joint_pd_tracking_native.c')and record['guard_binary']==identity(out/'guard-native'),'frozennewguard source/binary')
    return out,record,scene,sdk

def admit(args,e,input_bytes):
    out,record,scene,sdk=prepared_context(args,e,input_bytes);require(not record.get('metadata_started',False),'one first metadata admission');env=dict(os.environ,DYLD_LIBRARY_PATH=str(sdk),ASAN_OPTIONS='detect_leaks=0:halt_on_error=1',UBSAN_OPTIONS='halt_on_error=1');run=Run(out,record,env);record['metadata_started']=True;run.save()
    try:
        report=json.loads(run.command('metadata-first-load',[out/'collect','metadata',scene,out/'model.json']));record['SDK_loads']=1;require(report==dict(prepared=True,steps=0,explicit_forwards=0),'metadata-only SDK scope');record['support_admission']=validate_model(json.loads((out/'model.json').read_text()),e);record['metadata']=identity(out/'model.json');verify_sources(record['source']);record['metadata_admitted']=True
    except BaseException as error:record['failure']=repr(error);raise
    finally:run.save()
    print(json.dumps(dict(metadata_admitted=True,SDK_loads=1,steps=0,record=str(out/'record.json'))))

def capture(args,e,input_bytes):
    out,record,scene,sdk=prepared_context(args,e,input_bytes);require(record.get('metadata_admitted',False)and record['metadata']==identity(out/'model.json'),'previous separate metadata admission');validate_model(json.loads((out/'model.json').read_text()),e);env=dict(os.environ,DYLD_LIBRARY_PATH=str(sdk),ASAN_OPTIONS='detect_leaks=0:halt_on_error=1',UBSAN_OPTIONS='halt_on_error=1');run=Run(out,record,env);record['capture_started']=True;run.save()
    try:
        report=json.loads(run.command('capture-once',[out/'collect','capture',scene,out/'input.bin',out/'sdk-trace.bin',out/'model-capture.json',out/'progress.jsonl']));record['SDK_loads']=2;require(report['passed']and report['cases']==4 and report['steps']==3000 and report['explicit_forwards']==4 and report['post_step_forwards']==report['NN_calls']==report['render_calls']==0 and report['record_bytes']==1040 and report['sparse_moment_rows']==report['generalized_force_equalities']==27000,'exactnewSDKworkscope');record.update(progress(out/'progress.jsonl',True));require(identity(out/'model-capture.json')==record['metadata'],'actualcapturemodel equalsadmittedmetadata');rows=decode_sdk((out/'sdk-trace.bin').read_bytes(),e);verify_sources(record['source']);record.update(passed=True,native=report,trace=identity(out/'sdk-trace.bin'),progress=identity(out/'progress.jsonl'),committed_records=len(rows),all_actual_finite_words=360000,serialized_status_stage_and_code=True)
    except BaseException as error:record['failure']=repr(error);raise
    finally:
        record.update(progress(out/'progress.jsonl'))
        if(out/'sdk-trace.bin').exists():size=(out/'sdk-trace.bin').stat().st_size;record['partial_or_full_trace']=identity(out/'sdk-trace.bin');record['committed_records']=max(0,(size-16)//1040);record['trailing_partial_record_bytes']=max(0,size-16)%1040
        run.save()
    print(json.dumps(dict(passed=True,record=str(out/'record.json'),trace=record['trace'],steps=record['steps_completed'])))

def main():
    p=argparse.ArgumentParser(description=__doc__);m=p.add_mutually_exclusive_group(required=True);m.add_argument('--prepare',action='store_true');m.add_argument('--admit-metadata-prepared',action='store_true');m.add_argument('--capture-prepared',action='store_true');p.add_argument('--output',type=Path,required=True);p.add_argument('--coax',type=Path,default=COMPILER);p.add_argument('--scene',type=Path,default=Path.home()/'.cache/psionic/libero114/f13aa24a3da8c43c7225569f28c562979fa0e35a/evaluation/demo_000/scene.xml');p.add_argument('--sdk',type=Path,default=Path.home()/'.cache/psionic/groot-reference-env/lib/python3.12/site-packages/mujoco');args=p.parse_args();require(platform.system()=='Darwin'and platform.machine()=='arm64'and os.sys.byteorder=='little','qualifiedDarwinarm64profile');e,initial=frozen();(prepare if args.prepare else admit if args.admit_metadata_prepared else capture)(args,e,initial)
if __name__=='__main__':main()
