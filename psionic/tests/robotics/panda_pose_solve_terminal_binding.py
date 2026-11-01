#!/usr/bin/env python3
"""Bind NEW native event plan and terminal queries to already cached states.

Standard-library transport only: no geometry, trajectory, Coax, SDK or NN calls.
Does not modify the original independent generator, data or execution receipt.
"""
from pathlib import Path
import argparse,hashlib,json,math,struct

ROOT=Path(__file__).resolve().parents[3]
FIXTURE=ROOT/'psionic/fixtures/robotics/panda-pose-solve'
EXPERIMENT_SHA='45014fef818614becc72cfca4ea4adf6702bd588848fc2141181394b358dd598'
ANSWER=struct.Struct('<q17d2q')
def require(ok,message):
    if not ok:raise RuntimeError(message)
def identity(path):
    raw=Path(path).read_bytes();return dict(bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest())
def write(path,value):path.write_text(json.dumps(value,indent=2)+'\n')
def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--reference',type=Path,required=True);parser.add_argument('--output',type=Path,required=True);args=parser.parse_args();reference=args.reference.resolve();out=args.output.resolve();require(not out.exists(),'preserve earlier binding');out.mkdir(parents=True);record=dict(schema='psionic.panda-pose-solve.terminal-binding.v1',passed=False,trajectory_regenerated=False,geometry_calls=0,Coax_calls=0,SDK_calls=0,NN_calls=0)
    try:
        receipt=json.loads((reference/'reference.json').read_text());e=json.loads((FIXTURE/'experiment.json').read_text());require(receipt['passed']and receipt['experiment']==identity(FIXTURE/'experiment.json')and receipt['experiment']['sha256']==EXPERIMENT_SHA,'frozen original new reference');require(all(identity(reference/name)==value for name,value in receipt['outputs'].items()),'every cached original artifact exact');require(all(identity(ROOT/name)['sha256']==value for name,value in receipt['source_sha256'].items()),'every original executed source exact');indices=json.loads((reference/'trajectory-index.json').read_text());require(len(indices)==18 and [x['context']for x in indices]==list(range(18))and len(e['contexts'])==18,'complete new index');require(receipt['total_queries']==112 and receipt['trajectory_steps']==279,'frozen first integration inventory');terminal=bytearray();plan=[];next_event=112;offset=0
        for index,row in enumerate(indices[:14]):
            values=row['terminal_values'];require(len(values)==17 and all(math.isfinite(v)for v in values),'ALL cached terminal fields finite before transport');require(row['offset_rows']==offset and 0<=row['rows']<=96 and row['rows']==row['terminal_updates'],'exact contiguous step offsets/counts');require(e['contexts'][index]['index']==index,'same new physical context');status=0 if row['terminal_converged']else 7;payload=values if status==0 else[0.]*17;terminal+=ANSWER.pack(status,*payload,0,0)
            plan.append(dict(context=index,name=row['name'],input_f64=e['contexts'][index]['input_f64'],offset_records=offset,count_records=row['rows'],first_step_event=next_event,terminal_event=next_event+row['rows'],terminal_status=status,terminal_record_index=index));next_event+=row['rows']+1;offset+=row['rows']
        require(offset==279 and next_event==405 and len(terminal)==14*160,'complete279step14terminal405event inventory');(out/'terminal-expected.bin').write_bytes(terminal)
        native_plan=dict(schema='psionic.panda-pose-solve.native-plan.v1',frozen_before_native_product_comparison=True,experiment=identity(FIXTURE/'experiment.json'),reference=identity(reference/'reference.json'),queries=identity(reference/'queries.bin'),query_expected=identity(reference/'query-expected.bin'),trajectory_expected=identity(reference/'trajectory-expected.bin'),trajectory_index=identity(reference/'trajectory-index.json'),terminal_expected=identity(out/'terminal-expected.bin'),query_count=112,physical_context_count=14,step_event_count=279,terminal_event_count=14,event_count=405,event_record='4I64(case,kind,stage,status),17F64,2I64(updates,projected)=184bytes',ordering='kind0 all112 queries in queries.json order, then each physical context increasing: kind1 steps1..N followed by kind2 terminal budget0; no adjacency assumptions for signs',failed_payload='Canonical transport zero ONLY after native checks complete public output/spare/input/canary preservation. It is not a published zero output.',sign_pairs='Explicit(query.context,query.max_updates,query.sign) metadata; target sign±1, identical seed; derived rows may have other budgets.',alias_success_query_index=52,alias_case='dense moderateB,positive sign,max96,9updates,0projections',budgets=receipt['budgets'],trajectories=plan)
        write(out/'native-plan-v1.json',native_plan);record.update(passed=True,reference=identity(reference/'reference.json'),source={str(Path(__file__).resolve().relative_to(ROOT)):identity(__file__)},all_original_source_sha256=receipt['source_sha256'],cached_terminal_finite_words=14*17,terminal_records=14,terminal_successes=sum(x['terminal_status']==0 for x in plan),terminal_refusals=sum(x['terminal_status']!=0 for x in plan),outputs={name:identity(out/name)for name in ['terminal-expected.bin','native-plan-v1.json']})
    except BaseException as error:record['failure']=repr(error);raise
    finally:write(out/'terminal-binding-v1.json',record)
    print(json.dumps(record))
if __name__=='__main__':main()
