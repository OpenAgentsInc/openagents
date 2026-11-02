#!/usr/bin/env python3
# Firm reason: 2 (independent emitted-artifact, source and receipt equality oracle).
"""Validate completed rebind stages after a temporary metadata-serialization typo."""
import hashlib,json,sys
from pathlib import Path
ROOT=Path('/tmp/openagents-12625-complete-20261101').resolve();OUT=Path('/tmp/b3-rebased-975b-final-v4').resolve()
sys.path.insert(0,str(ROOT/'coax/tests'))
from b2_support import closure,source_inventory

def sha(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()
original=OUT/'qualification.json';original_sha=sha(original);proof=json.loads(original.read_text())
assert proof['status']=='running'
assert {r['label'] for r in proof['runs']}=={'raw-owner-emit','typed-emit','lab-emit','factory-emit','entry-red-emit','entry-green-emit','typed-checks','typed-mutation','owner-policy-mutation','ignored-start','incomplete-terminal','local-snapshot'}
assert all(r['exit_status']==r['expected'] for r in proof['runs'])
assert len(proof['artifact_equivalence'])==11
for row in proof['artifact_equivalence']:
 assert row['equal'] and row['prior_sha256']==row['actual_sha256'] and sha(row['prior'])==row['prior_sha256']
 actual=OUT/row['profile']/row['artifact'];assert sha(actual)==row['actual_sha256'] and Path(row['prior']).read_bytes()==actual.read_bytes()
for name,identity in proof['artifacts'].items():assert sha(OUT/name)==identity
for name,code,line in [('ignored-start','E0907',2),('incomplete-terminal','E0904',3),('local-snapshot','E0383',4)]:
 data=[json.loads(row) for row in ((OUT/(name+'.stdout')).read_bytes()+(OUT/(name+'.stderr')).read_bytes()).splitlines() if row.strip()]
 assert len(data)==1 and data[0]['code']==code and data[0]['line']==line
checks=json.loads((OUT/'typed-checks.json').read_text());assert checks['exit_status']==0 and checks['verdict']=='passed'
assert len([row for row in checks['claims'] if row['kind'] in ['#test','#forall']])==42
assert all(row['outcome']=='passed' for row in checks['claims'])
for name,count in [('typed-mutation',32),('owner-policy-mutation',34)]:
 data=json.loads((OUT/(name+'.json')).read_text());m=data['mutation'];assert data['exit_status']==0 and m['mutants']==count and m['killed']==count and m['survived']==[] and m['marked_equivalent']==0
source_after=source_inventory({**closure(ROOT/'coax/tests/systemone-outcomes.coax'),**closure(ROOT/'apps/openagents-desktop/zeron/settings.coax')});assert source_after==proof['source_before']
for field in ['inputs_before','prior_receipts','fresh_native_inputs','fresh_native_tools']:
 assert all(sha(path)==h for path,h in proof[field].items())
for field in ['compiler','prior_compiler']:assert sha(proof[field]['path'])==proof[field]['sha256']
assert sha(original)==original_sha
for path in [Path('/tmp/b3-rebased-final-rebind.py'),Path(__file__)]:
 target=OUT/path.name;target.write_bytes(path.read_bytes())
(OUT/'metadata-failure-summary.txt').write_text('Temporary rebind script completed all 12 requested commands successfully and persisted their logs/artifact hashes. Its final metadata assignment accidentally used a tuple key for prior_receipts_unchanged, causing KeyError and then JSON TypeError during final serialization. Original qualification.json remains its last persisted running receipt. This summary is not a raw traceback. Independent b3-finalize-rebind.py verified all commands, exact diagnostics, 11 byte-equal C/header artifacts, both zero-survivor mutation reports, complete source/native/tool/receipt continuity and original receipt identity without repeating a command.\n')
proof.update(status='passed',source_after=source_after,inputs_unchanged=True,tools_unchanged=True,prior_receipts_unchanged=True,native_inputs_unchanged=True,native_reexecuted=False,finalization={'original_receipt':str(original),'original_receipt_sha256':original_sha,'original_run_final_metadata_failure':'tuple-key metadata typo after every substantive stage passed','finalizer':str(Path(__file__)),'finalizer_sha256':sha(Path(__file__)),'substantive_stages_reexecuted':False,'validation':'independently read back every C/header equality, stage exit, diagnostic, mutation inventory, source, native input, tool and prior receipt'})
proof['artifacts']={str(path.relative_to(OUT)):sha(path) for path in OUT.rglob('*') if path.is_file() and path.name!='finalized-qualification.json'}
final=OUT/'finalized-qualification.json';final.write_text(json.dumps(proof,indent=2)+'\n');print(json.dumps({'status':'passed','emitted_artifacts':11,'native_reexecuted':False,'compiler':proof['compiler'],'receipt':str(final)}))
