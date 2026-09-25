#!/usr/bin/env python3
"""Assess only candidates whose frozen agreement verdict needs another signal."""
import argparse
import json
from pathlib import Path
import subprocess

p=argparse.ArgumentParser(description=__doc__)
p.add_argument('--manifest',type=Path,required=True)
p.add_argument('--records',type=Path,required=True)
p.add_argument('--binary',type=Path,required=True)
p.add_argument('--rule',type=Path,required=True)
p.add_argument('--partition',required=True)
a=p.parse_args()
rule=json.loads(a.rule.read_text())
rows=[r for r in json.loads(a.manifest.read_text()) if r['split']==a.partition]
eligible=[]
for row in rows:
    path=a.records/('report-audit-'+a.partition)/(row['trial']+'.json')
    audit=json.loads(path.read_text()) if path.exists() else {}
    if audit.get('score') is not None and audit['score']>=rule['report_at']:
        continue
    path=a.records/row['trial']/'astra/review.json'
    review=json.loads(path.read_text()) if path.exists() else {}
    scores=[f['score'] for f in review.get('findings',[]) if f.get('score') is not None]
    if scores and max(scores)>=rule['source_at']:
        eligible.append(row)
manifest=a.records/('readiness-gated-'+a.partition+'.json')
manifest.write_text(json.dumps(eligible,indent=2)+'\n')
print('Completion assessment needed for',len(eligible),'of',len(rows),'candidates; all remain in the measurement',flush=True)
subprocess.run(['python3',str(Path(__file__).with_name('readiness.py')),'--manifest',str(manifest),
    '--audits',str(a.records/('report-audit-'+a.partition)),'--out',str(a.records),
    '--binary',str(a.binary),'--partition',a.partition],check=True)
