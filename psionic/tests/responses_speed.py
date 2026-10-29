#!/usr/bin/env python3
"""Opt-in end-to-end SSE timing on an owned running test instance. No model
content is logged; raw reasoning/final state is checked by a digest only."""
import argparse, hashlib, json, os, statistics, time, urllib.request

def main():
    p=argparse.ArgumentParser();p.add_argument('url');p.add_argument('--out');a=p.parse_args();key=os.environ['PSIONIC_API_KEY']
    body=json.dumps(dict(model='gpt-oss-20b',input='List the integers from 1 to 100, separated by commas.',max_output_tokens=128,stream=True)).encode();rows=[];digest=None
    for repeat in range(-1,5):
        req=urllib.request.Request(a.url.rstrip('/')+'/v1/responses',data=body,headers={'Content-Type':'application/json','Authorization':'Bearer '+key})
        start=time.monotonic();first=None;visible=None;response=None
        with urllib.request.urlopen(req,timeout=125) as stream:
            for line in stream:
                if not line.startswith(b'data: '):continue
                event=json.loads(line[6:]);elapsed=time.monotonic()-start
                if event['type'].endswith('.delta') and first is None:first=elapsed
                if event['type']=='response.output_text.delta' and visible is None:visible=elapsed
                if event['type'] in ('response.completed','response.incomplete','response.failed'):response=event['response'];duration=elapsed
        assert response and response['status'] in ('completed','incomplete') and response['usage']['output_tokens']==128
        state=[{k:v for k,v in item.items() if k not in ('id','status')} for item in response['output']];current=hashlib.sha256(json.dumps(state,sort_keys=True,ensure_ascii=False).encode()).hexdigest()
        if digest is None:digest=current
        assert current==digest,'greedy output changed'
        row=dict(repeat=repeat,input_tokens=response['usage']['input_tokens'],output_tokens=128,seconds=duration,first_delta_seconds=first,first_visible_text_seconds=visible,end_to_end_output_tokens_per_second=128/duration)
        if repeat>=0:rows.append(row)
    result=dict(profile='psionic-gptoss-responses-v1',workload='list integers 1..100; greedy; 128 output IDs; one connection',output_state_sha256=digest,runs=rows,median_seconds=statistics.median(r['seconds'] for r in rows),median_output_tokens_per_second=statistics.median(r['end_to_end_output_tokens_per_second'] for r in rows),median_first_delta_seconds=statistics.median(r['first_delta_seconds'] for r in rows),limitations=['End-to-end rate includes tokenization, reset, prefill, decoding, serialization and transport.','Semantic first delta follows Harmony header generation; it is not the first selected token.','Five repeats are a smoke benchmark, not production capacity qualification.'])
    if a.out:open(a.out,'w').write(json.dumps(result,indent=2)+'\n')
    print(json.dumps({k:v for k,v in result.items() if k!='runs'},indent=2))
if __name__=='__main__':main()
