#!/usr/bin/env python3
"""New clock-glue oracle: saved state/control bytes plus SDK forward-only poses."""
import argparse,hashlib,json,math,pathlib,struct
def identity(p):
    b=pathlib.Path(p).read_bytes();return {'bytes':len(b),'sha256':hashlib.sha256(b).hexdigest()}
def authenticate(xml,initial,trace,scenario):
    root=pathlib.Path(__file__).resolve().parents[2]
    profile=json.loads((root/'fixtures/robotics/libero-panda/panda-clock-prefix-profile.json').read_text())
    case=next(x for x in profile['cases'] if x['scenario']==scenario)
    assert identity(xml)==case['files']['scene.xml'] and identity(initial)==case['files']['init.f64']
    assert identity(trace)==case['recorded_trace']
    assert identity(trace.parent/'record.json')==case['original_case_ledger']
    return case
def reference(xml,initial,trace,prefix,scenario,record):
    import numpy as np
    import mujoco as mj
    case=authenticate(xml,initial,trace,scenario)
    assert mj.__version__=='3.3.1'
    raw=trace.read_bytes();init=initial.read_bytes();frames=(len(raw)-64)//512
    expected_states=init+b''.join(raw[32+i*512+148:32+i*512+508] for i in range(250))
    assert prefix.with_suffix('.states.f64').read_bytes()==expected_states
    assert prefix.with_suffix('.records.ptr1').read_bytes()==raw[32:32+250*512]
    summary=json.loads(prefix.with_suffix('.json').read_text())
    assert summary['schema']=='psionic-panda-clock-prefix-v1' and summary['steps']==250 and summary['rows']==10
    assert summary['origins']==case['expected_requests'] and summary['requests']==len(case['expected_requests'])
    assert summary['holds']==int(scenario==2) and summary['refusals']==int(scenario==2)
    assert not summary['neural_model_initialized'] and not summary['controller_arithmetic_recomputed'] and not summary['task_outcome_evaluated']
    commands=np.fromfile(prefix.with_suffix('.recorded-commands.u32'),dtype='<u4').reshape(summary['requests'],16,7)
    for request,origin in enumerate(case['expected_requests']):
        for slot in range(16):
            at=(origin//25+slot)*25
            expected=np.frombuffer(raw[32+at*512+48:32+at*512+76],dtype='<u4') if slot<8 and at<frames else np.zeros(7,dtype='<u4')
            assert np.array_equal(commands[request,slot],expected)
    def state(k):return np.frombuffer(init if k==0 else raw[32+(k-1)*512+148:32+(k-1)*512+508],dtype='<f8')
    model=mj.MjModel.from_xml_path(str(xml));data=mj.MjData(model)
    assert model.nq==23 and model.nv==21 and model.na==0 and model.opt.timestep==.002 and int(model.opt.integrator)==0
    site=mj.mj_name2id(model,mj.mjtObj.mjOBJ_SITE,'gripper0_grip_site');body=mj.mj_name2id(model,mj.mjtObj.mjOBJ_BODY,'robot0_right_hand')
    fingers=[int(model.jnt_qposadr[mj.mj_name2id(model,mj.mjtObj.mjOBJ_JOINT,'gripper0_finger_joint'+str(i))]) for i in [1,2]]
    def forward(k):
        x=state(k);mj.mj_resetData(model,data);data.time=float(x[0]);data.qpos[:]=x[1:24];data.qvel[:]=x[24:45];mj.mj_forward(model,data)
    snapshots=prefix.with_suffix('.snapshots.bin').read_bytes();assert len(snapshots)==(summary['requests']+10)*112
    actual_obs=prefix.with_suffix('.observations.f32').read_bytes();assert len(actual_obs)==summary['requests']*32
    cached=[];fresh=[];max_error=0.;exact=0;obs_index=0
    for at in range(0,len(snapshots),112):
        kind,k,last,qpos=struct.unpack_from('<4q',snapshots,at);actual=np.array(struct.unpack_from('<10d',snapshots,at+32))
        assert kind in [0,1] and k%25==0 and qpos==k
        generation=max(0,k-1) if kind==0 else k;assert last==generation
        forward(generation);current=state(k)
        expected=np.concatenate(([current[0]],data.site_xpos[site],data.xquat[body],current[1+np.array(fingers)]))
        err=float(np.max(np.abs(actual-expected)));max_error=max(max_error,err);assert err<=1e-12,(kind,k,err)
        exact+=int(np.count_nonzero(actual.view('<u8')==expected.view('<u8')))
        (cached if kind==0 else fresh).append(k)
        if kind==0:
            w=min(1.,max(-1.,float(expected[4])));den=math.sqrt(1-w*w)
            axis=np.array([((float(v)*2)*math.acos(w))/den if den else 0. for v in expected[5:8]])
            physical=np.concatenate((expected[1:4],axis,expected[8:10])).astype('<f4').tobytes()
            assert actual_obs[obs_index*32:(obs_index+1)*32]==physical,(k,'F32observationbytes')
            obs_index+=1
    assert cached==case['expected_requests'] and fresh==list(range(0,250,25))
    result={'schema':'psionic-panda-clock-prefix-reference-v1','passed':True,'case':case['case'],'scenario':scenario,'states':250,'exact_state_F64words':250*45,'exact_PTR1_records':250,'cached_observations':len(cached),'fresh_observations':len(fresh),'exact_snapshot_F64words':exact,'snapshot_F64words':len(snapshots)//112*10,'snapshot_max_abs_error':max_error,'snapshot_frozen_atol':1e-12,'exact_model_request_F32words':len(cached)*8,'request_origins':cached,'SDK_version':mj.__version__,'reference_uses_forward_only':True,'SDK_physics_or_controller_or_neural_reference_rerun':False,'command_fixture_scope':'Recorded selected command rows with explicitly zero unconsumed test tail; not an authentic later NN reply.','source_scene':identity(xml),'source_reset':identity(initial),'source_trace':identity(trace),'oracle':identity(__file__),'payloads':{s:identity(prefix.with_suffix(s)) for s in ['.states.f64','.records.ptr1','.snapshots.bin','.observations.f32','.recorded-commands.u32','.json']},'scope':'New typed-clock adapter/effect-order SDK prefix. Existing original traces remain unchanged; no task outcome, fresh NN, controller arithmetic, pixel/renderer or full R13/R18 qualification.'}
    record.write_text(json.dumps(result,indent=2,sort_keys=True)+'\n');print(f'Clock prefix reference: {case["case"]} scenario{scenario},11250exactstatewords/250PTR1records; obsmax{max_error:.3g}')
def main():
    p=argparse.ArgumentParser();p.add_argument('xml',type=pathlib.Path);p.add_argument('initial',type=pathlib.Path);p.add_argument('trace',type=pathlib.Path);p.add_argument('scenario',type=int);p.add_argument('prefix',type=pathlib.Path);p.add_argument('--record',type=pathlib.Path);p.add_argument('--authenticate-only',action='store_true');a=p.parse_args()
    if a.authenticate_only:authenticate(a.xml,a.initial,a.trace,a.scenario);print('Frozen scene/reset/trace/original-ledger identities authenticated.');return
    assert a.record is not None;reference(a.xml,a.initial,a.trace,a.prefix,a.scenario,a.record)
if __name__=='__main__':main()
