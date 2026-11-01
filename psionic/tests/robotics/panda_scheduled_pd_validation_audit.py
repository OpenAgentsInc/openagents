#!/usr/bin/env python3
"""Independent NEW C/D audit: cached SDK/OWN traces only, no effects.

The old scalar law is carried as pure Fraction/RN definitions. NEW SDK states
get independent matrix-potential gravity and literal PD/actuator targets. Both
NEW trajectories get direct-array fsum metrics, including INITIAL margins.
No old scalar table, reference, metric or plant result is regenerated.
"""
from pathlib import Path
import argparse,ast,hashlib,json,math,struct,time
import panda_scheduled_pd_reference as law
import panda_gain_search_objective as metric
import panda_joint_pd_reference as pd
import panda_joint_pd_tracking_audit as actuator

ROOT=Path(__file__).resolve().parents[3];T=ROOT/'psionic/tests/robotics'
F=ROOT/'psionic/fixtures/robotics/panda-scheduled-pd-validation'
OLD=ROOT/'psionic/fixtures/robotics/panda-joint-pd-tracking'
EXP='32b5d8ae1cc6e07dba317677102681a831d5b94a19c5e7df78167b57f017f640'
REF='5d881d8540f2e46f7e2a21f451339d33fee2258acfa8239c3c8b4e46ce2f1442'
HEAD=struct.Struct('<6I');ROW=struct.Struct('<120d10q3dq');INDEX=struct.Struct('<6q')
CASE=struct.Struct('<32d13q');POSE=struct.Struct('<2d7q');VERDICT=struct.Struct('<2q')
MAGIC={'sdk':0x31534443,'own':0x314f4443}
require=metric.require;identity=metric.identity;sha=metric.sha;finite=metric.finite;write=metric.write
def bits(values):return struct.pack('<'+str(len(values))+'d',*values)
def verify(source):require(all(identity(ROOT/name)==item for name,item in source.items()),'every bound source/input unchanged')
def compare(actual,expected,atol,label,rtol=0):
    finite(actual,len(expected),label+' ALLactual');finite(expected,len(actual),label+' ALLexpected');require(actual,label+' nonempty')
    errors=[abs(a-b)for a,b in zip(actual,expected)];ratios=[d/(atol+rtol*abs(b))for d,b in zip(errors,expected)]
    result=dict(passed=max(ratios)<=1,values=len(actual),max_abs_error=max(errors),maximum_fraction_of_frozen_budget=max(ratios),atol=atol,rtol=rtol)
    require(result['passed'],label+' fixed budget');return result
def decode(raw,index,e,kind):
    require(len(raw)>=24 and HEAD.unpack_from(raw)==(MAGIC[kind],2,4,750,1072,1),'pose-aware trace header')
    require((len(raw)-24)%1072==0 and len(raw)<=24+6000*1072 and len(index)==384,'bounded exact row/index extents')
    rows=[ROW.unpack_from(raw,24+i*1072)for i in range((len(raw)-24)//1072)]
    finite([x for r in rows for x in r[:120]],120*len(rows),'ALL trace observation floats before any maximum')
    finite([x for r in rows for x in r[130:133]],3*len(rows),'ALL scheduler floats before any maximum')
    table=[];at=0
    for c in range(8):
        pose,scenario,offset,n,stage,code=INDEX.unpack_from(index,c*48)
        require((pose,scenario)==divmod(c,4)and offset==at and 0<=n<=750 and offset+n<=len(rows),'context order/offset/count')
        require(stage>=0 and code>=0 and ((n==750 and stage==code==0)or(n<750 and(stage or code))),'complete or honestly refused prefix')
        previous=bits(e['cases'][c]['initial_q9']+e['cases'][c]['initial_v9']);group=rows[at:at+n]
        for k,r in enumerate(group):
            require(r[120:124]==(scenario,k,0,0)and bits(r[:18])==previous,'prestate initial/previouspost exact signed lineage')
            require(0<=r[124]<=7 and 0<=r[128]<=9 and 0<=r[129]<=2 and r[133]in[0,1,2],'all clipping/regime integer domains')
            require(r[125]>=0 and r[126]in[0,270]and r[125]==min(k,r[126])and r[127]in[0,1],'reference/pulse integer domains')
            require(abs(r[119]-(k+1)/500.)<=e['numeric']['clock_atol_seconds'],'percontext post-state clock')
            previous=bits(r[101:119])
        table.append(dict(profile=pose,case_id=scenario,rows=group,failure_stage=stage,failure_code=code));at+=n
    require(at==len(rows),'no omitted or trailing rows');return table
def decode_scores(raws):
    cr,pr,vr=raws;require(len(cr)==8*360 and len(pr)==2*72 and len(vr)==16,'8/2/1 complete score extents')
    cards=[CASE.unpack_from(cr,i*360)for i in range(8)];poses=[POSE.unpack_from(pr,i*72)for i in range(2)];verdict=VERDICT.unpack(vr)
    finite([x for row in cards for x in row[:32]],256,'ALL case metrics before maxima');finite([x for row in poses for x in row[:2]],4,'ALL pose metrics before maxima')
    require([(r[32],r[33])for r in cards]==[divmod(i,4)for i in range(8)]and [r[2]for r in poses]==[0,1],'pose/scenario labels, not gain candidates')
    for r in cards:require(0<=r[34]<=750 and all(r[j]in[0,1]for j in[35,36,37,39,40,41])and 0<=r[38]<=7 and 0<=r[42]<=5250 and 0<=r[43]<=6750 and 0<=r[44]<=1500,'case status/count domains')
    for r in poses:require(r[3]in[0,1]and r[4]in[0,1]and 0<=r[5]<=21000 and 0<=r[6]<=27000 and 0<=r[7]<=6000 and 0<=r[8]<=7,'pose status/count domains')
    require(all(x in[0,1]for x in verdict),'verdict booleans');return cards,poses,verdict
def reader_guards(raw,index,e,score_raw):
    bad=[(raw[:-1],index),(raw+b'\0',index),(raw,index[:-1])]
    for value in[float('nan'),float('inf'),-float('inf')]:
        for byte in[0,119*8,1040,1056]:
            x=bytearray(raw);struct.pack_into('<d',x,len(raw)-1072+byte,value);bad.append((bytes(x),index))
    for byte,value in[(960,4),(968,750),(976,1),(984,1),(992,8),(1024,10),(1032,3),(1064,3)]:
        x=bytearray(raw);struct.pack_into('<q',x,len(raw)-1072+byte,value);bad.append((bytes(x),index))
    for byte,value in[(0,1),(8,3),(7*48+16,0),(7*48+24,751)]:
        x=bytearray(index);struct.pack_into('<q',x,byte,value);bad.append((raw,bytes(x)))
    for x,y in bad:
        try:decode(x,y,e,'sdk')
        except RuntimeError:pass
        else:raise RuntimeError('new C/D reader admitted poison')
    badscore=[]
    for which,offset in[(0,7*360+31*8),(1,72+8)]:
        x=list(score_raw);x[which]=x[which][:-1];badscore.append(x)
        for value in[float('nan'),float('inf'),-float('inf')]:
            x=list(score_raw);b=bytearray(x[which]);struct.pack_into('<d',b,offset,value);x[which]=bytes(b);badscore.append(x)
    for which,offset,value in[(0,7*360+32*8,9),(1,72+16,9),(2,8,2)]:
        x=list(score_raw);b=bytearray(x[which]);struct.pack_into('<q',b,offset,value);x[which]=bytes(b);badscore.append(x)
    for x in badscore:
        try:decode_scores(x)
        except RuntimeError:pass
        else:raise RuntimeError('new C/D metric reader admitted poison')
    return dict(passed=True,trace_refusals=len(bad),score_refusals=len(badscore),plant_steps=0,SDK_calls=0)
def python_inventory(path,seen=None):
    seen=set()if seen is None else seen;path=path.resolve()
    if path in seen:return seen
    seen.add(path)
    for n in ast.walk(ast.parse(path.read_text())):
        names=[x.name for x in n.names]if isinstance(n,ast.Import)else[n.module]if isinstance(n,ast.ImportFrom)and n.module else[]
        for name in names:
            candidate=T/(name.split('.')[0]+'.py')
            if candidate.exists():python_inventory(candidate,seen)
    return seen
def case_metrics(table,e,refs):
    me=dict(profiles=[dict(case_inputs_f64=[c['input_f64']for c in e['cases'][p*4:(p+1)*4]])for p in range(2)],source_coordinate_ranges=e['source_ranges9'])
    cards=[];details=[]
    for c,item in enumerate(table):
        p,s=divmod(c,4);compact=[tuple(r[101:119])+(p,s,k,r[122],r[123],r[124],r[128],r[129])for k,r in enumerate(item['rows'])]
        localrefs=refs[p*4*751*21:(p+1)*4*751*21];card=metric.direct_case(dict(profile=p,case_id=s,rows=compact),me,localrefs)
        initial=e['cases'][c]['initial_q9'];initialmargin=[min(q-lo,hi-q)for q,(lo,hi)in zip(initial,e['source_ranges9'])]
        # Validation adds INITIAL to the unchanged reducer's post-only margins.
        card[23:32]=[min(a,b)if card[34]else a for a,b in zip(initialmargin,card[23:32])];cards.append(card)
        n=card[34];qerr=[];verr=[]
        for k,r in enumerate(item['rows'][:n]):
            target=refs[(c*751+k+1)*21:(c*751+k+2)*21];qerr.append([r[101+j]-target[j]for j in range(7)]);verr.append([r[110+j]-target[7+j]for j in range(7)])
        finite([x for row in qerr+verr for x in row],n*14,'ALL diagnostic errors')
        details.append(dict(context=c,pose=p,scenario=s,complete=bool(card[36]),valid=bool(card[35]),feasible=bool(card[37]),reason=card[38],terminal=card[3],terminal_margin=1-card[3],terminal_ok=bool(card[39]),hold_ok=bool(card[40]),nominal_ok=bool(card[41]),hold_peak_rad=card[4],hold_margin_rad=.005-card[4],SSE=card[0],stationary_SSE=card[1],nominal_SSE_margin=card[1]-card[0],IAE_rad_seconds=card[2],peak_speed9=card[5:14],peak_effective_step_acceleration9=card[14:23],initial_and_post_source_margin9=card[23:32],Pjp_clipped_coordinates=card[42],Pa_control_clipped_coordinates=card[43],Pa_force_clipped_coordinates=card[44],final_position_error7_rad=qerr[-1]if n else None,final_rate_error7_rad_per_second=verr[-1]if n else None,effective_acceleration_not_SDK_qacc=True))
    poses=[metric.combined(cards,p)for p in range(2)];verdict=[int(all(p[3]for p in poses)),int(all(p[4]for p in poses))];return cards,poses,verdict,details
def scalar_and_wiring(table,e,refs,plans):
    counts=[0,0,0];margins=[None,None];target_a=[];target_b=[]
    for c,item in enumerate(table):
        case=e['cases'][c];N=plans[c]['steps']
        for k,r in enumerate(item['rows']):
            expected=law.schedule(r[31],r[15]);require(expected[0]==0 and struct.pack('<3dq',*expected[1:])==struct.pack('<3dq',*r[130:134]),'each NEW schedule bit-exact from its OWN reference/actual inputs')
            counts[expected[4]]+=1
            for j,threshold in enumerate([.1,.5]):d=abs(r[131]-threshold);margins[j]=d if margins[j]is None else min(margins[j],d)
            require(r[125:127]==(min(k,N),N),'independent tick/N exact');pulse=c%4==2 and 200<=k<250
            require(r[127]==int(pulse)and bits(r[83:92])==bits(case['pulse9']if pulse else[0.]*9),'signed external pulse exact')
            require(bits(r[74:81])==bits(r[67:74])and bits(r[81:83])==bits(case['finger_targets2']),'cap transport/fixed targets exact')
            target_a.extend(r[18:39]);target_b.extend(refs[(c*751+k)*21:(c*751+k+1)*21])
    angles=compare([target_a[i+j]for i in range(0,len(target_a),21)for j in range(7)],[target_b[i+j]for i in range(0,len(target_b),21)for j in range(7)],2e-12,'new reference positions')
    derivatives=compare([target_a[i+j]for i in range(0,len(target_a),21)for j in range(7,21)],[target_b[i+j]for i in range(0,len(target_b),21)for j in range(7,21)],2e-10,'new reference rates/accelerations',2e-12)
    return dict(passed=True,new_exact_schedule_contexts=sum(counts),regime_counts=counts,minimum_absolute_rate_distance_to_low_high_boundaries=margins,old_scalar62_regenerated=False,reference_angles=angles,reference_derivatives=derivatives)
def force_oracle(table,e,nodes,model,out):
    observed=[];expected=[];act_observed=[];act_expected=[];ambiguous=[];smallest=None;exact_caps=0
    for c,item in enumerate(table):
        case=e['cases'][c]
        for k,r in enumerate(item['rows']):
            kd=case['common_KdBase7'][:];kd[6]=law.schedule(r[31],r[15])[3]
            # Actual admitted reference samples are already checked separately
            # against the independent Decimal table. Use their exact words to
            # isolate arithmetic from the admitted polynomial rounding profile.
            case_input={pd.KEYS[0]:r[:7],pd.KEYS[2]:r[9:16],pd.KEYS[3]:r[18:25],pd.KEYS[4]:r[25:32],pd.KEYS[5]:case['common_Kp7'],pd.KEYS[6]:kd}
            g=pd.potential_gradient(nodes,model,r[:9]);want=pd.scalar(case_input,g);observed.extend(r[39:74]);expected.extend(x for key in pd.FIELDS for x in want[key]);require(r[124]==want['clipped'],'independent SDK Pjp clipping count exact')
            act,cc,fc=actuator.actuator(r[:9],r[9:18],r[74:83]);act_observed.extend(r[92:101]);act_expected.extend(act);require(r[128:130]==(cc,fc),'independent SDK actuator clipping counts exact')
            for j,requested in enumerate(want['requested']):
                cap=80 if j<5 else 12;margin=abs(abs(requested)-cap);smallest=margin if smallest is None else min(smallest,margin)
                if margin<=2e-8:ambiguous.append(dict(context=c,tick=k,joint=j,independent_requested=requested,recorded_requested=r[60+j],distance=margin))
                exact_caps+=abs(r[74+j])==cap
    (out/'SDK-expected-controller.f64').write_bytes(bits(expected));(out/'SDK-expected-actuator.f64').write_bytes(bits(act_expected))
    return dict(controller=compare(observed,expected,2e-8,'NEW SDK all35PD fields'),actuator=compare(act_observed,act_expected,2e-8,'NEW SDK scalar actuator force after unitgear admission'),independent_requested_clipping_minimum_distance=smallest,near_requested_crossings=ambiguous,exact_cap_transports_to_Pa=exact_caps,cap_transport_is_structural_not_an_ambiguous_requested_crossing=True,SDK_gravity_contexts=len(expected)//35,OWN_gravity_oracle_repeated=False)
def parity(sdk,own,e):
    fields=[('hinge_q',[*range(7),*range(101,108)],2e-8),('slide_q',[7,8,108,109],2e-9),('hinge_v',[*range(9,16),*range(110,117)],2e-7),('slide_v',[16,17,117,118],2e-8),('controller_and_controls',[*range(39,83)],2e-8),('actuator',list(range(92,101)),2e-8),('clock',[119],2.**-26)]
    result={};a=[r for item in sdk for r in item['rows']];b=[r for item in own for r in item['rows']]
    require([len(x['rows'])for x in sdk]==[len(x['rows'])for x in own],'same matched context extents')
    require(all(x[120:130]==y[120:130]for x,y in zip(a,b)),'all80-byte packet identity/status/clip/reference/pulse metadata exact')
    for name,lanes,cap in fields:result[name]=compare([x[j]for x in a for j in lanes],[x[j]for x in b for j in lanes],cap,'closed-feedback OWN/SDK '+name)
    result['schedule_branch_differences_diagnostic_only']=sum(x[133]!=y[133]for x,y in zip(a,b));result['integer_metadata_exact']=True;return result
def audit(args):
    out=args.output.resolve();require(not out.exists(),'preserve FIRST/failed independent audit');out.mkdir(parents=True);began=time.monotonic()
    r=dict(schema='psionic.panda-scheduled-pd-validation.independent.v1',passed=False,SDK_calls=0,plant_steps=0,Coax_calls=0,NN_calls=0,old_scalar62_regenerations=0,old_original4_metric_recomputations=0,reference_regeneration_calls=0,shadow_steps=0)
    try:
        require(sha(F/'experiment.json')==EXP and sha(F/'reference-qualification.json')==REF,'pre-effect experiment and once-only reference receipts')
        e=json.loads((F/'experiment.json').read_text());verify(e['metadata_inputs']);rr=json.loads((F/'reference-qualification.json').read_text());require(rr['passed']and rr['experiment']==identity(F/'experiment.json')and all(identity(F/n)==v for n,v in rr['outputs'].items()),'exact new analytic outputs');verify(rr['source'])
        refs=finite(struct.unpack('<126168d',(F/'reference-states.f64').read_bytes()),126168,'ALL independent analytic references before comparisons');plans=json.loads((F/'reference-plans.json').read_text());require(len(plans)==8 and [x['context']for x in plans]==list(range(8)),'all eight new plan identities')
        sdkdir=args.sdk_dir.resolve();owndir=args.own_dir.resolve();score=args.score_dir.resolve();sr=json.loads((sdkdir/'record.json').read_text());nr=json.loads((owndir/'record.json').read_text())
        require(sr['schema']=='psionic.panda-scheduled-pd-validation.sdk-reference.v1'and sr['passed']and sr['SDK_steps']==6000 and sr['shadow_steps']==0,'completed FIRST SDK6000/no-shadow capture')
        require(nr['schema']=='psionic.panda-scheduled-pd-validation.native-reference.v1'and nr['passed']and nr['owned_steps']==6000 and nr['SDK_calls']==nr['shadow_steps']==0 and nr['cached_modes_byte_identical'],'completed FIRST OWN6000/cached3 comparison; no shadow')
        require(sr['experiment']==nr['experiment']==identity(F/'experiment.json')and nr['capture_record']==identity(sdkdir/'record.json')and nr['capture_trace']==sr['trace']and nr['capture_index']==sr['index'],'exact paired capture/native/experiment association');verify(sr['source']);verify(nr['source'])
        require(sr['model']==e['SDK_admission']['prior_model_metadata']and identity(sdkdir/'sdk-model.json')==sr['model'],'qualified compiled-model identity before forward carried by capture')
        tables={};raws={};indices={}
        for kind,directory,receipt in[('sdk',sdkdir,sr),('own',owndir,nr)]:
            path=directory/(kind+'-trace.bin');ip=directory/(kind+'-index.bin');require(identity(path)==receipt['trace']and identity(ip)==receipt['index'],'original trace/index digests');raws[kind]=path.read_bytes();indices[kind]=ip.read_bytes();tables[kind]=decode(raws[kind],indices[kind],e,kind)
        all_schedules={kind:scalar_and_wiring(tables[kind],e,refs,plans)for kind in['sdk','own']};paired=parity(tables['sdk'],tables['own'],e)
        arm=ROOT/'psionic/fixtures/robotics/panda-fk/source-profile.json';branch=ROOT/'psionic/fixtures/robotics/panda-fingers/source-profile.json';mp=ROOT/'psionic/fixtures/robotics/panda-fingers/sdk-model.json';b=json.loads(branch.read_text());require(b['arm_profile']['sha256']==sha(arm),'explicit arm/branch profile association');nodes=json.loads(arm.read_text())['nodes']+b['nodes'];model=json.loads(mp.read_text());require(len(nodes)==len(model['bodies'])==16 and [n['name']for n in nodes]==[n['name']for n in model['bodies']],'all16 body names/order')
        forces=force_oracle(tables['sdk'],e,nodes,model,out);all_metrics={};outputs=['SDK-expected-controller.f64','SDK-expected-actuator.f64'];guard_raw=None
        for kind in['sdk','own']:
            names=[kind+'-case-scores.bin',kind+'-pose-scores.bin',kind+'-verdict.bin'];score_raw=[(score/name).read_bytes()for name in names]
            require(all(identity(score/name)==nr['modes']['debug']['outputs'][name]for name in names),'all score files are the measured byte-identical native outputs')
            actual,poses,verdict=decode_scores(score_raw);expected,expected_poses,expected_verdict,details=case_metrics(tables[kind],e,refs)
            cc=compare([x for row in actual for x in row[:32]],[x for row in expected for x in row[:32]],2e-12,kind+' 8 NEW direct-array fsum metric cards',2e-12);pc=compare([x for row in poses for x in row[:2]],[x for row in expected_poses for x in row[:2]],2e-12,kind+' 2 NEW pose reductions',2e-12)
            require(all(tuple(a[32:])==tuple(b[32:])for a,b in zip(actual,expected))and all(tuple(a[2:])==tuple(b[2:])for a,b in zip(poses,expected_poses))and tuple(verdict)==tuple(expected_verdict),'all new case/pose/verdict flags counts exact')
            prefix='independent-'+kind+'-';(out/(prefix+'case-scores.bin')).write_bytes(b''.join(CASE.pack(*x)for x in expected));(out/(prefix+'pose-scores.bin')).write_bytes(b''.join(POSE.pack(*x)for x in expected_poses));(out/(prefix+'verdict.bin')).write_bytes(VERDICT.pack(*expected_verdict));outputs +=[prefix+n for n in['case-scores.bin','pose-scores.bin','verdict.bin']]
            all_metrics[kind]=dict(passed=True,case_comparison=cc,pose_comparison=pc,verdict=list(verdict),all_eight_goals_met=all(bool(x[37])for x in expected),cases=details);guard_raw=score_raw if kind=='sdk'else guard_raw
        require([(x['valid'],x['complete'],x['feasible'],x['terminal_ok'],x['hold_ok'],x['nominal_ok'])for x in all_metrics['sdk']['cases']]==[(x['valid'],x['complete'],x['feasible'],x['terminal_ok'],x['hold_ok'],x['nominal_ok'])for x in all_metrics['own']['cases']],'SDK/OWN quality flags agree EXACTLY; missed goals remain reported separately')
        write(out/'quality.json',all_metrics);outputs.append('quality.json');source_paths=python_inventory(Path(__file__))|{arm,branch,mp};source={str(p.relative_to(ROOT)):identity(p)for p in sorted(source_paths)};verify(e['metadata_inputs']);verify(sr['source']);verify(nr['source']);verify(source)
        r.update(passed=True,experiment=identity(F/'experiment.json'),reference=identity(F/'reference-qualification.json'),SDK_record=identity(sdkdir/'record.json'),OWN_record=identity(owndir/'record.json'),SDK_trace=sr['trace'],OWN_trace=nr['trace'],source=source,each_trace_finite_floats=738000,old_law_proof_carried=e['original_law_qualification'],new_schedule_audit=all_schedules,independent_SDK_forces=forces,OWN_vs_SDK=paired,metrics=all_metrics,quality_goals_separate_from_numerical_gate=True,all_SDK_goals_met=all_metrics['sdk']['all_eight_goals_met'],all_OWN_goals_met=all_metrics['own']['all_eight_goals_met'],reader_guards=reader_guards(raws['sdk'],indices['sdk'],e,guard_raw),outputs={n:identity(out/n)for n in outputs},scope='Two frozen poses, one fixed law, eight new no-contact contexts. No search winner, old gate repeats, shadow, nonlinear stability, collision or hardware certificate.')
    except BaseException as error:r['failure']=repr(error);raise
    finally:r['seconds']=time.monotonic()-began;write(out/'record.json',r)
    print(json.dumps(dict(passed=True,record=str(out/'record.json'),SDK_goals=r['all_SDK_goals_met'],OWN_goals=r['all_OWN_goals_met'],metric_error=max(x['case_comparison']['max_abs_error']for x in all_metrics.values()))))
def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--sdk-dir',type=Path,required=True);p.add_argument('--own-dir',type=Path,required=True);p.add_argument('--score-dir',type=Path,required=True);p.add_argument('--output',type=Path,required=True);audit(p.parse_args())
if __name__=='__main__':main()
