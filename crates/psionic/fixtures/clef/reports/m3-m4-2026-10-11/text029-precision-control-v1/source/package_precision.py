"""Package completed precision control; never open weights or raw tensor files."""
from pathlib import Path
import importlib.util
import json

HERE=Path(__file__).resolve().parent
spec=importlib.util.spec_from_file_location('pub',HERE.parent/'publication/pack.py')
pub=importlib.util.module_from_spec(spec);spec.loader.exec_module(pub)
out=HERE/'precision-control-publishable-v1'
assert not out.exists()
run=HERE/'precision-2'
guard=json.loads((run/'guard.json').read_text())
receipt=json.loads((run/'results/receipt.json').read_text())
summary=json.loads((HERE/'precision-2-summary-complete.json').read_text())
assert guard['status']=='complete' and guard['exit_code']==0 and guard['sources_rechecked']
assert guard['memory']['owned_group_stopped'] and all(guard['memory'][k] is None for k in ('sampling_error','memory_rejection','cleanup_error'))
assert guard['result_sha256']==pub.sha((run/'results/receipt.json').read_bytes())
assert receipt['completed'] and len(receipt['layers'])==64 and not receipt['gpu_executed']
out.mkdir()
manifest={'schema':'clef.text029-precision-control-publication.v1','scope':receipt['scope'],
 'path_tokens':{token:meaning for (_,token),meaning in zip(pub.REPLACEMENTS,['Worktree root','Task scratch root','Pinned Python environment','Persistent Cargo target','User home'])},
 'files':[],'excluded':['model weights','raw tensors','executables','raw process memory samples'],
 'attempt1_manifest_sha256':pub.sha((HERE/'precision-failure-publishable-v1/manifest.json').read_bytes()),
 'attempt2_original_result_sha256':guard['result_sha256'],
 'acceptance':'Diagnostic only. Original independent-f32 40/104 all-top and max probability error 0.02 gate remains unchanged and unmet.'}
def save(name,raw):
 assert len(raw)<1_000_000 and (b'/' + b'Users/') not in raw,name
 path=out/name;path.parent.mkdir(parents=True,exist_ok=True);path.write_bytes(raw)
def copy(path,name,expected=None):
 raw=path.read_bytes()
 if expected is not None:assert pub.sha(raw)==expected,(path,expected)
 clean=pub.sanitize(raw);save(name,clean)
 manifest['files'].append({'path':name,'original_bytes':len(raw),'original_sha256':pub.sha(raw),'published_bytes':len(clean),'published_sha256':pub.sha(clean),'path_reconstruction_verified':True})
copy(run/'guard.json','guard.json')
copy(run/'results/receipt.json','receipt.json')
copy(run/'stdout.log','stdout.log')
for name in ('precision-2-summary.json','precision-2-summary-complete.json','precision-source-check.json','precision-source-check-repair1.json','precision-repair1.patch','precision-plan.md'):
 copy(HERE/name,name)
for name in ('precision_reference.py','guard_precision.py','precision_hash.py','layer0_math.py','layer35_math.py'):
 p=HERE/name;copy(p,'source/'+name,guard['sources'][str(p)])
for name in ('precision_summary.py','precision_summary_complete.py','package_precision.py'):
 copy(HERE/name,'source/'+name)
for name in ('mps_reference.py','guard_mps.py'):
 p=HERE.parent/name;copy(p,'source/dependencies/'+name,guard['sources'][str(p)])
for name in ('gguf_import.py','stream_reference.py'):
 p=HERE.parent.parent/'cpu27b-diagnostic/streamed-gguf'/name
 copy(p,'source/dependencies/'+name,guard['sources'][str(p)])
for name in ('mac_quiet_bench.py','mac_memory.py'):
 p=pub.ROOT/'crates/psionic/fixtures/clef/tools'/name
 copy(p,'source/dependencies/'+name,guard['sources'][str(p)])
for path,name in [(HERE/'precision-1/guard.json','attempt1/guard.json'),(HERE/'precision-1/results/receipt.json','attempt1/receipt.json'),(HERE/'precision-1/stdout.log','attempt1/stdout.log')]:copy(path,name)
readme='''# Full text029 precision controls

The fixed CPU diagnostic completed. It shows that projection precision can move the disputed decision across 0.5, but it does not reproduce every native operation or resolve the unchanged full-suite acceptance failure.

A is the saved independent CPU-f32 same-Q4 reference. B rounds every trunk `nn.Linear` weight and input through f16. C adds the reviewed original 32-key attention Q/K/V and half-exponential boundaries. Both retain the official chunk64 Delta operator, the same original GGUF, unchanged f32 head, exact 359 input tokens, question spans, option ordering, and lexical embeddings. Separate B=1 hidden states have no cache. This is one fixed diagnostic case, not a new acceptance reference.

## Results

The values below are P(true); P(false) is its complement. Complete logits, both probabilities, and tops are in `precision-2-summary-complete.json` and `receipt.json`.

| Question | A | B | C | Native staged | Native fused-fields |
| --- | ---: | ---: | ---: | ---: | ---: |
'''
for q in summary['questions']:
 readme+='| '+q['id']+' | '+' | '.join(f"{q['variants'][v]['probabilities'][0]:.12f} ({q['variants'][v]['top']})" for v in ('A','B','C','native_staged','native_fused_fields'))+' |\n'
readme+='''
B matches the three native tops, with maximum probability difference 0.00355583 from native staged. C is closer in maximum probability difference (0.00218395), but its disputed P(true) remains below 0.5 and its top differs. Selecting B on that basis would confuse a diagnostic control with acceptance. Neither result changes the original 40-request, 104-question all-top gate, which remains 103/104.

Final backbone-normalized memory relative L2 errors follow. Each denominator is the right-hand reference. The question spans tokens [192,194), true option [205,223), and false option [229,247). `receipt.json` also retains all 64 pre-final-normalization layer rows, hashes, and whole/question/option comparisons; do not conflate those with normalized final memory.

| Comparison | Whole memory | Question | True option | False option |
| --- | ---: | ---: | ---: | ---: |
'''
for name,row in summary['final_normalized_memory'].items():
 readme+='| '+name.replace('_minus_',' − ')+' | '+' | '.join(f"{row[v]['relative_l2']:.10f}" for v in ('whole','question','true_option','false_option'))+' |\n'
readme+='''
Projection rounding matters, but this does not establish a complete cause. CPU f32 GEMM and norm reductions, transcendental functions, fused multiply-add behavior, residual accumulation, and Delta order still differ from Metal. C's closer head probabilities do not imply uniformly closer hidden rows. No product fix or question-specific rule is proposed by this evidence.

## Checks and bounds

The saved A head replay reproduced hidden, lexical, and logit values exactly. B/C layer 0 matched the prior WX control bitwise, and both trajectories were identical before the first attention layer. Scalar/tensor byte-hash fixtures, attention and recurrence fixtures, finite values, input immutability, parameter bytes/versions, source hashes, and the original artifact snapshot passed. Complete hidden/memory/head-input arrays remain outside Git, identified by their hashes. Raw layer output totals 941,096,960 bytes.

The run took 203.784 seconds under a 300-second deadline, with two CPU threads, a 12 GiB sampled process-group cap, 24 GiB start requirement, 12 GiB available-memory floor, 2 GiB disk lease, and 25 GiB free-disk floor. Peak accounted memory was 5,238,472,704 bytes; minimum available memory was 94,505,287,680 bytes. Sampling reported no error or refusal. Owned-group cleanup succeeded and all leases were released. No GPU, quiet lease, build, desktop app, or system service was used or controlled.

The first attempt failed before a trunk forward because scalar head-parameter hashing attempted a rank-zero dtype view. Its sources and failure are preserved in the separately published failure bundle and linked by manifest hash here. The reviewed repair only flattens the hash byte view while preserving original shape metadata; no numerical operator or bound changed. Attempt 2 ran that hash self-check before loading weights.

The compact summary is derived from completed receipts and hash-verified saved rows with the Python standard library, without model execution. Its source and the earlier false-span-only summary are included. The full result receipt is below the publisher's one-megabyte file limit and has not been split or pruned.

## Identity and reproduction

The original GGUF SHA-256 is `9997675e4c9e9016273372b5fdedbdda7ad14285bc722949e4b9daca62bf3822`. Native comparison targets the previously recorded Stage6 HTTP server, SHA-256 `77c74a2cf4c737ae0b92278e7f8384a213dc53f1567963fb6b8c6d0dc7157991`, and its verified observer capture. It does not relabel results from a later attention build. Full immutable source/input identities and effective settings appear in `guard.json`.

Absolute account paths are replaced with tokens described in `manifest.json`; replacement was verified reversible against every original. The manifest records original and published hashes and sizes. Raw weights, arrays, binaries, and process-memory logs remain in scratch. Restoring the local path map and required pinned dependencies allows reproduction; this bundle alone does not contain model weights or every upstream dependency.
'''
save('README.md',readme.encode())
manifest['files'].append({'path':'README.md','published_bytes':len(readme.encode()),'published_sha256':pub.sha(readme.encode()),'generated_summary':True})
save('manifest.json',pub.encoded(manifest))
for entry in manifest['files']:assert pub.sha((out/entry['path']).read_bytes())==entry['published_sha256']
print(json.dumps({'output':str(out),'files':sum(x.is_file() for x in out.rglob('*')),'bytes':sum(x.stat().st_size for x in out.rglob('*') if x.is_file()),'largest_file_bytes':max(x.stat().st_size for x in out.rglob('*') if x.is_file()),'manifest_sha256':pub.sha((out/'manifest.json').read_bytes())},indent=2))
