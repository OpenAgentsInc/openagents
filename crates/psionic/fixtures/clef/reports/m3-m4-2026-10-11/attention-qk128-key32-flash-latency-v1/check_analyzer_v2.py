"""Check canonical binary matching and analyze saved samples only."""
from pathlib import Path
import hashlib, importlib.util, json, time
D=Path(__file__).resolve().parent
h=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
spec=importlib.util.spec_from_file_location('analyze_v2',D/'analyze_v2.py');module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
frozen=json.loads((D/'preparation-manifest.json').read_text())
assert all(h(D/name)==digest for name,digest in frozen.items())
original_receipt=h(D/'run-receipt.json');original_records=h(D/'run-1/records.jsonl')
T=D/'analyzer-v2-selftest';T.mkdir(exist_ok=False)
actual=T/'binary';actual.write_bytes(b'fixed tiny binary fixture\n');alias=T/'alias';alias.symlink_to(actual.name)
digest=h(actual);checks=[]
assert module.canonical_binary_hashes({str(alias):digest})=={str(actual.resolve()):digest};checks.append('symlink resolves to exact pinned hash')
for name,pins,exception in [('wrong_hash',{str(alias):'0'*64},ValueError),('duplicate_canonical',{str(alias):digest,str(actual):digest},ValueError),('missing_binary',{str(T/'missing'):digest},FileNotFoundError)]:
 try:module.canonical_binary_hashes(pins)
 except exception:checks.append(name+' refused')
 else:raise AssertionError(name+' was accepted')
start=time.monotonic();result=module.analyze(D/'run-1')
assert result['passed']
assert h(D/'run-receipt.json')==original_receipt and h(D/'run-1/records.jsonl')==original_records
assert all(h(D/name)==digest for name,digest in frozen.items())
(D/'run-1/analysis-v2.json').write_text(json.dumps(result,indent=2)+'\n')
receipt={'passed':True,'selftests':checks,'seconds':time.monotonic()-start,'analyzer_sha256':h(D/'analyze_v2.py'),'checker_sha256':h(D/'check_analyzer_v2.py'),'original_failed_run_receipt_sha256':original_receipt,'original_frozen_analyzer_sha256':h(D/'analyze.py'),'unchanged_records_sha256':original_records,'result_sha256':h(D/'run-1/analysis-v2.json'),'original_preparation_unchanged':True,'models_launched':0}
(D/'analyzer-v2-receipt.json').write_text(json.dumps(receipt,indent=2)+'\n');print(json.dumps(receipt,indent=2))
for g in result['groups']:print(json.dumps(g))
