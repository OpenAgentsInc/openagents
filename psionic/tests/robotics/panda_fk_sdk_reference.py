#!/usr/bin/env python3
"""Opt-in new forward-only all-link reference. Uses the installed C SDK; no package/repo imports."""
from pathlib import Path
import argparse,hashlib,json,math,os,platform,struct,subprocess,time,xml.etree.ElementTree as ET
ROOT=Path(__file__).resolve().parents[3]
PROFILE=ROOT/'psionic/fixtures/robotics/panda-fk/sdk-profile.json'
COLLECTOR=ROOT/'psionic/tests/robotics/panda_fk_sdk_collect.c'
PINNED=Path.home()/'.cache/psionic/libero114/f13aa24a3da8c43c7225569f28c562979fa0e35a/evaluation/demo_000'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('output',type=Path);parser.add_argument('--scene',type=Path,default=PINNED/'scene.xml');parser.add_argument('--original',type=Path,default=PINNED/'original.xml');parser.add_argument('--sdk',type=Path);args=parser.parse_args()
    assert os.sys.byteorder=='little';assert not args.output.exists(),'refuse existing/partial output directory'
    profile=json.loads(PROFILE.read_text());assert profile['frozen_before_capture'] and profile['case_shape']==[7,7] and profile['pose_shape']==[7,204] and profile['absolute_tolerance']==2e-12 and profile['relative_tolerance']==0
    assert sha(args.original)==profile['source_original_xml_sha256'];assert sha(args.scene)==profile['source_rewritten_xml_sha256']
    for p in [args.original,args.scene]:
        tree=ET.parse(p);assert not any(node.tag in ('include','plugin','extension') for node in tree.iter()),'unexpected XML include/plugin/extension'
    if args.sdk:sdk=args.sdk.resolve()
    else:
        matches=list((Path.home()/'.cache/psionic/groot-reference-env/lib').glob('python*/site-packages/mujoco'));assert len(matches)==1; sdk=matches[0]
    header=sdk/'include/mujoco/mujoco.h';library=sdk/'libmujoco.3.3.1.dylib';assert header.is_file() and library.is_file(),'requires the explicit installed 3.3.1 C SDK profile'
    work=args.output.resolve();work.mkdir(parents=True);cases=[value for row in profile['cases_radians'] for value in row];assert len(cases)==49 and all(math.isfinite(x) and abs(x)<=4 for x in cases)
    (work/'cases.f64').write_bytes(struct.pack('<49d',*cases));record={'schema':'psionic.panda-fk.sdk-reference.v1','passed':False,'scope':profile['scope'],'source_original_xml_sha256':sha(args.original),'source_rewritten_xml_sha256':sha(args.scene),'source_profile_sha256':sha(PROFILE),'collector_sha256':sha(COLLECTOR),'driver_sha256':sha(__file__),'sdk_header_sha256':sha(header),'sdk_library_sha256':sha(library),'sdk_version':'3.3.1','platform':platform.platform(),'machine':platform.machine(),'commands':[],'case_shape':[7,7],'pose_shape':[7,204],'absolute_tolerance_frozen_before_capture':2e-12,'relative_tolerance':0,'sdk_runtime_not_instrumented':True,'new_collector_is_sanitized':True,'no_steps_rendering_controller_NN_or_old_gates':True,'body_names':profile['body_names'],'site_name':profile['site_name'],'joint_names':profile['joint_names'],'reset':profile['reset'],'quaternion_policy':profile['quaternion_policy']}
    def save():(work/'record.json').write_text(json.dumps(record,indent=2)+'\n')
    def command(name,argv):
        environment=os.environ.copy();overrides={}
        if name=='collect':overrides['DYLD_LIBRARY_PATH']=str(sdk);environment.update(overrides)
        start=time.monotonic();r=subprocess.run([str(v) for v in argv],cwd=ROOT,capture_output=True,timeout=180,env=environment);(work/(name+'.stdout')).write_bytes(r.stdout);(work/(name+'.stderr')).write_bytes(r.stderr)
        record['commands'].append({'name':name,'arguments':[str(v) for v in argv],'environment_overrides':overrides,'exit':r.returncode,'duration_seconds':time.monotonic()-start,'stdout_sha256':hashlib.sha256(r.stdout).hexdigest(),'stderr_sha256':hashlib.sha256(r.stderr).hexdigest(),'stderr_bytes':len(r.stderr)});save();assert r.returncode==0,(name,r.stdout.decode(errors='replace'),r.stderr.decode(errors='replace'));assert not r.stderr,(name,r.stderr.decode(errors='replace'));return r
    start=time.monotonic();cc=os.environ.get('CC','cc')
    try:
        command('compile',[cc,'-std=c11','-O1','-g','-Wall','-Wextra','-Werror','-Wno-misleading-indentation','-fsanitize=address,undefined','-fno-sanitize-recover=all','-fno-omit-frame-pointer','-I'+str(sdk/'include'),COLLECTOR,library,'-Wl,-rpath,'+str(sdk),'-lm','-o',work/'collect'])
        command('collect',[work/'collect',args.scene,work/'cases.f64',work/'poses.f64',work/'model.json'])
        raw=(work/'poses.f64').read_bytes();assert len(raw)==7*204*8;values=struct.unpack('<1428d',raw);assert all(math.isfinite(v) for v in values)
        model=json.loads((work/'model.json').read_text());assert model['sdk_version_number']==331 and model['model']['nplugin']==0 and len(model['bodies'])==12 and len(model['arm_joints'])==7
        assert [b['name'] for b in model['bodies']]==profile['body_names'];assert [j['name'] for j in model['arm_joints']]==profile['joint_names'];assert model['site']['name']==profile['site_name']
        assert all(j['type_name']=='hinge' and j['type']==3 and j['qpos_adr']>=0 and j['dof_adr']>=0 for j in model['arm_joints'])
        record['case_sha256']=sha(work/'cases.f64');record['pose_sha256']=sha(work/'poses.f64');record['compiled_model_metadata_sha256']=sha(work/'model.json');record['collector_binary_sha256']=sha(work/'collect');record['forward_calls']=7;record['integration_steps']=0;record['numeric_words']=1428;record['all_metadata_finite_and_names_ordered']=True;record['passed']=True
    except BaseException as e:record['failure']=repr(e);raise
    finally:record['duration_seconds']=time.monotonic()-start;save()
    print(json.dumps({'passed':True,'record':str(work/'record.json'),'case_sha256':record['case_sha256'],'pose_sha256':record['pose_sha256'],'model_sha256':record['compiled_model_metadata_sha256'],'duration_seconds':record['duration_seconds']}))
if __name__=='__main__':main()
