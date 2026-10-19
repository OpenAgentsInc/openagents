#!/usr/bin/env python3
"""#11816: build storage-cleanup proofs for one synthetic owner's deleted computers (read-only)."""
import hashlib, json, subprocess, sys, time, importlib.util
from pathlib import Path
spec = importlib.util.spec_from_file_location('q16', '/space/config/deployment-tools/q16.py'); q = importlib.util.module_from_spec(spec); spec.loader.exec_module(q)
ROOT = Path('/space/qualification-11816'); owner = q.owner(sys.argv[1])
def canon_digest(v): return hashlib.sha256(json.dumps(v, sort_keys=True, separators=(',', ':')).encode()).hexdigest()
objects = json.loads(subprocess.run(['python3', '/space/config/deployment-tools/q16_objects.py'], capture_output=True, check=True, timeout=300).stdout)
s, sandboxes = q.native('GET', '/admin/api/v1/sandboxes?limit=500', admin=True)
live_native = {('sbx_' + x['id']) for x in sandboxes if x.get('lifecycle') not in ('deleted',)} if s == 200 and isinstance(sandboxes, list) else None
rows = json.loads(q.sql(f"SELECT json_agg(json_build_object('id',id,'phase',phase,'record',record,'binding',binding)) FROM computers.computers WHERE owner='{owner}' AND id IN (SELECT DISTINCT computer FROM computers.storage_reservations WHERE owner='{owner}' AND state='held')"))
receipt = {'observed_at': objects['observed_at'], 'snapshot_object_groups': objects['groups'], 'native_sandboxes': sorted(live_native) if live_native is not None else None}
receipt_bytes = json.dumps(receipt, sort_keys=True).encode(); (ROOT / f'cleanup-inventory-{sys.argv[1]}.json').write_bytes(receipt_bytes)
receipt_sha = hashlib.sha256(receipt_bytes).hexdigest()
proofs, refused = [], []
now = int(time.time() * 1000)
for r in rows or []:
    rec, binding = r['record'], r['binding'] or {}
    resources = sorted({b['resource'] for b in rec.get('boots', []) if b.get('resource')} | {c['resource'] for c in rec.get('creates', []) if c.get('resource')})
    ns = binding.get('operation_namespace')
    ops = sorted({canon_digest({'schema': 'openagents.computers.effect.v1', 'deployment': ns, 'operation': c['operation']}) if ns else c['operation'] for c in rec.get('creates', [])})
    retained = sum(objects['groups'].get(x[4:], {}).get('objects', 0) for x in resources)
    live = sum(1 for x in resources if live_native is None or x in live_native)
    proof = {'schema': 'openagents.computers.storage-cleanup.v1', 'owner': owner, 'computer': r['id'], 'resources': resources, 'create_operations': ops,
             'observed_ms': now, 'live_resources': live, 'retained_objects': retained, 'unfinished_uploads': 0, 'unresolved_operations': 0, 'inventory_receipt_sha256': receipt_sha}
    (proofs if r['phase'] == 'deleted' and live == 0 and retained == 0 else refused).append(proof)
(ROOT / f'cleanup-proofs-{sys.argv[1]}.json').write_text(json.dumps(proofs))
print(json.dumps({'owner': owner, 'computers_with_held_storage': len(rows or []), 'provable': len(proofs), 'not_yet': [{'computer': p['computer'], 'live': p['live_resources'], 'retained': p['retained_objects']} for p in refused], 'inventory_receipt_sha256': receipt_sha}))
