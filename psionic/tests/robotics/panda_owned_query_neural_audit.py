#!/usr/bin/env python3
"""Independent stdlib audit of ONLY the new neural/owned-query receipt/PTR1.

No NN, SDK API, renderer, controller, build, old reference or old gate runs.
Native replay is an observed bound-source receipt; this tool audits its new
canonical record/clock/hash chain independently, never repeats physics.
"""
from pathlib import Path
import argparse,hashlib,json,math,struct,time
ROOT=Path(__file__).resolve().parents[3]
F=ROOT/'psionic/fixtures/robotics/panda-owned-query-neural'
EXP='bfdd85bf14ed244f9c0b303c1f742a19086e821a229c207f242bc575836845bc'
PROFILE='panda-libero-mujoco331-gr00t-wide-fixed-noise-owned-query-shadow-v1'
SCHEMA='psionic-panda-owned-query-closed-loop-v1'
HEADER=struct.Struct('<4sHBB5Q');BODY=struct.Struct('<7I54Q')
def require(ok,message):
    if not ok:raise RuntimeError(message)
def sha(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()
def identity(path):p=Path(path);return dict(bytes=p.stat().st_size,sha256=sha(p))
def write(path,value):path.write_text(json.dumps(value,indent=2)+'\n')
def unique(pairs):
    result={}
    for key,value in pairs:require(key not in result,'duplicate JSON key');result[key]=value
    return result
def json_bytes(raw):return json.loads(raw,object_pairs_hook=unique)
def finite(values,n,label):require(len(values)==n and all(math.isfinite(x)for x in values),label+' exact extent/ALLfinite');return values
def integer(x):return type(x)is int
def boolean(x):return type(x)is bool
def digest_text(x):return type(x)is str and len(x)==64 and all(c in'0123456789abcdef'for c in x)
def frozen():
    require(sha(F/'experiment.json')==EXP,'pre-effect immutable experiment');e=json_bytes((F/'experiment.json').read_bytes())
    require(json_bytes((F/'input-freeze.json').read_bytes())['experiment']==identity(F/'experiment.json'),'original frozen input association')
    for name,item in e['inputs'].items():require(identity(name)==item,'selected executable/source/shader/PST/noise/pair input unchanged')
    for name,item in e['carried_receipts'].items():require(identity(ROOT/name)==item,'old qualification receipt carried unchanged')
    # Source identity and weight extent checks are not executions of old gates.
    require(len(e['source125_sha256'])==125 and all(sha(ROOT/name)==h for name,h in e['source125_sha256'].items()),'same125 source context')
    for w in e['prior_shards']:require(Path(w['path']).stat().st_size==w['bytes_observed']and not w['current_payload_hash_recomputed'],'prior fullshard identity carried, no GB rehash')
    return e
def read_case(raw):
    r=json_bytes(raw);required={'schema','profile','case','scenario','model_calls','ticks','physics_steps','success','fallbacks','refusals','torques_clipped','physics_replay_exact','wall_seconds','model_context_open_seconds','inference_seconds','inference_max_seconds','gpu_seconds','trace_records','trace_seed_sha256','trace_final_sha256','query_profile','policy_profile','SDK_geometry_retained','owned_model_scope'}
    require(type(r)is dict and set(r)==required,'exact existing V1 case record fields')
    require(r['schema']==SCHEMA and r['profile']==PROFILE and r['case']=='demo_000'and r['scenario']==0 and integer(r['scenario']),'owned profile/schema/original case/normal scenario')
    require(r['query_profile']=='panda-owned-query-shadow-v1'and r['policy_profile']=='panda-libero-mujoco331-gr00t-wide-fixed-noise-v1'and r['owned_model_scope']=='exact-demo000-scene-reset-assets','bound owned query/fixed-model/policy scope')
    require(boolean(r['SDK_geometry_retained'])and r['SDK_geometry_retained']and boolean(r['physics_replay_exact'])and r['physics_replay_exact']and boolean(r['success']),'SDK shadow geometry retained; native exact replay observed; typed success')
    ints=['model_calls','ticks','physics_steps','fallbacks','refusals','torques_clipped','trace_records'];require(all(integer(r[k])and r[k]>=0 for k in ints),'nonnegative exact integer record fields')
    T=r['ticks'];require(1<=T<=9 and(T==9 or r['success']),'full cap or authoritative source-success early stop, never silent refusal/partial')
    require(r['model_calls']==(T+7)//8 and r['physics_steps']==r['trace_records']==25*T,'exact action/request/forward record inventory')
    require(r['fallbacks']==r['refusals']==0 and r['torques_clipped']<=7*r['physics_steps'],'clean normal integration; torque clipping permitted and bounded')
    times=['wall_seconds','model_context_open_seconds','inference_seconds','inference_max_seconds','gpu_seconds'];finite([r[k]for k in times],5,'ALL observed timing fields');require(all(type(r[k])in[int,float]and r[k]>=0 for k in times),'nonnegative timing observations')
    require(digest_text(r['trace_seed_sha256'])and digest_text(r['trace_final_sha256']),'canonical lowercase SHA strings');return r
def first_decoded(raw):
    require(len(raw)==448,'exact first decoded16x7F32 extent');words=struct.unpack('<112I',raw);require(all((b&0x7f800000)!=0x7f800000 for b in words),'ALL112 first decoded raw F32 finite');return words
def trace(raw,r,start,decoded,seed):
    n=r['trace_records'];require(len(raw)==64+n*512 and raw[:32]==seed and seed.hex()==r['trace_seed_sha256'],'bounded exact PTR1 extent and independently bound seed')
    require(math.isfinite(start)and 0<=start<=100,'source initialtime admission');rows=[]
    # Validate ALL raw exponent words before clock maxima or other reductions.
    for i in range(n):
        b=raw[32+i*512:32+(i+1)*512];head=HEADER.unpack_from(b);values=BODY.unpack_from(b,48);a=values[:7];u=values[7:16];s=values[16:]
        require(all((x&0x7f800000)!=0x7f800000 for x in a)and all((x&0x7ff0000000000000)!=0x7ff0000000000000 for x in u+s),'ALL action7/control9/state45 finite bit words before maxima')
        rows.append((b,head,a,u,s))
    previous=seed;clock_errors=[];clipped_steps=0;action_ticks=[]
    for i,(b,head,a,u,s)in enumerate(rows):
        magic,version,flags,sub,epoch,serial,tick,ordinal,reserved=head;t=i//25
        require((magic,version,sub,epoch,serial,tick,ordinal,reserved)==(b'PTR1',1,i%25+1,1,t//8+1,t,i+1,0),'canonical magic/version/counters/epoch/requestserial/tick/substep/ordinal/reserved')
        require(flags in[0,2]and b[508:]==b'END1','normal prefix has no fallback/success bits; torque clip flag permitted')
        canonical=HEADER.pack(b'PTR1',1,flags,sub,epoch,serial,tick,ordinal,0)+BODY.pack(*a,*u,*s)+b'END1';require(b==canonical,'independent complete canonical512 bytes')
        actions=struct.unpack('<7f',struct.pack('<7I',*a));controls=struct.unpack('<9d',struct.pack('<9Q',*u));state=struct.unpack('<45d',struct.pack('<45Q',*s));finite(actions,7,'actionfloat');finite(controls,9,'controlfloat');finite(state,45,'statefloat')
        require(all(abs(x)<=1 for x in actions)and actions[6]in[-1,0,1],'normalized source action and discrete gripper range')
        require(all(abs(controls[j])<=(80 if j<5 else 12)for j in range(7))and 0<=controls[7]<=.04 and -.04<=controls[8]<=0,'fixed admitted actuator ranges')
        error=abs(state[0]-(start+ordinal*.002));require(error<=2.**-26,'every new SDK clock receipt2^-26seconds');clock_errors.append(error)
        if i%25==0:action_ticks.append(a)
        else:require(a==action_ticks[-1],'one accepted row held over exactly25 torque servo calls')
        if t<8:require(a==decoded[t*7:(t+1)*7],'first8 consumed action rows exactly equal actually saved first NN output')
        clipped_steps+=bool(flags&2);previous=hashlib.sha256(previous+b).digest()
    require(raw[-32:]==previous and previous.hex()==r['trace_final_sha256'],'ALL stdlib hash links/footer/native final receipt exact')
    require(clipped_steps<=r['torques_clipped']<=7*clipped_steps,'coordinate clipping total is bounded, not reconstructed from onebit substep flags')
    return dict(passed=True,canonical_records=n,canonical_bytes=512*n,finite_action_F32words=7*n,finite_control_state_F64words=54*n,all_clocks=n,max_clock_error_seconds=max(clock_errors),clock_atol_seconds=2.**-26,epoch=1,request_serials=sorted(set(x[1][5]for x in rows)),first_consumed_decoded_rows_bound=min(r['ticks'],8),second_full_decoded_array_persisted=False,torque_clipped_flagged_substeps=clipped_steps,reported_coordinate_clips=r['torques_clipped'],coordinate_clip_exact_reconstruction_claimed=False,seed_sha256=seed.hex(),final_chain_sha256=previous.hex(),success_flag_always_unset=True,native_replay_repeated=False)
def rebuild_trace(records,seed):
    prev=seed
    for b in records:prev=hashlib.sha256(prev+b).digest()
    return seed+b''.join(records)+prev
def decoder_guards():
    # NEW reader guards only, synthetic25-record fixture, no model/old corpus.
    seed=bytes(range(32));decoded=(0,)*112;rows=[]
    for i in range(25):
        sw=list(struct.unpack('<45Q',struct.pack('<45d',(i+1)*.002,*([0.]*44))));rows.append(HEADER.pack(b'PTR1',1,0,i+1,1,1,0,i+1,0)+BODY.pack(*([0]*16),*sw)+b'END1')
    raw=rebuild_trace(rows,seed);case=dict(schema=SCHEMA,profile=PROFILE,case='demo_000',scenario=0,model_calls=1,ticks=1,physics_steps=25,success=True,fallbacks=0,refusals=0,torques_clipped=0,physics_replay_exact=True,wall_seconds=1.,model_context_open_seconds=1.,inference_seconds=1.,inference_max_seconds=1.,gpu_seconds=.5,trace_records=25,trace_seed_sha256=seed.hex(),trace_final_sha256=raw[-32:].hex(),query_profile='panda-owned-query-shadow-v1',policy_profile='panda-libero-mujoco331-gr00t-wide-fixed-noise-v1',SDK_geometry_retained=True,owned_model_scope='exact-demo000-scene-reset-assets')
    read_case(json.dumps(case).encode());trace(raw,case,0.,decoded,seed);bad=[]
    for lane in range(61):
        for bits in([0x7f800000,0xff800000,0x7fc00001]if lane<7 else[0x7ff0000000000000,0xfff0000000000000,0x7ff8000000000001]):
            changed=list(rows);b=bytearray(changed[-1]);struct.pack_into('<I'if lane<7 else'<Q',b,48+lane*4 if lane<7 else 76+(lane-7)*8,bits);changed[-1]=bytes(b);bad.append(rebuild_trace(changed,seed))
    for at,value,size in[(0,0,4),(4,2,2),(6,1,1),(6,4,1),(7,24,1),(8,2,8),(16,2,8),(24,1,8),(32,24,8),(40,1,8),(508,0,4)]:
        changed=list(rows);b=bytearray(changed[-1]);b[at:at+size]=value.to_bytes(size,'little');changed[-1]=bytes(b);bad.append(rebuild_trace(changed,seed))
    changed=list(rows);b=bytearray(changed[-1]);struct.pack_into('<d',b,148,1.);changed[-1]=bytes(b);bad.append(rebuild_trace(changed,seed));bad +=[raw[:-1],raw+b'\0',raw[:-1]+bytes([raw[-1]^1])]
    for x in bad:
        r=dict(case,trace_final_sha256=x[-32:].hex())
        try:trace(x,r,0.,decoded,seed)
        except RuntimeError:pass
        else:raise RuntimeError('NEW PTR1 reader admitted poisoned fields/canonical metadata/hash')
    badcase=[]
    for key,value in[('ticks',0),('ticks',10),('ticks',True),('success',False),('fallbacks',1),('refusals',1),('physics_steps',24),('trace_records',24),('model_calls',2),('physics_replay_exact',False),('schema','psionic-panda-closed-loop-v1'),('query_profile','sdk'),('SDK_geometry_retained',False),('torques_clipped',176),('gpu_seconds',float('nan')),('wall_seconds',float('inf')),('trace_seed_sha256','A'*64)]:badcase.append(json.dumps(dict(case,**{key:value})).encode())
    badcase.append(json.dumps(case).encode()[:-1]+b',"ticks":1}')
    for x in badcase:
        try:read_case(x)
        except RuntimeError:pass
        else:raise RuntimeError('NEW case reader admitted bad complete/early/refusal/finite/schema/type metadata')
    return dict(passed=True,synthetic_valid_records=25,poisoned_trace_cases=len(bad),poisoned_case_records=len(badcase),all61_raw_numeric_lanes_tested=True,poison_hash_chains_recomputed_to_isolate_field_admission=True,SDK_calls=0,NN_calls=0,plant_steps=0,old_gate_calls=0)
def guards_only(out):
    require(not out.exists(),'preserve FIRST reader guard attempt');out.mkdir(parents=True);r=dict(schema='psionic.panda-owned-query-neural.reader-guards.v1',passed=False,source=identity(__file__))
    try:r.update(passed=True,guards=decoder_guards())
    except BaseException as error:r['failure']=repr(error);raise
    finally:write(out/'record.json',r)
    print(json.dumps(dict(passed=True,record=str(out/'record.json'),guards=r['guards'])))
def audit(args):
    out=args.output.resolve();require(not out.exists(),'preserve FIRST/failed independent actual audit');out.mkdir(parents=True);began=time.monotonic();r=dict(schema='psionic.panda-owned-query-neural.independent.v1',passed=False,NN_calls=0,SDK_calls=0,render_calls=0,build_calls=0,old_gate_calls=0,physics_replay_calls=0)
    try:
        e=frozen();d=args.run_dir.resolve();native=json_bytes((d/'record.json').read_bytes());require(native['schema']=='psionic.panda-owned-query-neural.execution.v1'and native['passed']and native['exit_code']==0 and not native['timed_out'],'first successful one-shot native invocation')
        require(native['experiment']==identity(F/'experiment.json'),'native invocation bound to pre-effect experiment')
        for name,h in native['source'].items():require(sha(ROOT/name)==h,'native driver/host source context unchanged')
        case_dir=d/e['case_output_relative'];cp=case_dir/'record.json';tp=case_dir/'physics.ptr';dp=case_dir/'first-decoded.f32'
        require(identity(cp)==native['case_record']and identity(tp)==native['trace']and identity(dp)==native['first_decoded'],'three original native artifact hashes');case=read_case(cp.read_bytes());decoded=first_decoded(dp.read_bytes());initial=finite(struct.unpack('<45d',Path(e['pair_path'],'init.f64').read_bytes()),45,'ALL fixed original initial F64words')
        xml=Path(e['pair_path'],'scene.xml').read_bytes();init=Path(e['pair_path'],'init.f64').read_bytes();noise=Path(e['noise_path']).read_bytes();seed=hashlib.sha256(hashlib.sha256(xml).digest()+hashlib.sha256(init).digest()+hashlib.sha256(noise).digest()+hashlib.sha256(PROFILE.encode()).digest()).digest();proof=trace(tp.read_bytes(),case,initial[0],decoded,seed)
        guard=json_bytes(args.guards.read_bytes());require(guard['passed']and guard['schema']=='psionic.panda-owned-query-neural.reader-guards.v1'and guard['source']==identity(__file__),'new reader was qualified before first effect, same source')
        T=case['ticks'];P=25*T;N=(T+7)//8;counts=dict(SOURCE_DERIVED_NOT_RUNTIME_INSTRUMENTED=True,completed_ticks=T,NN_calls_recorded=N,live_physics_steps_recorded=P,mandatory_native_replay_steps_source_and_exact_receipt=P,total_SDK_steps=2*P,SDK_resets=2,explicit_mj_forward=2+2*P,telemetry_sample_calls=1+N+P+T,robot_state_owned_query_paths=1+P,OFFSCREENimage_captures=2*N,model_open=1,setup_integration_steps=0,second_request_full_decoded_bytes_audited=False)
        observation=dict(completed_prespecified_tick_cap=T==9,early_authoritative_source_success=T<9 and case['success'],task_success_observed=case['success'],task_success_was_required=False,full_first_decoded_sha256=sha(dp),prior_frame_decoded_sha256=e['first_decoded_diagnostic']['prior_SHA256'],first_frame_hash_matches_prior_DIAGNOSTIC_ONLY=sha(dp)==e['first_decoded_diagnostic']['prior_SHA256'],no_model_parity_gate_from_hash=True,first_consumed_rows_bound=proof['first_consumed_decoded_rows_bound'],torque_coordinate_clips_observed=case['torques_clipped'],runtime_geometry_retained=True,NN_source_profile_is_public_endpoint_qualified_not_all_global_hidden_parity=True)
        write(out/'observations.json',observation);frozen();r.update(passed=True,experiment=identity(F/'experiment.json'),native_receipt=identity(d/'record.json'),case_record=identity(cp),trace=identity(tp),first_decoded=identity(dp),source={str(Path(__file__).resolve().relative_to(ROOT)):identity(__file__)},reader_guards=identity(args.guards),trace_audit=proof,exact_native_replay_observed=case['physics_replay_exact'],native_replay_evidence_scope='Exact bound-source SDK replay of all45F64statewords. Not independently rerun here; excludes render/NN/controller recomputation.',source_derived_counts=counts,observations=observation,timing=dict(native_loop_wall_excludes_context_open_and_replay_seconds=case['wall_seconds'],model_context_open_seconds=case['model_context_open_seconds'],inference_seconds=case['inference_seconds'],inference_max_seconds=case['inference_max_seconds'],GPU_compute_seconds=case['gpu_seconds'],simulated_seconds=.05*T,synchronous_simulation_paused_for_inference=True,no_wall_realtime_claim=True),outputs={'observations.json':identity(out/'observations.json')},scope=e['scope'])
    except BaseException as error:r['failure']=repr(error);raise
    finally:r['seconds']=time.monotonic()-began;write(out/'record.json',r)
    print(json.dumps(dict(passed=True,record=str(out/'record.json'),trace_records=proof['canonical_records'],first_frame_matches_prior_DIAGNOSTIC_ONLY=observation['first_frame_hash_matches_prior_DIAGNOSTIC_ONLY'])))
def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--decoder-guards',action='store_true');p.add_argument('--run-dir',type=Path);p.add_argument('--guards',type=Path);p.add_argument('--output',type=Path,required=True);a=p.parse_args();guards_only(a.output.resolve())if a.decoder_guards else audit(a)
if __name__=='__main__':main()
