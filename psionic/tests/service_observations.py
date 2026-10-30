#!/usr/bin/env python3
"""Replay retained Part III measurement claims without a model or device."""
import hashlib
import json
import math
from pathlib import Path

root = Path(__file__).resolve().parents[1] / 'fixtures/serving/partIII-2026-10-29'
def read(path):
    return json.loads((root / path).read_text())

def rank(values, percent):
    return sorted(values)[math.ceil(len(values) * percent / 100) - 1]

manifest = read('manifest.json')
actual = {str(p.relative_to(root)) for p in root.rglob('*') if p.is_file() and p.name != 'manifest.json'}
assert actual == set(manifest['files'])
for name, entry in manifest['files'].items():
    data = (root / name).read_bytes()
    assert len(data) == entry['bytes'] and hashlib.sha256(data).hexdigest() == entry['sha256'], name

for name in ('cuda', 'metal', 'metal-prefix-before', 'metal-prefix-after'):
    data = read(name + '/benchmark.json')
    for work in data['workloads']:
        assert len(work['client_us']) == data['samples_per_workload']
        for p in (50, 95, 99):
            assert work[f'p{p}_us'] == rank(work['client_us'], p)
        request = json.dumps(work['request'], sort_keys=True, separators=(',', ':')).encode()
        assert hashlib.sha256(request).hexdigest() == work['request_sha256']
    rows = [json.loads(line) for line in (root / name / 'metrics.jsonl').read_text().splitlines()]
    assert all(row['build'] == data['ready']['build'] for row in rows)
    for row in rows:
        assert row['schema'] == 'psionic-decision-metrics-v1'
        assert not set(row).intersection(('state', 'answers', 'instructions', 'probabilities', 'question_ids'))
        assert row['tokens'] >= row['prefix_tokens'] >= row['reused_tokens'] >= 0
        if row['kind'] == 0:
            assert row['status'] == 200 and row['questions'] == data['questions']
            assert row['prefix_hits'] == (2 if name == 'metal-prefix-after' else 0)
    startup = data['startup'][0]
    assert startup['execution_payload_bytes'] == 3288560
    assert startup['cpu_execution_payload_bytes'] - startup['execution_payload_bytes'] == 688458240

before = read('metal-prefix-before/benchmark.json')['workloads'][0]
after = read('metal-prefix-after/benchmark.json')['workloads'][0]
assert before['retained_answer'] == after['retained_answer'] and after['reused_tokens'] == 346
pairs = read('cpu-prefix-pairs/pairs.json')
assert len(pairs['workloads']) == 3
for work in pairs['workloads']:
    assert len(work['pairs']) == 100
    assert [p['first'] for p in work['pairs']] == ['before', 'after'] * 50
    for variant in ('before', 'after'):
        for p in (50, 95, 99):
            assert work[variant][f'p{p}_us'] == rank([x[variant + '_us'] for x in work['pairs']], p)
for variant in ('before', 'after'):
    rows = [json.loads(line) for line in (root / 'cpu-prefix-pairs' / (variant + '.jsonl')).read_text().splitlines()]
    requests = [row for row in rows if row['kind'] == 0]
    assert len(requests) == 303
    for row in requests:
        assert row['status'] == 200 and row['questions'] == 3
        assert row['prefix_hits'] == (2 if variant == 'after' else 0)
        assert row['question_phases_us'][0]['prefix_hit'] == 0

cold, full, verified = read('authentication/observations.json')
assert cold['cached_pages'] == 0 and cold['file_cache_at_open'] == 'cold'
assert full['cached_pages'] == verified['cached_pages'] == full['total_pages'] == 1148665
assert full['file_cache_at_open'] == verified['file_cache_at_open'] == 'warm'
assert cold['questions'] == full['questions'] == verified['questions'] == 0
managed = read('managed/operations.json')
assert managed['success'] and managed['before']['answers'] == managed['after']['answers']
assert managed['killed_child_pid'] != managed['replacement_child_pid']
assert managed['final_state'] == 'inactive' and managed['boot_state'] == 'static'
device = read('metal-operations/operations.json')
assert device['success'] and device['prefix_hit_rows'] == [2, 2, 2]
print('Part III observations: digests, raw ranks, request identities, reuse, memory and operational records pass')
