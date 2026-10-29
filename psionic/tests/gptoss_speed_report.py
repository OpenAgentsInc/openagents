#!/usr/bin/env python3
"""Summarize paired full-model speed trials; require all parity gates."""
import argparse, json, math, re, statistics
from pathlib import Path

def main():
    p=argparse.ArgumentParser();p.add_argument('log',type=Path);p.add_argument('out',type=Path);p.add_argument('--before-label',default='before: checked signed-64 decode indexing');p.add_argument('--after-label',default='after: admitted unsigned-32 decode indexing');a=p.parse_args();text=a.log.read_text()
    assert 'all paired full caches, logits, traces and expert IDs: bitwise equal' in text
    assert 'all greedy generated IDs and stops: bitwise equal' in text
    result=dict(profile='gpt-oss-cuda-f32-v1',model='original-20b',context_capacity=2048,concurrency=1,clock='host monotonic',variants={'0':a.before_label,'1':a.after_label},cases={})
    for case in ('paired','greedy'):
        rows=[]
        pattern=rf'{case} repeat=(-?\d+) variant=(\d) input=(\d+) (?:forced_)?output=(\d+) prefill=([\d.]+) decode=([\d.]+) tok_s=([\d.]+)'
        for repeat,variant,prefix,output,prefill,decode,rate in re.findall(pattern,text):
            rows.append(dict(repeat=int(repeat),variant=int(variant),input_tokens=int(prefix),output_tokens=int(output),prefill_seconds=float(prefill),decode_seconds=float(decode),decode_tokens_per_second=float(rate)))
        variants={}
        for v in range(2):
            subset=[r for r in rows if r['variant']==v];assert sorted(r['repeat'] for r in subset)==list(range(-1,5))
            measured=[r for r in subset if r['repeat']>=0];assert all(math.isfinite(r['decode_tokens_per_second']) and r['decode_tokens_per_second']>0 for r in measured)
            rates=sorted(r['decode_tokens_per_second'] for r in measured)
            variants[str(v)]=dict(runs=subset,median_decode_tokens_per_second=statistics.median(rates),minimum_decode_tokens_per_second=min(rates),maximum_decode_tokens_per_second=max(rates),median_prefill_seconds=statistics.median(r['prefill_seconds'] for r in measured))
        result['cases'][case]=dict(variants=variants,decode_rate_improvement_percent=(variants['1']['median_decode_tokens_per_second']/variants['0']['median_decode_tokens_per_second']-1)*100)
    result['limitations']=['The fixed 1024/128 case forces IDs; the greedy case uses actual model choices.','Both decode cases select on device and wait/read control per token, matching serving synchronization.','Core timing excludes HTTP/tokenization/serialization. Greedy rate excludes the first token chosen by prefill.','One warm-up per variant, five paired measured repeats; no concurrency or packaged-engine comparison.']
    a.out.write_text(json.dumps(result,indent=2)+'\n')
    print(json.dumps({k:{x:y for x,y in c.items() if x!='variants'}|{'rates':{v:data['median_decode_tokens_per_second'] for v,data in c['variants'].items()}} for k,c in result['cases'].items()},indent=2))
if __name__=='__main__':main()
