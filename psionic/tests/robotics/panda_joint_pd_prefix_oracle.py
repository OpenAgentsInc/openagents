#!/usr/bin/env python3
"""Independently audit controller fields on ONE new SDK-owned trajectory.

No Coax invocation, SDK load/call, integration, rendering, neural execution or
old44-query gate. Only authored pure matrix/potential/scalar definitions import.
"""
from pathlib import Path
import argparse,hashlib,json,math,struct,time
from panda_joint_pd_reference import potential_gradient,scalar,KEYS,FIELDS
ROOT=Path(__file__).resolve().parents[3]
FIXTURE=ROOT/'psionic/fixtures/robotics/panda-joint-pd-prefix'
EXPERIMENT_SHA='23a533517be639cf4b428683c106b9ce64d26ee75ff88564c50849346beb8d6f'
ROW=struct.Struct('<54dq')
HEADER=struct.Struct('<II')
def require(ok,message):
    if not ok:raise RuntimeError(message)
def sha(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()
def finite(values,count,label):
    require(len(values)==count and all(math.isfinite(v)for v in values),label+' ALL finite and complete');return list(values)
def decode(raw):
    require(len(raw)==8+250*ROW.size,'one exact250×440-byte trace');require(HEADER.unpack_from(raw)==(0x31525050,250),'PPR1/frozen250 header');rows=[]
    for i in range(250):
        row=ROW.unpack_from(raw,8+i*ROW.size);finite(row[:54],54,'all actual poststate/prePD/time fields');require(0<=row[54]<=7,'PJP clipped count domain');rows.append(row)
    return rows
def decoder_guards():
    good=HEADER.pack(0x31525050,250)+ROW.pack(*([0.]*54),0)*250;decode(good);bad=[good[:-1],good+ROW.pack(*([0.]*54),0)]
    for value in [float('nan'),float('inf'),-float('inf')]:
        b=bytearray(good);struct.pack_into('<d',b,8+249*ROW.size+52*8,value);bad.append(bytes(b))
    for value in [-1,8]:
        b=bytearray(good);struct.pack_into('<q',b,len(b)-8,value);bad.append(bytes(b))
    for magic,count in [(0,250),(0x31525050,249),(0x31525050,251)]:
        b=bytearray(good);HEADER.pack_into(b,0,magic,count);bad.append(bytes(b))
    for raw in bad:
        try:decode(raw)
        except RuntimeError:pass
        else:raise RuntimeError('decoder admitted late IEEE/count/header/extent poison')
    return dict(passed=True,refused_cases=len(bad),SDK_calls=0)
def compare(actual,expected,budget,label):
    finite(actual,len(expected),label+' actual');finite(expected,len(actual),label+' expected');require(actual,label+' nonempty');error=max(abs(a-b)for a,b in zip(actual,expected));require(error<=budget,f'{label}: {error:.17g} exceeds frozen {budget:g}');return dict(passed=True,values=len(actual),max_abs_error=error,atol=budget,rtol=0)
def initial_and_model():
    require(sha(FIXTURE/'experiment.json')==EXPERIMENT_SHA,'frozen new250-step experiment');e=json.loads((FIXTURE/'experiment.json').read_text());require(e['frozen_before_new_trajectory_or_SDK_calls'] and e['steps']==250 and e['step_seconds']==.002 and e['budgets']['independent_PD_fields_J_per_rad']==5e-10 and e['budgets']['rtol']==0,'frozen work/budget');require(e['SDK']['new_steps']==250 and e['SDK']['explicit_initial_forwards']==1 and e['SDK']['post_step_forwards']==e['SDK']['NN']==e['SDK']['render']==0 and e['SDK']['disableflags']==17,'SDK constraints-off prefix scope')
    raw=(FIXTURE/'initial.f64').read_bytes();require(len(raw)==48*8 and sha(FIXTURE/'initial.f64')==e['initial_sha256'],'all48 initial values identity');initial=finite(struct.unpack('<48d',raw),48,'all initial values');require(raw==struct.pack('<48d',*[x for key in ['q','v','target','target_v','kp','kd','finger_targets']for x in e['initial'][key]]),'initial JSON/raw bits exact');require(e['initial']['external9']==[0.]*9,'zero generalized external forces')
    query=ROOT/'psionic/fixtures/robotics/panda-joint-pd/native-qualification.json';q=json.loads(query.read_text());require(q['passed'],'qualified pure query definition reference');required=['psionic/tests/robotics/panda_joint_pd_reference.py','psionic/tests/robotics/panda_inverse_dynamics_matrix.py','psionic/fixtures/robotics/panda-fk/source-profile.json','psionic/fixtures/robotics/panda-fingers/source-profile.json','psionic/fixtures/robotics/panda-fingers/sdk-model.json','psionic/fixtures/robotics/panda-fingers/sdk-profile.json','psionic/fixtures/robotics/panda-fingers/sdk-dynamics.json','psionic/fixtures/robotics/panda-fk/sdk-model.json'];sources={ROOT/name:q['source_sha256'][name]for name in required};require(all(sha(path)==digest for path,digest in sources.items()),'exact previously qualified pure function/data definitions; no old gate')
    fingers=ROOT/'psionic/fixtures/robotics/panda-fingers';arm=ROOT/'psionic/fixtures/robotics/panda-fk';model=json.loads((fingers/'sdk-model.json').read_text());branch=json.loads((fingers/'source-profile.json').read_text());require(branch['arm_profile']['path']=='psionic/fixtures/robotics/panda-fk/source-profile.json' and branch['arm_profile']['sha256']==sha(arm/'source-profile.json'),'arm/branch provenance');nodes=json.loads((arm/'source-profile.json').read_text())['nodes']+branch['nodes'];require(len(nodes)==16 and [n['name']for n in nodes]==[b['name']for b in model['bodies']],'all16 compiled physical bodies');require(model['gravity']==[0,0,-9.81] and all(j['qpos0_reference']==0 for j in model['robot_joints']),'source gravity/reference coordinates')
    original=json.loads((fingers/'sdk-dynamics.json').read_text());require(original['source_rewritten_xml_sha256']==e['SDK']['source_XML_sha256'] and original['sdk_library_sha256']==e['SDK']['library_sha256'] and original['model_metadata_sha256']==sha(fingers/'sdk-model.json') and original['pose_profile_sha256']==sha(fingers/'sdk-profile.json'),'same fixed robot and installed SDK association');limits=[j['range']for j in json.loads((arm/'sdk-model.json').read_text())['arm_joints']];sources.update({query:sha(query),FIXTURE/'initial.f64':sha(FIXTURE/'initial.f64'),FIXTURE/'experiment.json':EXPERIMENT_SHA,Path(__file__).resolve():sha(__file__)})
    return e,initial,model,nodes,limits,sources
def admitted_prestate(q,v,initial,limits):
    finite(q+v,18,'complete reconstructed prestate');require(all(lo<=q[i]<=hi for i,(lo,hi)in enumerate(limits)) and all(abs(x)<=1 for x in q[7:]) and all(abs(x)<=20 for x in v[:7]) and all(abs(x)<=2 for x in v[7:]),'new controller prestate within fixed source/numerical boxes');require(all(lo<=initial['target'][i]<=hi for i,(lo,hi)in enumerate(limits)) and all(abs(x)<=20 for x in initial['target_v']),'target source/rate bounds');require(all(1e-6<=x<=500 for x in initial['kp']) and all(1e-6<=x<=100 for x in initial['kd']),'positive frozen gain boxes')
def bind_capture(path,trace,input_path,e):
    capture=json.loads(path.read_text());require(capture['schema']=='psionic.panda-joint-pd-prefix.sdk-reference.v1' and capture['passed'] and capture['prepared'] and capture['capture_started'],'one completed admitted SDK prefix');require(capture['experiment_sha256']==EXPERIMENT_SHA,'same frozen prefix experiment');require(capture['trace']['bytes']==trace.stat().st_size and capture['trace']['sha256']==sha(trace),'exact captured SDK trace identity');require(capture['input']['bytes']==input_path.stat().st_size and capture['input']['sha256']==sha(input_path),'actual SDK input identity');require(input_path.read_bytes()==HEADER.pack(0x31495050,48)+(FIXTURE/'initial.f64').read_bytes(),'actual SDK initial state/target/gains bit-identical')
    require(capture['source_XML']['sha256']==e['SDK']['source_XML_sha256'] and capture['SDK_library']['sha256']==e['SDK']['library_sha256'],'same qualified source scene and installed SDK');require(capture['steps_started']==capture['steps_completed']==capture['committed_records']==250 and capture['explicit_forwards_started']==capture['explicit_forwards_completed']==1,'one complete250-step/one-initial-forward capture');report=capture['native'];require(report['passed'] and report['steps']==250 and report['explicit_forwards']==1 and report['post_step_forwards']==report['NN_calls']==report['render_calls']==0,'exact new SDK scope');require(capture['support_admission']['passed'] and capture['support_admission']['modified_disableflags']==17 and capture['support_admission']['EULERDAMP_enabled'] is True and capture['serialized_status_fields']==0,'same source implicit-damping/constraints-off profile and honest record scope')
    required={'psionic/tests/robotics/panda_joint_pd_prefix_sdk_collect.c','psionic/tests/robotics/panda_joint_pd_prefix_format.h','psionic/tests/robotics/panda-joint-pd-prefix.coax','psionic/fixtures/robotics/panda-joint-pd-prefix/experiment.json','psionic/fixtures/robotics/panda-joint-pd-prefix/initial.f64'};require(required.issubset(capture['source']),'explicit prefix source/data inventory');sources={path:sha(path),trace:sha(trace),input_path:sha(input_path)}
    for name,item in capture['source'].items():
        p=ROOT/name;require(p.stat().st_size==item['bytes'] and sha(p)==item['sha256'],'capture source identity '+name);sources[p]=item['sha256']
    return capture,sources
def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--trace',type=Path,required=True);parser.add_argument('--capture-record',type=Path,required=True);parser.add_argument('--input',type=Path,required=True);parser.add_argument('--output',type=Path,required=True);args=parser.parse_args();out=args.output.resolve();require(not out.exists(),'preserve earlier/failed independent trajectory audit');out.mkdir(parents=True);record=dict(schema='psionic.panda-joint-pd-prefix.independent-controller.v1',passed=False,SDK_calls=0,Coax_calls=0,old44_query_gate_repeated=False);began=time.monotonic()
    def save():(out/'record.json').write_text(json.dumps(record,indent=2)+'\n')
    save()
    try:
        e,initial_words,model,nodes,limits,sources=initial_and_model();capture,more=bind_capture(args.capture_record.resolve(),args.trace.resolve(),args.input.resolve(),e);sources.update(more);raw=args.trace.read_bytes();rows=decode(raw);record['decoder_guards']=decoder_guards();record['source_sha256']={str(p.relative_to(ROOT))if p.is_relative_to(ROOT)else str(p):h for p,h in sorted(sources.items())};record.update(experiment_sha256=EXPERIMENT_SHA,trace_sha256=sha(args.trace),captured_trace_bytes=len(raw),records=250,all13500_actual_F64_words_finite=True,actual_clipping_counts=250,recorded_status_scope='No serialized status field; completed records follow checked successful calls. Only PJP clipping counter is serialized.');initial=e['initial'];q=list(initial['q']);v=list(initial['v']);expected=[];times=[];time_value=0.
        for i,row in enumerate(rows):
            admitted_prestate(q,v,initial,limits);gravity=potential_gradient(nodes,model,q);case={KEYS[0]:q[:7],KEYS[1]:q[7:],KEYS[2]:v[:7],KEYS[3]:initial['target'],KEYS[4]:initial['target_v'],KEYS[5]:initial['kp'],KEYS[6]:initial['kd']};wanted=scalar(case,gravity);finite([x for key in FIELDS for x in wanted[key]],35,'complete independent trajectory target');expected.append(wanted);time_value+=e['step_seconds'];times.append(time_value);q=list(row[:9]);v=list(row[9:18])
        require(len(expected)==250 and all(len(w[key])==7 and all(math.isfinite(x)for x in w[key])for w in expected for key in FIELDS),'all8750 expected force fields finite before maxima');record['all8750_expected_fields_finite']=True;record['metrics']={}
        for k,key in enumerate(FIELDS):record['metrics'][key]=compare([row[18+7*k+j]for row in rows for j in range(7)],[wanted[key][j]for wanted in expected for j in range(7)],5e-10,'new250 SDK trajectory '+key)
        require(all(row[54]==wanted['clipped']for row,wanted in zip(rows,expected)),'all250 independent clipping counts exact');record['clipping_counts_exact']=True;record['time']=compare([row[53]for row in rows],times,e['budgets']['SDK_time_s'],'one-add fixed-step source times');record['PJP_clip_histogram']={str(n):sum(row[54]==n for row in rows)for n in range(8)};target=b''.join(struct.pack('<35dq',*[x for key in FIELDS for x in wanted[key]],wanted['clipped'])for wanted in expected);(out/'expected-controller.bin').write_bytes(target);record.update(expected_controller_sha256=sha(out/'expected-controller.bin'),expected_shape='250×(35F64,int64clipped)',prestate_alignment='First=frozen initialq9/v9; later=previous SDK poststep q9/v9, never current poststate or stale geometry',oracle='analytic full-angle matrix COM-potential derivative + literal scalar PD; no own quaternion/cross-product implementation',SDK_trajectory_replayed=False)
        require(all(sha(p)==h for p,h in sources.items()),'all input/source/reference identities frozen at completion');record['passed']=True
    except BaseException as error:record['failure']=repr(error);raise
    finally:record['seconds']=time.monotonic()-began;save()
    print(json.dumps(dict(passed=True,record=str(out/'record.json'),metrics=record['metrics'],clipping_counts_exact=True)))
if __name__=='__main__':main()
