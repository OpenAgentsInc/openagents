#!/usr/bin/env python3
# Firm reason: 2 (independent source/artifact identity and qualification receipt oracle).
from pathlib import Path
import datetime,hashlib,json,subprocess
ROOT=Path('/tmp/openagents-12625-complete-20261101').resolve()
BASE=ROOT/'docs/verification/coax-12625-closure-2026-11-02'
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def read(p):return json.loads((BASE/p).read_text())
native=read('final-native-v6/record.json');assert sha(Path(native['compiler']))==native['compiler_sha256'];assert native['source_pins_unchanged'] and native['build']['all_passed'] and native['fixedpoints']['all_c_and_headers_identical']
assert all(x['returncode']==0 and not x['timed_out'] for x in native['native_gates'])
pins=read('final-native-v6/source-pins.json')['source_pins'];assert len(pins)==496 and all(sha(ROOT/p)==h for p,h in pins.items())
gate=read('final-native-v6/qualification/record.json');assert gate['passed'] and all(sha(ROOT/p)==h for p,h in gate['test_input_pins'].items())
seed=read('final-native-v6/seed-refresh.json');assert seed['applied'] and all(sha(ROOT/r['path'])==r['after_sha256'] for r in seed['files'])
authored=read('focused-v6-final/record.json');assert authored['passed'] and authored['inputs_unchanged']
assert '1256 checks, 1256 passed, 0 failed' in (BASE/'focused-v6-final/stdout.log').read_text()
for p,h in authored['inputs_after'].items():
 path=Path(p)
 if path.is_relative_to(ROOT):assert sha(path)==h
origins=read('origins/record.json');assert sha(ROOT/'coax/lib/compiler/checker.coax')==origins['shared_checker_sha256'];query=origins['final_query'];assert query['killed']==query['mutants']==371 and query['survived']==0 and query['marked_equivalent']==0 and query['stock_merged_records']==4
assert sha(ROOT/'coax/tests/results-origin-policy.coax')==origins['origin_policy_sha256']
projection=read('projection/record.json');assert projection['result']['killed']==projection['result']['mutants']==286 and projection['result']['survived']==0
for p,h in projection['pins'].items():assert sha(ROOT/p)==h
caches=read('caches/record.json');assert caches['status']=='complete' and caches['index']['killed']==138 and caches['index']['survivors']==0 and caches['literal']['killed']==73 and caches['literal']['survivors']==0
for p,h in caches['production_pins'].items():assert sha(ROOT/'coax/lib/compiler'/p)==h
assert sha(ROOT/caches['external_oracles'])==caches['external_oracles_sha256']
cost=read('full-compiler-v6/process.json');assert cost['outcome']=='observed-success' and cost['compiler_exited_zero'] and cost['inputs_unchanged']
work=[r['counters']['work'] for r in cost['rows'] if r['event']=='CR_run-return'][-1];assert work==1036428 and cost['header']['bytes']>0
consumer=read('consumer-v6/qualification.json');assert consumer['status']=='passed' and consumer['inputs_unchanged'] and consumer['native_inputs_unchanged'] and consumer['prior_receipts_unchanged'] and all(x['equal'] for x in consumer['artifact_equivalence']) and len(consumer['artifact_equivalence'])==11
for p,h in consumer['fresh_native_inputs'].items():assert sha(Path(p))==h
for name,n in [('typed-mutation.json',32),('owner-policy-mutation.json',34)]:
 r=read('consumer-v6/'+name);assert r['verdict']=='passed' and r['mutation']['killed']==n and not r['mutation']['survived'] and r['mutation']['marked_equivalent']==0
book='book-final-post-rebase' if (BASE/'book-final-post-rebase').exists() else 'book-final'
reader=read(book+'/record.json');assert all(r['exit_status']==0 and not r['timeout'] for r in reader['commands'] if r['name']!='tools')
assert '1 new without a reason' in (BASE/book/'tools.stdout').read_text() and 'psionic/tests/check_deepseek_recipe.sh' in (BASE/book/'tools.stdout').read_text()
metadata=read('release-metadata-v6/record.json');assert metadata['returncode']==0 and not metadata['timeout']
files={str(p.relative_to(BASE)):{'sha256':sha(p),'bytes':p.stat().st_size} for p in sorted(BASE.rglob('*')) if p.is_file() and p!=BASE/'record.json'}
result={'schema':'coax.12625.closure.v1','observed_utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'status':'passed-focused-issue-scope','issue':12625,'source_base_at_qualification':'92d8efaafdc9d7672960300341cb170782cd8303','latest_observed_main':subprocess.check_output(['git','rev-parse','origin/main'],cwd=ROOT,text=True).strip(),'compiler_sha256':native['compiler_sha256'],'all_496_current_compiler_sources_match':True,'all_native_gate_inputs_match':True,'all_canonical_seeds_match':True,'limits':{'work':1048576,'depth':32,'path':8,'origin':64},'matching_full_compiler':{'work':work,'spare':1048576-work,'record':'full-compiler-v6/process.json'},'authored_checks':{'checks':1256,'passed':1256,'failed':0,'record':'focused-v6-final/record.json'},'focused_mutation_records':['caches/record.json','projection/record.json','origins/record.json'],'query':query,'native_record':'final-native-v6/record.json','consumer_record':'consumer-v6/qualification.json','reader_record':book+'/record.json','normal_release_provenance_promoted':False,'full_mutation_sweep_run':False,'equivalent_waivers':False,'independent_remaining':['Whole desktop ninth-path-step E0381; component adoption is separately qualified.','Broader ownership/R2/target forms remain outside the initial native LP64 profile.','Normal all-gates release/cache and platform distribution not promoted.','R0 physical-device/frame-budget/reviewer/page-golden gates remain independent.','Tracked-tools policy retains upstream check_deepseek_recipe.sh missing reason header.'],'files':files}
(BASE/'record.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps({'status':result['status'],'files':len(files),'compiler':result['compiler_sha256'],'work':work,'spare':1048576-work}))
