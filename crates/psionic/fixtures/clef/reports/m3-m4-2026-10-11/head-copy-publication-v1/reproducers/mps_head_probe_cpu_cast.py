#!/usr/bin/env python3
"""Replay only the official BF16 head on saved inputs; never load its trunk."""
import json
import os
from pathlib import Path
import sys
import time
sys.path.insert(0,str(Path(__file__).resolve().parent))
import head_capture as capture
from mac_quiet_bench import file_sha

if not {'gpu','memory'} <= set(os.environ.get('OPENAGENTS_LEASES','').split(',')):
    raise SystemExit('requires GPU+memory leases and an external owned-process guard')
output=Path(sys.argv[1])
if output.exists():
    raise SystemExit('output exists; preserve the previous probe')
checkpoint=capture.BASE/'artifacts/clef-bf16'
inputs=capture.BASE/'reference27b/head-inputs-014-034'
metadata=json.loads((inputs/'run.json').read_text())
if file_sha(checkpoint/'joint_head.safetensors')!=metadata['head_sha256']:
    raise SystemExit('head weights changed')
torch,official=capture.setup(checkpoint)
from safetensors.torch import load_file
torch.mps.set_per_process_memory_fraction(min(1.0,1024**3/torch.mps.recommended_max_memory()))
head=official.JointSchemaHead(**json.loads((checkpoint/'joint_head_config.json').read_text()))
state=load_file(str(checkpoint/'joint_head.safetensors'))
head.load_state_dict(state,strict=True)
del state
head=head.to(dtype=torch.bfloat16)
head=head.to(device='mps').eval()
report={'kind':'official_bf16_head_only_mps_probe','torch':str(torch.__version__),
        'trunk_loaded':False,'head_sha256':metadata['head_sha256'],
        'capture_status':metadata['status'],'diagnostic_only':True,'cases':[]}
began=time.monotonic()
for case in capture.CASES:
    row=json.loads((inputs/(case+'.json')).read_text())
    before=inputs/row['before']['file']
    if file_sha(before)!=row['before']['sha256']:
        raise ValueError('saved head inputs changed')
    tensors=load_file(str(before))
    record=row['records_before'][0]
    questions=tuple(official.EncodedQuestion(q['question_id'],q['question_type'],tuple(q['question_span']),
        tuple(tuple(s) for s in q['option_spans']),tuple(q['option_ids'])) for q in record['questions'])
    encoded=official.EncodedRecord(input_ids=tuple(tensors['remapped_input_ids'][0].tolist()),
        questions=questions,record_id=record['record_id'])
    for repeat in range(2):
        gpu={name:value.to('mps') for name,value in tensors.items() if name in
             ['hidden','remapped_input_ids','attention_mask','lexical']}
        sentinel=torch.ones((1,1031),dtype=torch.long,device='mps')
        snapshots={name:value.detach().cpu().clone() for name,value in gpu.items()}
        with torch.inference_mode():
            result=head(gpu['hidden'],gpu['remapped_input_ids'],gpu['attention_mask'],[encoded],gpu['lexical'])[0]
        # Take the exact same-dtype input snapshots before any logit conversion.
        after={name:value.detach().cpu().clone() for name,value in gpu.items()}
        same={name:torch.equal(snapshots[name],after[name]) for name in gpu}
        logits=[value.detach().cpu().clone().float().tolist() for value in result]
        details={'id':case,'repeat':repeat,'input_unchanged':same,
                 'sentinel_unchanged':bool((sentinel.cpu().clone()==1).all()),'logits':logits,
                 'max_abs_logit_vs_full_bf16':max(abs(x-y) for a,b in zip(logits,row['bf16_logits']) for x,y in zip(a,b))}
        if not same['attention_mask']:
            a=snapshots['attention_mask'].reshape(-1).tolist();b=after['attention_mask'].reshape(-1).tolist()
            details['mask_changes']=[{'index':i,'before':x,'after':y} for i,(x,y) in enumerate(zip(a,b)) if x!=y]
        report['cases'].append(details)
        output.write_text(json.dumps(report,indent=2,allow_nan=False)+'\n')
report.update(completed=True,elapsed_seconds=time.monotonic()-began)
output.write_text(json.dumps(report,indent=2,allow_nan=False)+'\n')
print(json.dumps({'cases':len(report['cases']),'input_corruptions':sum(not all(x['input_unchanged'].values()) for x in report['cases']),
                  'elapsed_seconds':report['elapsed_seconds']}),flush=True)
