#!/usr/bin/env python3
"""Compare staged and fused Metal heads using owned, sequential HTTP servers.

Infrastructure only. This checks same-binary/same-GGUF parity, not BF16 model
acceptance or latency targets. Native HTTP exposes token counts, not token IDs;
exact ID equality is inferred from identical request bytes and the same encoder.
The runner separately requires identical exported pooled-row bytes. It never
claims that matching counts alone prove matching token IDs.
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
import tempfile
import urllib.error
import urllib.request

REPO=Path('workspace/openagents')
sys.path.insert(0,str(REPO/'crates/psionic/fixtures/clef/tools'))
import mac_quiet_bench as bench
import image_parity
from e2e import Raw,dump

GATE=1e-3


def write(path,value):
    path.write_text(json.dumps(value,indent=2,allow_nan=False)+'\n')


def prepare(config):
    if config['measurement_mode']!='shared' or len(config['variants'])!=1 or len(config['requests'])!=1:
        raise ValueError('one local variant and one JSONL suite in shared mode are required')
    base=dict(config['variants'][0])
    if 'url' in base or base.get('engine','psionic')!='psionic':raise ValueError('an owned Psionic binary is required')
    env=dict(base.get('env',{}))
    if any('PROFILE' in key or 'SKIP' in key for key in env):raise ValueError('diagnostic kernel modes are refused')
    if env.get('PSIONIC_CLEF_PREFIX_CACHE_BYTES','0')!='0':raise ValueError('prefix reuse is refused')
    if env.get('PSIONIC_CLEF_BATCH_REQUESTS','1')!='1':raise ValueError('batching is refused for serial head isolation')
    env.update(PSIONIC_CLEF_PREFIX_CACHE_BYTES='0',PSIONIC_CLEF_BATCH_REQUESTS='1',
               RAYON_NUM_THREADS='2',OMP_NUM_THREADS='2',VECLIB_MAXIMUM_THREADS='2')
    env.pop('PSIONIC_CLEF_METAL_HEAD',None)
    limit=config.get('deadline_seconds',1800)
    if type(limit) is not int or not 1<=limit<=3600:raise ValueError('deadline_seconds must be1..3600 per mode')
    if not 0<config['request_timeout']<=600:raise ValueError('request_timeout must be at most600 seconds')
    limits=bench.owned_process_limits(config,base)
    if limits['max_rss_bytes']>84*1024**3 or limits['minimum_available_memory_bytes'] is None or limits['minimum_available_memory_bytes']<12*1024**3:
        raise ValueError('at most84GiB process budget and at least12GiB host margin are required')
    variants=[dict(base,name=mode,env=dict(env,PSIONIC_CLEF_METAL_HEAD=mode)) for mode in ['staged','fused-fields']]
    cases=[]
    for index,line in enumerate(Path(config['requests'][0]).open(encoding='utf-8')):
        if not line.strip():raise ValueError('blank suite rows are refused')
        body=json.loads(line,parse_float=Raw,parse_int=Raw)
        body['model']=base['model']
        raw=dump(body).encode()
        request=json.loads(raw)
        if not request.get('questions') or request.get('images') or request.get('videos'):raise ValueError('text requests only')
        cases.append({'id':f'text-{index:03d}','request':request,'raw':raw,'digest':'sha256:'+bench.sha(raw)})
    if len(cases)!=config.get('expected_requests',40) or sum(len(case['request']['questions']) for case in cases)!=config.get('expected_questions',104):
        raise ValueError('suite completeness differs from required counts')
    if len(cases)>40 or len({c['digest'] for c in cases})!=len(cases):raise ValueError('at most40 unique requests are required')
    return variants,cases


def post(url,raw,timeout):
    request=urllib.request.Request(url,data=raw,headers={'Content-Type':'application/json'})
    began=time.monotonic()
    try:
        with bench.HTTP.open(request,timeout=timeout) as response:body,status=response.read(),response.status
    except urllib.error.HTTPError as error:body,status=error.read(),error.code
    return time.monotonic()-began,status,body


def exports(directory,cases):
    expected={case['digest']:case for case in cases}
    table={}
    data=(directory/'rows.f16').read_bytes()
    consumed=0
    indices={digest:set() for digest in expected}
    for line in (directory/'rows.jsonl').open(encoding='utf-8'):
        row=json.loads(line)
        digest=row['request_sha256'];question=row['question'];key=(digest,question)
        if digest not in expected or question not in expected[digest]['request']['questions'] or key in table:
            raise ValueError('unexpected or duplicate exported question')
        question_config=expected[digest]['request']['questions'][question]
        kind=question_config['type']
        options=['true','false'] if kind=='noul' else ([str(i) for i in range(len(question_config['criteria']))] if isinstance(question_config['criteria'],list) else list(question_config['criteria']))
        logits=row['logits']
        if len(logits)!=len(options) or not logits or any(type(x) not in (int,float) or not math.isfinite(x) for x in logits):
            raise ValueError('missing or nonfinite exported logits')
        if row['rows'][:2]!=['last','question'] or set(row['rows'][2:])!={'option:'+x for x in options} or len(row['rows'])!=2+len(options):
            raise ValueError('exported option identity differs')
        if row['dtype']!='f16le' or type(row['width']) is not int or row['width']<=0 or row['offset']!=consumed:
            raise ValueError('invalid pooled-row export layout')
        end=consumed+len(row['rows'])*row['width']*2
        if end>len(data):raise ValueError('truncated pooled-row export')
        if row['question_index'] in indices[digest]:raise ValueError('duplicate question index')
        indices[digest].add(row['question_index'])
        table[key]={'logits':logits,'rows':row['rows'],'width':row['width'],'index':row['question_index'],
                    'pooled_rows_sha256':bench.sha(data[consumed:end])}
        consumed=end
    if consumed!=len(data):raise ValueError('unindexed pooled-row bytes')
    for digest,case in expected.items():
        if indices[digest]!=set(range(len(case['request']['questions']))):raise ValueError('missing exported questions')
    return table


def compare(cases,responses,tables):
    rows=[];maximum=0.;tops=0;questions=0;passed=True
    for case in cases:
        a,b=[responses[mode][case['id']] for mode in ['staged','fused-fields']]
        tokens_equal=a['usage']['input_tokens']==b['usage']['input_tokens']
        metadata_equal=all(a['psionic'][key]==b['psionic'][key] for key in ['artifact_digest','head_digest','prefill_chunk','backend','accumulate','truncated_state_tokens'])
        for question in case['request']['questions']:
            x,y=[tables[mode][(case['digest'],question)] for mode in ['staged','fused-fields']]
            if any(x[key]!=y[key] for key in ['rows','width','index']):raise ValueError('raw-logit coordinate identities differ')
            delta=max(abs(u-v) for u,v in zip(x['logits'],y['logits']))
            raw_tops=max(range(len(x['logits'])),key=lambda i:x['logits'][i])==max(range(len(y['logits'])),key=lambda i:y['logits'][i])
            http_tops=image_parity.top_answer(a['answers'][question])==image_parity.top_answer(b['answers'][question])
            pooled=x['pooled_rows_sha256']==y['pooled_rows_sha256']
            ok=delta<=GATE and raw_tops and http_tops and tokens_equal and pooled and metadata_equal
            passed &= ok;tops+=int(raw_tops and http_tops);questions+=1;maximum=max(maximum,delta)
            rows.append({'id':case['id'],'question':question,'max_abs_logit_delta':delta,'raw_tops_match':raw_tops,'http_tops_match':http_tops,
                         'token_counts_match':tokens_equal,'pooled_rows_match':pooled,'model_metadata_match':metadata_equal,'passed':ok})
    return {'passed':passed,'requests':len(cases),'questions':questions,'top_agreements':tops,'max_abs_logit_delta':maximum,
            'logit_limit':GATE,'comparisons':rows,'token_identity':{'native_ids_observed':False,
            'basis':'identical serialized request bytes, binary, GGUF, head, and trunk flags; exact counts and pooled-row bytes checked',
            'limitation':'native HTTP export does not contain token IDs; ID equality is inferred, not directly observed'}}


def run(config):
    if not {'gpu','memory'}<=set(os.environ.get('OPENAGENTS_LEASES','').split(',')):raise ValueError('GPU and memory leases are required')
    variants,cases=prepare(config)
    output=Path(config['output_dir']);output.mkdir(parents=True,exist_ok=False)
    summary={'status':'failed','passed':False,'conditions':'shared machine; correctness with row export, not serving latency',
             'config':config,'effective_variants':variants,'started_at':bench.now(),'completed_modes':[]}
    for case in cases:(output/(case['id']+'.request.json')).write_bytes(case['raw'])
    write(output/'summary.json',summary)
    host=None
    responses={};tables={}
    previous={}
    def interrupt(signum,frame):raise TimeoutError('mode deadline exceeded') if signum==signal.SIGALRM else KeyboardInterrupt(f'signal{signum}')
    try:
        summary['artifacts']={key:{'path':config[key],'sha256':bench.file_sha(config[key])} for key in ['model_path']+(['clef_head'] if 'clef_head' in config else [])}
        summary['binary_sha256']=bench.file_sha(variants[0]['binary'])
        summary['suite_sha256']=bench.file_sha(config['requests'][0])
        summary['harness_sha256']=bench.file_sha(__file__)
        host=bench.Host(config,output);host.thread.start()
        for signum in [signal.SIGTERM,signal.SIGINT,signal.SIGALRM]:previous[signum]=signal.signal(signum,interrupt)
        host.settle()
        with (output/'records.jsonl').open('w') as records:
            for mode in variants:
                name=mode['name'];responses[name]={}
                deadline=time.monotonic()+config.get('deadline_seconds',1800)
                signal.setitimer(signal.ITIMER_REAL,config.get('deadline_seconds',1800))
                try:
                    with bench.server(config,mode,0,output,records,host,export_dir=output/name) as endpoint:
                        for case in cases:
                            host.guard()
                            seconds,status,raw=post(endpoint,case['raw'],min(config['request_timeout'],max(.1,deadline-time.monotonic())))
                            (output/f'{name}-{case["id"]}.response.json').write_bytes(raw)
                            response=json.loads(raw)
                            bench.emit(records,{'kind':'response','mode':name,'id':case['id'],'request_sha256':case['digest'],'status':status,
                                'seconds':seconds,'response':response,'load':host.guard()})
                            bench.validate_uncached_response(status,response)
                            image_parity.validate_response(response,case['request'])
                            if response['psionic']['backend']!='metal' or response['psionic']['truncated_state_tokens']!=0:
                                raise ValueError('wrong backend or truncated request')
                            responses[name][case['id']]=response
                    tables[name]=exports(output/name,cases)
                    summary['completed_modes'].append(name)
                    write(output/'summary.json',summary)
                finally:signal.setitimer(signal.ITIMER_REAL,0)
        summary.update(compare(cases,responses,tables));summary['status']='complete' if summary['passed'] else 'failed'
    except BaseException as error:summary.update(error_type=type(error).__name__,error=str(error))
    finally:
        signal.setitimer(signal.ITIMER_REAL,0)
        for signum,handler in previous.items():signal.signal(signum,handler)
        if host is not None:
            host.stop.set();host.thread.join(timeout=6)
            if host.thread.is_alive() or host.monitor_error:summary.update(status='failed',passed=False,host_error=host.monitor_error or 'monitor did not stop')
        summary['finished_at']=bench.now();write(output/'summary.json',summary)
    print(json.dumps({k:v for k,v in summary.items() if k not in ['comparisons','config','effective_variants']},indent=2),flush=True)
    return 0 if summary['passed'] else 1


def self_test():
    case={'id':'text-000','digest':'sha256:x','request':{'questions':{'q':{'type':'noul'}}}}
    response={'usage':{'input_tokens':3},'answers':{'q':{'type':'noul','noul':.75}},
              'psionic':{'artifact_digest':'a','head_digest':'h','prefill_chunk':512,'backend':'metal','accumulate':'f32','truncated_state_tokens':0}}
    row={'logits':[2.,1.],'rows':['last','question','option:true','option:false'],'width':32,'index':0,'pooled_rows_sha256':'same'}
    responses={mode:{case['id']:copy.deepcopy(response)} for mode in ['staged','fused-fields']}
    tables={mode:{(case['digest'],'q'):copy.deepcopy(row)} for mode in responses}
    assert compare([case],responses,tables)['passed']
    for mutation in ['logit','top','tokens','pooled','metadata']:
        rs=copy.deepcopy(responses);ts=copy.deepcopy(tables)
        if mutation=='logit':ts['fused-fields'][(case['digest'],'q')]['logits'][0]+=.002
        elif mutation=='top':rs['fused-fields'][case['id']]['answers']['q']['noul']=.25
        elif mutation=='tokens':rs['fused-fields'][case['id']]['usage']['input_tokens']=4
        elif mutation=='pooled':ts['fused-fields'][(case['digest'],'q')]['pooled_rows_sha256']='different'
        else:rs['fused-fields'][case['id']]['psionic']['head_digest']='different'
        assert not compare([case],rs,ts)['passed'],mutation
    with tempfile.TemporaryDirectory(prefix='field-head-selftest-',dir=Path(__file__).parent) as directory:
        target=Path(directory)
        export=dict(row,request_sha256=case['digest'],question='q',question_index=0,offset=0,dtype='f16le')
        export.pop('index');export.pop('pooled_rows_sha256')
        (target/'rows.f16').write_bytes(bytes(4*32*2))
        (target/'rows.jsonl').write_text(json.dumps(export)+'\n')
        assert len(exports(target,[case]))==1
        for mutation in ['missing','duplicate','nonfinite','truncated','wrong_question','wrong_options']:
            bad=copy.deepcopy(export)
            data=bytes(4*32*2)
            if mutation=='nonfinite':bad['logits']=[float('nan'),1.]
            elif mutation=='truncated':data=data[:-2]
            elif mutation=='wrong_question':bad['question']='other'
            elif mutation=='wrong_options':bad['rows'][-1]='option:other'
            lines=[] if mutation=='missing' else [bad,bad] if mutation=='duplicate' else [bad]
            (target/'rows.f16').write_bytes(data)
            (target/'rows.jsonl').write_text(''.join(json.dumps(value)+'\n' for value in lines))
            try:exports(target,[case])
            except ValueError:pass
            else:raise AssertionError(mutation)
    print('self-test: exact fixtures pass; logit/top/token/pooled/model and missing/duplicate/nonfinite/identity export failures are refused')


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('config',nargs='?');parser.add_argument('--self-test',action='store_true')
    args=parser.parse_args()
    if args.self_test:self_test()
    elif args.config:raise SystemExit(run(bench.load_config(args.config)))
    else:parser.error('config or --self-test required')
