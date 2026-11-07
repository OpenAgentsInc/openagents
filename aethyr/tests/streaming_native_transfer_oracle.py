# Firm reason: 2 (independent reconstruction of held-upload owners, attempted retirement and native receipt binding).
"""Check an explicit held-fence observation over an actual Vulkan upload."""
import copy
import hashlib
import json
from pathlib import Path
import struct
import sys
import streaming_native_oracle as native


def inspect(scene_dir, output, program, probe=None, report=None, stats=None):
    stats = native.inspect(scene_dir, output, program, report) if stats is None else stats
    raw = (output / 'receipt.json').read_bytes(); report = json.loads(raw) if report is None else report
    p = probe if probe is not None else json.loads((output / 'upload-probe.json').read_bytes())
    assert p['schema'] == 'aethyr.stream-native-upload-pending.v1'
    assert p['qualification'] == 'held fence observation after real Vulkan submission; no physical GPU delay'
    assert p['receiptSHA256'] == hashlib.sha256(raw).hexdigest()
    assert report['schema'] == 'aethyr.stream-walk-native-gpu.v2' and not report['route']
    q, guards = p['words'], p['guards']
    assert len(q) == 20 and all(type(x) is int for x in q) and guards == [0] * 8
    assert q[0] == 50 and q[1] > 0 and q[2] in (1,2) and 0 <= q[3] < 64
    frame = report['warmup'][0]
    target = next(i for i, s in enumerate(frame['stages']) if s[0] == q[3])
    stage = frame['stages'][target]
    assert q[4:8] == [stage[1],stage[2],frame['tick'],frame['tick']]
    assert q[4] > 0 and q[5] == 1 and frame['tick'] > 0
    assert 50_000_000 <= stage[6] < 10_000_000_000
    assert q[12] == q[13] and 0 <= q[12] < len(frame['allocations'])
    live = {}
    for freed, row, generation, epoch, field, count in frame['allocations'][:q[12]]:
        key = row, generation, epoch, field
        if freed: assert live.pop(key) == count
        else: assert key not in live; live[key] = count
    assert (q[3],q[4],q[5],4) in live  # The mapped upload allocation still exists.
    retained = sum(live.values()); assert q[8:10] == [retained,retained] and retained > 0
    scene = json.loads((scene_dir / 'scene.json').read_bytes())
    descriptor = next(m for m in scene['manifests'] if m['backend'] == 1 and m['tier'] == report['tier'])
    manifest = (scene_dir / (descriptor['sha256'] + '.rcm')).read_bytes()
    costs = [struct.unpack_from('<13q',manifest,48 + row * 440 + 44) for row in range(64)]
    kind = lambda row: 2 if manifest[48 + row * 440 + 4] == 5 else 1
    assert kind(q[3]) == q[2]
    assert all(costs[s[0]][2] == 0 or kind(s[0]) != q[2] for s in frame['stages'][:target])
    gpu_bytes = sum(costs[s[0]][2] for s in frame['stages'][:target + 1])
    assert q[10:12] == [gpu_bytes,gpu_bytes] and gpu_bytes > 0
    assert q[14:] == [1,1,1,0,0,0]  # Owned pending view; unchanged source; final retired success.
    source_rows = {row for row, generation, epoch, field in live if field == 0}
    source_record = bytearray()
    for row, resource in enumerate(scene['resources']):
        source = b''
        if row in source_rows:
            container = (scene_dir / (resource['container_sha256'] + ('.raw' if resource['raw'] else '.vpk'))).read_bytes()
            source = container[resource['offset']:resource['offset'] + resource['bytes']]
            assert hashlib.sha256(source).hexdigest() == resource['sha256']
        source_record += struct.pack('<qq',row,len(source)) + source
    expected = hashlib.sha256(source_record).hexdigest()
    assert p['sourceBefore'] == p['sourceAfter'] == expected
    return {**stats,'probe_schema':p['schema'],'transfer_kind':q[2],'held_poll_observations':q[1],
            'pending_cpu_bytes':retained,'pending_gpu_payload_bytes':q[10],
            'refused_early_calls':len(guards),'source_sha256_at_probe':expected,
            'physical_gpu_delay_qualified':False}


def fault_checks(scene, output, program):
    original = json.loads((output / 'upload-probe.json').read_bytes()); bad = []
    for field in range(20):
        p = copy.deepcopy(original); p['words'][field] = 0 if field in (1,2,14,15,16) else p['words'][field] + 1; bad.append(p)
    for field in range(8):
        p = copy.deepcopy(original);p['guards'][field] = 1;bad.append(p)
    for field in ('schema','qualification','receiptSHA256','sourceBefore','sourceAfter'):
        p = copy.deepcopy(original);p[field] = 'invalid';bad.append(p)
    for p in bad:
        try: inspect(scene,output,program,p)
        except (AssertionError,KeyError,StopIteration,IndexError,ValueError): pass
        else: raise AssertionError('altered held-upload evidence accepted')
    return len(bad)


if __name__ == '__main__':
    scene, output, program = map(Path,sys.argv[1:4]);result = inspect(scene,output,program)
    if '--fault-checks' in sys.argv[4:]:
        result['altered_probe_receipts_rejected'] = fault_checks(scene,output,program)
        result['altered_native_histories_rejected'] = native.fault_checks(scene,output,program)
    print(json.dumps(result,sort_keys=True))
