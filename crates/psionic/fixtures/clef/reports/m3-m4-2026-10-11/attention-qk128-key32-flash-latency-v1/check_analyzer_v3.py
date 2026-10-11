"""Validate offline bookkeeping and each engine's documented confidence rules."""
from pathlib import Path
import copy,hashlib,importlib.util,json
D=Path(__file__).resolve().parent
h=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
spec=importlib.util.spec_from_file_location('analyze_v3',D/'analyze_v3.py');m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
original={str(p.relative_to(D)):h(p) for p in [D/'run-receipt.json',D/'analyze.py',D/'analyze_v2.py',D/'analyzer-v2-failure.json',D/'run-1/records.jsonl']}
frozen=json.loads((D/'preparation-manifest.json').read_text());assert all(h(D/name)==digest for name,digest in frozen.items())
rows=[json.loads(l) for l in (D/'run-1/records.jsonl').read_text().splitlines() if l];requests=[r for r in rows if r['kind']=='request']
a=next(r for r in requests if r['variant']=='ollama');native=next(r for r in requests if r['variant']=='stage6-key32');body=json.loads(Path(a['workload']).read_text());checks=[]
m.validate_ollama_response(a['response'],body);checks.append('original Ollama entropy confidence accepted')
m.image_parity.validate_response(native['response'],body);checks.append('original native maximum-probability confidence accepted')
def refused(name,response,validator=m.validate_ollama_response):
 try:validator(response,body)
 except (ValueError,KeyError):checks.append(name+' refused')
 else:raise AssertionError(name+' accepted')
for name,edit in [
 ('Ollama maximum-probability confidence',lambda r:r['answers']['team'].update(confidence=max(r['answers']['team']['probabilities'].values()))),
 ('nonfinite confidence',lambda r:r['answers']['team'].update(confidence=float('nan'))),
 ('wrong top answer',lambda r:r['answers']['team'].update(choice='sales')),
 ('missing probability',lambda r:r['answers']['team']['probabilities'].pop('sales')),
 ('wrong score expectation',lambda r:r['answers']['severity'].update(score=0.0)),
 ('wrong question set',lambda r:r['answers'].pop('team')),
 ('cached answer',lambda r:r.update(cached=True))]:
 b=copy.deepcopy(a['response']);edit(b);refused(name,b)
b=copy.deepcopy(native['response']);b['answers']['team']['confidence']=a['response']['answers']['team']['confidence'];refused('native entropy confidence',b,m.image_parity.validate_response)
T=D/'analyzer-v3-selftest';T.mkdir(exist_ok=False);actual=T/'binary';actual.write_bytes(b'fixed tiny binary fixture\n');alias=T/'alias';alias.symlink_to(actual.name);digest=h(actual)
assert m.canonical_binary_hashes({str(alias):digest})=={str(actual.resolve()):digest};checks.append('binary symlink exact identity accepted')
for name,pins,exception in [('wrong binary hash',{str(alias):'0'*64},ValueError),('duplicate canonical identity',{str(alias):digest,str(actual):digest},ValueError),('missing binary',{str(T/'missing'):digest},FileNotFoundError)]:
 try:m.canonical_binary_hashes(pins)
 except exception:checks.append(name+' refused')
 else:raise AssertionError(name+' accepted')
result=m.analyze(D/'run-1')
assert result['structural_validation_passed'] and result['native_uncached_validation_passed']
assert result['passed'] is False and result['all_engine_uncached_comparison_passed'] is False
assert result['ollama_top_level_cached_count_histogram']=={'0':5,'37':14,'38':14,'39':3}
assert result==json.loads((D/'run-1/analysis-v3.json').read_text())
checks.append('all-engine uncached gate remains failed with exact disclosed cache histogram')
assert all(h(D/name)==digest for name,digest in original.items()) and all(h(D/name)==digest for name,digest in frozen.items())
pairs={}
for r in requests:
 if r['variant']!='ollama':pairs.setdefault((r['round'],r['workload'],r['sample']),{})[r['variant']]=r
exact=sum(p['stage6-key32']['response']['answers']==p['candidate-qk128-key32']['response']['answers'] for p in pairs.values())
receipt={'selftests_passed':True,'checks':checks,'models_launched':0,'original_evidence_unchanged':original,'source_sha256':{n:h(D/n) for n in ['analyze_v3.py','check_analyzer_v3.py']},'analysis_sha256':h(D/'run-1/analysis-v3.json'),'structural_validation_passed':True,'native_uncached_validation_passed':True,'all_engine_uncached_comparison_passed':False,'native_paired_responses':len(pairs),'native_answers_exact_equal_pairs':exact}
(D/'analyzer-v3-receipt.json').write_text(json.dumps(receipt,indent=2)+'\n');print(json.dumps(receipt,indent=2))
