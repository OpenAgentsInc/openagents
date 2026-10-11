#!/usr/bin/env python3
"""Encode only a frozen independent corpus; no model loading or execution."""
import argparse
from dataclasses import asdict
import hashlib
import json
import os
import sys
from pathlib import Path
from common import CHECKPOINT,HERE,ROOT,canonical,checkpoint_identity,load_corpus,load_official,require,sha,write


def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--corpus',type=Path,required=True);p.add_argument('--output',type=Path,required=True);a=p.parse_args()
    require(os.environ.get('CLEF_ACTIVATION_GUARDED')=='1','requires bounded guard')
    require(not a.output.exists(),'preserve earlier encoding evidence')
    rows,manifest=load_corpus(a.corpus);identity=checkpoint_identity()
    import torch,transformers
    torch.set_num_threads(2);torch.set_num_interop_threads(1);torch.set_default_device('cpu')
    require(str(torch.__version__)=='2.9.1' and transformers.__version__=='5.10.2','runtime versions differ')
    official=load_official();processor=transformers.AutoProcessor.from_pretrained(CHECKPOINT,local_files_only=True)
    # This hash-only overlap check never loads any reference model outputs.
    acceptance=ROOT/'crates/psionic/fixtures/clef/e2e/requests.jsonl'
    sys.path.insert(0,str(ROOT/'crates/psionic/fixtures/clef/tools'))
    from e2e import Raw,dump
    def sorted_tree(value):
        if isinstance(value,dict):return {k:sorted_tree(value[k]) for k in sorted(value)}
        if isinstance(value,list):return [sorted_tree(x) for x in value]
        return value
    def overlap_hash(value):return hashlib.sha256(dump(sorted_tree(value)).encode()).hexdigest()
    blocked_requests=set();blocked_states=set()
    for line in acceptance.read_text().split('\n'):
        if not line.strip():continue
        item=json.loads(line,parse_float=Raw,parse_int=Raw)
        body=item.get('request',item)
        blocked_requests.add(overlap_hash(body))
        if 'state' in body:blocked_states.add(overlap_hash(body['state']))
    encoded=[]
    for row in rows:
        require(overlap_hash(row['request']) not in blocked_requests,'exact acceptance request overlap')
        require(overlap_hash(row['request']['state']) not in blocked_states,'exact acceptance state overlap')
        record=official.encode_record(processor.tokenizer,row['request'],processor=processor)
        require(record.media is None and 0<len(record.input_ids)<=1024,'record exceeds text-only token bound: '+row['id'])
        encoded.append({'id':row['id'],'partition':row['partition'],'request_sha256':row['request_sha256'],
                        'tokens':len(record.input_ids),'record':asdict(record),'record_sha256':hashlib.sha256(canonical(asdict(record))).hexdigest()})
    report={'status':'prepared_not_model_executed','corpus_sha256':manifest['corpus_sha256'],'corpus_manifest_sha256':sha(a.corpus/'manifest.json'),
            'checkpoint':identity,'official_source_sha256':sha(CHECKPOINT/'joint_schema_model.py'),'tokenizer_sha256':sha(CHECKPOINT/'tokenizer.json'),
            'script_sha256':sha(__file__),'records':encoded,'exact_acceptance_request_or_state_overlaps':0,'acceptance_outputs_read':False,
            'overlap_checked_request_file_sha256':sha(acceptance),'overlap_numeric_semantics':'Preserved numeric spellings with official Raw/dump helper; object keys sorted; Unicode separators remain within JSON lines.','overlap_helper_sha256':sha(ROOT/'crates/psionic/fixtures/clef/tools/e2e.py'),'model_loaded':False,'gpu_executed':False}
    write(a.output,report);print(json.dumps({'status':report['status'],'records':len(encoded),'tokens_min':min(r['tokens'] for r in encoded),'tokens_max':max(r['tokens'] for r in encoded)}))


if __name__=='__main__':main()
