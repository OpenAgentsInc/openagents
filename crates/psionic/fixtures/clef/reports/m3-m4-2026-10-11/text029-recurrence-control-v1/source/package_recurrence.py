"""Package completed D evidence without opening model weights or raw arrays."""
from pathlib import Path
import importlib.util
import json
import math
HERE=Path(__file__).resolve().parent
spec=importlib.util.spec_from_file_location('pub',HERE.parent/'publication/pack.py')
pub=importlib.util.module_from_spec(spec);spec.loader.exec_module(pub)
run=HERE/'recurrence-1';out=HERE/'recurrence-control-publishable-v1'
g=json.loads((run/'guard.json').read_text());r=json.loads((run/'results/receipt.json').read_text())
assert g['status']=='complete' and g['exit_code']==0 and g['sources_rechecked']
assert g['memory']['owned_group_stopped'] and all(g['memory'][k] is None for k in ('sampling_error','memory_rejection','cleanup_error'))
assert g['result_sha256']==pub.sha((run/'results/receipt.json').read_bytes())
assert r['completed'] and len(r['layers'])==64 and r['call_counts']=={'recurrent_delta':48,'online_attention':16}
assert r['A_head_replay_exact'] and r['C_head_replay_exact'] and r['layer0_WXR_exact']
assert not out.exists();out.mkdir()
manifest={'schema':'clef.text029-recurrence-control-publication.v1','scope':r['scope'],'files':[],
 'path_tokens':{token:meaning for (_,token),meaning in zip(pub.REPLACEMENTS,['Worktree root','Task scratch root','Pinned Python environment','Persistent Cargo target','User home'])},
 'previous_precision_manifest_sha256':pub.sha((HERE/'precision-control-publishable-v1/manifest.json').read_bytes()),
 'excluded':['model weights','raw arrays','binaries','raw process memory samples'],
 'original_result_sha256':g['result_sha256'],'acceptance':'Diagnostic only; original104-question all-top gate is unchanged.'}
def save(name,raw):
 assert len(raw)<1_000_000 and (b'/' + b'Users/') not in raw,name
 p=out/name;p.parent.mkdir(parents=True,exist_ok=True);p.write_bytes(raw)
def copy(path,name,expected=None):
 raw=path.read_bytes()
 if expected is not None:assert pub.sha(raw)==expected,(path,expected)
 clean=pub.sanitize(raw);save(name,clean)
 manifest['files'].append({'path':name,'original_bytes':len(raw),'original_sha256':pub.sha(raw),'published_bytes':len(clean),'published_sha256':pub.sha(clean),'path_reconstruction_verified':True})
for n in ('guard.json','stdout.log'):copy(run/n,n)
copy(run/'results/receipt.json','receipt.json')
for n in ('recurrence-source-check.json','recurrence-control-plan.md'):copy(HERE/n,n)
for n in ('recurrence_reference.py','guard_recurrence.py','precision_hash.py','layer0_math.py','layer35_math.py'):
 p=HERE/n;copy(p,'source/'+n,g['sources'][str(p)])
for n in ('mps_reference.py','guard_mps.py'):
 p=HERE.parent/n;copy(p,'source/dependencies/'+n,g['sources'][str(p)])
for n in ('gguf_import.py','stream_reference.py'):
 p=HERE.parent.parent/'cpu27b-diagnostic/streamed-gguf'/n;copy(p,'source/dependencies/'+n,g['sources'][str(p)])
for n in ('mac_quiet_bench.py','mac_memory.py'):
 p=pub.ROOT/'crates/psionic/fixtures/clef/tools'/n;copy(p,'source/dependencies/'+n,g['sources'][str(p)])
for kind in ('memory','disk'):copy(HERE/f'recurrence-1-{kind}-lease.json',f'leases/{kind}.json')
copy(Path(__file__),'source/package_recurrence.py')
d=r['heads']['D_projection_attention_half_recurrent']
def probs(logits):
 v=[math.exp(x-max(logits)) for x in logits];total=math.fsum(v);return [x/total for x in v]
summary={'schema':'clef.text029-recurrence-summary.v1','original_guard_sha256':pub.sha((run/'guard.json').read_bytes()),
 'original_result_sha256':g['result_sha256'],'questions':[],'bounds':g['bounds'],'seconds':g['seconds'],'memory':g['memory'],
 'comparisons':{k:d[k] for k in ('final_vs_A','final_vs_C','final_vs_native','final_spans','logits_vs_A','logits_vs_C','logits_vs_native')},
 'call_counts':r['call_counts'],'layer0_WXR_exact':r['layer0_WXR_exact'],'A_head_replay_exact':r['A_head_replay_exact'],
 'C_head_replay_exact':r['C_head_replay_exact'],'limitations':r['limitations'],'scope':r['scope']}
for i,q in enumerate(r['record']['questions']):
 row={'id':q['question_id'],'option_ids':q['option_ids'],'variants':{}}
 for name,logits in [('A',r['saved_A_logits'][i]),('C',r['saved_C_logits'][i]),('D',d['logits'][i])]+[(f'native_{k}',v[i]) for k,v in r['native_logits'].items()]:
  row['variants'][name]={'logits':logits,'probabilities':probs(logits),'top':q['option_ids'][max(range(len(logits)),key=lambda j:logits[j])]}
 summary['questions'].append(row)
summaryraw=pub.encoded(summary);save('summary.json',pub.sanitize(summaryraw))
manifest['files'].append({'path':'summary.json','published_bytes':(out/'summary.json').stat().st_size,'published_sha256':pub.sha((out/'summary.json').read_bytes()),'generated_from_saved_receipts_only':True})
readme='''# Full text029 recurrent Delta control

The one fixed D diagnostic completed. D retains completed control C's projection and key32 attention half boundaries and replaces only the official chunk64 Delta function with the pinned official recurrent function. The original same-Q4 f32 40-request, 104-question acceptance gate is unchanged. This diagnostic does not constitute a new acceptance reference or product fix.

## Results

The table gives P(true) and the selected top. Full logits and both probabilities for all three questions are in `summary.json` and `receipt.json`.

| Question | A | C | D | Native staged | Native fused-fields |
| --- | ---: | ---: | ---: | ---: | ---: |
'''
for q in summary['questions']:
 readme+='| '+q['id']+' | '+' | '.join(f"{q['variants'][v]['probabilities'][0]:.12f} ({q['variants'][v]['top']})" for v in ('A','C','D','native_staged','native_fused-fields'))+' |\n'
readme+='\nFinal normalized-memory relative L2 errors follow. Each denominator is the right-hand reference. The question spans [192,194), true option [205,223), and false option [229,247). All 64 pre-final-normalization layer comparisons are preserved separately in `receipt.json`.\n\n| Comparison | Whole | Question | True option | False option |\n| --- | ---: | ---: | ---: | ---: |\n'
for label,key in [('D_minus_A','final_vs_A'),('D_minus_C','final_vs_C'),('D_minus_native','final_vs_native')]:
 row=d['final_spans'][label]
 readme+='| '+label.replace('_minus_',' − ')+' | '+f"{d[key]['relative_l2']:.10f}"+' | '+' | '.join(f"{row[v]['relative_l2']:.10f}" for v in ('question','true_option','false_option'))+' |\n'
readme+='''
The operator substitution isolates chunked-versus-recurrent arithmetic. It still does not emulate native Metal exactly: official recurrence dots Q with the updated state, while the archived native scan uses `sq + delta * k_dot_q` with its own FMA and reduction order. GEMM and normalization reductions, transcendental functions, and residual accumulation also differ. Interpret the full trajectory and all questions; a single changed top is not a general correction or gate closure.

## Checks and resources

A and C saved layer63 head replays reproduced their normalized hidden, lexical, and logit values exactly. D layer0 matched saved WXR input hashes, Delta output/final-state hashes, and layer output exactly. The independent f64 arithmetic fixtures and scalar/tensor hash self-test passed. All 48 recurrent and 16 attention calls completed with finite outputs, immutable inputs/weights, exact identities, and no cache. All 64 layer arrays and final memory/head inputs remain in scratch, identified by hashes; no raw arrays or weights are included here.

'''
readme+=f"The run took {g['seconds']:.3f} seconds. Peak accounted memory was {g['memory']['peak_accounted_bytes']:,} bytes; minimum available memory was {g['memory']['lowest_available_memory_bytes']:,} bytes. "
readme+='''The unchanged limits were a 12 GiB sampled group cap, 24 GiB start requirement, 12 GiB available-memory floor, 300-second deadline, two CPU threads, a 2 GB disk lease, a 2 GiB output cap, and 25 GiB free-disk floor. The guard reported no sampling, resource, or cleanup error. Its owned group stopped and leases were released. No GPU, quiet lease, build, desktop app, or system service was used or controlled. This was one run, without retries or limit changes.

## Identity

The GGUF SHA-256 remains `9997675e4c9e9016273372b5fdedbdda7ad14285bc722949e4b9daca62bf3822`. Native comparisons use the same archived Stage6 HTTP results and observer capture as A/B/C; no later binary has been substituted. The guard checks frozen executed sources and input hashes before and after execution, plus the verified artifact snapshot.

The manifest links the complete previous precision bundle and records original and published hashes. Absolute account paths are replaced with reversible tokens. No full receipt rows were removed. The plan is preserved as originally reviewed; its initial source-assessment status predates implementation and execution.
'''
save('README.md',readme.encode());manifest['files'].append({'path':'README.md','published_bytes':len(readme.encode()),'published_sha256':pub.sha(readme.encode()),'generated_summary':True})
save('manifest.json',pub.encoded(manifest))
for row in manifest['files']:assert pub.sha((out/row['path']).read_bytes())==row['published_sha256']
print(json.dumps({'output':str(out),'bytes':sum(x.stat().st_size for x in out.rglob('*') if x.is_file()),'files':sum(x.is_file() for x in out.rglob('*')),'manifest_sha256':pub.sha((out/'manifest.json').read_bytes()),'questions':summary['questions'],'comparisons':summary['comparisons']},indent=2,ensure_ascii=False))
