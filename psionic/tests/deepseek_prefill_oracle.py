#!/usr/bin/env python3
"""Owned scheduling/bucketing oracle; fixture gate jitter dominates bounded data."""
import argparse,gzip,json
from pathlib import Path
R=Path(__file__).resolve().parents[2];D=R/'psionic/fixtures/deepseek'
def fixture():
    ids=[[[j for j in sorted(range(3),key=lambda j:(-((t+l+3*j)%7),j))[:2]] for t in range(13)] for l in range(2)]
    plans=[]
    for cap in [2,4,7,13]:
        chunks=[]
        for first in range(0,13,cap):
            n=min(cap,13-first);layers=[]
            for l in range(2):
                flat=sum(ids[l][first:first+n],[]);order=[a for e in range(3) for a,v in enumerate(flat) if v==e];counts=[flat.count(e) for e in range(3)];offsets=[0]
                for count in counts:offsets.append(offsets[-1]+count)
                keys=[e for e in range(3) if counts[e]];layers.append(dict(ids=flat,order=order,offsets=offsets,keys=keys))
            chunks.append(dict(first=first,count=n,layers=layers))
        plans.append(dict(cap=cap,chunks=chunks,fetches=sum(len(l['keys']) for c in chunks for l in c['layers'])))
    return dict(schema=1,ids=ids,plans=plans)
def main():
    p=argparse.ArgumentParser();p.add_argument('--write',action='store_true');p.add_argument('--header',action='store_true');a=p.parse_args();j=fixture()
    if a.header:
        print('#include <stdint.h>');print('static const int64_t dpf_ids[]={'+','.join(str(v) for layer in j['ids'] for row in layer for v in row)+'};')
        print('static const int64_t dpf_fetches[]={'+','.join(str(x['fetches']) for x in j['plans'])+'};')
        print('typedef struct {int64_t cap,first,layer,groups,order[26],offsets[4],keys[3];} DpfPlan;')
        rows=[]
        for plan in j['plans']:
            for chunk in plan['chunks']:
                for layer,row in enumerate(chunk['layers']):
                    fields=[plan['cap'],chunk['first'],layer,len(row['keys'])];parts=list(map(str,fields))+['{'+','.join(map(str,row[name]))+'}' for name in ['order','offsets','keys']];rows.append('{'+','.join(parts)+'}')
        print('static const DpfPlan dpf_plans[]={'+','.join(rows)+'};');return
    data=(json.dumps(j,separators=(',',':'))+'\n').encode();path=D/'prefill-goldens.json.gz'
    if a.write:path.write_bytes(gzip.compress(data,mtime=0))
    else:assert gzip.decompress(path.read_bytes())==data
    print('Prefill: independent gate IDs, stable assignment buckets, offsets and one-fetch-per-expert counts for chunks 2/4/7/13 agree')
if __name__=='__main__':main()
