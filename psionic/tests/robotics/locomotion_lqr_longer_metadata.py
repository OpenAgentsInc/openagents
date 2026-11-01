#!/usr/bin/env python3
"""Seal additive continuation metadata and byte/AST carry; no numeric oracle run."""
from __future__ import annotations
import argparse
import ast
import hashlib
import json
from pathlib import Path
import struct

ROOT=Path(__file__).resolve().parents[3]
OLD=ROOT/'psionic/fixtures/robotics/locomotion-lqr'
OUT=ROOT/'psionic/fixtures/robotics/locomotion-lqr-longer'
SOURCE=Path(__file__).resolve()
CONTRACT_SHA='7d5acb6089b4f2017dfba36b1509e4f7fad178fcdb6f0841832ec88ca3185057'
OLD_AUDITOR_SHA='da8f9503c88df82dfe3dcc16e931070c209ef47a83353e8e3d5e57c874d31aea'
OLD_MATH_SHA='c105189fbe2f539d52fca631c722d532798fb10789e7237218edbaac142e8ec9'

def need(ok,message):
    if not ok:raise ValueError(message)

def identity(path):
    b=Path(path).read_bytes();return {'bytes':len(b),'sha256':hashlib.sha256(b).hexdigest()}

def load(path):return json.loads(Path(path).read_text())
def write(path,value):Path(path).write_text(json.dumps(value,indent=2,allow_nan=False)+'\n')
def relative(path):return str(Path(path).relative_to(ROOT))

def function(source,name):
    tree=ast.parse(source);node=next(n for n in tree.body if isinstance(n,(ast.FunctionDef,ast.AsyncFunctionDef)) and n.name==name)
    return ast.get_source_segment(source,node)+'\n'

def carried_numeric_binding():
    a=ROOT/'psionic/tests/robotics/locomotion_lqr_learning_audit.py';b=ROOT/'psionic/tests/robotics/locomotion_lqr_longer_audit.py';math=ROOT/'psionic/tests/robotics/locomotion_lqr_reference.py'
    need(identity(a)['sha256']==OLD_AUDITOR_SHA,'original da8 auditor unchanged');need(identity(math)['sha256']==OLD_MATH_SHA,'original c105 oracle unchanged')
    old=a.read_text();new=b.read_text();exact=[];wire=[]
    for name in ('need','identity','load','write','bits','finite','math_kernel','header','decode_state','close'):
        need(function(old,name)==function(new,name),'exact carried declaration '+name);exact.append(name)
    reverse={'0x3153584c':'0x3153514c','0x3149584c':'0x3149514c','0x3143584c':'0x3143514c','0x3145584c':'0x3145514c','0x3147584c':'0x3147514c','LXS1':'LQS1','LXI1':'LQI1','LXC1':'LQC1','LXE1':'LQE1','LXG1':'LQG1'}
    def old_wire(s):
        for x,y in reverse.items():s=s.replace(x,y)
        return s
    for name in ('snapshot','decode_iterations','decode_coefficients'):
        need(function(old,name)==old_wire(function(new,name)),'only explicit wire magic/label change '+name);wire.append(name)
    old_chunk=function(old,'audit_chunk');new_chunk=function(new,'audit_chunk');marker='    # EVERY coefficient'
    need(old_chunk[old_chunk.index(marker):]==new_chunk[new_chunk.index(marker):],'WHOLE numeric chunk suffix byte-exact')
    old_eval=function(old,'audit_evaluation');new_eval=old_wire(function(new,'audit_evaluation'));marker='    raw=header(args.evaluations'
    need(old_eval[old_eval.index(marker):]==new_eval[new_eval.index(marker):],'WHOLE numeric evaluation suffix byte-exact')
    replay=old_wire(function(new,'audit_replay'))
    replay=replay.replace("    need(canonical['inputs']['descriptor']==identity(args.descriptor),'canonical replay same context')\n",'')
    replay=replay.replace('256','64').replace('257','65')
    need(function(old,'audit_replay')==replay,'replay only declared batch labels/context guard')
    prior=function(new,'admit_previous').replace('psionic.locomotion-lqr-longer.learning-audit.v1','psionic.locomotion-lqr.learning-audit.v1')
    need(function(old,'admit_previous')==prior,'prior chain only new namespace')
    return {'schema':'psionic.locomotion-lqr-longer.numeric-carry.v1','passed':True,'old_auditor':identity(a),'new_auditor':identity(b),'whole_carried_math_producer':identity(math),'exact_declarations':exact,'wire_magic_and_labels_only':wire,'numeric_chunk_suffix_byte_exact':True,'numeric_evaluation_suffix_byte_exact':True,'replay_only_batch256_labels_and_added_context_guard':True,'prior_audit_only_new_namespace':True,'original_29_reader_and_19_lineage_tests_repeated':0,'original_math_or_expected_generation_calls':0,'product_learner_optimizer_RNG_SDK_calls':0}

def freeze(contract):
    need(not OUT.exists(),'preserve FIRST continuation freeze');need(identity(contract)['sha256']==CONTRACT_SHA,'sealed final contract')
    OUT.mkdir(parents=True);(OUT/'contract.md').write_bytes(contract.read_bytes())
    original=load(OLD/'experiment.json');top=load(OLD/'learning-execution/record.json');old_descriptor=identity(OLD/'learning-execution/descriptor.json')
    need(top['passed'] is True and top['lifecycle_state']=='complete','original completed captured prefix')
    resume={};inputs={}
    def bind(path):inputs[relative(path)]=identity(path)
    names=['experiment.json','contract.md','input-freeze.json','reference.json','native-qualification.json','source-qualification.json','unit-refusals.json','data-final-publication.json','learning-execution/descriptor.json','learning-execution/record.json','independent-learning/publication.json','independent-learning/summary.json','independent-learning/curves.json','independent-learning/curves.csv','learning-execution/evaluation/baseline-evaluations.bin','learning-execution/evaluation/baseline-quality.bin','independent-learning/audits/evaluation-baseline.json']
    for name in names:bind(OLD/name)
    for name in ['locomotion_lqr_learning_audit.py','locomotion_lqr_reference.py']:bind(ROOT/'psionic/tests/robotics'/name)
    for seed in (0,1,7):
        snapshot=OLD/f'learning-execution/checkpoints/final-{seed}.bin';audit=OLD/f'independent-learning/audits/canonical-seed-{seed}-batch-112.json';bind(snapshot);bind(audit)
        q=load(audit);last=next(x for x in top['chunks'] if x['seed']==seed and x['first_batch']==112)
        need(last['artifacts']['checkpoint.bin']==identity(snapshot) and last['audit']['record']==identity(audit),'original top-to-endpoint/audit exact linkage')
        need(q['passed'] is True and q['inputs']['after']==identity(snapshot) and q['inputs']['descriptor']==old_descriptor,'original independent endpoint/context')
        need(q['auditor']['sha256']==OLD_AUDITOR_SHA and q['result']['seed']==seed and q['result']['first_batch']==112 and q['result']['batches']==16,'original auditor/last chunk identity')
        raw=snapshot.read_bytes();need(len(raw)==488 and struct.unpack_from('<4I',raw)==(0x3153514c,1,440,0) and raw[16:48]==bytes.fromhex(old_descriptor['sha256']),'original wrapper profile/context')
        remapped=bytearray(raw[48:])
        for w in range(16):struct.pack_into('<Q',remapped,40+w*24+16,131072+w*24576)
        resume[str(seed)]={'snapshot_path':relative(snapshot),'snapshot':identity(snapshot),'audit_path':relative(audit),'audit':identity(audit),'remapped_raw':{'bytes':440,'sha256':hashlib.sha256(remapped).hexdigest()},'old_batch':128,'old_Adam_step':512,'counter_word_offsets':[40+w*24+16 for w in range(16)],'new_counter_starts':[131072+w*24576 for w in range(16)]}
    training=dict(original['training']);training.update(batches=512,first_batch=128,additional_batches=384,counter='131072+world*24576+(batch-128)*64',canonical_steps=589824,canonical_updates=4608,deterministic_eval_steps=2304,replay_after_completed_batches=256,replay_zero_based_batch=256)
    training['all_seed_goals']=dict(original['training']['all_seed_goals'],complete_batches=512)
    actions=[{'kind':'import','seed':s} for s in (0,1,7)]+[{'kind':'canonical','seed':s,'first_batch':b,'batches':16} for s in (0,1,7) for b in range(128,512,16)]+[{'kind':'replay','seed':s,'first_batch':256,'batches':1} for s in (0,1,7)]+[{'kind':'evaluation','label':f'seed-{s}','seed':s} for s in (0,1,7)]
    need(len(actions)==81,'exact bounded action inventory')
    e={'schema':'psionic.locomotion-lqr-longer.experiment.v1','frozen_before_import_learning':True,'contract':identity(OUT/'contract.md'),'original_experiment':identity(OLD/'experiment.json'),'original_learning_record':identity(OLD/'learning-execution/record.json'),'original_descriptor':old_descriptor,'original_auditor':identity(ROOT/'psionic/tests/robotics/locomotion_lqr_learning_audit.py'),'original_math_producer':identity(ROOT/'psionic/tests/robotics/locomotion_lqr_reference.py'),'original_independent_reference':identity(OLD/'reference.json'),'original_quality_misses_preserved':True,'metadata_inputs':inputs,'resume_sources':resume,'training':training,'original_training_verbatim':original['training'],'unchanged_domains':original['domains'],'unchanged_budgets':original['budgets'],'actions':actions,'new_workload':{'imports':3,'canonical_chunks':72,'canonical_plant_steps':589824,'canonical_AdamW_updates':4608,'canonical_tape_pairs':36864,'canonical_coefficients':2359296,'replay_plant_steps':1536,'replay_AdamW_updates':12,'replay_tape_pairs':96,'replay_coefficients':6144,'final_deterministic_eval_steps':2304,'old_prefix_plant_steps_repeated':0,'baseline_eval_steps_repeated':0,'old_expected_or_reader_tests_repeated':0},'counter_reservations':{'original_consumed_half_open':[0,131072],'new_world_consumed_half_open':[[131072+w*24576,131072+(w+1)*24576] for w in range(16)],'batch512_endpoints_never_consumed':True,'same_seed_keys':True},'wire':{'header':'<4I +32B descriptorSHA','snapshot':[0x3153584c,1,440,0],'iteration':[0x3149584c,1,62696,16],'coefficient':[0x3143584c,1,88,32768],'evaluation':[0x3145584c,1,72,6],'quality':[0x3147584c,1,200,1],'coefficient_scope':'source-bound RECOMPUTED observations, not internal instrumentation'},'failure_policy':'Stop and preserve first failure/partial output; no retry, tuning, early stop or best checkpoint/seed selection.','quality_scope':'Same fixed all-seed goals, nominal continuous-Gaussian analytic critic and finite-top53 sampler; no guarantee that added budget meets goals.','scope':'Additive continuation only; original 128-batch outcomes, all old sources and curves stay byte-exact. No next chapter or model/SDK/hardware effects.'}
    write(OUT/'experiment.json',e);carry=carried_numeric_binding();write(OUT/'numeric-carry.json',carry)
    record={'schema':'psionic.locomotion-lqr-longer.input-freeze.v1','passed':True,'producer':identity(SOURCE),'auditor':identity(ROOT/'psionic/tests/robotics/locomotion_lqr_longer_audit.py'),'experiment':identity(OUT/'experiment.json'),'contract':identity(OUT/'contract.md'),'numeric_carry':identity(OUT/'numeric-carry.json'),'new_resume_rows':3,'original_expected_or_reader_generation_calls':0,'product_learner_optimizer_RNG_SDK_calls':0}
    write(OUT/'input-freeze.json',record);print(json.dumps(record))

def main():
    p=argparse.ArgumentParser();p.add_argument('--freeze',action='store_true');p.add_argument('--contract',type=Path);args=p.parse_args()
    need(args.freeze and args.contract is not None,'explicit data-only freeze arguments');freeze(args.contract)

if __name__=='__main__':main()
