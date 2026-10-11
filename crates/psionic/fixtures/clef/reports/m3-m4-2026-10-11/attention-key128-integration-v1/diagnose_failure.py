"""Read saved responses only; keep the failed raw-logit gate unchanged."""
from pathlib import Path
import json,math,statistics,hashlib,sys
import acceptance_helpers as helper
import e2e
D=Path(__file__).resolve().parent
old=D.parent/'flash-head-stage6-combined-retry';new=D/'flash-text-check'
reference=helper.REPO/'crates/psionic/fixtures/clef/e2e/psionic-cpu-q4_k_m.jsonl'
refs=[json.loads(l) for l in reference.open()]
assert len(refs)==40
rows=[];options=[]
for i,reference_row in enumerate(refs):
    identifier=f'text-{i:03d}'
    a=json.loads((old/f'fused-fields-{identifier}.response.json').read_text())
    b=json.loads((new/f'candidate-{identifier}.response.json').read_text())
    ref=reference_row['body']
    assert reference_row['index']==i and reference_row['status']==200
    assert ref['usage']['input_tokens']==a['usage']['input_tokens']==b['usage']['input_tokens']
    for key in ['artifact_digest','head_digest']:assert ref['psionic'][key]==a['psionic'][key]==b['psionic'][key]
    assert set(ref['answers'])==set(a['answers'])==set(b['answers'])
    for q in ref['answers']:
        pa,pb,pr=[e2e.probabilities(x['answers'][q]) for x in [a,b,ref]]
        assert pa.keys()==pb.keys()==pr.keys()
        da=[];db=[];ab=[]
        for o in pr:
            assert all(math.isfinite(x) for x in [pa[o],pb[o],pr[o]])
            x,y=abs(pa[o]-pr[o]),abs(pb[o]-pr[o]);da.append(x);db.append(y);ab.append(abs(pa[o]-pb[o]))
            options.append({'id':identifier,'question':q,'option':o,'key32_error':x,'key128_error':y})
        rows.append({'id':identifier,'question':q,'key32_max_abs_dp':max(da),'key128_max_abs_dp':max(db),
                     'key32_mean_abs_dp':statistics.mean(da),'key128_mean_abs_dp':statistics.mean(db),
                     'candidate_vs_key32_max_abs_dp':max(ab),
                     'key32_top_matches_cpu':e2e.top(a['answers'][q])==e2e.top(ref['answers'][q]),
                     'key128_top_matches_cpu':e2e.top(b['answers'][q])==e2e.top(ref['answers'][q])})
summary=json.loads((new/'summary.json').read_text())['corpus'];fails=[r for r in summary['comparisons'] if not r['passed']]
result={'scope':'offline diagnostic only; raw-logit gate remains failed; CPU goldens have probabilities, not raw logits',
        'reference_sha256':helper.bench.file_sha(reference),'questions':len(rows),'option_probabilities':len(options),
        'failed_raw_logit_questions':len(fails),'failed_raw_logit_requests':len({r['id'] for r in fails}),
        'max_abs_raw_logit_delta':summary['max_abs_logit_delta'],'median_question_max_abs_logit_delta':statistics.median(r['max_abs_logit_delta'] for r in summary['comparisons']),
        'key32':{'top_agreements':sum(r['key32_top_matches_cpu'] for r in rows),'max_abs_probability_delta':max(r['key32_max_abs_dp'] for r in rows),'mean_abs_probability_delta':statistics.mean(o['key32_error'] for o in options)},
        'key128':{'top_agreements':sum(r['key128_top_matches_cpu'] for r in rows),'max_abs_probability_delta':max(r['key128_max_abs_dp'] for r in rows),'mean_abs_probability_delta':statistics.mean(o['key128_error'] for o in options)},
        'question_max_error_improved':sum(r['key128_max_abs_dp']<r['key32_max_abs_dp'] for r in rows),
        'question_max_error_worsened':sum(r['key128_max_abs_dp']>r['key32_max_abs_dp'] for r in rows),
        'candidate_vs_key32_max_abs_probability_delta':max(r['candidate_vs_key32_max_abs_dp'] for r in rows),'comparisons':rows}
helper.write(D/'failure-diagnosis.json',result)
print(json.dumps({k:v for k,v in result.items() if k!='comparisons'},indent=2))
