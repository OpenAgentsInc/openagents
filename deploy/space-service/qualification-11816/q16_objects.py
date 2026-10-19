#!/usr/bin/env python3
"""#11816: group active snapshot objects by sandbox ID and age (read-only; prints no object contents)."""
import importlib.util, json, re, sys, xml.etree.ElementTree as ET
from collections import defaultdict
from datetime import datetime, timezone
sys.path.insert(0, '/space/config/deployment-tools')
spec = importlib.util.spec_from_file_location('inv', '/space/config/deployment-tools/storage_inventory.py'); inv = importlib.util.module_from_spec(spec); spec.loader.exec_module(inv)
cred = inv.secret('object-credentials', inv.identity()); store = inv.protocol().Store(cred, 'openagentsgemini-oa-space-service-snapshots')
groups = defaultdict(lambda: {'objects': 0, 'bytes': 0, 'newest': None, 'oldest': None}); cursor = None; now = datetime.now(timezone.utc)
for _ in range(50):
    q = {'list-type': 2, 'max-keys': 1000}
    if cursor: q['continuation-token'] = cursor
    page = ET.fromstring(store.request('GET', query=q)[1])
    for row in page.findall('{*}Contents'):
        key = row.findtext('{*}Key'); m = re.search(r'([0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12})', key)
        sid = m.group(1) if m else 'other:' + key.split('/')[0]
        t = row.findtext('{*}LastModified'); g = groups[sid]; g['objects'] += 1; g['bytes'] += int(row.findtext('{*}Size'))
        g['newest'] = max(g['newest'] or t, t); g['oldest'] = min(g['oldest'] or t, t)
    if page.findtext('{*}IsTruncated') == 'false': break
    cursor = page.findtext('{*}NextContinuationToken')
print(json.dumps({'observed_at': now.isoformat(), 'groups': groups}))
