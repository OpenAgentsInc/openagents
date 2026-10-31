#!/usr/bin/env python3
"""Opt-in NEW SDK capture: frozen q/v/a, M+bias/bodyRNE and matched PcSample.
Only installed binary C SDK calls; no imported LIBERO/robosuite/NVIDIA code.
"""
from pathlib import Path
import argparse,hashlib,json,math,os,platform,struct,subprocess,sys,time
ROOT=Path(__file__).resolve().parents[3]
FIXTURE=ROOT/'psionic/fixtures/robotics/panda-inverse-dynamics'
def require(ok,message):
    if not ok:raise RuntimeError(message)
def sha(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()
def words(path):
    data=Path(path).read_bytes();require(len(data)%8==0,'F64 extent');return struct.unpack('<'+'d'*(len(data)//8),data)
def finite_rows(values,count,width,label):
    require(len(values)==count*width and all(math.isfinite(x) for x in values),label+' complete finite array');return [values[i:i+width] for i in range(0,len(values),width)]
def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--output',type=Path,required=True);a=p.parse_args();out=a.output.resolve();require(sys.platform=='darwin' and platform.machine()=='arm64' and sys.byteorder=='little','qualified Darwin-arm64 LE SDK profile');require(not out.exists(),'preserve existing capture');out.mkdir(parents=True)
    profile=json.loads((FIXTURE/'sdk-profile.json').read_text());addendum=json.loads((FIXTURE/'sdk-pc-sample-profile.json').read_text());experiment=json.loads((FIXTURE/'experiment.json').read_text());collector=ROOT/'psionic/tests/robotics/panda_inverse_dynamics_sdk_collect.c'
    require(profile['frozen_before_capture'] and addendum['frozen_before_capture'] and profile['SDK_case_indices']==list(range(15)) and profile['SDK_contexts']==15 and profile['M_bias_shape']==[15,90] and profile['body_RNE_shape']==[15,9] and addendum['shape']==[15,144],'frozen complete capture contract')
    require(sha(FIXTURE/'experiment.json')==profile['experiment_sha256']=='6b255a86a733e7a1d2f797691cec0b3fa3954b3b0d61e5a70de51a4691a81019' and sha(FIXTURE/'cases.f64')==profile['input24_sha256']==experiment['case_f64_sha256'],'exact frozen24 case experiment')
    require(sha(FIXTURE/'sdk-profile.json')==addendum['profile_sha256'] and sha(collector)==profile['collector_sha256'],'frozen addendum/collector source')
    for name,h in profile['source_profiles'].items():require(sha(ROOT/name)==h,'source JSON identity '+name)
    for name,h in addendum['source_contract'].items():require(sha(ROOT/name)==h,'PcSample source contract '+name)
    raw=(FIXTURE/'cases.f64').read_bytes();require(len(raw)==24*27*8 and raw==b''.join(struct.pack('<27d',*r['q'],*r['v'],*r['a']) for r in experiment['cases']),'complete JSON/binary q/v/a binding');inputs=finite_rows(words(FIXTURE/'cases.f64'),24,27,'case input');require([r['name'] for r in experiment['cases'][:15]]==profile['SDK_case_names'],'exact case IDs/order')
    cache=Path.home()/'.cache/psionic';scene_dir=cache/'libero114/f13aa24a3da8c43c7225569f28c562979fa0e35a/evaluation/demo_000';original=scene_dir/'original.xml';scene=scene_dir/'scene.xml';require(sha(original)==profile['source_original_xml_sha256'] and sha(scene)==profile['source_rewritten_xml_sha256'],'pinned complete original/rewritten model data')
    # The existing rewritten XML is data only, with no include/plugin loader.
    import xml.etree.ElementTree as ET
    xml=ET.fromstring(scene.read_bytes());require(not list(xml.iter('include')) and not list(xml.iter('plugin')),'no external include/plugin execution')
    libraries=list((cache/'groot-reference-env/lib').glob('python*/site-packages/mujoco/libmujoco.3.3.1.dylib'));require(len(libraries)==1,'one installed authorized SDK');library=libraries[0];sdk=library.parent;require(sha(library)=='f8c9735ed948d6830d1d3d31d50216ec14ca6a6e95c7346a4dc8300e1b8ffed4','pinned SDK binary')
    (out/'sdk-input.bin').write_bytes(struct.pack('<II',0x31444950,15)+raw[:15*27*8]);record={'schema':'psionic.panda-inverse-dynamics.sdk-capture.v1','passed':False,'profile_sha256':sha(FIXTURE/'sdk-profile.json'),'pc_sample_profile_sha256':sha(FIXTURE/'sdk-pc-sample-profile.json'),'experiment_sha256':sha(FIXTURE/'experiment.json'),'collector_sha256':sha(collector),'driver_sha256':sha(Path(__file__)),'input_sha256':sha(out/'sdk-input.bin'),'sdk_version':'3.3.1','sdk_library_sha256':sha(library),'SDK_headers':{name:sha(sdk/'include/mujoco'/name) for name in ('mujoco.h','mjmodel.h','mjdata.h')},'platform':platform.platform(),'commands':[],'calls':profile['calls'],'source_original_xml_sha256':sha(original),'source_rewritten_xml_sha256':sha(scene),'case_count':15,'zero_velocity_contexts':3,'nonzero_velocity_contexts':12,'scope':profile['excluded']}
    def save():(out/'record.json').write_text(json.dumps(record,indent=2)+'\n')
    def command(name,argv,env=None):
        began=time.monotonic();r=subprocess.run([str(x) for x in argv],cwd=ROOT,capture_output=True,env=env,timeout=120);(out/(name+'.stdout')).write_bytes(r.stdout);(out/(name+'.stderr')).write_bytes(r.stderr);record['commands'].append({'name':name,'argv':[str(x) for x in argv],'exit':r.returncode,'seconds':time.monotonic()-began,'stderr_bytes':len(r.stderr),'stdout_sha256':hashlib.sha256(r.stdout).hexdigest(),'stderr_sha256':hashlib.sha256(r.stderr).hexdigest()});save();require(r.returncode==0 and not r.stderr,(name,r.stdout.decode(errors='replace'),r.stderr.decode(errors='replace')))
    began=time.monotonic();save()
    try:
        binary=out/'collector';command('compile',[os.environ.get('CC','cc'),'-std=c11','-O1','-g','-fsanitize=address,undefined','-fno-sanitize-recover=all','-fno-omit-frame-pointer','-fno-fast-math','-ffp-contract=off','-I'+str(sdk/'include'),collector,library,'-o',binary]);env=dict(os.environ);env['DYLD_LIBRARY_PATH']=str(sdk)
        command('capture',[binary,scene,out/'sdk-input.bin',out/'sdk-mass-bias.f64',out/'sdk-rne-body.f64',out/'sdk-model.json',out/'sdk-pc-samples.f64'],env)
        model=json.loads((out/'sdk-model.json').read_text());old_model_path=ROOT/profile['source_model_fixture']['path'];require(sha(old_model_path)==profile['source_model_fixture']['sha256'],'prior compiled source facts');old=json.loads(old_model_path.read_text())
        require(model['bodies']==old['bodies'] and model['robot_joints']==old['robot_joints'] and model['site']==old['site'] and model['model']==old['model'] and model['qpos0']==old['qpos0'] and model['gravity']==old['gravity'],'same exact compiled16body/9joint model, not subtree inference')
        require(model['context']['ntendon']==0 and model['gravity']==[0,0,-9.81],'gravity/tendon force convention');require([x['name'] for x in model['context']['arm_actuators']]==['robot0_torq_j'+str(i+1) for i in range(7)],'PcSample named arm actuators')
        mb=finite_rows(words(out/'sdk-mass-bias.f64'),15,90,'SDK M/bias');rne=finite_rows(words(out/'sdk-rne-body.f64'),15,9,'SDK raw body RNE');pc=finite_rows(words(out/'sdk-pc-samples.f64'),15,144,'SDK whole PcSample');force=[];consistency=[]
        for case,m,r,s in zip(inputs[:15],mb,rne,pc):
            q,v,a=case[:9],case[9:18],case[18:];expected=[math.fsum(m[i*9+j]*a[j] for j in range(9))+m[81+i] for i in range(9)];corrected=[r[i]+model['robot_joints'][i]['armature']*a[i] for i in range(9)];require(all(math.isfinite(x) for x in expected+corrected),'finite inverse-force construction');force.extend(expected);consistency.extend(abs(x-y) for x,y in zip(expected,corrected))
            require(s[12:19]==q[:7] and s[19:26]==v[:7] and s[123:130]==m[81:88],'same-input fresh PcSample q/v/bias identity');require(s[26:75]==tuple(m[i*9+j] for i in range(7) for j in range(7)),'same-context PcSample M7 identity')
            require(s[130:137]==tuple(x['ctrlrange'][0] for x in model['context']['arm_actuators']) and s[137:144]==tuple(x['ctrlrange'][1] for x in model['context']['arm_actuators']),'same named actuator limits')
        worst=max(consistency);require(worst<=profile['SDK_internal_RNE_plus_armature_atol'],'frozen internal SDK RNE+armature consistency');(out/'sdk-force.f64').write_bytes(struct.pack('<135d',*force));prior=ROOT/profile['previously_observed_subfields']['prior_fixture'];require(sha(prior)==profile['previously_observed_subfields']['prior_fixture_sha256'],'earlier M9 reference identity');require((out/'sdk-mass-bias.f64').read_bytes()[5*90*8:6*90*8]==prior.read_bytes()[4*90*8:5*90*8],'previously observed M/bias subfields exact; no new qualification')
        record['same_input_RNE_plus_armature']={'values':135,'max_abs_error':worst,'atol':profile['SDK_internal_RNE_plus_armature_atol'],'rtol':0,'passed':True};record['fresh_pc_sample_words']=2160;record['model_source_facts_exact']=True;record['previously_observed_case5_M_bias_byte_alias']=True;record['artifacts']={name:{'sha256':sha(out/name),'bytes':(out/name).stat().st_size} for name in ('sdk-mass-bias.f64','sdk-rne-body.f64','sdk-model.json','sdk-pc-samples.f64','sdk-force.f64')};require(sha(collector)==profile['collector_sha256'] and sha(Path(__file__))==record['driver_sha256'],'capture source changed');record['passed']=True
    except BaseException as e:record['failure']=repr(e);raise
    finally:record['seconds']=time.monotonic()-began;save()
    print(json.dumps({'passed':True,'new_forward_contexts':15,'physics_steps':0,'record':str(out/'record.json'),'seconds':record['seconds']}))
if __name__=='__main__':main()
