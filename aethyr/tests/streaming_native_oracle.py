# Firm reason: 2 (independent reconstruction of native SDK allocation lifetimes, source identities, binary32 walking and complete recovery readbacks).
"""Inspect actual Vulkan streaming receipts; no SDK calls or Coax imports.

The fixed-step SHA strings are retained observations, not an independent
simulation serializer. This oracle independently checks movement, allocation
and dependency lifetimes, retained source identity and every readback byte.
"""
from collections import Counter, defaultdict
import copy
import hashlib
import json
from pathlib import Path
import struct
import sys
import tempfile


def f32(x):
    return struct.unpack('<f', struct.pack('<f', x))[0]


def inspect(scene_dir, output, program):
    scene_bytes = (scene_dir / 'scene.json').read_bytes()
    scene = json.loads(scene_bytes)
    report = json.loads((output / 'receipt.json').read_bytes())
    assert report['schema'] in ('aethyr.stream-walk-native-gpu.v1', 'aethyr.stream-walk-native-gpu.v2')
    hud = report['schema'].endswith('.v2')
    assert report['backend'] == 1 and report['tier'] in (0, 1)
    assert report['shaderSHA256'] == hashlib.sha256(program.read_bytes()).hexdigest()
    if hud:
        assert report['presentationSchema'] == 'aethyr.stream-walk-display.v1'
        assert report['hudShaderSHA256'] == hashlib.sha256((program.parent / 'stream-hud.spv').read_bytes()).hexdigest()
        assert report['presentShaderSHA256'] == hashlib.sha256((program.parent / 'stream-present.spv').read_bytes()).hexdigest()
    assert report['sceneSHA256'] == hashlib.sha256(scene_bytes).hexdigest()
    descriptor = next(m for m in scene['manifests'] if m['backend'] == 1 and m['tier'] == report['tier'])
    assert report['manifestSHA256'] == descriptor['sha256']
    manifest = (scene_dir / (descriptor['sha256'] + '.rcm')).read_bytes()
    assert hashlib.sha256(manifest).hexdigest() == descriptor['sha256']
    costs = [struct.unpack_from('<13q', manifest, 48 + row * 440 + 44) for row in range(64)]
    categories = [manifest[48 + row * 440 + 5] for row in range(64)]
    dependencies = []
    sources = []
    for row, source in enumerate(scene['resources']):
        at = 48 + row * 440
        dependencies.append([struct.unpack_from('<I', manifest, at + 152 + i * 36)[0]
                             for i in range(struct.unpack_from('<I', manifest, at + 148)[0])])
        container = (scene_dir / (source['container_sha256'] + ('.raw' if source['raw'] else '.vpk'))).read_bytes()
        assert hashlib.sha256(container).hexdigest() == source['container_sha256']
        data = container[source['offset']:source['offset'] + source['bytes']]
        assert hashlib.sha256(data).hexdigest() == source['sha256']
        sources.append(data)
        assert costs[row][0] == len(data) and costs[row][5] == costs[row][2]
    assert len(report['warmup']) == 32 and len(report['route']) in (0, 3200)
    frames = report['warmup'] + report['route'] + report['cleanup']
    allocations, attempts, generation, ready = {}, set(), {}, {}
    outcomes = defaultdict(set)
    cpu = peak = events = 0
    owner_live, owner_peak, category_live, category_peak = Counter(), Counter(), Counter(), Counter()
    category_resource_peak = Counter()
    caps = (8192, 512, 4096, 2048, 8192, 4096, 2048)
    z, velocity, tick = f32(56), f32(0), 0
    elapsed, stage_elapsed, retained_at_recovery = [], [], None
    for index, frame in enumerate(frames):
        ns, direction = frame['frameNs'], frame['direction']
        assert ns in (15625000, 62500000) and direction in (-1, 0, 1)
        steps = ns // 15625000
        for _ in range(steps):
            if index >= len(report['warmup']) + len(report['route']):
                tick += 1
                continue
            difference = f32(f32(direction * 7.5) - velocity)
            change = min(abs(difference), .9375)
            velocity = f32(velocity + (change if difference > 0 else -change if difference < 0 else 0))
            candidate = f32(z + f32(velocity * .015625))
            if 0 <= candidate < 768:
                z = candidate
            else:
                velocity = f32(0)
            tick += 1
        assert frame['tick'] == tick
        assert frame['positionBits'] == [struct.unpack('<I', struct.pack('<f', v))[0] for v in (32, 0, z)]
        assert len(frame['stepDigests']) == steps
        assert all(len(h) == 64 and all(c in '0123456789abcdef' for c in h) for h in frame['stepDigests'])
        for record in frame['records']:
            applied, kind, row, gen, epoch, decoded, status = record[:7]
            key = row, gen, epoch
            assert key in attempts and 1 <= applied <= tick and decoded == 0 and status == 0
            assert kind in (0, 1, 2) and kind not in outcomes[key]
            assert record[7:15] == list(struct.unpack('>8I', bytes.fromhex(scene['resources'][row]['sha256'])))
            expected = hashlib.sha256(sources[row] if kind == 0 else b'').digest()
            assert record[15:23] == list(struct.unpack('>8I', expected))
            if kind == 2:
                assert 0 in outcomes[key] and all(d in ready for d in dependencies[row])
                ready[row] = gen
            elif kind == 1:
                assert 0 in outcomes[key]
                if ready.get(row) == gen:
                    del ready[row]
            outcomes[key].add(kind)
        for freed, row, gen, epoch, field, n in frame['allocations']:
            assert freed in (0, 1) and 0 <= row < 64 and gen > 0 and epoch == 1 and 0 <= field < 5
            assert n == (costs[row][0], costs[row][1], costs[row][3], costs[row][4], costs[row][5])[field] > 0
            key = row, gen, epoch, field
            category = 6 if field >= 3 else categories[row]
            if not freed:
                assert key not in allocations and (row not in generation or generation[row] == gen)
                allocations[key] = n
                generation[row] = gen
                attempts.add((row, gen, epoch))
            else:
                assert key in allocations
                assert allocations.pop(key) == n
                if not any(k[0] == row for k in allocations):
                    del generation[row]
            sign = -1 if freed else 1
            cpu += sign * n
            owner_live[row] += sign * n
            category_live[category] += sign * n
            peak = max(peak, cpu)
            owner_peak[row] = max(owner_peak[row], owner_live[row])
            category_peak[category] = max(category_peak[category], category_live[category])
            assert cpu >= 0 and all(0 <= category_live[c] <= caps[c] for c in range(7))
            events += 1
        for row, gen, epoch, *times in frame['stages']:
            assert (row, gen, epoch) in attempts and len(times) == 5
            assert all(t >= 0 for t in times) and sum(times[:4]) <= times[4]
            stage_elapsed.append(times)
        info, gpu = frame['info'], frame['gpu']
        assert len(info) == 28 and len(gpu) == (20 if hud else 19)
        if hud:
            assert gpu[19] == 0
        live_gpu = {row for row in generation if costs[row][2]}
        assert info[14] == cpu <= info[10]
        assert gpu[0] == len(live_gpu) and gpu[1] == sum(costs[row][2] for row in live_gpu)
        assert cpu + gpu[1] <= info[8] <= 32768
        category_with_gpu = category_live.copy()
        for row in live_gpu:
            category_with_gpu[categories[row]] += costs[row][2]
        for category, value in category_with_gpu.items():
            assert 0 <= value <= caps[category]
            category_resource_peak[category] = max(category_resource_peak[category], value)
        assert 0 <= info[6] <= info[4] <= 4 and 0 <= info[5] <= info[4] and info[7] <= 2
        assert gpu[2:4] == [0, 0] and gpu[9:12] == [0, 0, 1]
        assert gpu[16] >= gpu[1] + 4096 + 320 * 180 * 8 + (98304 + 256 + 320 * 180 * 8 if hud else 0)
        if hud:
            assert 600 < frame['hud'][0] < 2048 and frame['hud'][0] % 6 == 0
            assert frame['hud'][1:] == [gpu[7], 98304, 1, 1]
        assert frame['elapsedNs'] >= 0
        elapsed.append(frame['elapsedNs'] / 1e6)
        if index == 31:
            retained = b''.join(struct.pack('<qq', row, len(sources[row]) if row in generation else 0)
                                + (sources[row] if row in generation else b'') for row in range(64))
            retained_at_recovery = hashlib.sha256(retained).hexdigest()
            assert report['recovery']['cpuBytes'] == cpu and report['recovery']['attempts'] == len(attempts)
            assert report['recovery']['uploaded'] == len(live_gpu)
    recovery = report['recovery']
    assert recovery['sourceSHA256'] == recovery['sourceAfterSHA256'] == retained_at_recovery
    assert recovery['stateSHA256'] == recovery['stateAfterSHA256'] and len(recovery['stateSHA256']) == 64
    before, after = (output / 'before.rgba').read_bytes(), (output / 'after.rgba').read_bytes()
    assert before == after and len(before) == 320 * 180 * 4
    if hud:
        vertices = (output / 'before-hud.f32').read_bytes()
        assert vertices == (output / 'after-hud.f32').read_bytes() and len(vertices) % 48 == 0
        assert 600 < len(vertices) // 48 < 2048
    assert len(set(struct.iter_unpack('4B', before))) > 1
    assert not allocations and not generation and not ready and cpu == 0
    assert set(outcomes) == attempts and all(kinds == {0, 1, 2} for kinds in outcomes.values())
    assert len(stage_elapsed) == len(attempts)
    final, sdk = report['finalInfo'], report['finalGPU']
    assert len(final) == 28 and len(sdk) == (20 if hud else 19)
    assert final[20] == final[21] == len(attempts) and final[15] == peak
    assert all(final[i] == 0 for i in (4, 5, 6, 7, 8, 10, 11, 12, 13, 14, 22, 23))
    assert final[24] == 1 and sdk[0:7] == [0, 0, 0, 0, 0, 2, 2] and sdk[9:12] == [0, 0, 1]
    if hud:
        assert report['finalHUD'][1:] == [sdk[7], 98304, 0, 0]
        assert sdk[19] == 0 and report['fixedGrantsRetainedThroughDestruction']
    assert report['observations'] == [1] * 11 + [0] * 4 + [1] and report['passed']
    if report['route']:
        assert set(k[0] for k in attempts) == set(range(64))
        def position(frame):
            return struct.unpack('<f', struct.pack('<I', frame['positionBits'][2]))[0]
        assert position(report['route'][1599]) > 704 and position(report['route'][-1]) < 64
        assert len(report['routeCells']) == 2
        for leg in report['routeCells']:
            assert [c[3] for c in leg] == list(range(1, 13))
        assert report['routeCells'][1][0][2] > report['routeCells'][0][0][2] > 0
    elapsed.sort()
    quantiles = lambda values: {name: sorted(values)[int((len(values) - 1) * p)] for name, p in [('p50', .5), ('p95', .95), ('p99', .99)]}
    return {'tier': report['tier'], 'device': report['device'], 'frames': len(frames), 'ticks': tick, 'attempts': len(attempts),
            'physical_events': events, 'cpu_peak': peak, 'owner_cpu_peaks': dict(sorted(owner_peak.items())),
            'category_cpu_peaks': dict(sorted(category_peak.items())),
            'category_settled_cpu_logical_gpu_peaks': dict(sorted(category_resource_peak.items())),
            'sdk_memory_peak': sdk[17], 'core_static_bytes': final[27], 'sdk_static_bytes': sdk[18],
            'frame_ms': quantiles(elapsed), 'stage_ms': {name: quantiles([t[i] / 1e6 for t in stage_elapsed])
                                                       for i, name in enumerate(('allocation', 'source', 'decode', 'upload', 'total'))},
            'recovery_ms': recovery['elapsedNs'] / 1e6, 'recovery_image_sha256': hashlib.sha256(before).hexdigest(),
            'source_sha256': retained_at_recovery, 'final_zero_owners': True}


def fault_checks(scene_dir, output, program):
    report = json.loads((output / 'receipt.json').read_bytes())
    before, after = (output / 'before.rgba').read_bytes(), (output / 'after.rgba').read_bytes()
    faults = ('image_bit', 'short_image', 'state_hash', 'source_hash', 'cpu_bytes', 'gpu_count', 'gpu_bytes',
              'allocation_size', 'allocation_generation', 'missing_allocation', 'duplicate_free', 'missing_completion',
              'ready_key', 'scene_identity', 'shader_identity', 'missing_step', 'direction', 'position', 'tick',
              'final_count', 'schema', 'negative_time')
    hud = report['schema'].endswith('.v2')
    if hud:
        faults += ('hud_shader_identity', 'present_shader_identity', 'hud_draw_count', 'hud_early_release',
                   'close_violation', 'close_claim', 'changed_hud', 'display_schema')
    with tempfile.TemporaryDirectory(prefix='aethyr-native-oracle-') as temporary:
        target = Path(temporary)
        if report['schema'].endswith('.v2'):
            for file in ('before-hud.f32', 'after-hud.f32'):
                (target / file).write_bytes((output / file).read_bytes())
        for fault in faults:
            changed = copy.deepcopy(report)
            image = after
            if hud:
                (target / 'after-hud.f32').write_bytes((output / 'after-hud.f32').read_bytes())
            if fault == 'image_bit':
                image = bytes([after[0] ^ 1]) + after[1:]
            elif fault == 'short_image':
                image = after[:-1]
            elif fault in ('state_hash', 'source_hash'):
                changed['recovery']['stateAfterSHA256' if fault == 'state_hash' else 'sourceAfterSHA256'] = 'f' * 64
            elif fault in ('cpu_bytes', 'gpu_count', 'gpu_bytes'):
                frame = changed['warmup'][-1]
                frame['info' if fault == 'cpu_bytes' else 'gpu'][14 if fault == 'cpu_bytes' else 0 if fault == 'gpu_count' else 1] += 1
            elif fault in ('allocation_size', 'allocation_generation', 'missing_allocation'):
                frame = next(f for f in changed['warmup'] if f['allocations'])
                if fault == 'missing_allocation':
                    del frame['allocations'][0]
                else:
                    frame['allocations'][0][5 if fault == 'allocation_size' else 2] += 1
            elif fault == 'duplicate_free':
                frame = next(f for f in changed['cleanup'] if f['allocations'])
                frame['allocations'].append(frame['allocations'][0][:])
            elif fault in ('missing_completion', 'ready_key'):
                kind = 0 if fault == 'missing_completion' else 2
                frame, index = next((f, i) for f in changed['warmup'] for i, r in enumerate(f['records']) if r[1] == kind)
                if kind == 0:
                    del frame['records'][index]
                else:
                    frame['records'][index][2] = 63
            elif fault in ('scene_identity', 'shader_identity'):
                changed['sceneSHA256' if fault == 'scene_identity' else 'shaderSHA256'] = 'f' * 64
            elif fault == 'missing_step':
                changed['warmup'][0]['stepDigests'].pop()
            elif fault == 'direction':
                changed['warmup'][0]['direction'] = 1
            elif fault == 'position':
                changed['warmup'][0]['positionBits'][2] ^= 1
            elif fault == 'tick':
                changed['warmup'][0]['tick'] += 1
            elif fault == 'final_count':
                changed['finalInfo'][21] += 1
            elif fault == 'schema':
                changed['schema'] += '.unknown'
            elif fault == 'negative_time':
                changed['warmup'][0]['elapsedNs'] = -1
            elif fault in ('hud_shader_identity', 'present_shader_identity'):
                changed['hudShaderSHA256' if fault == 'hud_shader_identity' else 'presentShaderSHA256'] = 'f' * 64
            elif fault == 'hud_draw_count':
                changed['warmup'][0]['hud'][1] += 1
            elif fault == 'hud_early_release':
                changed['warmup'][0]['hud'][3] = 0
            elif fault == 'close_violation':
                changed['finalGPU'][19] = 1
            elif fault == 'close_claim':
                changed['fixedGrantsRetainedThroughDestruction'] = False
            elif fault == 'changed_hud':
                vertices = (output / 'after-hud.f32').read_bytes()
                (target / 'after-hud.f32').write_bytes(bytes([vertices[0] ^ 1]) + vertices[1:])
            elif fault == 'display_schema':
                changed['presentationSchema'] += '.unknown'
            (target / 'receipt.json').write_text(json.dumps(changed))
            (target / 'before.rgba').write_bytes(before)
            (target / 'after.rgba').write_bytes(image)
            try:
                inspect(scene_dir, target, program)
            except AssertionError:
                continue
            raise AssertionError('tampered native history accepted: ' + fault)
    return len(faults)


def compare_tiers(low, high):
    low, high = [json.loads((p / 'receipt.json').read_bytes()) for p in (low, high)]
    assert (low['tier'], high['tier']) == (0, 1)
    assert low['sceneSHA256'] == high['sceneSHA256'] and low['manifestSHA256'] != high['manifestSHA256']
    assert low['recovery']['sourceSHA256'] == high['recovery']['sourceSHA256']
    assert low['routeCells'] == high['routeCells']
    count = 0
    for section in ('warmup', 'route', 'cleanup'):
        assert len(low[section]) == len(high[section])
        for a, b in zip(low[section], high[section]):
            assert all(a[k] == b[k] for k in ('tick', 'positionBits', 'direction', 'frameNs', 'records'))
            assert a['info'][4:8] == b['info'][4:8]
            count += 1
    return {'tier_frames_compared': count, 'exact_movement_events_actor_identity': True,
            'canonical_state_hashes_shared_across_cost_profiles': False}


def compare_presentation(old, new):
    old, new = [json.loads((p / 'receipt.json').read_bytes()) for p in (old, new)]
    assert old['schema'] == 'aethyr.stream-walk-native-gpu.v1' and new['schema'] == 'aethyr.stream-walk-native-gpu.v2'
    assert old['manifestSHA256'] == new['manifestSHA256'] and old['routeCells'] == new['routeCells']
    ticks = 0
    for section in ('warmup', 'route', 'cleanup'):
        assert len(old[section]) == len(new[section])
        for a, b in zip(old[section], new[section]):
            assert all(a[k] == b[k] for k in ('tick', 'positionBits', 'direction', 'frameNs', 'records', 'stepDigests', 'allocations'))
            ticks += len(a['stepDigests'])
    return {'canonical_paid_steps_unchanged_by_hud': ticks, 'exact_events_allocations_actor_identity': True}


if __name__ == '__main__':
    if sys.argv[1] == '--compare-tiers':
        result = compare_tiers(Path(sys.argv[2]), Path(sys.argv[3]))
    elif sys.argv[1] == '--compare-presentation':
        result = compare_presentation(Path(sys.argv[2]), Path(sys.argv[3]))
    else:
        scene, output, program = map(Path, sys.argv[1:4])
        result = inspect(scene, output, program)
        if '--fault-checks' in sys.argv[4:]:
            result['tampered_histories_rejected'] = fault_checks(scene, output, program)
    print(json.dumps(result, sort_keys=True))
