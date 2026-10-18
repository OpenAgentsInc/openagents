#!/usr/bin/env python3
"""Build #11816 case receipts and the qualification packet from scrubbed raw evidence.

Usage: build_packet.py SPEC.json DOCS_SDK_DIR
SPEC names identity, observed_unix, flags, limits, economics, trial files, and
for each case its status and raw source file (already under DOCS_SDK_DIR).
"""
import hashlib, json, sys
from pathlib import Path

spec = json.loads(Path(sys.argv[1]).read_text())
root = Path(sys.argv[2])
base = spec['evidence_dir']  # relative to docs/sdk
def sha(p): return hashlib.sha256((root / p).read_bytes()).hexdigest()
cases = {}
for case, entry in spec['cases'].items():
    status = entry['status']
    if entry.get('source'):
        source = {'path': entry['source'], 'sha256': sha(entry['source'])}
        receipt = {'schema': 'openagents.computers.space-case-receipt.v1', 'identity': spec['identity'],
                   'observed_unix': spec['observed_unix'], 'cases': {case: status}, 'source': source}
        rpath = f"{base}/receipts/{case}.json"
        (root / rpath).parent.mkdir(parents=True, exist_ok=True)
        (root / rpath).write_text(json.dumps(receipt, indent=2, sort_keys=True) + '\n')
        cases[case] = {'status': status, 'artifact': {'path': rpath, 'sha256': sha(rpath)}}
    else:
        cases[case] = {'status': status, 'artifact': None}
trials = []
for f in spec['trial_files']:
    for line in (root / f).read_text().splitlines():
        r = json.loads(line)
        # A snapshot trial succeeds only if the guest actually restored from a base snapshot.
        ok = r['succeeded'] and not (r['kind'] == 'snapshot' and spec.get('snapshot_boot_mode_observed') != 'snapshot')
        trials.append({'kind': r['kind'], 'cache': r['cache'], 'concurrency': r['concurrency'], 'succeeded': ok,
                       'timings': {'create_ms': r.get('create_ms'), 'reconcile_ms': None, 'readiness_ms': r.get('readiness_ms'),
                                   'attach_ms': None, 'repository_ms': None, 'model_ms': None,
                                   'first_useful_ms': r.get('first_useful_ms') if r['succeeded'] else r.get('first_useful_ms')}})
packet = {'schema': 'openagents.computers.space-qualification.v1', 'identity': spec['identity'], 'observed_unix': spec['observed_unix'],
          'advertised': spec['advertised'], 'accounts': spec['accounts'], 'dedicated_test_accounts': True,
          'private_management': spec['private_management'], 'rollback_ready': spec['rollback_ready'], 'pricing_enabled': False,
          'limits': spec['limits'], 'cases': cases, 'trials': trials, 'economics': spec['economics']}
(root / 'space-qualification-packet.json').write_text(json.dumps(packet, indent=2) + '\n')
print(json.dumps({'cases': {s: sum(1 for c in cases.values() if c['status'] == s) for s in ('passed', 'failed', 'pending')}, 'trials': len(trials)}))
