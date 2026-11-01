#!/usr/bin/env python3
"""New scalar/carry/brake audit; no plant, SDK or old oracle/gate execution.

Three already-passed metric cards are carried exactly (local-ID remap only).
Only the new brake has fresh direct-array metrics and matrix gravity targets.
"""
from pathlib import Path
from fractions import Fraction
import argparse,hashlib,json,math,struct,time
import panda_gain_search_objective as metrics
import panda_joint_pd_reference as pd
ROOT=Path(__file__).resolve().parents[3];F=ROOT/'psionic/fixtures/robotics/panda-scheduled-pd';A=ROOT/'psionic/fixtures/robotics/panda-gain-search';OLD=ROOT/'psionic/fixtures/robotics/panda-joint-pd-tracking'
EXP='16f8fa543403fc92ec068908292f618a9b7f41593acd179d2586235deb387b74'
ROW=struct.Struct('<120d10q3dq');SCALAR=struct.Struct('<q3dq');CASE=struct.Struct('<32d13q');ASSESS=struct.Struct('<2d7q');HEAD=struct.Struct('<4I');INDEX=struct.Struct('<5q')
require=metrics.require;identity=metrics.identity;sha=metrics.sha;finite=metrics.finite;write=metrics.write
def bits(x):return struct.pack('<d',x)
def word(x):return struct.unpack('<Q',bits(x))[0]
def frac(x):return Fraction.from_float(x)
def rn(x):return float(x)
def schedule(ref,actual):
    # Integer exponent admission keeps invalid rawNaN payloads inert.
    if any((word(x)>>52)&2047==2047 for x in[ref,actual]):return(2,0.,0.,0.,0)
    if abs(ref)>20 or abs(actual)>20:return(3,0.,0.,0.,0)
    exact=frac(ref)-frac(actual);error=rn(exact)
    if exact==0 and ref==actual==0:error=-0. if word(ref)>>63 and not(word(actual)>>63)else 0.
    absolute=abs(error)
    if absolute<=.1:gain=4.;regime=0
    elif absolute>=.5:gain=20.;regime=2
    else:
        numerator=rn(frac(absolute)-frac(.1));denominator=rn(frac(.5)-frac(.1));fraction=rn(frac(numerator)/frac(denominator));increment=rn(Fraction(16)*frac(fraction));gain=rn(Fraction(4)+frac(increment));regime=1
    return(0,error,absolute,gain,regime)
def command(raw):
    # Independent byte reconstruction of all712bytes from original1040B Pnt.
    require(len(raw)>=1040,'fresh row extent');return raw[144:312]+raw[312:592]+raw[992:1000]+raw[592:664]+raw[664:736]+raw[1000:1040]+raw[736:808]
def decode(raw,index):
    require(len(raw)>=16 and HEAD.unpack_from(raw)==(0x31525350,4,750,1072)and(len(raw)-16)%1072==0 and len(raw)<=16+3000*1072,'PSR1 bounded extent/header');require(len(index)==160,'four exact indexrows');table=[];offset=0
    for c in range(4):
        case,at,n,stage,code=INDEX.unpack_from(index,c*40);require(case==c and at==offset and 0<=n<=750 and stage>=0 and code>=0,'index identity/order/status');require((n==750 and stage==code==0)or(c==3 and n<750 and(stage!=0 or code!=0)),'three full carries/one honest brake prefix');rows=[]
        for k in range(n):
            chunk=raw[16+(at+k)*1072:16+(at+k+1)*1072];require(len(chunk)==1072,'row nottruncated');r=ROW.unpack(chunk);finite(r[:120],120,'ALL committed command/state fields');finite(r[130:133],3,'ALL schedule diagnostics');require(r[120:124]==(c,k,0,0)and 0<=r[124]<=7 and 0<=r[128]<=9 and 0<=r[129]<=2 and r[133]in[0,1,2],'ordered successful committed rows/counter regimes');rows.append((chunk,r))
        table.append(dict(case_id=c,rows=rows,failure_stage=stage,failure_code=code));offset+=n
    require(16+offset*1072==len(raw),'no missing/trailing PSR records');return table
def carry_binding():
    path=F/'metric-carry-inputs-v1.json';require(sha(path)=='6ce37dadaa7db497a052dd15197485667f5152d1985fb0106495d51792f095cc','pre-result metriccarryaddendum');r=json.loads(path.read_text());require(r['passed']and r['experiment']==identity(F/'experiment.json')and all(identity(ROOT/name)==item for name,item in r['inputs'].items()),'all prior qualified metric card/error inputs unchanged');return identity(path)
def trace_guards(raw,index):
    bad=[(raw[:-1],index),(raw+b'\0',index),(raw,index[:-1])]
    for value in[float('nan'),float('inf'),-float('inf')]:
        for offset in[0,119*8,1040+16]:
            x=bytearray(raw);struct.pack_into('<d',x,len(x)-1072+offset,value);bad.append((bytes(x),index))
    for offset,value in[(960,99),(968,999),(976,1),(984,1),(992,8),(1024,10),(1032,3),(1064,3)]:
        x=bytearray(raw);struct.pack_into('<q',x,len(x)-1072+offset,value);bad.append((bytes(x),index))
    for offset,value in[(3*40+8,0),(3*40+16,751),(0,1)]:
        x=bytearray(index);struct.pack_into('<q',x,offset,value);bad.append((raw,bytes(x)))
    for x,y in bad:
        try:decode(x,y)
        except RuntimeError:pass
        else:raise RuntimeError('NEW PSR/index reader admitted late poison')
    return dict(passed=True,refused_cases=len(bad),plant_steps=0,SDK_calls=0)
def scalar_reference(e,out):
    cases=e['scalar_cases'];expected=[];powers=[];regimes=[0,0,0]
    for c in cases:
        ref,rate=struct.unpack('<2d',struct.pack('<2Q',int(c['reference_bits_u64_hex'],16),int(c['actual_bits_u64_hex'],16)));want=schedule(ref,rate);require(want[0]==c['expected_status'],'independent both-finite-before-bounds status');finite(want[1:4],3,'ALL expected scalar diagnostics');expected.append(want)
        if want[0]==0:
            _,error,absolute,gain,regime=want;require(4<=gain<=20,'closed gain sector');force=rn(frac(gain)*frac(error));power=-frac(force)*frac(error);require(power<=0,'requested damping relative-error power');powers.append(str(power));regimes[regime]+=1
    (out/'schedule-expected.bin').write_bytes(b''.join(SCALAR.pack(*r)for r in expected));denom=rn(frac(.5)-frac(.1));slope=Fraction(16)/frac(denom);intercept=Fraction(4)-slope*frac(.1);require(intercept==0 and 0<slope<=40,'stored-literal underlying real ramp slope bound');facts=dict(passed=True,cases=len(cases),valid=len(powers),refusals=len(cases)-len(powers),refused_rows_are_unavailable_transport_not_published_gain=True,regime_counts=regimes,requested_component_relative_power_nonpositive=True,gain_sector=[4,20],stored_denominator_F64_hex=denom.hex(),stored_ramp_slope_exact=str(slope),stored_real_intercept_exact=str(intercept),ideal_decimal_interior='Kd=40*abs(e); D=40*abs(e)*e; incremental damping=80*abs(e), up to40',stored_literal_incremental_upper_bound=40,not_a_derivative_of_rounded_code=True,not_clipped_total_torque_or_global_plant_passivity=True);write(out/'schedule-sector-facts.json',facts);return facts

def generate_scalars(out):
    require(not out.exists(),'preserve first scalar reference');out.mkdir(parents=True);r=dict(schema='psionic.panda-scheduled-pd.scalar-reference.v1',passed=False,plant_steps=0,SDK_calls=0,Coax_calls=0,old_gate_runs=0)
    try:
        require(sha(F/'experiment.json')==EXP,'immutable scalar experiment');e=json.loads((F/'experiment.json').read_text());require(identity(F/'scalar-inputs.f64')==e['scalar_input'],'all62 frozen rawinputbits');require((F/'scalar-inputs.f64').read_bytes()==b''.join(struct.pack('<2Q',int(c['reference_bits_u64_hex'],16),int(c['actual_bits_u64_hex'],16))for c in e['scalar_cases']),'exact binary/JSON corpus identity');metric_binding=carry_binding();facts=scalar_reference(e,out);r.update(passed=True,metric_carry_input_binding=metric_binding,experiment=identity(F/'experiment.json'),input=identity(F/'scalar-inputs.f64'),source={str(Path(__file__).resolve().relative_to(ROOT)):identity(__file__)},facts=facts,outputs={name:identity(out/name)for name in['schedule-expected.bin','schedule-sector-facts.json']})
    except BaseException as error:r['failure']=repr(error);raise
    finally:write(out/'record.json',r)
    print(json.dumps(dict(passed=True,record=str(out/'record.json'),expected=r['outputs']['schedule-expected.bin'],cases=facts['cases'])))

def scalar_audit(actual_path,e,reference):
    receipt=json.loads((reference/'record.json').read_text());require(receipt['passed']and receipt['experiment']==identity(F/'experiment.json')and receipt['input']==e['scalar_input'],'original scalar reference receipt');require(all(identity(ROOT/name)==item for name,item in receipt['source'].items()),'executed scalar source identity');require(all(identity(reference/name)==item for name,item in receipt['outputs'].items()),'exact original expected values/facts');expected=(reference/'schedule-expected.bin').read_bytes();actual=actual_path.read_bytes();require(len(expected)==len(actual)==len(e['scalar_cases'])*40,'complete62 scalar rows')
    rows=[SCALAR.unpack_from(actual,i*40)for i in range(len(e['scalar_cases']))];finite([x for row in rows for x in row[1:4]],len(rows)*3,'ALL actual scalar words before bit comparison');require(actual==expected,'all scalar status/diagnostic/regime words bit-exact');return dict(passed=True,cases=len(rows),all_words_bit_exact=True,expected_receipt=identity(reference/'record.json'),expected_table=identity(reference/'schedule-expected.bin'),facts=receipt['facts'],reference_generation_repeated=False)

def compare(a,b,atol,label,rtol=0):
    finite(a,len(b),label+' ALLactual');finite(b,len(a),label+' ALLexpected');require(a,label+' nonempty');errors=[abs(x-y)for x,y in zip(a,b)];budget=[v/(atol+rtol*abs(y))for v,y in zip(errors,b)];require(max(budget)<=1,label+' frozenbudget');return dict(passed=True,values=len(a),max_abs_error=max(errors),maximum_fraction_of_budget=max(budget),atol=atol,rtol=rtol)
def audit(args):
    out=args.output.resolve();require(not out.exists(),'preserve first/failed audit');out.mkdir(parents=True);r=dict(schema='psionic.panda-scheduled-pd.independent.v1',passed=False,plant_steps=0,SDK_calls=0,Coax_calls=0,old_reference_regeneration_calls=0,old_gate_runs=0);began=time.monotonic()
    try:
        require(sha(F/'experiment.json')==EXP,'immutable common-law experiment');e=json.loads((F/'experiment.json').read_text());require(identity(F/'prospective-descriptor.json')==e['prospective_descriptor']and identity(F/'prospective-seal.json')==e['prospective_seal'],'descriptor sealed before newbrake');require(identity(F/'scalar-inputs.f64')==e['scalar_input']and identity(F/'common-base-cases.f64')==e['common_base_cases_identity'],'fixedscalar/commoncases');require(all(identity(ROOT/name)==item for name,item in e['metadata_inputs'].items()),'all old source/raw-data associations unchanged');metric_binding=carry_binding();original_coupled=json.loads((OLD/'experiment.json').read_text());require(all(sha(ROOT/name)==digest for name,digest in original_coupled['source_sha256'].items()),'unchanged coupled model/independent matrix definitions');native=args.native_dir.resolve();score=args.score_dir.resolve();trace_path=native/'own-trace.bin';index_path=native/'trace-index.bin';raw=trace_path.read_bytes();table=decode(raw,index_path.read_bytes());scalar=scalar_audit(args.scalar_actual.resolve(),e,args.scalar_reference.resolve());base=e['common_base_cases_f64'];refs=finite(struct.unpack('<63084d',(OLD/'reference-states.f64').read_bytes()),63084,'ALL existing analytic reference fields');old_lineage=json.loads((A/'lineage.json').read_text());carry_matches=0;new_schedule_counts=[0,0,0];new_command_bytes=0
        for c,item in enumerate(table):
            prev=struct.pack('<18d',*base[c][:18]);oldraw=None
            if c<3:
                path=Path('/tmp/panda-gain-search-owned-first/raw/profile-8-case-%d.bin'%c);binding=e['carry_sources'][c];require(identity(path)==binding['raw'],'exact old archived profile8 raw beforecarry');oldraw=path.read_bytes();require(struct.unpack_from('<5q',oldraw)==(8,c,750,0,0)and len(oldraw)==40+750*1040,'original completed rawcontext')
            for k,(chunk,row)in enumerate(item['rows']):
                require(chunk[:144]==prev,'every fresh prestate equals initial/previouspost');prev=chunk[808:952];want=schedule(row[31],row[15]);require(want[0]==0 and struct.pack('<3dq',*row[130:134])==struct.pack('<3dq',*want[1:]),'all recorded schedules bit-exact against independentFractionRN');new_schedule_counts[want[4]]+=1;D=rn(frac(want[3])*frac(want[1]));compare([row[52]],[D],2e-8,'D7 scheduled product')
                if c<3:
                    oldchunk=oldraw[40+k*1040:40+(k+1)*1040];require(chunk[:1040]==oldchunk and command(chunk)==command(oldchunk)and len(command(chunk))==712,'exact oldfresh1040 and independentfull712 command reconstruction');carry_matches+=1;new_command_bytes+=712
        require(carry_matches==2250,'all2250 commands carried exactly; no fallback')
        old_actual=(A/'case-scores.bin').read_bytes();old_expected=(A/'independent-case-scores.bin').read_bytes();case_raw=(score/'case-scores.bin').read_bytes();assessment_raw=(score/'assessment.bin').read_bytes();require(len(case_raw)==1440 and len(assessment_raw)==72,'four360cards plus one72 assessment');actual=[CASE.unpack_from(case_raw,c*360)for c in range(4)];assessment=ASSESS.unpack(assessment_raw);finite([x for row in actual for x in row[:32]],128,'ALL streamed case metrics');finite(assessment[:2],2,'ALL combined metrics');expected=[];carry_metrics=[]
        for c in range(3):
            at=(8*4+c)*360;prior=bytearray(old_actual[at:at+360]);struct.pack_into('<q',prior,256,0);require(case_raw[c*360:(c+1)*360]==prior,'three streamed metric cards EXACT except documented local-ID8to0');ind=bytearray(old_expected[at:at+360]);struct.pack_into('<q',ind,256,0);expected.append(list(CASE.unpack(ind)));carry_metrics.append(dict(case_id=c,old_profile=8,new_local_ID=0,only_changed_bytes=[256,264],old_actual_sha256=hashlib.sha256(old_actual[at:at+360]).hexdigest(),new_actual_sha256=hashlib.sha256(prior).hexdigest(),independent_expected_sha256=hashlib.sha256(ind).hexdigest()))
        item=table[3];compact=[]
        for k,(_,row)in enumerate(item['rows']):compact.append(tuple(row[101:119])+(0,3,k,row[122],row[123],row[124],row[128],row[129]))
        metric_e=dict(profiles=[dict(case_inputs_f64=base)],source_coordinate_ranges=e['validity']['source_limits_all_nine']);brake=metrics.direct_case(dict(profile=0,case_id=3,rows=compact),metric_e,refs);expected.append(brake);fresh_metric=compare(actual[3][:32],brake[:32],2e-12,'NEW brake direct-arrayfsum32',2e-12);require(tuple(actual[3][32:])==tuple(brake[32:]),'NEWbrake13flags/counts exact');want=metrics.combined(expected,0);combined=compare(assessment[:2],want[:2],2e-12,'NEWfourcase combined metrics',2e-12);require(tuple(assessment[2:])==tuple(want[2:]),'combined local-ID/validity/feasibility/clipping exact')
        # Only the genuinely NEW750-step brake gets new independent matrix
        # gravity/P-D/request/clamp targets. Carried old metric work is reused.
        arm_path=ROOT/'psionic/fixtures/robotics/panda-fk/source-profile.json';branch_path=ROOT/'psionic/fixtures/robotics/panda-fingers/source-profile.json';model_path=ROOT/'psionic/fixtures/robotics/panda-fingers/sdk-model.json';branch=json.loads(branch_path.read_text());require(branch['arm_profile']['sha256']==sha(arm_path),'arm/branch binding');nodes=json.loads(arm_path.read_text())['nodes']+branch['nodes'];model=json.loads(model_path.read_text());require(len(nodes)==len(model['bodies'])==16 and [n['name']for n in nodes]==[n['name']for n in model['bodies']],'all16 nodes incompiledmodel order');pd_actual=[];pd_expected=[]
        for k,(_,row)in enumerate(table[3]['rows']):
            target=refs[3*751*21+k*21:3*751*21+(k+1)*21];gain=base[3][46:53];gain=gain[:];gain[6]=row[132];g=pd.potential_gradient(nodes,model,row[:9]);qcase={pd.KEYS[0]:row[:7],pd.KEYS[2]:row[9:16],pd.KEYS[3]:target[:7],pd.KEYS[4]:target[7:14],pd.KEYS[5]:base[3][39:46],pd.KEYS[6]:gain};target_pd=pd.scalar(qcase,g);pd_actual+=row[39:74];pd_expected.extend(x for key in pd.FIELDS for x in target_pd[key]);require(row[124]==target_pd['clipped'],'newbrake independent Pjp clips exact')
        pd_proof=compare(pd_actual,pd_expected,2e-8,'NEW brake all35PDfields')if pd_actual else dict(passed=True,values=0,note='No committed brake states; invalid/incomplete case remains visible.')
        (out/'independent-case-scores.bin').write_bytes(b''.join(CASE.pack(*x)for x in expected));(out/'independent-assessment.bin').write_bytes(ASSESS.pack(*want));(out/'new-brake-expected-PD.f64').write_bytes(struct.pack('<'+str(len(pd_expected))+'d',*pd_expected));write(out/'metric-carry.json',dict(passed=True,cases=carry_metrics,old_independent_error_receipt=identity(A/'independent-objective.json'),old_independent_expected_table=identity(A/'independent-case-scores.bin'),old_streamed_table=identity(A/'case-scores.bin'),old_metric_oracle_reexecuted=False));r.update(passed=True,experiment=identity(F/'experiment.json'),trace=identity(trace_path),index=identity(index_path),scalar_proof=scalar,metric_carry_input_binding=metric_binding,reader_guards=trace_guards(raw,index_path.read_bytes()),schedule_counts_all_recorded=new_schedule_counts,exact_new_schedule_rows=sum(new_schedule_counts),exact_carried_command_queries=carry_matches,exact_carried_command_bytes=new_command_bytes,old_metric_cases_carried=3,old_metric_cases_recomputed=0,new_brake_direct_metric=fresh_metric,new_fourcase_combination=combined,new_brake_PD=pd_proof,new_brake_committed_steps=len(table[3]['rows']),source={str(Path(__file__).resolve().relative_to(ROOT)):identity(__file__),str((ROOT/'psionic/tests/robotics/panda_gain_search_objective.py').relative_to(ROOT)):identity(ROOT/'psionic/tests/robotics/panda_gain_search_objective.py'),str((ROOT/'psionic/tests/robotics/panda_joint_pd_reference.py').relative_to(ROOT)):identity(ROOT/'psionic/tests/robotics/panda_joint_pd_reference.py'),str(arm_path.relative_to(ROOT)):identity(arm_path),str(branch_path.relative_to(ROOT)):identity(branch_path),str(model_path.relative_to(ROOT)):identity(model_path),str((ROOT/'psionic/tests/robotics/panda_inverse_dynamics_matrix.py').relative_to(ROOT)):identity(ROOT/'psionic/tests/robotics/panda_inverse_dynamics_matrix.py')},assessment=list(assessment),all_four_valid=bool(assessment[3]),all_four_goals_met=bool(assessment[4]),new_local_profile_ID=0,not_old_grid_winner=True,C_D_not_executed=True,outputs={name:identity(out/name)for name in['independent-case-scores.bin','independent-assessment.bin','new-brake-expected-PD.f64','metric-carry.json']})
    except BaseException as error:r['failure']=repr(error);raise
    finally:r['seconds']=time.monotonic()-began;write(out/'record.json',r)
    print(json.dumps(dict(passed=True,record=str(out/'record.json'),new_brake_rows=r['new_brake_committed_steps'],all_four_goals_met=r['all_four_goals_met'],brake_metric=fresh_metric)))
def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--generate-scalars',action='store_true');p.add_argument('--native-dir',type=Path);p.add_argument('--score-dir',type=Path);p.add_argument('--scalar-actual',type=Path);p.add_argument('--scalar-reference',type=Path);p.add_argument('--output',type=Path,required=True);a=p.parse_args();generate_scalars(a.output.resolve())if a.generate_scalars else audit(a)
if __name__=='__main__':main()
