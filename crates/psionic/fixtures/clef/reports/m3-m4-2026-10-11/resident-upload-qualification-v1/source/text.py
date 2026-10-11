"""Require exact f32 logits and f16 pooled bytes from one bounded resident server."""
import argparse
import copy
import json
import math
from pathlib import Path
import shutil
import signal
import struct
import time
import proof

helper=proof.helper;bench=proof.bench

def bit_logit(values):
    proof.require(values and all(type(x) in (int,float) and math.isfinite(x) for x in values),'invalid raw logits')
    return b''.join(struct.pack('<f',value) for value in values)

def compare(cases,old,new,a,b):
    rows=[]
    for case in cases:
        x,y=old[case['id']],new[case['id']]
        for response in (x,y):
            bench.validate_uncached_response(200,response)
            helper.image_parity.validate_response(response,case['request'])
            proof.require(response['psionic']['backend']=='metal' and response['psionic']['truncated_state_tokens']==0,'wrong backend/truncation')
        counts=x['usage']['input_tokens']==y['usage']['input_tokens']
        metadata=all(x['psionic'][k]==y['psionic'][k] for k in ('artifact_digest','head_digest','prefill_chunk','backend','accumulate','truncated_state_tokens'))
        for question in case['request']['questions']:
            u,v=a[case['digest'],question],b[case['digest'],question]
            proof.require(all(u[k]==v[k] for k in ('rows','width','index')) and u['width']==5120,'raw export coordinates differ')
            proof.require(len(u['logits'])==len(v['logits']),'logit count differs')
            bits=bit_logit(u['logits'])==bit_logit(v['logits'])
            pooled=u['pooled_bytes']==v['pooled_bytes']
            tops=helper.image_parity.top_answer(x['answers'][question])==helper.image_parity.top_answer(y['answers'][question])
            rows.append({'id':case['id'],'question':question,'logit_bits_equal':bits,'pooled_bytes_equal':pooled,
                         'http_tops_match':tops,'token_counts_match':counts,'input_tokens':y['usage']['input_tokens'],
                         'model_metadata_match':metadata,'baseline_logits':u['logits'],'candidate_logits':v['logits'],
                         'baseline_pooled_sha256':u['pooled_rows_sha256'],'candidate_pooled_sha256':v['pooled_rows_sha256'],
                         'max_abs_logit_delta':max(abs(i-j) for i,j in zip(u['logits'],v['logits'])),
                         'passed':bits and pooled and tops and counts and metadata})
    return {'passed':all(v['passed'] for v in rows),'requests':len(cases),'questions':len(rows),
            'bitwise_logits_equal':sum(v['logit_bits_equal'] for v in rows),
            'bitwise_pooled_rows_equal':sum(v['pooled_bytes_equal'] for v in rows),'comparisons':rows,
            'criterion':'All f32 raw-logit bits and pooled f16 bytes equal; no tolerance.'}

def prepare():
    frozen,sources=proof.frozen();candidate=proof.candidate()
    fixture=proof.load(proof.HERE/'gpu-fixture-1/guard.json')
    proof.require(fixture['status']=='complete' and fixture['exit']==0 and fixture['tests_passed']==1 and fixture['tests_failed']==0 and fixture['tests_ignored']==0 and fixture['fixture_binary']==candidate['fixture_binary'] and fixture['sources_rechecked'] and fixture['candidate_receipt_sha256']==proof.sha(proof.HERE/'candidate.json') and fixture['memory']['owned_group_stopped'] and not any(fixture['memory'].get(k) for k in ('memory_rejection','cleanup_error','sampling_error')),'small GPU fixture must pass first')
    summary=proof.load(proof.OLD/'27b-text-check/summary.json')
    proof.require(summary['status']=='complete' and summary['passed'] and summary['corpus']['questions']==104
                  and summary['long_history']['questions']==6 and summary['completed_modes']==['candidate','baseline-long'],'baseline coverage incomplete')
    old_mode=summary['effective_variants'][0]
    expected=dict(proof.ENV);expected.pop('PSIONIC_CLEF_METAL_WEIGHT_UPLOAD');expected['PSIONIC_CLEF_METAL_FFN_WEIGHTS']='q4k-on-demand'
    proof.require(old_mode['env']==expected and old_mode['chunk']==512 and old_mode['model']=='clef','baseline configuration differs')
    cases=[];responses={};old_dir=proof.OLD/'27b-text-check'
    records=[json.loads(line) for line in (old_dir/'records.jsonl').read_text().splitlines()]
    startup=[v for v in records if v.get('kind')=='startup' and v.get('variant')=='candidate']
    shutdown=[v for v in records if v.get('kind')=='shutdown' and v.get('variant')=='candidate']
    proof.require(len(startup)==len(shutdown)==1 and startup[0]['owned'] and shutdown[0]['owned_group_stopped']
                  and not any(shutdown[0].get(k) for k in ('memory_rejection','sampling_error','cleanup_error')),'baseline cleanup incomplete')
    for index,name in enumerate([f'text-{i:03d}' for i in range(40)]+['long-4k','long-16k']):
        raw=(old_dir/(name+'.request.json')).read_bytes();request=json.loads(raw);digest='sha256:'+bench.sha(raw)
        case={'id':name,'raw':raw,'request':request,'digest':digest}
        response=proof.load(old_dir/('candidate-'+name+'.response.json'))
        saved=[v for v in records if v.get('kind')=='response' and v.get('mode')=='candidate' and v['id']==name]
        proof.require(len(saved)==1 and saved[0]['request_sha256']==digest and saved[0]['response']==response and saved[0]['status']==200,'baseline response binding differs')
        bench.validate_uncached_response(200,response);helper.image_parity.validate_response(response,request)
        if name.startswith('long-'):
            lo,hi=(3500,5000) if name=='long-4k' else (15000,17000)
            proof.require(lo<=response['usage']['input_tokens']<=hi,'fixed long token length differs')
        cases.append(case);responses[name]=response
    proof.require(len({v['digest'] for v in cases})==42 and sum(len(c['request']['questions']) for c in cases)==110,'case coverage differs')
    return frozen,sources,candidate,cases,responses,helper.exports(old_dir/'candidate',cases)

def run():
    proof.require_leases();proof.clean_inherited_environment()
    out=proof.HERE/'text-1';out.mkdir(exist_ok=False)
    result={'status':'failed','passed':False,'scope':'Fixed resident upload correctness and capacity; no latency claim.',
            'bounds':{'max_rss_gib':52,'minimum_start_gib':64,'available_floor_gib':12,'startup_seconds':600,'request_seconds':300,'server_seconds':1800},
            'started_at':bench.now()};proof.write(out/'summary.json',result)
    host=None;handlers={}
    def interrupt(signum,frame):raise TimeoutError('fixed server deadline') if signum==signal.SIGALRM else KeyboardInterrupt(str(signum))
    try:
        frozen,sources,binary,cases,old,tables=prepare()
        config=proof.limits();variant={'name':'positional','binary':binary['binary'],'model':'clef','chunk':512,'env':proof.ENV}
        result.update(provenance={'inputs_sha256':proof.sha(proof.HERE/'inputs.json'),'source_sha256':proof.sha(proof.HERE/'qualification-source.json'),
                                 'candidate':binary,'baseline_sha256':proof.BASELINE_SHA,'same_request_bytes':True},effective_variant=variant)
        proof.write(out/'summary.json',result)
        for case in cases:(out/(case['id']+'.request.json')).write_bytes(case['raw'])
        proof.disk_check(start=True)
        host=bench.Host(config,out);host.thread.start();host.settle()
        for sig in (signal.SIGTERM,signal.SIGINT,signal.SIGALRM):handlers[sig]=signal.signal(sig,interrupt)
        answers={};deadline=time.monotonic()+1800;signal.setitimer(signal.ITIMER_REAL,1800)
        with (out/'records.jsonl').open('x') as records:
            with bench.server(config,variant,0,out,records,host,export_dir=out/'positional') as endpoint:
                for case in cases:
                    host.guard();proof.disk_check()
                    seconds,status,raw=helper.post(endpoint,case['raw'],min(300,max(.1,deadline-time.monotonic())))
                    (out/(case['id']+'.response.json')).write_bytes(raw);response=json.loads(raw)
                    bench.emit(records,{'kind':'response','id':case['id'],'request_sha256':case['digest'],'status':status,'seconds':seconds,'response':response})
                    bench.validate_uncached_response(status,response);helper.image_parity.validate_response(response,case['request'])
                    answers[case['id']]=response
        signal.setitimer(signal.ITIMER_REAL,0)
        comparison=compare(cases,old,answers,tables,helper.exports(out/'positional',cases))
        proof.require(comparison['requests']==42 and comparison['questions']==110,'comparison coverage differs')
        result['comparison']=comparison
        proof.frozen();proof.candidate();proof.disk_check()
        result.update(status='complete' if comparison['passed'] else 'failed',passed=comparison['passed'],sources_rechecked=True)
    except BaseException as error:result.update(error_type=type(error).__name__,error=str(error))
    finally:
        signal.setitimer(signal.ITIMER_REAL,0)
        for sig,handler in handlers.items():signal.signal(sig,handler)
        if host is not None:
            host.stop.set();host.thread.join(timeout=6)
            if host.thread.is_alive() or host.monitor_error:result.update(status='failed',passed=False,host_error=host.monitor_error or 'monitor cleanup failed')
        try:proof.disk_check();proof.frozen()
        except BaseException as error:result.update(status='failed',passed=False,post_cleanup_error=str(error))
        result['finished_at']=bench.now();proof.write(out/'summary.json',result)
    print(json.dumps({k:v for k,v in result.items() if k not in ('comparison','provenance')},indent=2))
    return 0 if result['passed'] else 1

if __name__=='__main__':raise SystemExit(run())
