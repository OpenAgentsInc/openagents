#!/usr/bin/env python3
"""Count retained live calls once; replayed native replies do not add spend."""
import argparse
from collections import defaultdict
import json
from pathlib import Path

p = argparse.ArgumentParser(description=__doc__)
p.add_argument('records', type=Path)
p.add_argument('output', type=Path)
a = p.parse_args()
native = {}
unknown = []
jev = defaultdict(int)
for path in sorted(a.records.rglob('*.json')):
    if path.name not in {'review.json','program.json','assessment.json'} and not (path.parent.name == 'reproduced' and path.name.startswith('reply-')) and not path.parent.name.startswith(('report-audit-','report-features-','execution-audit-')):
        continue
    value = json.loads(path.read_text())
    if path.parent.name == 'reproduced' and path.name.startswith('reply-'):
        value = {'schema': 'openagents.coder-one.reproduced-review.v1', 'reply': value}
    if value.get('schema') not in {'openagents.coder-one.candidate-review.v1','openagents.coder-one.public-program.v1','openagents.coder-one.readiness.v1','openagents.coder-one.report-audit.v1','openagents.coder-one.report-features.v1','openagents.coder-one.execution-audit.v1','openagents.coder-one.reproduced-review.v1'}:
        continue
    relative = str(path.relative_to(a.records))
    reply = value.get('reply')
    if reply and reply.get('id'):
        key = reply['id']
        identity = {k:reply[k] for k in ['model','usage','cost_usd']}
        if key in native and any(native[key][k] != identity[k] for k in identity):
            raise ValueError('One provider reply has conflicting usage')
        native.setdefault(key, dict(identity, records=[]))['records'].append(relative)
    elif value.get('schema') == 'openagents.coder-one.reproduced-review.v1':
        if value.get('error') and not value.get('reviewer_source'):
            unknown.append(relative)
    elif path.name in {'review.json','program.json','assessment.json'} and not any(value.get(k) for k in ['reviewer_source','luna_source','generator_source']):
        unknown.append(relative)
    if path.name == 'review.json':
        jev[path.parent.name] += sum((f.get('input_tokens') if value.get('schema') == 'openagents.coder-one.reproduced-review.v1' else f.get('jev_input_tokens')) or 0 for f in value.get('findings',[]))
    elif path.name == 'program.json':
        jev['public-'+path.parent.name] += value.get('jev_input_tokens') or 0
    elif path.parent.name.startswith(('report-audit-','report-features-','execution-audit-')):
        jev[path.parent.name] += value.get('input_tokens') or 0
summary = dict(schema='openagents.truth-check-costs.v1',
    native_requests_with_usage=len(native), known_native_list_price_usd=sum(r['cost_usd'] or 0 for r in native.values()),
    native_cost_unknown_records=unknown, known_jev_input_tokens=dict(jev),
    known_jev_list_price_usd=sum(jev.values())*.042/1_000_000,
    notes=['List-price accounting is not an invoice for the logged-in subscription.',
           'Native response IDs deduplicate replayed replies.',
           'Provider failures without usage have unknown cost, not zero.',
           'This ledger excludes the benchmark executor trials; those are reported separately.'], native=native)
a.output.write_text(json.dumps(summary,indent=2)+'\n')
print(json.dumps({k:v for k,v in summary.items() if k!='native'},indent=2))
