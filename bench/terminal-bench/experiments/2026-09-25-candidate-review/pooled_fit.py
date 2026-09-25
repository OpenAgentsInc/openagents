#!/usr/bin/env python3
"""Fit pooled development data, reserving fresh outcomes for confirmation."""
import argparse
from collections import Counter
import hashlib
import importlib.util
import json
from pathlib import Path
import numpy as np

helper = Path(__file__).parent.parent / '2026-09-25-truthful-checks/measure.py'
spec = importlib.util.spec_from_file_location('intervals', helper)
intervals = importlib.util.module_from_spec(spec)
spec.loader.exec_module(intervals)
REPORT = ['strict_grader_accepts','admits_unmet','rests_on_reading','left_untested','checked_against_task']
FAMILIES = [[f'report.{s}' for s in REPORT] + ['report.admitted']]
FAMILIES.append(FAMILIES[0] + ['execution.'+s for s in ['observed','unresolved','required']] + ['audit.'+s for s in ['observed','current','required']])
FAMILIES.append(FAMILIES[1] + ['source'])

def read(path):
    return json.loads(path.read_text()) if path.exists() else {}

def evidence(item, root):
    trial, part = item['trial'], item.get('evidence_split', item['split'])
    result, identities = {}, {}
    for prefix, path in [
        ('report', root/f'report-features-{part}'/f'{trial}.json'),
        ('execution',root/'execution-v2'/f'execution-audit-{part}'/f'{trial}.json'),
        ('audit',root/'execution-v2'/f'report-audit-{part}'/f'{trial}.json')]:
        value=read(path)
        identities[prefix] = hashlib.sha256(path.read_bytes()).hexdigest() if path.exists() else None
        if value and not value.get('error'):
            for k,v in (value.get('answers') or {}).items():result[prefix+'.'+k]=v.get('noul')
            if prefix=='report':result['report.admitted']=float(value['admitted'])
    source_path=root/trial/'astra/review.json'
    source=read(source_path)
    identities['source']=hashlib.sha256(source_path.read_bytes()).hexdigest() if source else None
    scores=[f['score'] for f in source.get('findings',[]) if f.get('score') is not None]
    result['source']=max(scores) if scores else None
    path=root/trial/'readiness/assessment.json'
    ready=read(path)
    identities['readiness']=hashlib.sha256(path.read_bytes()).hexdigest() if ready else None
    result['readiness']=(ready.get('assessment') or {}).get('failure_probability') if not ready.get('error') else None
    return result,identities

def vector(features, names):
    values=[features.get(n) for n in names]
    for v in values:
        if v is not None and (not np.isfinite(v) or not 0<=v<=1):raise ValueError('Invalid feature')
    return [v if v is not None else .5 for v in values] + [float(v is not None) for v in values]

def dot(x,w):return np.einsum("ij,j->i",x,w,optimize=False)

def sigmoid(x):return 1/(1+np.exp(-np.clip(x,-40,40)))

def fit(x,y,groups,ridge):
    counts=Counter(groups)
    weights=np.array([1/counts[g] for g in groups]);weights*=len(y)/weights.sum()
    w=np.zeros(x.shape[1]);b=0.0
    for _ in range(3000):
        residual=(sigmoid(dot(x,w)+b)-y)*weights
        w-=.5*(np.einsum("ij,i->j",x,residual,optimize=False)+ridge*w)/len(y)
        b-=.5*residual.mean()
    if not np.isfinite(w).all() or not np.isfinite(b):raise ValueError("Nonfinite fit")
    return w,b

def metrics(rows,predictions,t):
    for row,p in zip(rows,predictions):row['calls']['verdict.fusion']='fail' if p>=t else None
    return intervals.stats(rows,'verdict.fusion')

p=argparse.ArgumentParser(description=__doc__)
p.add_argument('--manifest',type=Path,required=True)
p.add_argument('--records',type=Path,required=True)
p.add_argument('--baseline',type=Path)
p.add_argument('--partition',required=True)
p.add_argument('--model',type=Path)
p.add_argument('--out',type=Path,required=True)
a=p.parse_args()
items=[r for r in read(a.manifest) if r['split']==a.partition]
baseline={(r['job'],r['trial']):r for r in read(a.baseline)['predictions']} if a.baseline else None
rows=[]
for item in items:
    row={k:item[k] for k in ['job','trial','task']}
    row['features'],row['record_sha256']=evidence(item,a.records)
    row['calls']={}
    if baseline:
        b=baseline[(row['job'],row['trial'])]
        row.update(reward=b['reward'],calls=dict(b['calls']))
    rows.append(row)
model=read(a.model) if a.model else None
result=dict(partition=a.partition,trials=len(rows),tasks=len({r['task'] for r in rows}))
if not model:
    if a.partition!='development' or baseline is None:raise ValueError('Fit only on labeled development')
    y=np.array([float(r['reward']==0) for r in rows]);groups=np.array([r['task'] for r in rows]);unique=sorted(set(groups))
    candidates=[]
    for family,names in enumerate(FAMILIES):
        x=np.array([vector(r['features'],names) for r in rows])
        for ridge in [.1,1.,10.]:
            oof=np.zeros(len(y))
            for task in unique:
                held=groups==task
                w,b=fit(x[~held],y[~held],groups[~held],ridge)
                oof[held]=sigmoid(dot(x[held],w)+b)
            table=[]
            for n in range(50,96):
                t=n/100
                m=metrics(rows,oof,t)
                table.append(dict(cutoff=t,called_tasks=len(set(groups[oof>=t])),metrics=m))
            candidates.append(dict(family=family,ridge=ridge,features=names,predictions=oof.tolist(),thresholds=table))
    eligible=[]
    for i,c in enumerate(candidates):
        for t in c['thresholds']:
            rate=t['metrics']['fail_precision']
            if rate['total']>=5 and rate['value']>=.9 and t['called_tasks']>=3:
                eligible.append((t['metrics']['failure_recall']['correct'],rate['correct']-rate['total'],-c['family'],c['ridge'],t['cutoff'],i))
    result['cross_validation']=candidates
    if eligible:
        *_,cutoff,i=max(eligible);chosen=candidates[i]
        x=np.array([vector(r['features'],chosen['features']) for r in rows])
        w,b=fit(x,y,groups,chosen['ridge'])
        model=dict(schema='openagents.evidence-fusion.v1',features=chosen['features'],weights=w.tolist(),bias=float(b),fail_at=cutoff,ridge=chosen['ridge'],family=chosen['family'],
                   out_of_fold=metrics(rows,chosen['predictions'],cutoff),selection='Task-held-out pooled development predictions only; fresh outcomes excluded; otherwise unknown.')
    else:
        for r in rows:r['calls']['verdict.fusion']=None
result['model']=model
if model:
    x=np.array([vector(r['features'],model['features']) for r in rows])
    predictions=sigmoid(dot(x,np.array(model['weights']))+model['bias'])
    for r,p in zip(rows,predictions):
        # With no observed feature, abstain regardless of the fitted intercept.
        usable=any(r['features'].get(f) is not None for f in model['features'])
        r['score']=float(p) if usable else None
        r['calls']['verdict.fusion']='fail' if usable and p>=model['fail_at'] else None
    if baseline:
        signals=['checks.final','verdict.combined','verdict.fusion']
        result.update(signals={s:intervals.stats(rows,s) for s in signals},paired_task_bootstrap={s:intervals.paired_bootstrap(rows,'verdict.fusion',s) for s in signals[:-1]})
result['predictions']=rows
a.out.write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps({k:v for k,v in result.items() if k not in ['predictions','cross_validation']},indent=2))
