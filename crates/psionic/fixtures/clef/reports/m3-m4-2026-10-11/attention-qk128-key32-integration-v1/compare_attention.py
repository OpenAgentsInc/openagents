#!/usr/bin/env python3
"""Check QK128/key32 against frozen key32 logits, plus fresh long-history exports.

Infrastructure only. This is same-GGUF correctness, not a latency measurement
or a BF16 model-quality gate. Pooled-row differences are diagnostic: attention
rounding may change them. Native token IDs are inferred from exact request
bytes and unchanged encoder source; HTTP exposes only their counts.
"""
import argparse
import copy
import json
import math
import os
from pathlib import Path
import signal
import sys
import time
import acceptance_helpers as helper

bench=helper.bench
ROOT=Path(__file__).resolve().parent
GATE=1e-3
EXPECTED_BASELINE='77c74a2cf4c737ae0b92278e7f8384a213dc53f1567963fb6b8c6d0dc7157991'
ENV={'PSIONIC_CLEF_METAL_ATTENTION':'fused-tensorops','PSIONIC_CLEF_METAL_GEMM':'weight-shapes',
     'PSIONIC_CLEF_METAL_HEAD':'fused-fields','PSIONIC_CLEF_METAL_DELTA':'sequential',
     'PSIONIC_CLEF_PREFIX_CACHE_BYTES':'0','PSIONIC_CLEF_BATCH_REQUESTS':'1',
     'RAYON_NUM_THREADS':'2','OMP_NUM_THREADS':'2','VECLIB_MAXIMUM_THREADS':'2'}

def require(ok,why):
    if not ok:raise ValueError(why)

def load(path):return json.loads(Path(path).read_text())
def same_hash(path,digest):require(bench.file_sha(path)==digest,'hash changed: '+str(path))
def write(path,value):helper.write(path,value)

def compare(cases,a,b,ta,tb):
    rows=[]
    for case in cases:
        x,y=a[case['id']],b[case['id']]
        helper.image_parity.validate_response(x,case['request']);helper.image_parity.validate_response(y,case['request'])
        counts=x['usage']['input_tokens']==y['usage']['input_tokens']
        metadata=all(x['psionic'][k]==y['psionic'][k] for k in ['artifact_digest','head_digest','prefill_chunk','backend','accumulate','truncated_state_tokens'])
        if case['id'].startswith('long-'):
            lo,hi=(3500,5000) if case['id']=='long-4k' else (15000,17000)
            require(lo<=x['usage']['input_tokens']<=hi,'long request has unexpected actual token count')
        for question in case['request']['questions']:
            u,v=ta[case['digest'],question],tb[case['digest'],question]
            require(all(u[k]==v[k] for k in ['rows','width','index']),'export coordinates differ')
            require(len(u['logits'])==len(v['logits'])>0,'logit lengths differ')
            require(all(type(f) in (int,float) and math.isfinite(f) for f in u['logits']+v['logits']),'nonfinite logits')
            delta=max(abs(f-g) for f,g in zip(u['logits'],v['logits']))
            tops=max(range(len(u['logits'])),key=lambda i:u['logits'][i])==max(range(len(v['logits'])),key=lambda i:v['logits'][i])
            http=helper.image_parity.top_answer(x['answers'][question])==helper.image_parity.top_answer(y['answers'][question])
            rows.append({'id':case['id'],'question':question,'max_abs_logit_delta':delta,'raw_tops_match':tops,'http_tops_match':http,
                'token_counts_match':counts,'input_tokens':x['usage']['input_tokens'],'model_metadata_match':metadata,
                'pooled_rows_match':u['pooled_rows_sha256']==v['pooled_rows_sha256'],
                'baseline_logits':u['logits'],'candidate_logits':v['logits'],
                'passed':delta<=GATE and tops and http and counts and metadata})
    return {'passed':all(r['passed'] for r in rows),'requests':len(cases),'questions':len(rows),
            'top_agreements':sum(r['raw_tops_match'] and r['http_tops_match'] for r in rows),
            'max_abs_logit_delta':max(r['max_abs_logit_delta'] for r in rows),
            'pooled_rows_equal':sum(r['pooled_rows_match'] for r in rows),'logit_limit':GATE,'comparisons':rows}

def source_audit(receipts):
    sources={}
    for name,entry in receipts['source_manifests'].items():
        same_hash(entry['path'],entry['sha256'])
        sources[name]={x['path']:x['sha256'] for x in load(entry['path'])['files']}
    a,b=sources['baseline'],sources['candidate']
    changed={p for p in a.keys()|b.keys() if a.get(p)!=b.get(p) and p.startswith('crates/psionic/crates/')}
    prefix='crates/psionic/crates/psionic-backend-metal/src/'
    allowed={prefix+p for p in ['clef_attention.rs','kernels/clef_fused_attention.metal','clef_prefill/tests.rs',
             'clef_delta.rs','clef_delta/tests.rs','clef_prefill/delta.rs','clef_prefill/delta_tests.rs','kernels/clef_delta_block.metal']}
    generated={'crates/psionic/crates/psionic-backend-cuda/src/kernels/'+p for p in ['quantized_matvec_stub.c','clef_prefill_stub.c']}
    require(changed<=allowed|generated,'unexpected product changes: '+str(sorted(changed-allowed-generated)))
    serve=[p for p in a if p.startswith('crates/psionic/crates/psionic-serve/')]
    require(serve and all(a[p]==b.get(p) for p in serve),'serving/encoder source differs')
    require(all(p in b for p in a if p.startswith('crates/psionic/crates/')),'product source disappeared')
    return {'changed_product_files':sorted(changed),'unchanged_serving_files':len(serve),
            'disabled_changes':'BlockDelta is explicitly sequential; CUDA stubs do not execute on Metal',
            'native_token_ids_observed':False,'token_identity_basis':'exact request bytes and identical encoder/serving source; counts also checked'}

def prepare(config,verify_binary=True,hash_weights=True):
    require(config['model_key'] in ['flash','27b'],'unknown model')
    require(config['measurement_mode']=='shared' and len(config['variants'])==1,'one shared owned server required')
    base=dict(config['variants'][0]);require('url' not in base and base.get('engine','psionic')=='psionic','external server refused')
    model=config['model_key'];large=model=='27b'
    require(base['model']==('clef' if large else 'clef-flash') and base['chunk']==(512 if large else 2048),'model or chunk differs')
    require(config['max_rss_gib']==(52 if large else 36) and config['minimum_available_memory_gib']==12,'memory bounds differ')
    require(config['startup_timeout']==(600 if large else 180) and config['request_timeout']==300 and config['deadline_seconds']==1800,'deadlines differ')
    require(config['max_start_load']==150 and config['max_run_load']==180,'load bounds differ')
    env=dict(ENV,PSIONIC_CLEF_METAL_FFN_WEIGHTS='q4k-on-demand' if large else 'resident-f16')
    require(all(k in env and env[k]==v for k,v in base.get('env',{}).items()),'unknown or conflicting mode flag')
    base.update(env=env,name='candidate')
    receipts=load(config['baseline_receipts'])
    require(receipts['baseline_binary_sha256']==EXPECTED_BASELINE,'baseline binary differs')
    same_hash(ROOT/'acceptance_helpers.py',receipts['helper_sha256'])
    audit=source_audit(receipts)
    corpus=receipts['corpora'][model];directory=Path(corpus['directory'])
    for name,digest in corpus['files'].items():same_hash(directory/name,digest)
    old=load(directory/'summary.json')
    require(old['status']=='complete' and old['passed'] is True and old['requests']==40 and old['questions']==104,'baseline incomplete')
    require(old['binary_sha256']==EXPECTED_BASELINE and old['harness_sha256']==receipts['helper_sha256'],'baseline provenance differs')
    same_hash(config['requests'][0],old['suite_sha256'])
    old_mode=next(v for v in old['effective_variants'] if v['name']=='fused-fields')
    effective=dict(old_mode['env']);effective.setdefault('PSIONIC_CLEF_METAL_DELTA','sequential');effective.setdefault('PSIONIC_CLEF_METAL_FFN_WEIGHTS','resident-f16')
    require(effective==env and old_mode['chunk']==base['chunk'] and old_mode['model']==base['model'],'baseline execution configuration differs')
    for key,entry in corpus['artifacts'].items():
        require(key in config and Path(config[key]).resolve()==Path(entry['path']).resolve(),'artifact path differs')
        if hash_weights:same_hash(config[key],entry['sha256'])
    _,cases=helper.prepare(config)
    for case in cases:require((directory/(case['id']+'.request.json')).read_bytes()==case['raw'],'request bytes differ from baseline')
    records=[json.loads(line) for line in (directory/'records.jsonl').open()]
    startups=[r for r in records if r.get('kind')=='startup' and r.get('variant')=='fused-fields']
    shutdowns=[r for r in records if r.get('kind')=='shutdown' and r.get('variant')=='fused-fields']
    require(len(startups)==len(shutdowns)==1 and startups[0]['owned'] is True and shutdowns[0]['owned_group_stopped'] is True and not shutdowns[0].get('cleanup_error'),'baseline lifecycle incomplete')
    for k,v in startups[0]['effective_env'].items():require(env.get(k)==v,'baseline effective flag differs')
    observed=[r for r in records if r.get('kind')=='response' and r.get('mode')=='fused-fields']
    require(len(observed)==40 and {r['id'] for r in observed}=={c['id'] for c in cases},'baseline response coverage differs')
    responses={}
    for case in cases:
        response=load(directory/('fused-fields-'+case['id']+'.response.json'))
        record=next(r for r in observed if r['id']==case['id'])
        require(record['request_sha256']==case['digest'] and record['response']==response,'response receipt mismatch')
        validate(record['status'],response,case);responses[case['id']]=response
    tables=helper.exports(directory/'fused-fields',cases)
    longs=[]
    require(len(config['long_requests'])==2,'two long fixtures required')
    for size,path in zip(['4k','16k'],config['long_requests']):
        same_hash(path,receipts['long_request_files'][path])
        body=json.loads(Path(path).read_text(),parse_float=helper.Raw,parse_int=helper.Raw);body['model']=base['model']
        raw=helper.dump(body).encode();request=json.loads(raw)
        require(request.get('questions') and not request.get('images') and not request.get('videos'),'long fixture must be text')
        longs.append({'id':'long-'+size,'raw':raw,'request':request,'digest':'sha256:'+bench.sha(raw)})
    require(len({c['digest'] for c in cases+longs})==42,'duplicate request')
    binary_proof={}
    if verify_binary:
        same_hash(receipts['baseline_binary'],EXPECTED_BASELINE)
        binaries=load(config['candidate_binaries']);binary_proof=binaries['server']
        require(binary_proof['source_manifest_unchanged'] is True,'candidate sources changed during build')
        require(Path(base['binary']).name==binary_proof['name'],'candidate binary name differs')
        same_hash(base['binary'],binary_proof['sha256'])
        require(load(ROOT/'guard.json')['status']=='complete','candidate build guard did not complete')
    baseline=dict(base,name='baseline-long',binary=receipts['baseline_binary'])
    return base,baseline,cases,longs,responses,tables,{'source_audit':audit,'baseline_receipts_sha256':bench.file_sha(config['baseline_receipts']),
        'baseline_binary_sha256':EXPECTED_BASELINE,'candidate_binary':binary_proof,'artifacts':corpus['artifacts'],
        'source_manifests':receipts['source_manifests'],'helper_sha256':receipts['helper_sha256']}

def validate(status,response,case):
    bench.validate_uncached_response(status,response)
    helper.image_parity.validate_response(response,case['request'])
    require(response['psionic']['backend']=='metal' and response['psionic']['truncated_state_tokens']==0,'wrong backend or truncated request')

def run(config):
    leases=set(os.environ.get('OPENAGENTS_LEASES','').split(','))
    require({'gpu','memory'}<=leases and 'quiet' not in leases,'GPU and memory leases, without quiet, required')
    out=Path(config['output_dir']);out.mkdir(parents=True,exist_ok=False)
    summary={'status':'failed','passed':False,'started_at':bench.now(),'config':config,
             'conditions':'shared-machine correctness with export; timings are not latency evidence',
             'harness_sha256':bench.file_sha(__file__),'completed_modes':[],
             'dependency_sha256':{name:bench.file_sha(helper.REPO/'crates/psionic/fixtures/clef/tools'/name) for name in ['mac_quiet_bench.py','mac_memory.py','image_parity.py','e2e.py']}}
    write(out/'summary.json',summary)
    host=None;previous={}
    def interrupt(signum,_frame):raise TimeoutError('bounded model deadline exceeded') if signum==signal.SIGALRM else KeyboardInterrupt(f'signal {signum}')
    try:
        candidate,baseline,cases,longs,oldresponses,oldtables,proof=prepare(config)
        summary['provenance']=proof;summary['effective_variants']=[candidate,baseline]
        for c in cases+longs:(out/(c['id']+'.request.json')).write_bytes(c['raw'])
        write(out/'summary.json',summary)
        host=bench.Host(config,out);host.thread.start()
        for sig in [signal.SIGTERM,signal.SIGINT,signal.SIGALRM]:previous[sig]=signal.signal(sig,interrupt)
        host.settle()
        answers={};tables={}
        with (out/'records.jsonl').open('w') as records:
            for variant,requests in [(candidate,cases+longs),(baseline,longs)]:
                name=variant['name'];answers[name]={};deadline=time.monotonic()+1800
                signal.setitimer(signal.ITIMER_REAL,1800)
                try:
                    with bench.server(config,variant,0,out,records,host,export_dir=out/name) as endpoint:
                        for c in requests:
                            host.guard()
                            seconds,status,raw=helper.post(endpoint,c['raw'],min(300,max(.1,deadline-time.monotonic())))
                            (out/(name+'-'+c['id']+'.response.json')).write_bytes(raw)
                            response=json.loads(raw)
                            bench.emit(records,{'kind':'response','mode':name,'id':c['id'],'request_sha256':c['digest'],'status':status,'seconds':seconds,'response':response,'load':host.guard()})
                            validate(status,response,c);answers[name][c['id']]=response
                    tables[name]=helper.exports(out/name,requests);summary['completed_modes'].append(name)
                    if name=='candidate':
                        summary['corpus']=compare(cases,oldresponses,answers[name],oldtables,tables[name]);write(out/'summary.json',summary)
                        require(summary['corpus']['passed'],'corpus strict gate failed; skip further model loads')
                finally:signal.setitimer(signal.ITIMER_REAL,0)
        summary['long_history']=compare(longs,answers['baseline-long'],answers['candidate'],tables['baseline-long'],tables['candidate'])
        summary['passed']=summary['corpus']['passed'] and summary['long_history']['passed']
        summary['status']='complete' if summary['passed'] else 'failed'
    except BaseException as error:summary.update(error_type=type(error).__name__,error=str(error))
    finally:
        signal.setitimer(signal.ITIMER_REAL,0)
        for sig,handler in previous.items():signal.signal(sig,handler)
        if host is not None:
            host.stop.set();host.thread.join(timeout=6)
            if host.thread.is_alive() or host.monitor_error:summary.update(status='failed',passed=False,host_error=host.monitor_error or 'monitor did not stop')
        summary['finished_at']=bench.now();write(out/'summary.json',summary)
    print(json.dumps({k:v for k,v in summary.items() if k not in ['config','corpus','long_history','effective_variants']},indent=2),flush=True)
    return 0 if summary['passed'] else 1

def self_test():
    helper.self_test()
    c={'id':'text-000','digest':'sha256:x','request':{'questions':{'q':{'type':'noul'}}}}
    r={'model':'clef','usage':{'input_tokens':3,'output_tokens':0},'answers':{'q':{'type':'noul','noul':.75}},'psionic':{'artifact_digest':'a','head_digest':'h','prefill_chunk':512,'backend':'metal','accumulate':'f32','truncated_state_tokens':0}}
    row={'logits':[2.,1.],'rows':['last','question','option:true','option:false'],'width':32,'index':0,'pooled_rows_sha256':'same'}
    # The shared validator requires a model and the complete question map.
    c['request']['model']='clef'
    a={'text-000':r};ta={(c['digest'],'q'):row}
    require(compare([c],a,a,ta,ta)['passed'],'valid synthetic comparison refused')
    changed=copy.deepcopy(ta);changed[c['digest'],'q']['pooled_rows_sha256']='different'
    require(compare([c],a,a,ta,changed)['passed'],'pooled rows incorrectly became an acceptance gate')
    for failure in ['logit','rawtop','httptop','tokens','metadata','nan','coordinates']:
        b=copy.deepcopy(a);tb=copy.deepcopy(ta)
        if failure=='logit':tb[c['digest'],'q']['logits'][0]+=.002
        elif failure=='rawtop':tb[c['digest'],'q']['logits']=[0.,1.]
        elif failure=='httptop':b[c['id']]['answers']['q']['noul']=.25
        elif failure=='tokens':b[c['id']]['usage']['input_tokens']=4
        elif failure=='metadata':b[c['id']]['psionic']['head_digest']='different'
        elif failure=='nan':tb[c['digest'],'q']['logits'][0]=float('nan')
        else:tb[c['digest'],'q']['index']=1
        try:result=compare([c],a,b,ta,tb)
        except ValueError:continue
        require(not result['passed'],'synthetic failure accepted: '+failure)
    print('attention comparison self-tests passed')

if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('config',nargs='?');p.add_argument('--self-test',action='store_true');p.add_argument('--audit-only',action='store_true');args=p.parse_args()
    if args.self_test:self_test()
    elif args.config:
        config=bench.load_config(args.config)
        if args.audit_only:
            *_,proof=prepare(config,verify_binary=False,hash_weights=False);print(json.dumps(proof,indent=2))
        else:raise SystemExit(run(config))
    else:p.error('config required')
