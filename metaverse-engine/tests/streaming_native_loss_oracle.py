# Firm reason: 2 (independent native loss-result, full-image, allocation, source/dependency and fixed-step reconstruction).
"""SDK result injection is qualified separately from physical device loss."""
from collections import Counter, defaultdict
import copy
import hashlib
import json
from pathlib import Path
import struct
import sys


def f32(x):
    return struct.unpack('<f', struct.pack('<f', x))[0]


def inspect(scene_dir, output, program, report=None, images=None):
    r = report if report is not None else json.loads((output / 'receipt.json').read_bytes())
    assert r['schema'] == 'aethyr.stream-walk-native-gpu.v3'
    assert r['qualification'] == 'SDK result injection after actual Vulkan submissions; no physical GPU reset'
    mode = r['lossMode']
    assert mode in ('--auto-loss', '--auto-repeat', '--auto-restart', '--auto-timeout', '--auto-fail', '--auto-oom', '--auto-route', '--auto-late')
    stopped = mode in ('--auto-repeat', '--auto-timeout', '--auto-fail', '--auto-oom', '--auto-late')
    assert r['backend'] == 1 and r['tier'] in (0, 1)
    scene_raw = (scene_dir / 'scene.json').read_bytes(); scene = json.loads(scene_raw)
    assert r['sceneSHA256'] == hashlib.sha256(scene_raw).hexdigest()
    descriptor = next(m for m in scene['manifests'] if m['backend'] == 1 and m['tier'] == r['tier'])
    assert r['manifestSHA256'] == descriptor['sha256']
    manifest = (scene_dir / (descriptor['sha256'] + '.rcm')).read_bytes()
    assert hashlib.sha256(manifest).hexdigest() == descriptor['sha256']
    for field, path in [('shaderSHA256', program), ('hudShaderSHA256', program.parent / 'stream-hud.spv'), ('presentShaderSHA256', program.parent / 'stream-present.spv')]:
        assert r[field] == hashlib.sha256(path.read_bytes()).hexdigest()
    assert r['presentationSchema'] == 'aethyr.stream-walk-display.v1'
    costs = [struct.unpack_from('<13q', manifest, 48 + i * 440 + 44) for i in range(64)]
    categories = [manifest[48 + i * 440 + 5] for i in range(64)]
    dependencies, sources = [], []
    for row, source in enumerate(scene['resources']):
        at = 48 + row * 440
        dependencies.append([struct.unpack_from('<I', manifest, at + 152 + i * 36)[0] for i in range(struct.unpack_from('<I', manifest, at + 148)[0])])
        raw = (scene_dir / (source['container_sha256'] + ('.raw' if source['raw'] else '.vpk'))).read_bytes()
        assert hashlib.sha256(raw).hexdigest() == source['container_sha256']
        payload = raw[source['offset']:source['offset'] + source['bytes']]
        assert hashlib.sha256(payload).hexdigest() == source['sha256'] and len(payload) == costs[row][0]
        sources.append(payload)
    expected_groups = 4 if mode == '--auto-repeat' else 3 if mode == '--auto-route' else 1
    assert r['groups'] == expected_groups
    assert len(r['warmup']) == (36 if mode == '--auto-repeat' else 33)
    assert len(r['route']) == (3202 if mode == '--auto-route' else 0 if stopped else 8)
    frames = r['warmup'] + r['route'] + r['cleanup']
    live, generation, attempts, outcomes, ready = {}, {}, set(), defaultdict(set), {}
    category_live, category_peak, owner_live, owner_peak = Counter(), Counter(), Counter(), Counter()
    costs_by_category = Counter(); peak = current = event_count = stages = 0
    snapshots = {}; z = f32(56); velocity = f32(0); tick = 0; elapsed = []
    caps = (8192, 512, 4096, 2048, 8192, 4096, 2048)
    for index, frame in enumerate(frames):
        ns, direction = frame['frameNs'], frame['direction']
        assert ns in (0, 15625000, 62500000) and direction in (-1, 0, 1)
        count = ns // 15625000
        for _ in range(count):
            if index < len(r['warmup']) + len(r['route']):
                delta = f32(f32(direction * 7.5) - velocity); change = min(abs(delta), .9375)
                velocity = f32(velocity + (change if delta > 0 else -change if delta < 0 else 0))
                candidate = f32(z + f32(velocity * .015625))
                if 0 <= candidate < 768: z = candidate
                else: velocity = f32(0)
            tick += 1
        assert frame['tick'] == tick and len(frame['stepDigests']) == count
        assert frame['positionBits'] == [struct.unpack('<I', struct.pack('<f', x))[0] for x in (32, 0, z)]
        assert all(len(mark) == 64 and len(bytes.fromhex(mark)) == 32 for mark in frame['stepDigests'])
        for record in frame['records']:
            applied, kind, row, gen, epoch, decoded, status = record[:7]; key = row, gen, epoch
            assert len(record) == 23 and key in attempts and 0 < applied <= tick and decoded == status == 0
            assert kind in (0, 1, 2) and kind not in outcomes[key]
            assert record[7:15] == list(struct.unpack('>8I', bytes.fromhex(scene['resources'][row]['sha256'])))
            assert record[15:23] == list(struct.unpack('>8I', hashlib.sha256(sources[row] if kind == 0 else b'').digest()))
            if kind == 2:
                assert 0 in outcomes[key] and all(d in ready for d in dependencies[row]); ready[row] = gen
            elif kind == 1:
                assert 0 in outcomes[key]
                if ready.get(row) == gen: del ready[row]
            outcomes[key].add(kind)
        for freed, row, gen, epoch, field, n in frame['allocations']:
            assert freed in (0, 1) and 0 <= row < 64 and gen > 0 and epoch == 1 and 0 <= field < 5
            assert n == (costs[row][0], costs[row][1], costs[row][3], costs[row][4], costs[row][5])[field] > 0
            key = row, gen, epoch, field; category = 6 if field >= 3 else categories[row]
            if not freed:
                assert key not in live and (row not in generation or generation[row] == gen)
                live[key] = n; generation[row] = gen; attempts.add((row, gen, epoch))
            else:
                assert key in live and live.pop(key) == n
                if not any(k[0] == row for k in live): del generation[row]
            sign = -1 if freed else 1; current += sign * n; category_live[category] += sign * n; owner_live[row] += sign * n
            peak = max(peak, current); category_peak[category] = max(category_peak[category], category_live[category]); owner_peak[row] = max(owner_peak[row], owner_live[row])
            assert current >= 0 and all(0 <= category_live[c] <= caps[c] for c in range(7)); event_count += 1
        for row, gen, epoch, *times in frame['stages']:
            assert (row, gen, epoch) in attempts and len(times) == 5 and all(t >= 0 for t in times) and sum(times[:4]) <= times[4]; stages += 1
        info, gpu = frame['info'], frame['gpu']; assert len(info) == 28 and len(gpu) == 20
        assert info[14] == current <= info[10] and current + gpu[1] <= info[8] <= 32768
        assert 0 <= info[6] <= info[4] <= 4 and 0 <= info[5] <= info[4] and info[7] <= 2
        assert gpu[2:4] == [0, 0] and gpu[9:12] == [0, 0, 1] and gpu[19] == 0
        # A refused replacement owns CPU bytes without a live physical GPU.
        gpu_rows = {i for i in generation if costs[i][2]} if gpu[4] else set()
        assert gpu[0] == len(gpu_rows) and gpu[1] == sum(costs[i][2] for i in gpu_rows)
        combined = category_live.copy()
        for row in gpu_rows: combined[categories[row]] += costs[row][2]
        for category, n in combined.items():
            assert 0 <= n <= caps[category]; costs_by_category[category] = max(costs_by_category[category], n)
        assert 600 < frame['hud'][0] < 2048 and frame['hud'][0] % 6 == 0
        assert frame['hud'][2] == 98304 and frame['hud'][1] >= gpu[7]
        assert frame['elapsedNs'] >= 0; elapsed.append(frame['elapsedNs'] / 1e6)
        retained = b''.join(struct.pack('<qq', row, len(sources[row]) if row in generation else 0) + (sources[row] if row in generation else b'') for row in range(64))
        snapshots[tick] = (hashlib.sha256(retained).hexdigest(), current, len(attempts))
    assert not live and not generation and not ready and current == 0
    assert set(outcomes) == attempts and all(kinds == {0, 1, 2} for kinds in outcomes.values()) and stages == len(attempts)
    events = r['lossEvents']; expected_count = 4 if mode == '--auto-repeat' else 2 if mode == '--auto-restart' else 3 if mode == '--auto-route' else 1
    assert len(events) == expected_count
    assert [e['tick'] for e in events] == ([128, 3332, 9732] if mode == '--auto-route' else [128] * expected_count)
    recovered = 0
    for e in events:
        assert e['stateRetained'] == e['sourceRetained'] == 1
        assert len(bytes.fromhex(e['stateBefore'])) == 32 and e['stateBefore'] == e['stateAfter']
        assert (e['sourceBefore'], e['cpu'], e['reads']) == snapshots[e['tick']] and e['sourceBefore'] == e['sourceAfter']
        assert e['endNs'] >= e['startNs'] and e['realResult'] == 0
        if e['outcome'] == 2:
            recovered += 1
            assert e['result'] == -4 and e['reason'] == 0 and e['oldClosed'] == 1 and e['pending'] == 0
            assert e['uploaded'] == e['granted'] > 0
            assert 0 <= e['endNs'] / 1e6 - e['beganMs'] < 10000
    expected_recovered = 3 if mode in ('--auto-repeat', '--auto-route') else 1 if mode in ('--auto-loss', '--auto-restart') else 0
    assert recovered == expected_recovered
    policy = r['lossPolicy']; assert len(policy) == 7 and policy[0] == 0 and bool(policy[1]) == stopped
    assert policy[2] == policy[3] == (3 if mode in ('--auto-repeat', '--auto-route') else 2 if mode == '--auto-restart' else 0 if mode in ('--auto-timeout', '--auto-oom', '--auto-late') else 1)
    assert policy[5] == (1 if mode == '--auto-repeat' else 2 if mode in ('--auto-timeout', '--auto-late') else 3 if mode in ('--auto-fail', '--auto-oom') else 0)
    assert r['stoppedFrameRefused'] and r['quiesceNs'] >= 0
    if mode == '--auto-repeat': assert [e['serial'] for e in events] == [1, 2, 3, 3] and [e['outcome'] for e in events] == [2, 2, 2, 4]
    if mode == '--auto-restart': assert [e['outcome'] for e in events] == [3, 2] and events[0]['beganMs'] == events[1]['beganMs']
    if mode == '--auto-timeout':
        assert events[0]['result'] == 2 and events[0]['pending'] == 1
        assert r['stopRetained'][:2] == [1, 1] and all(x > 0 for x in r['stopRetained'][2:])
        assert 10000000000 <= r['warmup'][-1]['elapsedNs'] < 10200000000
        assert 10000000 <= r['quiesceNs'] < 1000000000
    if mode == '--auto-late':
        assert events[0]['result'] == 0 and events[0]['pending'] == 0
        assert r['stopRetained'][:2] == [0, 0] and all(x > 0 for x in r['stopRetained'][2:])
        assert 10000000000 <= r['warmup'][-1]['elapsedNs'] < 10300000000
    if mode == '--auto-fail': assert events[0]['outcome'] == 4 and events[0]['oldClosed'] == 1 and events[0]['uploaded'] == 0
    if mode == '--auto-oom': assert events[0]['result'] == -1 and events[0]['outcome'] == 4 and events[0]['oldClosed'] == 0
    image_count = expected_groups - (1 if stopped else 0)
    images = images if images is not None else {p.name: p.read_bytes() for p in output.glob('loss-*.rgba')}
    for i in range(expected_groups):
        before = images[f'loss-before-{i}.rgba']; assert len(before) == 230400 and len(set(struct.iter_unpack('4B', before))) > 1
        if i < image_count: assert before == images[f'loss-after-{i}.rgba']
        else: assert f'loss-after-{i}.rgba' not in images
    final, gpu = r['finalInfo'], r['finalGPU']; submission = r['finalSubmission']
    assert final[0] == tick and final[20] == final[21] == len(attempts) and final[15] == peak
    assert all(final[i] == 0 for i in (4, 5, 6, 7, 8, 10, 11, 12, 13, 14, 22, 23)) and final[24] == 1
    closes = 4 if mode in ('--auto-repeat', '--auto-route') else 3 if mode == '--auto-restart' else 2 if mode == '--auto-loss' else 1
    assert gpu[:7] == [0, 0, 0, 0, 0, closes, closes] and gpu[9:12] == [0, 0, 1] and gpu[16] == gpu[19] == 0
    assert submission[0] == submission[2] == submission[5] == submission[6] == 0
    assert submission[4] == (4 if mode == '--auto-repeat' else 3 if mode == '--auto-route' else 2 if mode == '--auto-restart' else 0 if mode == '--auto-timeout' else 1)
    assert r['finalHUD'][2:] == [98304, 0, 0] and r['fixedGrantsRetainedThroughDestruction']
    assert r['observations'] == [1] * 12 and r['passed']
    if mode == '--auto-route':
        assert {key[0] for key in attempts} == set(range(64))
        assert [e['tick'] for e in events] == [128, 3332, 9732]
        assert all([c[3] for c in leg] == list(range(1, 13)) for leg in r['routeCells'])
        assert r['routeCells'][1][0][2] > r['routeCells'][0][0][2] > 0 and z < 64
    quantiles = {name: sorted(elapsed)[int((len(elapsed) - 1) * p)] for name, p in [('p50', .5), ('p95', .95), ('p99', .99)]}
    return {'mode': mode, 'tier': r['tier'], 'device': r['device'], 'events': len(events), 'recoveries': recovered,
            'ticks': tick, 'attempts': len(attempts), 'allocation_events': event_count, 'cpu_peak': peak,
            'category_cpu_peaks': dict(category_peak), 'category_cpu_logical_gpu_peaks': dict(costs_by_category),
            'explicit_vulkan_peak': gpu[17], 'core_static_bytes': final[27], 'sdk_static_bytes': gpu[18],
            'equal_image_channels': image_count * 230400, 'frame_ms': quantiles, 'quiesce_ms': r['quiesceNs'] / 1e6,
            'final_zero_owners': True, 'physical_device_fault_qualified': False}


def fault_checks(scene, output, program):
    original = json.loads((output / 'receipt.json').read_bytes()); images = {p.name: p.read_bytes() for p in output.glob('loss-*.rgba')}
    faults = ['schema', 'provenance', 'shader', 'source', 'state', 'cpu', 'paid_tick', 'real_result', 'final_owner', 'policy', 'observation', 'allocation', 'completion', 'motion', 'step', 'fixed_order', 'pending_final']
    if any(name.startswith('loss-after') for name in images): faults += ['image']
    if original['lossMode'] == '--auto-timeout': faults += ['early_stop', 'early_retirement']
    if original['lossMode'] == '--auto-late': faults += ['early_stop', 'completed_still_pending']
    for fault in faults:
        changed = copy.deepcopy(original); pictures = images.copy()
        if fault == 'schema': changed['schema'] = 'aethyr.stream-walk-native-gpu.v4'
        elif fault == 'provenance': changed['qualification'] = 'physical device loss'
        elif fault == 'shader': changed['shaderSHA256'] = '0' * 64
        elif fault == 'source': changed['lossEvents'][0]['sourceBefore'] = '0' * 64
        elif fault == 'state': changed['lossEvents'][0]['stateAfter'] = '0' * 64
        elif fault == 'cpu': changed['lossEvents'][0]['cpu'] += 1
        elif fault == 'paid_tick': changed['lossEvents'][0]['tick'] += 1
        elif fault == 'real_result': changed['lossEvents'][0]['realResult'] = -4
        elif fault == 'final_owner': changed['finalInfo'][14] = 1
        elif fault == 'policy': changed['lossPolicy'][3] += 1
        elif fault == 'observation': changed['observations'][4] = 0
        elif fault == 'allocation': next(f for f in changed['warmup'] if f['allocations'])['allocations'][0][5] += 1
        elif fault == 'completion': next(f for f in changed['warmup'] if f['records'])['records'][0][3] += 1
        elif fault == 'motion': changed['warmup'][0]['positionBits'][2] ^= 1
        elif fault == 'step': changed['warmup'][0]['stepDigests'].pop()
        elif fault == 'fixed_order': changed['finalGPU'][19] = 1
        elif fault == 'pending_final': changed['finalSubmission'][0] = 1
        elif fault == 'image':
            name = next(name for name in pictures if name.startswith('loss-after')); pictures[name] = bytes([pictures[name][0] ^ 1]) + pictures[name][1:]
        elif fault == 'early_stop': changed['warmup'][-1]['elapsedNs'] = 9999999999
        elif fault == 'completed_still_pending': changed['stopRetained'][0] = 1
        elif fault == 'early_retirement': changed['stopRetained'][0] = 0
        try: inspect(scene, output, program, changed, pictures)
        except AssertionError: continue
        raise AssertionError('altered native loss receipt accepted: ' + fault)
    return len(faults)


def compare_profiles(first, second):
    a, b = [json.loads((p / 'receipt.json').read_bytes()) for p in (first, second)]
    assert a['schema'] == b['schema'] == 'aethyr.stream-walk-native-gpu.v3'
    assert a['lossMode'] == b['lossMode'] == '--auto-route'
    assert a['sceneSHA256'] == b['sceneSHA256'] and a['routeCells'] == b['routeCells']
    same_costs = a['manifestSHA256'] == b['manifestSHA256']
    assert [e['sourceBefore'] for e in a['lossEvents']] == [e['sourceBefore'] for e in b['lossEvents']]
    count = steps = 0
    for section in ('warmup', 'route', 'cleanup'):
        assert len(a[section]) == len(b[section])
        for x, y in zip(a[section], b[section]):
            assert all(x[k] == y[k] for k in ('tick', 'positionBits', 'direction', 'frameNs', 'records'))
            assert x['info'][4:8] == y['info'][4:8]
            if same_costs: assert x['stepDigests'] == y['stepDigests']
            count += 1; steps += len(x['stepDigests'])
    return {'frames_compared': count, 'paid_steps': steps, 'identical_accepted_motion_events_actor_identity': True,
            'same_manifest': same_costs, 'canonical_digest_equality': same_costs}


if __name__ == '__main__':
    if sys.argv[1] == '--compare':
        print(json.dumps(compare_profiles(Path(sys.argv[2]), Path(sys.argv[3])), sort_keys=True)); sys.exit(0)
    scene, output, program = map(Path, sys.argv[1:4]); result = inspect(scene, output, program)
    if '--fault-checks' in sys.argv[4:]: result['altered_receipts_rejected'] = fault_checks(scene, output, program)
    print(json.dumps(result, sort_keys=True))
