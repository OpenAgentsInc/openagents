#!/usr/bin/env python3
"""Bound unresolved review contributions without reading labels or changing a model."""
import argparse
import json
import math
from pathlib import Path


def bounds(model, features, pending):
    names=model['features'];weights=model['weights'];n=len(names)
    if len(weights)!=2*n or not set(pending)<=set(names):raise ValueError('Invalid model or pending features')
    low=high=model['bias']
    for i,name in enumerate(names):
        w,p=weights[i],weights[n+i]
        if name in pending:
            values=[.5*w,p,w+p]
            low+=min(values);high+=max(values)
        else:
            v=features.get(name)
            if v is not None and (not math.isfinite(v) or not 0<=v<=1):raise ValueError('Invalid observed feature')
            contribution=.5*w if v is None else w*v+p
            low+=contribution;high+=contribution
    sigmoid=lambda z:1/(1+math.exp(-max(-40,min(40,z))))
    return sigmoid(low),sigmoid(high)


def verdict(model,features,pending):
    lo,hi=bounds(model,features,pending)
    if not pending and not any(features.get(f) is not None for f in model['features']):return None,(lo,hi),False
    if lo>=model['fail_at']:return 'fail',(lo,hi),False
    if hi<model['fail_at']:return None,(lo,hi),False
    return None,(lo,hi),True


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--predictions',type=Path,required=True)
    p.add_argument('--records',type=Path,required=True)
    p.add_argument('--manifest',type=Path,required=True)
    p.add_argument('--out',type=Path,required=True)
    p.add_argument('--needed',type=Path,required=True)
    a=p.parse_args();value=json.loads(a.predictions.read_text());model=value['model']
    manifest={(r['job'],r['trial']):r for r in json.loads(a.manifest.read_text())}
    needed=[];skipped=0
    for row in value['predictions']:
        packet=a.records/row['trial']/'readiness/assessment.json'
        process=a.records/row['trial']/'readiness/process.json'
        # An attempted call with a recorded terminal process error is unavailable,
        # while a review never acquired remains pending.
        attempted=packet.exists() or (process.exists() and 'deadline' in json.loads(process.read_text()).get('status',''))
        pending=[] if attempted else ['readiness']
        call,interval,unresolved=verdict(model,row['features'],pending)
        row['score_interval']=interval;row['pending']=pending;row['needs_review']=unresolved
        row['score']=interval[0] if not pending else None
        row['calls']['verdict.fusion']=call
        if unresolved:needed.append(manifest[(row['job'],row['trial'])])
        elif pending:skipped+=1
    value.update(unresolved=len(needed),reviews_skipped=skipped,
                 bounds_meaning='Range of frozen model scores for all possible pending answers, including unavailable. Not a statistical confidence interval.')
    a.out.write_text(json.dumps(value,indent=2)+'\n');a.needed.write_text(json.dumps(needed,indent=2)+'\n')
    print(json.dumps(dict(trials=len(value['predictions']),unresolved=len(needed),skipped=skipped)))

if __name__=='__main__':main()
