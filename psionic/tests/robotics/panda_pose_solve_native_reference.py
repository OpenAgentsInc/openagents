#!/usr/bin/env python3
"""New whole-pose solve endpoint and external one-step replay qualification.

Kind1 is NOT an instrumented internal solver trace. Expected data is cached
from the independent new oracle; no old54/4200 gate or SDK work runs here.
"""
from pathlib import Path
import argparse, hashlib, json, math, os, platform, struct, time
from panda_pose_ik_native_reference import Run,identity,sha,graph,finite,require,quaternion_matrix,metric
ROOT=Path(__file__).resolve().parents[3]
FIXTURE=ROOT/'psionic/fixtures/robotics/panda-pose-solve'
SOURCE=ROOT/'psionic/lib/robotics/panda-pose-solve.coax'
NATIVE=ROOT/'psionic/tests/robotics/panda_pose_solve_native.c'
EXPERIMENT_SHA='45014fef818614becc72cfca4ea4adf6702bd588848fc2141181394b358dd598'
QUERY=struct.Struct('<14dq');ANSWER=struct.Struct('<q17d2q');PAYLOAD=struct.Struct('<17d2q');EVENT=struct.Struct('<4q17d2q')

def load_inputs(oracle,terminal):
    require(sha(FIXTURE/'experiment.json')==EXPERIMENT_SHA,'frozen new whole-solver experiment');e=json.loads((FIXTURE/'experiment.json').read_text())
    r=json.loads((oracle/'reference.json').read_text());plan=json.loads((terminal/'native-plan-v1.json').read_text());binding=json.loads((terminal/'terminal-binding-v1.json').read_text())
    require(r['passed'] and r['experiment']==identity(FIXTURE/'experiment.json') and r['budgets']==e['budgets'] and r['total_queries']==112 and r['physical_contexts']==14 and r['trajectory_steps']==279 and r['SDK_calls']==r['Coax_calls']==0,'exact new independent reference')
    require(all(identity(oracle/name)==value for name,value in r['outputs'].items()) and all(sha(ROOT/name)==digest for name,digest in r['source_sha256'].items()),'all independent reference/source bytes')
    require(binding['passed'] and not binding['trajectory_regenerated'] and binding['geometry_calls']==binding['Coax_calls']==binding['SDK_calls']==0 and binding['reference']==identity(oracle/'reference.json'),'cached-only terminal binding')
    require(all(identity(terminal/name)==value for name,value in binding['outputs'].items()) and all(identity(ROOT/name)==value for name,value in binding['source'].items()),'exact terminal source/artifacts')
    require(plan['reference']==identity(oracle/'reference.json') and plan['query_count']==112 and plan['physical_context_count']==14 and plan['step_event_count']==279 and plan['event_count']==405 and plan['terminal_event_count']==14,'exact new event inventory')
    for key,path in [('queries',oracle/'queries.bin'),('query_expected',oracle/'query-expected.bin'),('trajectory_expected',oracle/'trajectory-expected.bin'),('trajectory_index',oracle/'trajectory-index.json'),('terminal_expected',terminal/'terminal-expected.bin')]:require(plan[key]==identity(path),'pinned plan '+key)
    contexts=(FIXTURE/'contexts.f64').read_bytes();base=(FIXTURE/'base-queries.bin').read_bytes();require(len(contexts)==18*112 and len(base)==76*120 and identity(FIXTURE/'contexts.f64')==e['contexts_file'] and identity(FIXTURE/'base-queries.bin')==e['base_queries_file'],'frozen18 contexts/76 base+AND queries')
    for name,digest in e['source_sha256'].items():require(sha(ROOT/name)==digest,'frozen source '+name)
    raw=(oracle/'queries.bin').read_bytes();meta=json.loads((oracle/'queries.json').read_text());require(len(raw)==112*120 and raw[:76*120]==base and len(meta)==112,'all112 queries with original76 prefixes')
    queries=[QUERY.unpack_from(raw,i*120)for i in range(112)];groups={};triples=[]
    for i,(row,m)in enumerate(zip(queries,meta)):
        finite(row[:14],14,'queryinput');require(m['index']==i and 0<=m['context']<18 and m['sign']in[-1,1] and row[14]==m['max_updates'] and 0<=row[14]<=96,'all actual query headers/budgets')
        c=list(struct.unpack_from('<14d',contexts,m['context']*112));wanted=c[:10]+[m['sign']*x for x in c[10:]]
        require(raw[i*120:i*120+112]==struct.pack('<14d',*wanted),'query context/sign bit identity')
        triples.append((m['context'],m['sign'],m['max_updates']));groups.setdefault((m['context'],m['max_updates']),{})[m['sign']]=i
    require(len(set(triples))==112,'no duplicate query metadata')
    for group in groups.values():
        require(set(group)=={-1,1},'explicit two-sign query group')
        a,b=group[1],group[-1];require(raw[a*120:a*120+80]==raw[b*120:b*120+80] and queries[a][14]==queries[b][14],'same seed/position/budget for signed query')
        for j in range(4):require(struct.unpack_from('<Q',raw,a*120+80+j*8)[0]^struct.unpack_from('<Q',raw,b*120+80+j*8)[0]==1<<63,'exact opposite quaternion bit signs')
    index=json.loads((oracle/'trajectory-index.json').read_text());require(len(index)==18,'all context trajectory metadata')
    derived={(i,s,n)for i,v in enumerate(index[:14])if v['terminal_converged'] and v['rows']>0 for s in[1,-1]for n in[v['rows']-1,v['rows']]}-set(triples[:76]);require(set(triples[76:])==derived and len(derived)==36,'derived n-1/n query headers before comparison')
    offset=0
    for i,t in enumerate(plan['trajectories']):
        require(t['context']==i and 0<=t['count_records']<=96 and t['offset_records']==offset and t['count_records']==index[i]['rows'],'all14 independent external replay bounds')
        finite(t['input_f64'],14,'replayinput');require(struct.pack('<14d',*t['input_f64'])==contexts[i*112:(i+1)*112],'replay uses frozen positive-sign context')
        offset+=t['count_records']
    require(offset==279 and plan['alias_success_query_index']==52 and meta[52]['context']==9 and meta[52]['sign']==1 and queries[52][14]==96 and meta[52]['status']==0 and meta[52]['successful_updates']==9,'independent dense alias context/header')
    # Validate fixture predicates before the first product call, never infer pairs.
    shell=[float.fromhex(x)for x in['0x1.0000000001197p+0','0x1.0000000001198p+0','0x1.fffffffffdcd1p-1','0x1.fffffffffdcd0p-1']]
    require([abs(x*x-1)<=2e-12 for x in shell]==[True,False,True,False],'independent exact binary64 shell fixtures')
    return e,r,plan,queries,meta,groups

def decode(raw):
    require(len(raw)==405*EVENT.size,'exact405x184-byte event transport');rows=[EVENT.unpack_from(raw,i*EVENT.size)for i in range(405)]
    finite([x for row in rows for x in row[4:21]],6885,'ALL actual or expected fields before maxima')
    for i,row in enumerate(rows):
        require(row[1]in[0,1,2] and row[3]in[0,7],'event kind/status domain')
        if row[3]:require(raw[i*184+32:(i+1)*184]==bytes(152),'unavailable failure transport zero after C atomicity assertion')
        else:
            require(0<=row[21]<=96 and 0<=row[22]<=7*row[21],'actual/expected count domains')
            if row[1]==1:require(row[21]==1,'external one-step replay counter')
            if row[1]==2:require(row[21]==row[22]==0,'fresh terminal zero-work counters')
            if row[1]!=1:require(row[18]<=1e-6 and row[19]<=1e-6,'both successful physical criteria')
    return rows

def expected_events(oracle,terminal,plan):
    q=(oracle/'query-expected.bin').read_bytes();trajectory=(oracle/'trajectory-expected.bin').read_bytes();end=(terminal/'terminal-expected.bin').read_bytes();require(len(q)==112*160 and len(trajectory)==279*152 and len(end)==14*160,'exact all cached expected extents');parts=[]
    for i in range(112):
        row=q[i*160:(i+1)*160];status=struct.unpack_from('<q',row)[0];parts.append(struct.pack('<4q',i,0,0,status)+row[8:])
    for t in plan['trajectories']:
        for k in range(t['count_records']):parts.append(struct.pack('<4q',t['context'],1,k+1,0)+trajectory[(t['offset_records']+k)*152:(t['offset_records']+k+1)*152])
        row=end[t['context']*160:(t['context']+1)*160];parts.append(struct.pack('<4q',t['context'],2,t['count_records'],struct.unpack_from('<q',row)[0])+row[8:])
    raw=b''.join(parts);decode(raw);return raw

def compare(actual,expected,e,groups):
    a=decode(actual);b=decode(expected);errors={x:[]for x in['joint','position','physical_rotation','position_error','angle_error','weighted_error']};good=bad=0
    for i,(x,y)in enumerate(zip(a,b)):
        require(x[:4]==y[:4] and x[21:]==y[21:],'exact event/status/update/projection inventory '+str(i))
        if x[3]:bad+=1;continue
        good+=1;errors['joint'].extend(abs(v-w)for v,w in zip(x[4:11],y[4:11]));errors['position'].extend(abs(v-w)for v,w in zip(x[11:14],y[11:14]));errors['physical_rotation'].extend(abs(v-w)for v,w in zip(quaternion_matrix(x[14:18]),quaternion_matrix(y[14:18])));errors['position_error'].append(abs(x[18]-y[18]));errors['angle_error'].append(abs(x[19]-y[19]));errors['weighted_error'].append(abs(x[20]-y[20]))
    for g in groups.values():
        first,second=g[1],g[-1];require(actual[first*184+32:(first+1)*184]==actual[second*184+32:(second+1)*184] and a[first][3]==a[second][3],'explicit same-context/budget sign output bytes')
    names={'joint':'joint_radians_atol','position':'position_metres_atol','physical_rotation':'physical_rotation_matrix_max_entry_atol','position_error':'position_error_metres_atol','angle_error':'angle_error_radians_atol','weighted_error':'weighted_error_metres_atol'}
    return dict(passed=True,events=405,successful_records=good,unavailable_records=bad,all_actual_finite_words=6885,all_expected_finite_words=6885,exact_status_and_counters=True,metrics={name:metric(values,e['budgets'][names[name]],name)for name,values in errors.items()},replay_scope='Kind1 external Ppo one-step replay; actual whole Pps endpoints/status/counts are kind0; no internal instrumentation.')

def guard_reader(actual,expected):
    cases=0
    for data in[actual,expected]:
        bad=[data[:-1],data+b'\0']
        for v in[math.nan,math.inf,-math.inf]:
            x=bytearray(data);struct.pack_into('<d',x,len(x)-24,v);bad.append(bytes(x))
        for offset,value in[(3*8,6),(21*8,-1)]:
            x=bytearray(data);struct.pack_into('<q',x,offset,value);bad.append(bytes(x))
        for x in bad:
            try:decode(x)
            except RuntimeError:cases+=1
            else:raise RuntimeError('reader admitted actual/expected poison')
    require(cases==14,'all14 late IEEE/extent/status/counter guard cases');return dict(passed=True,cases=14,actual_and_expected=True)

def native(args):
    require(platform.system()=='Darwin' and platform.machine()=='arm64' and os.sys.byteorder=='little','qualified Darwin-arm64 scalar-F64 profile')
    oracle=args.oracle.resolve();terminal=args.terminal.resolve()if args.terminal else oracle;e,reference,plan,queries,metadata,groups=load_inputs(oracle,terminal);expected=expected_events(oracle,terminal,plan)
    out=args.output.resolve();require(not out.exists(),'preserve every prior/partial attempt');out.mkdir(parents=True);compiler=args.compiler.resolve();record=dict(schema='psionic.panda-pose-solve.native.v1',passed=False,SDK_calls=0,NN_calls=0,simulation_steps=0,old_corpora_repeated=False,old_gates_repeated=False,oracle_regenerated=False,experiment=identity(FIXTURE/'experiment.json'),reference=identity(oracle/'reference.json'),terminal_binding=identity(terminal/'terminal-binding-v1.json'),compiler=identity(compiler),commands=[],budgets=e['budgets'])
    run=Run(out,record);run.save();began=time.monotonic()
    try:
        inputs=graph(SOURCE)
        for p in[NATIVE,Path(__file__).resolve(),ROOT/'psionic/tests/robotics/panda_pose_ik_native_reference.py']+[p for folder in[FIXTURE,oracle,terminal]for p in folder.iterdir()if p.is_file()]:inputs[p]=identity(p)
        record['frozen_inputs']={str(p.relative_to(ROOT))if p.is_relative_to(ROOT)else str(p):v for p,v in sorted(inputs.items())};record['preflight']=dict(queries=112,physical_contexts=14,replay_steps=279,events=405,query_budget_bounds=[min(q[14]for q in queries),max(q[14]for q in queries)],derived_headers_verified=36,explicit_sign_groups=len(groups),binary64_shell_predicates_verified=True,guard_seed_equals_alias_query52=True);run.save()
        packed=struct.pack('<4I',0x31535050,112,14,1)+(oracle/'queries.bin').read_bytes()+b''.join(QUERY.pack(*t['input_f64'],t['count_records'])for t in plan['trajectories'])+struct.pack('<21d',*[x[0]for x in e['joint_limits_radians']],*[x[1]for x in e['joint_limits_radians']],*queries[52][:7])+struct.pack('<q',52);(out/'input.bin').write_bytes(packed);(out/'expected-events.bin').write_bytes(expected);record['input']=identity(out/'input.bin');record['expected_events']=identity(out/'expected-events.bin');emissions={}
        for mode,extra in[('debug',[]),('release',['--release'])]:
            directory=out/mode;directory.mkdir();c=directory/'panda-pose-solve.c';h=directory/'panda-pose-solve.h';c.write_bytes(run.command('emit-'+mode,[compiler,'c',SOURCE.relative_to(ROOT),'--library',*extra,'--header',h]));emissions[mode]=(c,h)
        record['emissions']={m:dict(C=identity(c),header=identity(h))for m,(c,h)in emissions.items()};run.save();outputs=[];common=['-std=c11','-Wall','-Wextra','-Werror','-Wno-unused-function','-Wno-misleading-indentation','-fno-fast-math','-ffp-contract=off','-fno-builtin-sin','-fno-builtin-cos']
        for mode,source_mode,flags in[('debug','debug',['-O0','-g']),('release','release',['-O2']),('sanitized','debug',['-O1','-g','-fsanitize=address,undefined','-fno-sanitize-recover=all','-fno-omit-frame-pointer'])]:
            directory=out/('native-'+mode);directory.mkdir();c,h=emissions[source_mode];binary=directory/'native';run.command('compile-Coax-'+mode,[os.environ.get('CC','cc'),*common,'-Wno-unused-parameter',*flags,'-c',c,'-o',directory/'coax.o']);run.command('compile-effects-'+mode,[os.environ.get('CC','cc'),*common,*flags,'-I'+str(h.parent),NATIVE,directory/'coax.o','-lm','-o',binary]);report=json.loads(run.command('run-'+mode,[binary,out/'input.bin',directory/'events.bin']))
            require(report['passed'] and report['SDK_calls']==0 and report['queries']==112 and report['physical_trajectories']==14 and report['trajectory_steps']==279 and report['events']==405 and report['record_bytes']==184 and report['target_sign_checks']==112 and report['alias_cases']==12 and report['seed_endpoints']==14 and report['XYZ_endpoints']==6 and report['norm_shell_contexts']==4 and report['nonfinite_lane_patterns']==70 and report['fpcr']==0,'exact new native inventory')
            actual=(directory/'events.bin').read_bytes();comparison=compare(actual,expected,e,groups);record.setdefault('modes',{})[mode]=dict(native=report,comparison=comparison,binary=identity(binary),output=identity(directory/'events.bin'));outputs.append(actual);run.save()
        require(outputs[0]==outputs[1]==outputs[2],'all complete new event streams bit-identical');record['new_reader_guards']=guard_reader(outputs[0],expected);require(all(identity(p)==v for p,v in inputs.items()) and identity(compiler)==record['compiler'],'all executed source/reference/compiler bytes unchanged');record.update(passed=True,modes_byte_identical=True,output_sha256=hashlib.sha256(outputs[0]).hexdigest())
    except BaseException as error:record['failure']=repr(error);raise
    finally:record['seconds']=time.monotonic()-began;record['partial_or_full_outputs']={str(p.relative_to(out)):identity(p)for p in out.glob('native-*/events.bin')};run.save()
    print(json.dumps(dict(passed=True,record=str(out/'record.json'),assertions={m:v['native']['assertions']for m,v in record['modes'].items()},output_sha256=record['output_sha256'])))

def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--compiler',type=Path,required=True);p.add_argument('--output',type=Path,required=True);p.add_argument('--oracle',type=Path,default=FIXTURE);p.add_argument('--terminal',type=Path);native(p.parse_args())
if __name__=='__main__':main()
