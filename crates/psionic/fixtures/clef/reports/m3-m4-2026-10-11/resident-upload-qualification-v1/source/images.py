"""Run the unchanged image gate only after exact text equality passes."""
import proof
import native_image_parity

def run():
    proof.require_leases();proof.frozen();binary=proof.candidate()
    text=proof.load(proof.HERE/'text-1/summary.json')
    proof.require(text['status']=='complete' and text['passed'] and text['sources_rechecked'],'text qualification must pass first')
    proof.require(text['comparison']['questions']==110 and text['comparison']['bitwise_logits_equal']==110
                  and text['comparison']['bitwise_pooled_rows_equal']==110,'text coverage differs')
    proof.require(text['provenance']['candidate']['binary_sha256']==binary['binary_sha256'],'text uses another binary')
    config=proof.load(proof.HERE/'images-config.json');config['binary']=binary['binary']
    code=native_image_parity.run(config)
    out=proof.HERE/'images-1';summary={'status':'failed','passed':False,'text_receipt_sha256':proof.sha(proof.HERE/'text-1/summary.json')}
    try:
        run=proof.load(out/'run.json');data=proof.load(out/'0-images.json')
        proof.require(code==0 and run['status']=='complete' and run['sources_rechecked'],'image server run failed')
        proof.require(run['memory']['owned_group_stopped'] and not any(run['memory'].get(k) for k in ('memory_rejection','cleanup_error','sampling_error')),'image cleanup failed')
        proof.require(data['passed'] and len(data['cases'])==6 and sum(len(c['question_comparisons']) for c in data['cases'])==14,'image answer coverage differs')
        proof.require(len(data['http_refusals'])==10 and all(c['passed'] for c in data['http_refusals']),'refusal coverage differs')
        for case in data['cases']:
            response=case['response'];proof.bench.validate_uncached_response(200,response)
            proof.require(response['psionic']['backend']=='metal' and response['psionic']['truncated_state_tokens']==0,'image backend/truncation differs')
        proof.frozen();proof.candidate();proof.disk_check()
        summary.update(status='complete',passed=True,cases=6,questions=14,refusals=10,
                       acceptance='Existing BF16 image top/token/expected-content and refusal gate; probability differences remain diagnostic.',
                       result_sha256=proof.sha(out/'0-images.json'),run_sha256=proof.sha(out/'run.json'))
    except BaseException as error:summary.update(error_type=type(error).__name__,error=str(error))
    proof.write(out/'summary.json',summary);print(__import__('json').dumps(summary,indent=2));return 0 if summary['passed'] else 1
if __name__=='__main__':raise SystemExit(run())
