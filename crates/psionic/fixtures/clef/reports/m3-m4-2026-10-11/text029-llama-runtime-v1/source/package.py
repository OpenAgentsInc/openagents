"""Publish compact proof from the single authorized diagnostic sequence."""
import importlib.util
import json
import math
from pathlib import Path

HERE=Path(__file__).resolve().parent
spec=importlib.util.spec_from_file_location('publication',HERE.parent.parent/'publication/pack.py')
pub=importlib.util.module_from_spec(spec);spec.loader.exec_module(pub)
out=HERE/'publishable-v2'
assert not out.exists()
out.mkdir()
manifest={'schema':'clef.text029-llama-publication.v1','scope':'Third-runtime attribution diagnostic; existing Torch104-question acceptance remains unchanged.',
          'files':[],'excluded':['model weights','raw tensor arrays','executable/static libraries','raw process memory samples'],
          'path_tokens':{token:meaning for (_,token),meaning in zip(pub.REPLACEMENTS,['Worktree root','Task scratch root','Pinned Python environment','Persistent Cargo target','User home'])}}
def save(name,raw):
    assert len(raw)<1_000_000 and (b'/' + b'Users/') not in raw,name
    p=out/name;p.parent.mkdir(parents=True,exist_ok=True);p.write_bytes(raw)
def copy(path,name,expected=None):
    raw=path.read_bytes()
    if expected is not None:assert pub.sha(raw)==expected,path
    clean=pub.sanitize(raw);save(name,clean)
    manifest['files'].append({'path':name,'original_bytes':len(raw),'original_sha256':pub.sha(raw),
                            'published_bytes':len(clean),'published_sha256':pub.sha(clean),'path_reconstruction_verified':True})
def generated(name,value):
    raw=pub.sanitize(pub.encoded(value));save(name,raw)
    manifest['files'].append({'path':name,'published_bytes':len(raw),'published_sha256':pub.sha(raw),'generated_from_saved_receipts_only':True})
summary={'stages':{},'acceptance_scope':manifest['scope'],'numerical_agreement':'failed; execution completed, but third-runtime probabilities substantially differ from both CPU and native. Matching native tops does not establish native correctness.'}
last_guard=None
for mode in ('build','capture','replay'):
    run=HERE/(mode+'-1')
    if not (run/'guard.json').exists():break
    guard=json.loads((run/'guard.json').read_text());last_guard=guard
    assert guard['status'] in ('complete','failed') and guard.get('memory',{}).get('owned_group_stopped'),mode
    summary['stages'][mode]={'status':guard['status'],'seconds':guard['seconds'],'bounds':guard['bounds'],
                            'memory':guard['memory'],'guard_sha256':pub.sha((run/'guard.json').read_bytes())}
    for name in ('guard.json','stdout.log'):
        if (run/name).exists():copy(run/name,mode+'/'+name)
    receipt=run/'results/receipt.json'
    if receipt.exists():
        copy(receipt,mode+'/receipt.json',guard.get('result_sha256'))
        result=json.loads(receipt.read_text())
        summary['stages'][mode]['result_status']=result['status']
        if mode=='build' and guard['status']=='complete':summary['binary_sha256']=result['binary_sha256']
        if mode=='capture' and guard['status']=='complete':
            assert result['completed'] and len(result['files'])==65
            summary['capture']={key:result[key] for key in ('upstream','record','gguf_sha256_from_prior_verification','driver','files')}
        if mode=='replay' and guard['status']=='complete':
            assert result['completed'] and len(result['layers'])==64 and result['A_head_replay_exact']
            summary['comparison']={key:result[key] for key in ('heads','final_rows','observer_replay_screen')}
    for lease in ('lease','memory','disk'):
        f=HERE/f'{mode}-1-{lease}.json'
        if f.exists():copy(f,f'leases/{mode}-{lease}.json')
    if guard['status']!='complete':break
assert last_guard
copy(HERE/'source-check.json','source-check.json',last_guard['sources'][str(HERE/'source-check.json')])
for name in ('driver.cpp','CMakeLists.txt','common.py','build.py','capture.py','replay.py','guard.py','plan.md'):
    copy(HERE/name,'source/'+name,last_guard['sources'][str(HERE/name)])
for path in [HERE.parent.parent/'mps_reference.py',HERE.parent/'precision_hash.py',
             pub.ROOT/'crates/psionic/fixtures/clef/tools/mac_quiet_bench.py',pub.ROOT/'crates/psionic/fixtures/clef/tools/mac_memory.py']:
    copy(path,'source/dependencies/'+path.name,last_guard['sources'][str(path)])
copy(Path(__file__),'source/package.py')
generated('summary.json',summary)
readme='''# Exact-input llama.cpp third-runtime diagnostic

This is one fixed text-029 diagnostic using the original same-Q4 GGUF and all 359 exact token IDs, spans, and options. It does not replace the frozen Torch reference, change the all 104-top/0.02 probability gate, or close M4.

The build uses public commit `62a6f74bab748b1755f4cd8dfc0871caf865c6c8`, not M1's `b11538` tag (`79e2e74eb11022c1ba2e438df7f0ca2d4c10f8b6`). Static CPU execution disables GPU backends, dynamic plugins, BLAS, Accelerate, repacking, KleidiAI, and llamafile. It verifies Q4_K uses Q8_K RHS traits and requires 48 fused CPU Delta graph operators and no Flash Attention. Its arithmetic differs from both native f16 and Torch f32.

The upstream graph evaluates an embedded quantized head. Those logits are labeled separately and excluded from head-equivalent comparisons. The saved-row replay instead uses the unchanged official head loaded as CPU f32 and identical GGUF lexical rows; it does not repeat backbone RMSNorm on `result_norm`.

## Stage results

All stages executed successfully, but third-runtime numerical agreement failed: its probabilities substantially differ from both the independent CPU and native. Matching native's top answers does not establish native correctness or resolve the frozen Torch acceptance failure.

| Stage | Status | Seconds | Peak accounted bytes | Minimum available bytes |
| --- | --- | ---: | ---: | ---: |
'''
for mode,value in summary['stages'].items():
    readme+=f"| {mode} | {value['status']} | {value['seconds']:.3f} | {value['memory']['peak_accounted_bytes']:,} | {value['memory']['lowest_available_memory_bytes']:,} |\n"
if 'comparison' in summary:
    readme+='\nAll 64 layer comparisons, all 3 question logits/probabilities, and the separate native observer replay screen are preserved in `replay/receipt.json`. The A saved-head replay was bitwise exact.\n\n| Question | CPU A P(true), top | Native observer P(true), top | Llama trunk + external head P(true), top |\n| --- | ---: | ---: | ---: |\n'
    questions=summary['capture']['record']['questions'];heads=summary['comparison']['heads']
    for i,q in enumerate(questions):
        cells=[]
        for mode in ('cpu','native_observer','llama'):
            logits=heads[mode]['logits'][i];exp=[math.exp(v-max(logits)) for v in logits];probs=[v/math.fsum(exp) for v in exp]
            cells.append(f"{probs[0]:.12f} ({q['option_ids'][max(range(len(logits)),key=lambda j:logits[j])]})")
        readme+='| '+q['question_id']+' | '+' | '.join(cells)+' |\n'
    readme+='\nWhole final-memory relative L2: llama versus CPU '+str(summary['comparison']['final_rows']['llama_vs_cpu']['relative_l2'])+', llama versus native '+str(summary['comparison']['final_rows']['llama_vs_native']['relative_l2'])+'. Full metrics and span trajectories remain in the receipt.\n'
else:
    readme+='\nThe sequence stopped at the first failed stage. No downstream comparison is claimed. Failure logs and receipts remain unchanged.\n'
readme+='''
## Limits and identity

Build limits were 8 GiB/start20/floor12/600s; capture 32 GiB/start44/floor12/600s; replay 4 GiB/start16/floor12/120s. Each uses two CPU threads, a 2 GB disk lease, 2 GiB output cap, and 25 GiB free-disk floor. Guards are sampled memory limits, not hard allocation limits. The sequence has no retries or raised bounds. Owned groups were cleaned up; no GPU, quiet lease, desktop app, or system service was used or controlled.

The original GGUF SHA-256 is `9997675e4c9e9016273372b5fdedbdda7ad14285bc722949e4b9daca62bf3822`; its previously verified file snapshot is checked before and after, without rehashing 19 GB. Build-to-capture-to-replay source descriptors and binary hashes remain linked. Exact native IDs were reconstructed later from frozen production encoder sources; Stage6 HTTP itself did not export IDs.

The publication manifest records original and path-sanitized hashes with reversible path tokens. Complete receipts are preserved. Raw arrays, model weights, binaries, and raw process-memory samples stay in scratch. This third runtime can help attribute implementation differences; it cannot by itself establish which remaining arithmetic difference causes the original top mismatch.
'''
raw=readme.encode();save('README.md',raw)
manifest['files'].append({'path':'README.md','published_bytes':len(raw),'published_sha256':pub.sha(raw),'generated_summary':True})
save('manifest.json',pub.encoded(manifest))
for row in manifest['files']:assert pub.sha((out/row['path']).read_bytes())==row['published_sha256']
print(json.dumps({'output':str(out),'files':sum(p.is_file() for p in out.rglob('*')),
                  'bytes':sum(p.stat().st_size for p in out.rglob('*') if p.is_file()),
                  'manifest_sha256':pub.sha((out/'manifest.json').read_bytes()),'stages':summary['stages']},indent=2))
