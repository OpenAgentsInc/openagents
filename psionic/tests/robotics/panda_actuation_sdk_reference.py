"""Installed-SDK effects only; frozen metadata admission before 18 NEW forwards.
Pure validate_model() is reusable without importing or executing the SDK.
"""
from pathlib import Path
import argparse,copy,hashlib,json,math,os,platform,struct,subprocess,time
import xml.etree.ElementTree as ET
ROOT=Path(__file__).resolve().parents[3]
FIXTURE=ROOT/'psionic/fixtures/robotics/panda-actuation'
COLLECTOR=ROOT/'psionic/tests/robotics/panda_actuation_sdk_collect.c'
EXPERIMENT_SHA='4da210b559aed5f07ad4ab8d5ae5a7393c26fef0885d16dee19057ec605fbe5e'
CACHE=Path.home()/'.cache/psionic/libero114/f13aa24a3da8c43c7225569f28c562979fa0e35a/evaluation/demo_000'
JOINT_NAMES=['robot0_joint'+str(i+1) for i in range(7)]+['gripper0_finger_joint1','gripper0_finger_joint2']
ACTUATOR_NAMES=['robot0_torq_j'+str(i+1) for i in range(7)]+['gripper0_gripper_finger_joint1','gripper0_gripper_finger_joint2']
def require(ok,message):
    if not ok:raise RuntimeError(message)
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def identity(p):return {'bytes':Path(p).stat().st_size,'sha256':sha(p)}
def finite_tree(value):
    if isinstance(value,dict):return all(finite_tree(x) for x in value.values())
    if isinstance(value,list):return all(finite_tree(x) for x in value)
    return not isinstance(value,float) or math.isfinite(value)
def validate_model(model,experiment):
    """Reject unsupported *used* source coefficients and flags, before cases.
    Unused fixed-gain/NONE-dynamics parameter tails are captured, finite facts;
    they are never silently interpreted as extra supported dynamics.
    """
    require(finite_tree(model),'ALL metadata floating words finite')
    require(model['schema']=='psionic.panda-actuation.sdk-model.v1' and model['sdk_version_number']==331,'SDK/model schema')
    e=experiment['source_profile_expectations'];m=model['model'];o=model['options']
    for k,v in {'nq':23,'nv':21,'nu':9,'na':0,'njnt':11,'nbody':26,'nplugin':0,'ntendon':0,'nflex':0}.items():require(m[k]==v,'model '+k)
    require(m['nJmom']>=9 and o['disableactuator']==e['disabled_actuator_group_mask']==0,'sparse capacity/actuator mask')
    require(o['disableflags']&((1<<5)|(1<<7)|(1<<10))==0,'PASSIVE/CLAMPCTRL/ACTUATION enabled')
    callback_keys={'control','passive','contactfilter','sensor','time','act_dyn','act_gain','act_bias'}
    require(set(model['callbacks'])==callback_keys and all(v is True for v in model['callbacks'].values()),'exact eight callbacks null')
    require(len(model['qpos0'])==23 and len(model['qpos_spring'])==23,'all position/reference extents')
    joints=model['joints'];acts=model['actuators'];require(len(joints)==9 and len(acts)==9,'nine named joints/actuators')
    for i,(j,a) in enumerate(zip(joints,acts)):
        require(j['index']==i and j['id']==e['joint_ids'][i] and j['name']==JOINT_NAMES[i] and j['type']==(3 if i<7 else 2),'joint identity/type')
        require(j['qpos_adr']==j['dof_adr']==i and j['stiffness']==0 and j['damping']==e['joint_damping'][i],'joint address/stiffness/damping')
        require(j['actuator_force_limited']==0 and j['actuator_gravcomp']==0,'joint force/gravcomp flags')
        require(len(j['actuator_force_range'])==2 and len(j['axis'])==3,'joint fact extents')
        require(a['index']==i and a['id']==i and a['name']==ACTUATOR_NAMES[i] and a['transmission_type']==0 and a['transmission_ids'][0]==j['id'],'named joint transmission')
        require(len(a['transmission_ids'])==2 and a['gear']==e['gear'],'unit scalar transmission gear')
        require(a['dynamic_type']==0 and a['activation_adr']==-1 and a['activation_count']==0 and a['activation_early']==0 and a['activation_limited']==0 and a['plugin']==-1,'stateless nonplugin activation')
        require(a['gain_type']==0 and len(a['gain_params'])==10 and a['gain_params'][0]==(1 if i<7 else 1000),'fixed gain')
        require(len(a['dynamic_params'])==10 and len(a['bias_params'])==10,'all parameter extents')
        require(a['bias_type']==(0 if i<7 else 1) and (i<7 or a['bias_params'][:3]==[0,-1000,0]),'NONE/affine bias used coefficients')
        require(a['control_limited']==1 and a['control_range']==(e['arm_ctrl_ranges'][i] if i<7 else e['finger_ctrl_ranges'][i-7]),'control clipping profile')
        require(a['force_limited']==(0 if i<7 else 1) and (i<7 or a['force_range']==[-20,20]),'force clipping profile')
        require(len(a['force_range'])==2 and len(a['activation_range'])==2 and 0<=a['group']<31,'range/group fact extents')
    require(len(model['all_joint_facts'])==11 and all(j['stiffness']==0 and j['actuator_force_limited']==0 and j['actuator_gravcomp']==0 for j in model['all_joint_facts']),'all joint unsupported flags/stiffness')
    bodies=model['bodies'];geoms=model['geoms'];require(len(bodies)==26 and len(geoms)==m['ngeom'],'all body/geom metadata extents')
    for i,b in enumerate(bodies):
        require(b['id']==i and b['gravcomp']==0 and 0<=b['parent_id']<26 and 0<=b['root_id']<26,'body hierarchy/zero gravity compensation')
        require(len(b['inertia'])==3 and len(b['inertial_position'])==3 and len(b['inertial_quaternion_wxyz'])==4,'body inertia fact extents')
        n=b['geom_count'];at=b['geom_adr'];require(n>=0 and (n==0 or (0<=at<=len(geoms)-n and all(g['body_id']==i for g in geoms[at:at+n]))),'body geom mappings')
    for i,g in enumerate(geoms):require(g['id']==i and 0<=g['body_id']<26 and len(g['fluid12'])==12 and len(g['size'])==3 and len(g['position'])==3 and len(g['quaternion_wxyz'])==4,'geom/fluid fact extents')
    require(len(o['wind'])==len(o['gravity'])==3,'fluid/gravity option extents')
    return {'passed':True,'joint_actuator_pairs':9,'bodies':26,'geoms':len(geoms),'before_case_forwards':True,'source_used_parameters_only':True,'fluid_facts_not_owned_fluid_qualification':True}
def guard_checks(model,experiment):
    """NEW metadata-reader negative controls; no SDK execution."""
    specs=[(['model','nu'],8),(['model','na'],1),(['model','nplugin'],1),(['model','ntendon'],1),(['model','nflex'],1),(['options','disableactuator'],1),(['options','disableflags'],1<<10),(['callbacks','control'],False),(['joints',0,'qpos_adr'],1),(['joints',8,'dof_adr'],7),(['joints',7,'damping'],99),(['joints',0,'stiffness'],1),(['joints',0,'actuator_force_limited'],1),(['actuators',0,'gear'],[2,0,0,0,0,0]),(['actuators',0,'transmission_type'],1),(['actuators',0,'dynamic_type'],1),(['actuators',0,'activation_adr'],0),(['actuators',0,'activation_count'],1),(['actuators',0,'activation_early'],1),(['actuators',0,'activation_limited'],1),(['actuators',0,'gain_type'],1),(['actuators',0,'gain_params'],[2]+[0]*9),(['actuators',7,'bias_params'],[0,-999,0]+[0]*7),(['actuators',7,'bias_type'],0),(['actuators',7,'force_limited'],0),(['actuators',7,'force_range'],[-21,20]),(['actuators',0,'control_range'],[-81,80]),(['actuators',0,'plugin'],0),(['all_joint_facts',10,'stiffness'],1),(['bodies',25,'gravcomp'],1),(['geoms',-1,'fluid12'],[0]*11),(['geoms',-1,'size'],[0,0,float('nan')])]
    specs += [(['callbacks'],{k:v for k,v in model['callbacks'].items() if k!='time'}),(['callbacks'],dict(model['callbacks'],unexpected=True))]
    validate_model(model,experiment)
    for path,value in specs:
        bad=copy.deepcopy(model);at=bad
        for key in path[:-1]:at=at[key]
        at[path[-1]]=value
        try:validate_model(bad,experiment)
        except (RuntimeError,KeyError,TypeError,IndexError):pass
        else:raise RuntimeError('metadata poison admitted '+str(path))
    return {'passed':True,'positive_cases':1,'refused_cases':len(specs),'SDK_calls':0}
def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--output',type=Path,required=True);parser.add_argument('--scene',type=Path,default=CACHE/'scene.xml');parser.add_argument('--original',type=Path,default=CACHE/'original.xml');parser.add_argument('--sdk',type=Path,default=Path.home()/'.cache/psionic/groot-reference-env/lib/python3.12/site-packages/mujoco');parser.add_argument('--capture-prepared',action='store_true');a=parser.parse_args()
    require(platform.system()=='Darwin' and platform.machine()=='arm64' and os.sys.byteorder=='little','qualified Darwin-arm64 target');exp_path=FIXTURE/'experiment.json';profile_path=FIXTURE/'sdk-capture-profile.json';require(sha(exp_path)==EXPERIMENT_SHA,'frozen experiment');experiment=json.loads(exp_path.read_text());profile=json.loads(profile_path.read_text());require(profile['frozen_before_SDK_loading'] and profile['experiment_sha256']==EXPERIMENT_SHA and profile['row_shape']==[18,162],'frozen capture schema');require(identity(a.scene)['sha256']==profile['source_rewritten_XML_sha256'] and sha(a.original)==profile['source_original_XML_sha256'],'original/rewritten source XML')
    cases=FIXTURE/'cases.f64';require(sha(cases)==experiment['case_sha256']==profile['case_sha256'],'frozen cases');raw=cases.read_bytes();require(len(raw)==18*27*8 and all(math.isfinite(v) for v in struct.unpack('<486d',raw)),'complete finite cases');require(raw==struct.pack('<486d',*[v for row in experiment['cases'] for k in ['q','v','ctrl'] for v in row[k]]),'case JSON/binary identity')
    sdk=a.sdk.resolve();library=sdk/'libmujoco.3.3.1.dylib';require(sha(library)==experiment['SDK_binary_sha256'],'pinned installed SDK');headers={str(x.relative_to(sdk)):sha(x) for x in sorted((sdk/'include/mujoco').glob('*.h'))};sources={str(x.relative_to(ROOT)):sha(x) for x in [COLLECTOR,Path(__file__),exp_path,profile_path,cases]};out=a.output.resolve();env=os.environ.copy();env['DYLD_LIBRARY_PATH']=str(sdk);record=None
    def save():(out/'record.json').write_text(json.dumps(record,indent=2)+'\n')
    def command(name,args):
        began=time.monotonic();r=subprocess.run([str(x) for x in args],cwd=ROOT,capture_output=True,env=env,timeout=180);(out/(name+'.stdout')).write_bytes(r.stdout);(out/(name+'.stderr')).write_bytes(r.stderr);record['commands'].append({'name':name,'exit':r.returncode,'arguments':[str(x) for x in args],'seconds':time.monotonic()-began,'stdout':identity(out/(name+'.stdout')),'stderr':identity(out/(name+'.stderr'))});save();require(r.returncode==0 and not r.stderr,(name,r.stdout.decode(errors='replace'),r.stderr.decode(errors='replace')));return r.stdout
    if not a.capture_prepared:
        require(not out.exists(),'preserve earlier/partial SDK attempt');out.mkdir(parents=True);assets=[]
        for xml in [a.scene,a.original]:
            tree=ET.parse(xml);require(not any(n.tag in ['include','plugin','extension'] for n in tree.iter()),'no unbound XML imports/plugins')
        for n in ET.parse(a.scene).getroot().findall('./asset/*'):
            if n.get('file'):
                path=Path(n.get('file'));path=path if path.is_absolute() else a.scene.parent/path;assets.append({'kind':n.tag,'name':n.get('name',''),'path':str(path),'identity':identity(path)})
        record={'schema':'psionic.panda-actuation.sdk-reference.v1','passed':False,'prepared':False,'metadata_admitted_before_capture':False,'source_sha256':sources,'SDK_library_sha256':sha(library),'SDK_public_headers':headers,'source_XML':{'original':identity(a.original),'rewritten':identity(a.scene)},'source_assets':assets,'commands':[],'forward_calls_started':0,'integration_steps':0,'render_calls':0,'NN_calls':0,'SDK_runtime_uninstrumented':True,'collector_ASan_UBSan_fail_fast':True};save()
        command('compile',[os.environ.get('CC','cc'),'-std=c11','-O1','-g','-Wall','-Wextra','-Werror','-Wno-misleading-indentation','-fno-fast-math','-ffp-contract=off','-fsanitize=address,undefined','-fno-sanitize-recover=all','-fno-omit-frame-pointer','-I'+str(sdk/'include'),COLLECTOR,library,'-Wl,-rpath,'+str(sdk),'-lm','-o',out/'collect'])
        command('metadata',[out/'collect','metadata',a.scene,out/'model.json']);model=json.loads((out/'model.json').read_text());record['support_admission']=validate_model(model,experiment);record['reader_guards']=guard_checks(model,experiment);record.update(prepared=True,metadata_admitted_before_capture=True,binary_sha256=sha(out/'collect'),metadata_sha256=sha(out/'model.json'));save();print(json.dumps({'prepared':True,'case_forwards':0,'metadata_sha256':record['metadata_sha256'],'record':str(out/'record.json')}));return
    record=json.loads((out/'record.json').read_text());require(record['prepared'] and not record['passed'] and not record.get('capture_started',False) and record['forward_calls_started']==0 and record['source_sha256']==sources and record['binary_sha256']==sha(out/'collect') and record['metadata_sha256']==sha(out/'model.json') and record['SDK_library_sha256']==sha(library) and record['SDK_public_headers']==headers,'immutable admitted preflight');validate_model(json.loads((out/'model.json').read_text()),experiment)
    for asset in record['source_assets']:require(identity(asset['path'])==asset['identity'],'unchanged asset')
    (out/'input.bin').write_bytes(struct.pack('<II',0x31464150,18)+raw);record['capture_started']=True;save()
    try:
        stdout=command('capture',[out/'collect','capture',a.scene,out/'input.bin',out/'values.f64',out/'model-capture.json',out/'progress.jsonl']);report=json.loads(stdout);require(report['forward_calls']==18 and report['integration_steps']==report['render_calls']==report['NN_calls']==0,'exact SDK work scope');progress=[json.loads(x) for x in (out/'progress.jsonl').read_text().splitlines()];require(progress==[{'case':i,'event':event} for i in range(18) for event in ['before_forward','after_forward']],'all eighteen forwards once');require(sha(out/'model-capture.json')==record['metadata_sha256'],'actual instance has exact admitted metadata');data=(out/'values.f64').read_bytes();require(len(data)==18*162*8 and all(math.isfinite(x) for x in struct.unpack('<2916d',data)),'all 2916 raw SDK observations finite');record.update(passed=True,forward_calls_started=18,forward_calls_completed=18,raw_shape=[18,162],raw_words=2916,values=identity(out/'values.f64'),progress=identity(out/'progress.jsonl'),native=report)
    except BaseException as error:record['failure']=repr(error);raise
    finally:
        if (out/'progress.jsonl').exists():
            events=[json.loads(x) for x in (out/'progress.jsonl').read_text().splitlines()];record['forward_calls_started']=sum(x['event']=='before_forward' for x in events);record['forward_calls_completed']=sum(x['event']=='after_forward' for x in events)
        save()
    print(json.dumps({'passed':True,'record':str(out/'record.json'),'values':record['values']}))
if __name__=='__main__':main()
