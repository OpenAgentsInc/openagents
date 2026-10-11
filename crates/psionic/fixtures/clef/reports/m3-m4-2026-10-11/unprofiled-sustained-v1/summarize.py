#!/usr/bin/env python3
"""Summarize every owned command without attributing a hardware cause."""
import argparse
from collections import Counter
import json
import math
from pathlib import Path
import statistics


def summarize(receipt, rows):
    complete=[r for r in rows if r.get('kind')=='complete']
    phases=[r for r in rows if r.get('kind')=='phase']
    timings=[r for r in rows if r.get('kind')=='timing']
    keys=Counter((r['phase'],r['ordinal']) for r in timings)
    expected={(p,i) for p in range(4) for i in range(128)}
    valid=(receipt.get('status')=='complete' and len(complete)==1 and complete[0]['passed']
           and complete[0]['timed_commands']==512 and complete[0]['total_commands']==514
           and complete[0]['total_dispatches']==2053 and not complete[0]['profiled']
           and len(phases)==4 and {p['phase'] for p in phases}==set(range(4))
           and all(p['commands']==128 and p['bitwise_passed'] and p['sources_unchanged'] for p in phases)
           and set(keys)==expected and all(v==1 for v in keys.values())
           and all(r['maximum_inflight']==[1,4,4,1][r['phase']]
                   and r['variant']==['wait_each','queued_four','queued_four','wait_each'][r['phase']]
                   and r['slot']==r['ordinal']%r['maximum_inflight']
                   and math.isfinite(r['gpu_ms']) and r['gpu_ms']>0 for r in timings))
    result={'coverage_and_correctness_passed':valid,'timed_rows':len(timings),'phases':[],
            'scope':'unprofiled owned full-FFN command intervals; shared machine; no model request',
            'limitations':['GPU command intervals are not active-shader occupancy or clock-frequency measurements.',
                           'CPU output checks between phases may change the next phase conditions.',
                           'The initial phase is not order-balanced; report its ramp separately.',
                           'Only drained phase outputs are compared fully; no per-command CPU readback occurs during streams.',
                           'No thermal, power-policy, or scheduling cause follows from a throughput change alone.']}
    for phase in range(4):
        data=sorted((r for r in timings if r['phase']==phase),key=lambda r:r['ordinal'])
        if not data:continue
        origin=data[0]['gpu_start_seconds']
        gaps=[1000*(b['gpu_start_seconds']-a['gpu_end_seconds']) for a,b in zip(data,data[1:])]
        windows=[]
        for first in range(0,len(data),16):
            part=data[first:first+16]
            windows.append({'first_ordinal':part[0]['ordinal'],'calls':len(part),'start_relative_gpu_seconds':part[0]['gpu_start_seconds']-origin,
                            'median_gpu_ms':statistics.median(r['gpu_ms'] for r in part),
                            'minimum_gpu_ms':min(r['gpu_ms'] for r in part),'maximum_gpu_ms':max(r['gpu_ms'] for r in part)})
        result['phases'].append({'phase':phase,'variant':data[0]['variant'],'calls':len(data),
            'median_gpu_ms':statistics.median(r['gpu_ms'] for r in data),'first_gpu_ms':data[0]['gpu_ms'],'last_gpu_ms':data[-1]['gpu_ms'],
            'median_between_command_gpu_gap_ms':statistics.median(gaps) if gaps else None,
            'negative_gap_count':sum(x<0 for x in gaps),'largest_gap_ms':max(gaps) if gaps else None,
            'gpu_first_to_last_ms':1000*(data[-1]['gpu_end_seconds']-data[0]['gpu_start_seconds']),
            'sum_gpu_intervals_ms':sum(r['gpu_ms'] for r in data),
            'median_cpu_encode_ms':statistics.median(1000*(r['commit_before_seconds']-r['encode_start_seconds']) for r in data),
            'windows_of_16':windows})
    result['completion']=complete[0] if len(complete)==1 else None
    return result


def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('run',type=Path);args=parser.parse_args()
    receipt=json.loads((args.run/'receipt.json').read_text())
    rows=[json.loads(line) for line in (args.run/'stdout.jsonl').read_text().splitlines()]
    result=summarize(receipt,rows)
    (args.run/'summary.json').write_text(json.dumps(result,indent=2,allow_nan=False)+'\n')
    print(json.dumps(result,indent=2,allow_nan=False))
    raise SystemExit(0 if result['coverage_and_correctness_passed'] else 1)

if __name__=='__main__':main()
