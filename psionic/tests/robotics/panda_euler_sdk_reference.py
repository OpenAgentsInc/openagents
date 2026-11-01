"""First three SDK Euler steps after immutable metadata admission.

This authored evaluator links the installed approved MuJoCo SDK. It never
imports the SDK Python package, runs a downloaded script, renders or opens NN.
"""
from pathlib import Path
import argparse, copy, hashlib, json, math, os, platform, struct, subprocess, time
import xml.etree.ElementTree as ET
from panda_actuation_sdk_reference import validate_model, CACHE

ROOT=Path(__file__).resolve().parents[3]
FIXTURE=ROOT/'psionic/fixtures/robotics/panda-euler'
COLLECTOR=ROOT/'psionic/tests/robotics/panda_euler_sdk_collect.c'
PROFILE=FIXTURE/'sdk-capture-profile.json'
SOURCE_ADDENDUM=FIXTURE/'sdk-source-addendum-v1.json'
EXPERIMENT_SHA='9f4eb555739fdc6ac2d61f02d68a1d432599078ce77eb792eba2ee68b1572262'
PA_EXPERIMENT=ROOT/'psionic/fixtures/robotics/panda-actuation/experiment.json'
PA_EXPERIMENT_SHA='4da210b559aed5f07ad4ab8d5ae5a7393c26fef0885d16dee19057ec605fbe5e'
def require(ok,message):
    if not ok:raise RuntimeError(message)
def sha(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()
def identity(path):return {'bytes':Path(path).stat().st_size,'sha256':sha(path)}
def validate_step_model(model):
    require(sha(PA_EXPERIMENT)==PA_EXPERIMENT_SHA,'frozen actuator coefficient policy')
    pa=json.loads(PA_EXPERIMENT.read_text())
    validate_model(model,pa)
    options=model['options']
    require(options['disableflags']==17 and options['enableflags']==0,'exact constraints/contact-off profile')
    require(options['integrator']==0 and options['timestep']==0.002,'source Euler period')
    require(options['density']==1.2 and options['viscosity']==0.00002 and options['wind']==[0,0,0],'source air profile')
    require(options['gravity']==[0,0,-9.81],'source gravity')
    return {'passed':True,'original_disableflags':0,'modified_disableflags':17,'EULERDAMP_enabled':True,'step_seconds':0.002,'before_any_case_forward_or_step':True}
def step_guard_checks(model):
    changes=[('disableflags',0),('disableflags',17|(1<<14)),('integrator',1),('timestep',0.004),('density',1),('viscosity',0.001),('wind',[1,0,0]),('gravity',[0,0,-1])]
    for key,value in changes:
        bad=copy.deepcopy(model);bad['options'][key]=value
        try:validate_step_model(bad)
        except RuntimeError:pass
        else:raise RuntimeError('Euler profile poison admitted '+key)
    return {'passed':True,'positive_cases':1,'new_Euler_profile_negative_cases':len(changes),'SDK_calls':0}
def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output',type=Path,required=True)
    parser.add_argument('--scene',type=Path,default=CACHE/'scene.xml')
    parser.add_argument('--sdk',type=Path,default=Path.home()/'.cache/psionic/groot-reference-env/lib/python3.12/site-packages/mujoco')
    parser.add_argument('--capture-prepared',action='store_true')
    args=parser.parse_args();require(platform.system()=='Darwin' and platform.machine()=='arm64' and os.sys.byteorder=='little','qualified Darwin-arm64 target')
    require(sha(FIXTURE/'experiment.json')==EXPERIMENT_SHA,'frozen experiment');experiment=json.loads((FIXTURE/'experiment.json').read_text());profile=json.loads(PROFILE.read_text())
    require(profile['frozen_before_SDK_load'] and profile['experiment_sha256']==EXPERIMENT_SHA and profile['row_shape']==[3,154],'frozen acquisition schema')
    source_profile=dict(profile['source_sha256']);addendum=json.loads(SOURCE_ADDENDUM.read_text());require(addendum['original_profile_sha256']==sha(PROFILE) and set(addendum['source_sha256'])=={str(Path(__file__).resolve().relative_to(ROOT))},'exact pre-load tooling correction');source_profile.update(addendum['source_sha256'])
    for name,digest in source_profile.items():require(sha(ROOT/name)==digest,'frozen acquisition source '+name)
    require(sha(args.scene)==experiment['scene_sha256'],'exact source XML');sdk=args.sdk.resolve();library=sdk/'libmujoco.3.3.1.dylib';require(sha(library)==experiment['SDK_sha256'],'exact installed SDK')
    cases=(FIXTURE/'cases.f64').read_bytes();require(len(cases)==6*36*8,'six finite frozen input rows');numbers=struct.unpack('<216d',cases);require(all(math.isfinite(x)for x in numbers),'finite input words');require(cases==struct.pack('<216d',*[x for case in experiment['cases']for name in ['q','v','ctrl','external']for x in case[name]]),'JSON/input bit identity')
    sources={str(p.relative_to(ROOT)):identity(p)for p in [COLLECTOR,Path(__file__).resolve(),ROOT/'psionic/tests/robotics/panda_actuation_sdk_collect.c',ROOT/'psionic/tests/robotics/panda_actuation_sdk_reference.py',PA_EXPERIMENT,PROFILE,SOURCE_ADDENDUM,FIXTURE/'experiment.json',FIXTURE/'cases.f64']}
    headers={str(p.relative_to(sdk)):sha(p)for p in sorted((sdk/'include/mujoco').glob('*.h'))}
    env=os.environ.copy();env['DYLD_LIBRARY_PATH']=str(sdk);env['ASAN_OPTIONS']='detect_leaks=0:halt_on_error=1';env['UBSAN_OPTIONS']='halt_on_error=1'
    out=args.output.resolve();record=None
    def save():(out/'record.json').write_text(json.dumps(record,indent=2)+'\n')
    def command(name,argv):
        start=time.monotonic();run=subprocess.run([str(x)for x in argv],cwd=ROOT,capture_output=True,env=env,timeout=180);(out/(name+'.stdout')).write_bytes(run.stdout);(out/(name+'.stderr')).write_bytes(run.stderr)
        record['commands'].append({'name':name,'argv':[str(x)for x in argv],'exit':run.returncode,'seconds':time.monotonic()-start,'stdout':identity(out/(name+'.stdout')),'stderr':identity(out/(name+'.stderr'))});save();require(not run.returncode and not run.stderr,(name,run.stdout.decode(errors='replace'),run.stderr.decode(errors='replace')));return run.stdout
    if not args.capture_prepared:
        require(not out.exists(),'preserve prior/partial acquisition');out.mkdir(parents=True)
        tree=ET.parse(args.scene);require(not any(n.tag in ['include','plugin','extension']for n in tree.iter()),'no unbound imports/plugins')
        assets=[]
        for node in tree.getroot().findall('./asset/*'):
            if node.get('file'):
                name=Path(node.get('file'));name=name if name.is_absolute()else args.scene.parent/name;assets.append({'path':str(name),'identity':identity(name)})
        require(len(assets)==86 and len({str(Path(a['path']).resolve())for a in assets})==85,'exact source declarations/unique asset paths')
        assets=list({str(Path(a['path']).resolve()):a for a in assets}.values())
        record={'schema':'psionic.panda-euler.sdk-reference.v1','passed':False,'prepared':False,'capture_started':False,'source':sources,'SDK_library':identity(library),'SDK_headers':headers,'source_XML':identity(args.scene),'assets':assets,'commands':[],'SDK_runtime_uninstrumented':True,'collector_ASan_UBSan_fail_fast':True,'ASAN_leak_detection':False,'selected_experiment_rows':[3,4,5],'explicit_forwards_started':0,'steps_started':0,'NN_calls':0,'render_calls':0,'post_step_forwards':0};save()
        command('compile',[os.environ.get('CC','cc'),'-std=c11','-O1','-g','-Wall','-Wextra','-Werror','-Wno-misleading-indentation','-fno-fast-math','-ffp-contract=off','-fsanitize=address,undefined','-fno-sanitize-recover=all','-fno-omit-frame-pointer','-I'+str(sdk/'include'),COLLECTOR,library,'-Wl,-rpath,'+str(sdk),'-lm','-o',out/'collect'])
        command('metadata',[out/'collect','metadata',args.scene,out/'model.json']);model=json.loads((out/'model.json').read_text());record['support_admission']=validate_step_model(model);record['new_profile_guard_checks']=step_guard_checks(model);record.update(prepared=True,binary=identity(out/'collect'),metadata=identity(out/'model.json'));save();print(json.dumps({'prepared':True,'explicit_forwards':0,'steps':0,'record':str(out/'record.json')}));return
    record=json.loads((out/'record.json').read_text());require(record['prepared'] and not record['passed'] and not record['capture_started'],'one first acquisition only');require(record['source']==sources and record['binary']==identity(out/'collect') and record['metadata']==identity(out/'model.json') and record['SDK_library']==identity(library) and record['SDK_headers']==headers and record['source_XML']==identity(args.scene),'immutable prepared support context')
    for asset in record['assets']:require(asset['identity']==identity(asset['path']),'unchanged model asset')
    validate_step_model(json.loads((out/'model.json').read_text()));(out/'input.bin').write_bytes(struct.pack('<II',0x31554550,3)+cases[3*36*8:]);record['capture_started']=True;save()
    try:
        report=json.loads(command('capture',[out/'collect','capture',args.scene,out/'input.bin',out/'values.f64',out/'model-capture.json',out/'progress.jsonl']))
        require(report['passed'] and report['explicit_forwards']==3 and report['steps']==3 and report['post_step_forwards']==report['NN_calls']==report['render_calls']==0,'exact first SDK work scope')
        events=[json.loads(x)for x in (out/'progress.jsonl').read_text().splitlines()];require(events==[{'case':row,'event':event}for row in range(3)for event in ['before_forward','after_forward','before_step','after_step']],'every new stage once')
        require(identity(out/'model-capture.json')==record['metadata'],'actual capture instance has exact admitted profile');raw=(out/'values.f64').read_bytes();require(len(raw)==3*154*8 and all(math.isfinite(x)for x in struct.unpack('<462d',raw)),'all observations finite')
        record.update(passed=True,native=report,values=identity(out/'values.f64'),input=identity(out/'input.bin'),progress=identity(out/'progress.jsonl'),explicit_forwards_started=3,explicit_forwards_completed=3,steps_started=3,steps_completed=3)
    except BaseException as error:record['failure']=repr(error);raise
    finally:
        if (out/'progress.jsonl').exists():
            events=[json.loads(x)for x in (out/'progress.jsonl').read_text().splitlines()]
            for kind,key in [('before_forward','explicit_forwards_started'),('after_forward','explicit_forwards_completed'),('before_step','steps_started'),('after_step','steps_completed')]:record[key]=sum(x['event']==kind for x in events)
        save()
    print(json.dumps({'passed':True,'record':str(out/'record.json'),'values':record['values']}))
if __name__=='__main__':main()
