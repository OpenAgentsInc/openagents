#!/usr/bin/env python3
"""Summarize five fixed core runs; correctness and throughput are distinct."""
import argparse,json,math,re,statistics
from pathlib import Path

def main():
    ap=argparse.ArgumentParser();ap.add_argument('log',type=Path);ap.add_argument('output',type=Path);a=ap.parse_args();text=a.log.read_text()
    rows=[];warm=[]
    for repeat,prefill,decode,rate in re.findall(r'forced-token batched workload repeat=(-?\d+) input=1024 output=128 prefill=([\d.]+) decode=([\d.]+) tok_s=([\d.]+)',text):
        row=dict(repeat=int(repeat),prefill_seconds=float(prefill),decode_seconds=float(decode),decode_tokens_per_second=float(rate));assert all(math.isfinite(row[k]) and row[k]>0 for k in ('prefill_seconds','decode_seconds','decode_tokens_per_second'))
        (warm if row['repeat']<0 else rows).append(row)
    assert len(warm)==1 and [r['repeat'] for r in rows]==list(range(5)), 'one warm-up and five measured repeats required'
    def phase(name):
        values=sorted(r[name] for r in rows)
        return dict(p50=statistics.median(values),p95_nearest_rank=values[math.ceil(.95*len(values))-1],minimum=values[0],maximum=values[-1])
    pre=phase('prefill_seconds');dec=phase('decode_seconds');rate=phase('decode_tokens_per_second')
    result=dict(profile='gpt-oss-cuda-f32-v1',model='original-20b',input_tokens=1024,forced_output_tokens=128,context_capacity=2048,concurrency=1,clock='host monotonic wall time',corpus='ID[i]=1000+(i*7919)%190000, first 128 repeated as continuation',warmup=warm,runs=rows,prefill_seconds=pre,decode_seconds=dec,decode_tokens_per_second=rate,goals=dict(prefill_at_most_2_seconds=pre['p50']<=2,decode_at_least_25_tokens_per_second=rate['p50']>=25),limitations=['Forced tokens measure core work, not sampled quality.','No packaged-reference or concurrency 4/8 qualification.','Five repeats give a coarse tail estimate.'])
    a.output.write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result,indent=2))
if __name__=='__main__':main()
