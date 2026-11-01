#!/usr/bin/env python3
"""Independent direct-array metric and deterministic policy audit on NEW cache.

No plant, SDK, Coax invocation, old reference regeneration or old gate run.
math.fsum reductions are independent of Coax's streaming accumulation order.
"""
from pathlib import Path
import argparse,hashlib,json,math,struct,time
import panda_gain_search_reference as contract
ROOT=contract.ROOT;F=contract.F;OLD=contract.OLD
EXPERIMENT='67c872e87b64041c9a0fde7014f828a345a0d48148d496fe087575d49fe38b31'
ROW=struct.Struct('<18d8q');HEAD=struct.Struct('<6I');INDEX=struct.Struct('<6q')
CASE=struct.Struct('<32d13q');CANDIDATE=struct.Struct('<2d7q');SELECTION=struct.Struct('<4q2d2q')
require=contract.require;identity=contract.identity;sha=contract.sha;finite=contract.finite;write=contract.write
def decode(cache,index):
    require(len(cache)>=24 and HEAD.unpack_from(cache)==(0x31434750,9,4,750,208,1),'exact PGC1 header');require((len(cache)-24)%208==0 and len(cache)<=24+27000*208 and len(index)==36*48,'exact bounded cache/index record extents');rows=[ROW.unpack_from(cache,24+i*208)for i in range((len(cache)-24)//208)];finite([x for r in rows for x in r[:18]],18*len(rows),'ALL poststate fields before any metric');table=[];next_offset=0
    for i in range(36):
        p,c,offset,n,stage,code=INDEX.unpack_from(index,i*48);require((p,c)==divmod(i,4)and offset==next_offset and 0<=n<=750 and offset+n<=len(rows),'all36 canonical prefix extents/order');require(stage>=0 and code>=0 and ((n==750 and stage==code==0)or(n<750 and(stage!=0 or code!=0))),'complete/failure index status');group=rows[offset:offset+n]
        for k,r in enumerate(group):
            require(r[18:23]==(p,c,k,0,0),'only committed successful raw states; exact profile/case/tick');require(0<=r[23]<=7 and 0<=r[24]<=9 and 0<=r[25]<=2,'all source clipping counter domains')
        table.append(dict(profile=p,case_id=c,rows=group,failure_stage=stage,failure_code=code));next_offset+=n
    require(next_offset==len(rows),'no omitted/trailing cache records');return table
def decode_score_bytes(case_raw,candidate_raw,selection_raw):
    require(len(case_raw)==36*360 and len(candidate_raw)==9*72 and len(selection_raw)==64,'exact360/72/64 output inventories');cases=[CASE.unpack_from(case_raw,i*360)for i in range(36)];candidates=[CANDIDATE.unpack_from(candidate_raw,i*72)for i in range(9)];selection=SELECTION.unpack(selection_raw);finite([x for r in cases for x in r[:32]],1152,'ALL Coaxcase metrics before comparisons');finite([x for r in candidates for x in r[:2]],18,'ALL Coaxcandidate metrics');finite(selection[4:6],2,'ALL Coaxselection metric gaps');require([(r[32],r[33])for r in cases]==[divmod(i,4)for i in range(36)]and [r[2]for r in candidates]==list(range(9)),'exact scorecard identity/order')
    for row in cases:
        require(0<=row[34]<=750 and all(row[j]in[0,1]for j in[35,36,37,39,40,41])and 0<=row[38]<=7 and 0<=row[42]<=5250 and 0<=row[43]<=6750 and 0<=row[44]<=1500,'all case status/count domains')
    for row in candidates:require(row[3]in[0,1]and row[4]in[0,1]and 0<=row[5]<=21000 and 0<=row[6]<=27000 and 0<=row[7]<=6000 and 0<=row[8]<=7,'all candidate status/count domains')
    require(-1<=selection[0]<=8 and -1<=selection[1]<=8 and 0<=selection[2]<=9 and 0<=selection[3]<=4 and -21000<=selection[6]<=21000 and -8<=selection[7]<=8,'selector domain')
    return cases,candidates,selection
def decode_scores(directory):return decode_score_bytes(*[(directory/name).read_bytes()for name in['case-scores.bin','candidates.bin','selection.bin']])
def direct_case(item,e,refs):
    p=item['profile'];c=item['case_id'];case=e['profiles'][p]['case_inputs_f64'][c];initialq=case[:9];initialv=case[9:18];ranges=e['source_coordinate_ranges'];accepted=[];previous=initialv;reason=0
    for r in item['rows']:
        q=r[:9];v=r[9:18]
        # Successful compact rows have already passed transport finite/status/
        # counter admission. Numerical/source limits remain scorer policy.
        if any(abs(x)>(100 if j<7 else 1)for j,x in enumerate(q))or any(abs(x)>(20 if j<7 else 2)for values in[v,previous]for j,x in enumerate(values)):reason=7;break
        if any(not(lo<=x<=hi)for x,(lo,hi)in zip(q,ranges)):reason=4;break
        accepted.append((q,v,previous,r[23:26]));previous=v
    n=len(accepted);complete=n==750 and reason==0
    if not complete and reason==0:reason=1
    errors=[];stationary=[];rate_errors=[];speeds=[abs(x)for x in initialv];acc=[0.]*9;margins=[0.]*9;hold=terminal=0.;clip=[0,0,0]
    for k,(q,v,prev,counts)in enumerate(accepted):
        ref=refs[c*751*21+(k+1)*21:c*751*21+(k+2)*21];error=[q[j]-ref[j]for j in range(7)];stationary.append([initialq[j]-ref[j]for j in range(7)]);rate=[v[j]-ref[7+j]for j in range(7)];errors.append(error);rate_errors.append(rate);hold=max(hold,max(abs(q[j]-initialq[j])for j in range(7)))
        if k+1>=626:terminal=max(terminal,max(abs(x)/.01 for x in error),max(abs(x)/.05 for x in rate))
        for j in range(9):
            speeds[j]=max(speeds[j],abs(v[j]));acc[j]=max(acc[j],abs((v[j]-prev[j])/.002));margin=min(q[j]-ranges[j][0],ranges[j][1]-q[j]);margins[j]=margin if k==0 else min(margins[j],margin)
        clip=[x+y for x,y in zip(clip,counts)]
    flat=[x for row in errors for x in row];base=[x for row in stationary for x in row];finite(flat,n*7,'ALL independent errors');finite(base,n*7,'ALL independent stationary errors');finite([x for row in rate_errors for x in row],n*7,'ALL independent rate errors');position_square=math.fsum(x*x for x in flat);stationary_square=math.fsum(x*x for x in base);iae=.002*math.fsum(abs(x)for x in flat);floats=finite([position_square,stationary_square,iae,terminal,hold]+speeds+acc+margins,32,'ALL independent direct-array metrics');terminal_ok=int(complete and terminal<=1);hold_ok=int(complete and(c!=0 or hold<=.005));nominal_ok=int(complete and(c!=1 or position_square<stationary_square));feasible=terminal_ok&hold_ok&nominal_ok;ints=[p,c,n,int(complete),int(complete),feasible,reason,terminal_ok,hold_ok,nominal_ok]+clip;return floats+ints
def combined(cards,p):
    group=cards[p*4:(p+1)*4];valid=int(all(r[35]and r[36]for r in group));feasible=int(valid and all(r[37]for r in group));reason=next((r[38]for r in group if not r[35]or not r[36]),0);return[max(r[3]for r in group),math.fsum(r[2]for r in group),p,valid,feasible,sum(r[42]for r in group),sum(r[43]for r in group),sum(r[44]for r in group),reason]
def select(table):
    eligible=[r for r in table if r[3]and r[4]];ordered=sorted(eligible,key=lambda r:(r[0],r[1],r[5],r[2]));count=len(ordered)
    if count==0:return[-1,-1,0,0,0.,0.,0,0]
    winner=ordered[0]
    if count==1:return[winner[2],-1,1,0,0.,0.,0,0]
    runner=ordered[1];keys=[(runner[0],winner[0]),(runner[1],winner[1]),(runner[5],winner[5]),(runner[2],winner[2])];first=next(i+1 for i,(a,b)in enumerate(keys)if a!=b);return[winner[2],runner[2],count,first,runner[0]-winner[0],runner[1]-winner[1],runner[5]-winner[5],runner[2]-winner[2]]
def policy_guards():
    def blank():return[[0.,0.,p,0,0,0,0,0,0]for p in range(9)]
    cases=[];t=blank();cases.append((t,[-1,-1,0,0,0.,0.,0,0]));t=blank();t[7]=[.5,2.,7,1,1,0,0,0,0];cases.append((t,[7,-1,1,0,0.,0.,0,0]))
    for a,b,winner,runner,key in[([.5,5.,1,1,1,2,0,0,0],[.6,1.,8,1,1,0,0,0,0],1,8,1),([.5,1.,6,1,1,99,0,0,0],[.5,2.,0,1,1,0,0,0,0],6,0,2),([.5,1.,8,1,1,1,0,0,0],[.5,1.,1,1,1,2,0,0,0],8,1,3),([.5,1.,2,1,1,0,0,0,0],[.5,1.,8,1,1,0,0,0,0],2,8,4)]:
        t=blank();t[a[2]]=a;t[b[2]]=b;expected=[winner,runner,2,key,b[0]-a[0],b[1]-a[1],b[5]-a[5],b[2]-a[2]];cases.append((t,expected))
    for table,expected in cases:require(select(table)==expected,'independent exact lex/none/tie/negative-later-gap policy witness')
    return dict(passed=True,cases=len(cases),plant_steps=0,SDK_calls=0)
def compare(actual,expected,label):
    finite(actual,len(expected),label+' ALLactual');finite(expected,len(actual),label+' ALLexpected');require(actual,label+' nonempty');differences=[abs(a-b)for a,b in zip(actual,expected)];ratios=[d/(2e-12+2e-12*abs(b))for d,b in zip(differences,expected)];require(max(ratios)<=1,label+' frozen2e-12abs+2e-12relative');return dict(passed=True,values=len(actual),max_abs_error=max(differences),maximum_fraction_of_frozen_budget=max(ratios),atol=2e-12,rtol=2e-12)
def reader_guards(cache,index,score_dir):
    # Qualify only NEW cache/score readers; no plant or old reader/gate calls.
    count=0;bad=[(cache[:-1],index),(cache+b'\0',index),(cache,index[:-1])]
    if len(cache)>24:
        for value in[float('nan'),float('inf'),-float('inf')]:
            copy=bytearray(cache);struct.pack_into('<d',copy,len(copy)-208+17*8,value);bad.append((bytes(copy),index))
        for lane,value in[(0,99),(1,99),(2,999),(3,1),(4,1),(5,8),(6,10),(7,3)]:
            copy=bytearray(cache);struct.pack_into('<q',copy,len(copy)-208+144+lane*8,value);bad.append((bytes(copy),index))
    for a,b in bad:
        try:decode(a,b)
        except RuntimeError:count+=1
        else:raise RuntimeError('NEW cache reader admitted late poison')
    original=[(score_dir/name).read_bytes()for name in['case-scores.bin','candidates.bin','selection.bin']];score_count=0;bad_scores=[]
    for which,offset in[(0,35*360+31*8),(1,8*72+8),(2,40)]:
        values=original[:];values[which]=values[which][:-1];bad_scores.append(values)
        for value in[float('nan'),float('inf'),-float('inf')]:
            values=original[:];copy=bytearray(values[which]);struct.pack_into('<d',copy,offset,value);values[which]=bytes(copy);bad_scores.append(values)
    for which,offset in[(0,35*360+32*8),(1,8*72+2*8)]:
        values=original[:];copy=bytearray(values[which]);struct.pack_into('<q',copy,offset,99);values[which]=bytes(copy);bad_scores.append(values)
    for values in bad_scores:
        try:decode_score_bytes(*values)
        except RuntimeError:score_count+=1
        else:raise RuntimeError('NEW metric reader admitted late poison')
    return dict(passed=True,refused_cache_cases=count,refused_score_cases=score_count,SDK_calls=0,plant_steps=0)
def audit(args):
    out=args.output.resolve();require(not out.exists(),'preserve every original/failed audit');out.mkdir(parents=True);r=dict(schema='psionic.panda-gain-search.independent-objective.v1',passed=False,SDK_calls=0,plant_steps=0,Coax_calls=0,reference_generation_calls=0,baseline_resimulation_steps=0,old_gates_repeated=False);began=time.monotonic()
    try:
        require(sha(F/'experiment.json')==EXPERIMENT,'immutable presimulation inputs/objective/heldouts');e=json.loads((F/'experiment.json').read_text());require(all(identity(F/name)==item for name,item in e['input_files'].items())and identity(F/'baseline-carry.json')==e['baseline_carry'],'all frozen profile/heldout/baseline inputs');require(all(identity(ROOT/name)==item for name,item in e['old_reference_data'].items()),'unchanged original once-generated references and qualification');require(all(identity(ROOT/name)==item for name,item in e['source'].items()),'all inputproducer/source metadata frozen');cache_dir=args.cache_dir.resolve();score_dir=args.score_dir.resolve();lineage_path=cache_dir/'lineage.json';lineage=json.loads(lineage_path.read_text());cache_path=cache_dir/'compact-cache.bin';index_path=cache_dir/'compact-index.bin';require(lineage['schema']=='psionic.panda-gain-search.compact-lineage.v1'and lineage['passed']and lineage['experiment']==identity(F/'experiment.json')and lineage['baseline_new_plant_steps']==0 and lineage['baseline_fresh_only'],'completed once-only grid/baseline carry lineage');require(lineage['compact']==identity(cache_path)and lineage['index']==identity(index_path),'exact compact data/extraction index');baseline_binding=json.loads((F/'baseline-carry.json').read_text());require(lineage['baseline_trace']==baseline_binding['previous_trace']and lineage['baseline_trace']['sha256']==contract.BASE_SHA and lineage['baseline_qualification']==identity(OLD/'native-qualification.json'),'old wholetrace/native proof carry');raw_inventory=lineage['raw_trajectories'];require(len(raw_inventory)==32 and [(x['profile'],x['case_id'])for x in raw_inventory]==[(p,c)for p in range(9)if p!=1 for c in range(4)]and all(0<=x['records']<=750 for x in raw_inventory),'exact32 NEW trajectory slots, no baseline runtime slot')
        cache=cache_path.read_bytes();index=index_path.read_bytes();table=decode(cache,index);require(all(len(table[x['profile']*4+x['case_id']]['rows'])==x['records']and (table[x['profile']*4+x['case_id']]['failure_stage'],table[x['profile']*4+x['case_id']]['failure_code'])==(x['stage'],x['code'])and x['raw']['bytes']==40+x['records']*1040 for x in raw_inventory),'all committed new prefix counts/status/rawbyte extents exact');require(all(len(table[4+c]['rows'])==750 for c in range(4)),'all four exact carried baseline trajectories complete');all_lineage=lineage['all_trajectories'];require(len(all_lineage)==36 and [(x['profile'],x['case_id'])for x in all_lineage]==[divmod(i,4)for i in range(36)],'all36 exact extraction lineage identities');lineage_offset=0
        for item,entry in zip(table,all_lineage):
            require(entry['offset_records']==lineage_offset and entry['records']==len(item['rows'])and(entry['stage'],entry['code'])==(item['failure_stage'],item['failure_code']),'all36 extraction offsets/counts/status exact');lineage_offset+=entry['records']
            if item['profile']==1:require(entry['raw']==baseline_binding['previous_trace']and entry['origin']=='carriedoldfresh-prefix-only','each baseline context remains old fulltrace/fresh-only')
            else:require(entry in raw_inventory and entry['origin']=='newownedtrajectory','each new trajectory identity belongs to exact32raw inventory');refs=finite(struct.unpack('<63084d',(OLD/'reference-states.f64').read_bytes()),63084,'ALL existing analytic reference fields before metrics');expected=[direct_case(item,e,refs)for item in table];want_candidates=[combined(expected,p)for p in range(9)];want_selection=select(want_candidates);actual,candidates,selection=decode_scores(score_dir);float_cases=compare([x for row in actual for x in row[:32]],[x for row in expected for x in row[:32]],'all36 independently reduced casecards');require(all(tuple(a[32:])==tuple(b[32:])for a,b in zip(actual,expected)),'ALL468 identity/sample/validity/goal/reason/clip integers exact');float_candidates=compare([x for row in candidates for x in row[:2]],[x for row in want_candidates for x in row[:2]],'all9 independent combined objectivecards');require(all(tuple(a[2:])==tuple(b[2:])for a,b in zip(candidates,want_candidates)),'all63 candidate counts/flags/reasons exact');F64_policy=select(candidates);require(tuple(selection[:4]+selection[6:])==tuple(F64_policy[:4]+F64_policy[6:]),'exact F64 policy winner/runner/count/first-key/clipgap/idgap on independently checked scoretable');require(tuple(selection[:4]+selection[6:])==tuple(want_selection[:4]+want_selection[6:]),'same exact selection under independent direct-array objective');selector_float=compare(selection[4:6],F64_policy[4:6],'two exact-table selector metricgaps');independent_selector_float=compare(selection[4:6],want_selection[4:6],'two independently reduced selector metricgaps')
        for name,fmt,rows in [('independent-case-scores.bin',CASE,expected),('independent-candidates.bin',CANDIDATE,want_candidates),('independent-selection.bin',SELECTION,[want_selection])]: (out/name).write_bytes(b''.join(fmt.pack(*row)for row in rows))
        scorecards=[]
        for p,row in enumerate(want_candidates):
            cards=expected[p*4:(p+1)*4];scorecards.append(dict(profile=p,kp_scale=e['profiles'][p]['kp_scale'],kd_scale=e['profiles'][p]['kd_scale'],carried=p==1,valid=bool(row[3]),feasible=bool(row[4]),worst_terminal=row[0],total_position_IAE_rad_seconds=row[1],PJP_clipped_coordinates=row[5],Pa_control_clipped_coordinates=row[6],Pa_force_clipped_coordinates=row[7],reason=row[8],cases=[dict(case=c,processed_samples=card[34],valid=bool(card[35]),complete=bool(card[36]),feasible=bool(card[37]),reason=card[38],terminal_ok=bool(card[39]),hold_ok=bool(card[40]),nominal_ok=bool(card[41]),terminal=card[3],terminal_limit_margin=1.-card[3],hold_peak=card[4],hold_limit_margin=.005-card[4],position_square=card[0],stationary_square=card[1],nominal_SSE_margin=card[1]-card[0],IAE=card[2],peak_speed9=card[5:14],peak_effective_acceleration9=card[14:23],source_limit_margin9=card[23:32],PJP_clipped_coordinates=card[42],Pa_control_clipped_coordinates=card[43],Pa_force_clipped_coordinates=card[44])for c,card in enumerate(cards)]))
        write(out/'scorecards.json',scorecards);source={str(Path(__file__).resolve().relative_to(ROOT)):identity(__file__),str((ROOT/'psionic/tests/robotics/panda_gain_search_reference.py').relative_to(ROOT)):identity(ROOT/'psionic/tests/robotics/panda_gain_search_reference.py')};require(sha(F/'experiment.json')==EXPERIMENT and all(identity(ROOT/name)==item for name,item in source.items()),'executed immutable oracle/manifest identity');r.update(passed=True,experiment=identity(F/'experiment.json'),lineage=identity(lineage_path),cache=identity(cache_path),cache_index=identity(index_path),source=source,method='Direct-array math.fsum sums independently checked against new Coax streaming; exact F64 lexicographic policy on validated table checked separately.',all_cached_fields_finite=sum(len(x['rows'])for x in table)*18,all_existing_reference_fields_finite=63084,all_case_metrics_finite=1152,all_candidate_metrics_finite=18,all_selector_gaps_finite=2,new_plant_steps_committed=sum(x['records']for x in raw_inventory),new_plant_steps_maximum=24000,baseline_carried_rollouts=4,reader_guards=reader_guards(cache,index,score_dir),exact_policy_guards=policy_guards(),case_metric_comparison=float_cases,candidate_metric_comparison=float_candidates,selector_policy_comparison=selector_float,independent_selector_comparison=independent_selector_float,all_integer_flags_counts_exact=True,winner=want_selection[0],runner_up=want_selection[1],feasible_candidates=want_selection[2],no_feasible_is_valid_outcome=want_selection[0]==-1,selection=list(selection),scorecards=scorecards,outputs={name:identity(out/name)for name in['independent-case-scores.bin','independent-candidates.bin','independent-selection.bin','scorecards.json']},scope=e['limits'])
    except BaseException as error:r['failure']=repr(error);raise
    finally:r['seconds']=time.monotonic()-began;write(out/'record.json',r)
    print(json.dumps(dict(passed=True,record=str(out/'record.json'),winner=r['winner'],feasible_candidates=r['feasible_candidates'],case_metrics=float_cases)))
def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--cache-dir',type=Path,required=True);p.add_argument('--score-dir',type=Path,required=True);p.add_argument('--output',type=Path,required=True);audit(p.parse_args())
if __name__=='__main__':main()
