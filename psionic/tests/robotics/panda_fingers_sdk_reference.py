#!/usr/bin/env python3
"""NEW five-case 16body forward-only SDK data, with separate future mass/bias facts.
Only the installed MuJoCo C SDK executes; no third-party Python/repo imports.
"""
from pathlib import Path
import argparse,hashlib,json,math,os,platform,struct,subprocess,time,xml.etree.ElementTree as ET
ROOT=Path(__file__).resolve().parents[3]
FIXTURE=ROOT/'psionic/fixtures/robotics/panda-fingers'
COLLECTOR=ROOT/'psionic/tests/robotics/panda_fingers_sdk_collect.c'
CACHE=Path.home()/'.cache/psionic/libero114/f13aa24a3da8c43c7225569f28c562979fa0e35a/evaluation/demo_000'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def finite_tree(value):
    if isinstance(value,dict):return all(finite_tree(v) for v in value.values())
    if isinstance(value,list):return all(finite_tree(v) for v in value)
    return not isinstance(value,float) or math.isfinite(value)
def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('output',type=Path);p.add_argument('--scene',type=Path,default=CACHE/'scene.xml');p.add_argument('--original',type=Path,default=CACHE/'original.xml');p.add_argument('--sdk',type=Path);a=p.parse_args();assert not a.output.exists(),'refuse existing/partial output';assert os.sys.byteorder=='little'
    profile=json.loads((FIXTURE/'sdk-profile.json').read_text());source=json.loads((FIXTURE/'source-profile.json').read_text());assert profile['frozen_before_capture'] and profile['case_shape']==[5,9] and profile['pose_shape']==[5,268] and profile['absolute_tolerance']==2e-12 and profile['relative_tolerance']==0
    assert profile['arm_radians']==source['new_sdk_frozen_arm_radians'] and profile['finger_cases_metres']==source['new_sdk_frozen_slide_pairs_metres'];assert profile['body_names'][12:]==[n['name'] for n in source['nodes']]
    assert sha(a.original)==profile['source_original_xml_sha256']==source['source']['original_xml_sha256'];assert sha(a.scene)==profile['source_rewritten_xml_sha256']==source['source']['rewritten_xml_sha256']
    for path in (a.original,a.scene):
        tree=ET.parse(path);assert not any(n.tag in ('include','plugin','extension') for n in tree.iter()),'unexpected XML include/plugin/extension'
    if a.sdk:sdk=a.sdk.resolve()
    else:
        matches=list((Path.home()/'.cache/psionic/groot-reference-env/lib').glob('python*/site-packages/mujoco'));assert len(matches)==1;sdk=matches[0]
    header=sdk/'include/mujoco/mujoco.h';library=sdk/'libmujoco.3.3.1.dylib';assert header.is_file() and library.is_file()
    out=a.output.resolve();out.mkdir(parents=True);cases=[v for row in profile['cases_radians_then_metres'] for v in row];assert len(cases)==45 and all(math.isfinite(v) and abs(v)<=4 for v in cases);(out/'cases.f64').write_bytes(struct.pack('<45d',*cases))
    record={'schema':'psionic.panda-fingers.sdk-reference.v1','passed':False,'sdk_version':'3.3.1','platform':platform.platform(),'machine':platform.machine(),'source_original_xml_sha256':sha(a.original),'source_rewritten_xml_sha256':sha(a.scene),'frozen_source_profile_sha256':sha(FIXTURE/'source-profile.json'),'frozen_sdk_profile_sha256':sha(FIXTURE/'sdk-profile.json'),'collector_sha256':sha(COLLECTOR),'driver_sha256':sha(__file__),'sdk_header_sha256':sha(header),'sdk_library_sha256':sha(library),'sdk_public_header_inventory':{str(q.relative_to(sdk)):sha(q) for q in sorted((sdk/'include/mujoco').glob('*.h'))},'case_shape':[5,9],'pose_shape':[5,268],'absolute_tolerance_frozen_before_capture':2e-12,'relative_tolerance':0,'body_names':profile['body_names'],'joint_names':profile['joint_names'],'joint_units':profile['joint_units'],'site_name':profile['site_name'],'quaternion_policy':profile['quaternion_policy'],'commands':[],'new_collector_is_sanitized':True,'SDK_runtime_uninstrumented':True,'scope':profile['scope']}
    def save():(out/'record.json').write_text(json.dumps(record,indent=2)+'\n')
    def command(name,args):
        env=os.environ.copy();overrides={}
        if name=='collect':overrides={'DYLD_LIBRARY_PATH':str(sdk)};env.update(overrides)
        begin=time.monotonic();r=subprocess.run([str(v) for v in args],cwd=ROOT,capture_output=True,env=env,timeout=180);(out/(name+'.stdout')).write_bytes(r.stdout);(out/(name+'.stderr')).write_bytes(r.stderr);record['commands'].append({'name':name,'arguments':[str(v) for v in args],'environment_overrides':overrides,'exit':r.returncode,'duration_seconds':time.monotonic()-begin,'stdout_sha256':hashlib.sha256(r.stdout).hexdigest(),'stderr_sha256':hashlib.sha256(r.stderr).hexdigest(),'stderr_bytes':len(r.stderr)});save();assert r.returncode==0,(name,r.stdout.decode(),r.stderr.decode());assert not r.stderr,(name,r.stderr.decode());return r
    begin=time.monotonic();cc=os.environ.get('CC','cc')
    try:
        command('compile',[cc,'-std=c11','-O1','-g','-Wall','-Wextra','-Werror','-Wno-misleading-indentation','-fsanitize=address,undefined','-fno-sanitize-recover=all','-fno-omit-frame-pointer','-I'+str(sdk/'include'),COLLECTOR,library,'-Wl,-rpath,'+str(sdk),'-lm','-o',out/'collect'])
        command('collect',[out/'collect',a.scene,out/'cases.f64',out/'poses.f64',out/'model.json',out/'dynamics.f64'])
        poses=(out/'poses.f64').read_bytes();dynamic=(out/'dynamics.f64').read_bytes();assert len(poses)==5*268*8 and all(math.isfinite(v) for v in struct.unpack('<1340d',poses));assert len(dynamic)==5*90*8 and all(math.isfinite(v) for v in struct.unpack('<450d',dynamic))
        model=json.loads((out/'model.json').read_text());assert finite_tree(model) and model['sdk_version_number']==331 and model['model']['nplugin']==0;assert [b['name'] for b in model['bodies']]==profile['body_names'];assert [j['name'] for j in model['robot_joints']]==profile['joint_names'];assert model['site']['name']==profile['site_name']
        for i,j in enumerate(model['robot_joints']):assert j['type_name']==('hinge' if i<7 else 'slide') and j['type']==(3 if i<7 else 2) and j['qpos_adr']>=0 and j['dof_adr']>=0
        record.update(passed=True,forward_calls=5,integration_steps=0,mass_expand_calls=5,pose_numeric_words=1340,case_sha256=sha(out/'cases.f64'),pose_sha256=sha(out/'poses.f64'),compiled_model_metadata_sha256=sha(out/'model.json'),collector_binary_sha256=sha(out/'collect'),no_rendering_controller_NN_or_old_gates=True)
        dynamics={'schema':'psionic.panda-fingers.sdk-dynamics-reference.v1','captured':True,'qualified_owned_dynamics':False,'case_count':5,'shape':[5,90],'mass_shape_per_case':[9,9],'bias_words_per_case':9,'case_layout':'81rowmajor namedDOF mass entries,then9qfrc_bias entries','joint_names':profile['joint_names'],'input_units':profile['joint_units'],'case_sha256':record['case_sha256'],'dynamics_sha256':sha(out/'dynamics.f64'),'bytes':len(dynamic),'qvel':'All model qvel zero by reset, asserted after forward. This is not an equilibrium claim.','gravity':model['gravity'],'forward_calls_shared_with_pose_capture':5,'additional_forward_calls':0,'integration_steps':0,'mass_expand_calls':5,'model_metadata_sha256':record['compiled_model_metadata_sha256'],'sdk_library_sha256':record['sdk_library_sha256'],'scope':'Raw SDK facts only for future model-only mass/bias work. No ownedmass/gravity/bias implementation or allmixed-coordinate unit model qualified.'};(out/'dynamics-reference.json').write_text(json.dumps(dynamics,indent=2)+'\n')
    except BaseException as e:record['failure']=repr(e);raise
    finally:record['duration_seconds']=time.monotonic()-begin;save()
    print(json.dumps({'passed':True,'record':str(out/'record.json'),'case_sha256':record['case_sha256'],'pose_sha256':record['pose_sha256'],'model_sha256':record['compiled_model_metadata_sha256'],'dynamics_sha256':dynamics['dynamics_sha256'],'seconds':record['duration_seconds']}))
if __name__=='__main__':main()
