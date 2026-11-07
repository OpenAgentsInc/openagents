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
import os
from pathlib import Path
import struct
import subprocess
import sys
import tempfile
from streaming_assets_oracle import png as fixture_png


def f32(x):
    return struct.unpack('<f', struct.pack('<f', x))[0]


def inspect_category(scene_dir, output, report, costs):
    """Reconstruct the refused candidate independently from its source PNG."""
    scene = json.loads((scene_dir / 'scene.json').read_bytes())
    p = report['categoryPressure']; assert p['schema'] == 'aethyr.stream-category-pressure.v1'
    probe = scene['overcap_probe']; source = (scene_dir / (probe['sha256'] + '.raw')).read_bytes()
    assert hashlib.sha256(source).hexdigest() == probe['sha256'] == p['sourceSHA256'] and len(source) == probe['bytes']
    width, height, pixels, work = fixture_png(source)
    assert (width, height) == (8, 8) and pixels == bytes((180,172,128,255)) * 64
    mip_bytes = 4 * sum((8 >> level) ** 2 for level in range(4))
    plan = (len(source), mip_bytes, mip_bytes, 168, work, mip_bytes, 4, 0, 0, 8, 8, 4, mip_bytes // 4)
    digest_words = lambda data: struct.pack('<8I', *struct.unpack('>8I', hashlib.sha256(data).digest()))
    revision = b'RCM1/positions4-1/indices-1/query-box-1/query-capsule-1/htx-1/apw-1/cell-body-1'
    companion = (struct.pack('<4sIII', b'RCM1', 1, 1, 440) + digest_words(revision) +
                 struct.pack('<IBBBBI', 1, 5, 1, 1, 0, 8) + digest_words(source) + struct.pack('<13q', *plan) + bytes(292))
    assert len(companion) == 488 and (output / 'pressure.rcm').read_bytes() == companion
    assert hashlib.sha256(companion).hexdigest() == p['manifestSHA256']
    warm = report['warmup'][-1]; retained = sum(plan[:4]); scratch = plan[4] + plan[5]
    live = {}; queue_serial = 0
    for frame in report['warmup']:
        for freed, row, gen, epoch, field, extent in frame['allocations']:
            key = row, gen, epoch, field
            if freed: del live[key]
            else: live[key] = extent; queue_serial += 1
    owners = len({key[0] for key in live})
    assert p['words'] == [2,retained,scratch,sum(costs[0][:4]),512,0,2048,warm['info'][8],32768,owners,1,1,1]
    q = p['words']; assert q[1] > q[4] - q[3] and q[2] <= q[6] - q[5] and q[1] + q[2] <= q[8] - q[7]
    assert p['before'] == p['after'] and len(p['before']) == 9
    assert p['before'][:7] == [warm['tick'],warm['info'][10],warm['info'][11],queue_serial,1,warm['gpu'][0] + 6,warm['gpu'][1] // 4 + 256064]
    assert p['before'][7] == warm['info'][20] and p['before'][8] > 0
    assert p['cpuBytes'] == warm['info'][14] and p['gpuBytes'] == warm['gpu'][1] and p['gpuOwners'] == warm['gpu'][0] and p['traceEvents'] == 0
    assert p['stateBefore'] == p['stateNormalized'] != p['stateAfter']
    assert all(len(bytes.fromhex(p[key])) == 32 for key in ('stateBefore','stateAfter','stateNormalized'))
    image = (output / 'pressure-before.rgba').read_bytes()
    assert image == (output / 'pressure-after.rgba').read_bytes() == (output / 'before.rgba').read_bytes() and len(image) == 230400
    assert len(report['route']) == 8 and report['route'][0]['tick'] == 129 and report['route'][-1]['tick'] == 136
    return {'requested_retained':retained,'requested_scratch':scratch,'texture_used':q[3],'texture_cap':q[4],
            'aggregate_used':q[7],'aggregate_cap':q[8],'manifest_sha256':p['manifestSHA256'],
            'unchanged_image_channels':len(image),'continued_paid_steps':8}


def inspect(scene_dir, output, program):
    scene_bytes = (scene_dir / 'scene.json').read_bytes()
    scene = json.loads(scene_bytes)
    report = json.loads((output / 'receipt.json').read_bytes())
    assert report['schema'] in ('aethyr.stream-walk-native-gpu.v1', 'aethyr.stream-walk-native-gpu.v2', 'aethyr.stream-walk-native-gpu.v4', 'aethyr.stream-walk-native-gpu.v5')
    pressure = report['schema'].endswith('.v4')
    failure = report['schema'].endswith('.v5')
    hud = report['schema'] != 'aethyr.stream-walk-native-gpu.v1'
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
    failed_key = None; failure_case = -1; failure_status = 0
    if failure:
        fault = report['loadFailure']; assert fault['schema'] == 'aethyr.stream-native-load-failure.v1'
        assert fault['qualification'] == 'explicit allocator/source/scratch effect input; no physical OS exhaustion'
        failure_case = fault['case']; q = fault['effect']; assert 0 <= failure_case <= 6 and len(q) == 10
        assert q[:5] == [1 if failure_case < 5 else failure_case - 3, failure_case if failure_case < 5 else -1, 0, 1, 0]
        assert q[5] > 0 and q[6] == 1
        failure_status = 2 if failure_case == 5 else 3
        expected_bytes = (costs[0][0],costs[0][1],costs[0][3],costs[0][4],costs[0][5])[failure_case] if failure_case < 5 else 0 if failure_case == 5 else costs[0][4] - 1
        assert q[7:] == [expected_bytes,12 if failure_case < 5 else -1 if failure_case == 5 else 3,failure_status]
        failed_key = (q[4],q[5],q[6])
    assert len(report['warmup']) == 32 and (len(report['route']) == 8 if pressure or failure else len(report['route']) in (0, 3200))
    frames = report['warmup'] + report['route'] + report['cleanup']
    allocations, attempts, generation, ready = {}, set(), {}, {}
    outcomes = defaultdict(set)
    staged = set(); allocated_fields = defaultdict(set); gpu_owners = {}; completed_at = {}; destroyed_at = {}; retry_generation = None
    cpu = peak = events = 0
    owner_live, owner_peak, category_live, category_peak = Counter(), Counter(), Counter(), Counter()
    category_resource_peak = Counter()
    caps = (8192, 512, 4096, 2048, 8192, 4096, 2048)
    z, velocity, tick = f32(56), f32(0), 0
    elapsed, stage_elapsed, retained_at_recovery = [], [], None
    for index, frame in enumerate(frames):
        if failure:
            for row,gen,epoch,*_ in frame['stages']:
                key = row,gen,epoch
                assert 0 <= row < 64 and gen > 0 and epoch == 1 and key not in staged
                staged.add(key); attempts.add(key)
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
            assert key in attempts and 1 <= applied <= tick
            is_failure = key == failed_key and kind == 0
            assert decoded == (failure_status if is_failure else 0) and status == (6 if is_failure else 0)
            assert kind in (0, 1, 2) and kind not in outcomes[key]
            assert record[7:15] == list(struct.unpack('>8I', bytes.fromhex(scene['resources'][row]['sha256'])))
            actual_source = sources[row] if kind == 0 else b''
            if is_failure and failure_case <= 5: actual_source = b'' if failure_case == 0 else bytes(len(sources[row]))
            expected = hashlib.sha256(actual_source).digest()
            assert record[15:23] == list(struct.unpack('>8I', expected))
            if kind == 2:
                assert key != failed_key
                assert 0 in outcomes[key] and all(d in ready for d in dependencies[row])
                ready[row] = gen
            elif kind == 1:
                assert 0 in outcomes[key]
                if ready.get(row) == gen:
                    del ready[row]
                destroyed_at[key] = applied
            elif kind == 0: completed_at[key] = applied
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
                allocated_fields[row,gen,epoch].add(field)
                if (row,gen,epoch) == failed_key and failure_case < 5: assert field < failure_case
            else:
                assert key in allocations
                assert allocations.pop(key) == n
                if not any(k[0] == row for k in allocations):
                    del generation[row]
                if failure and field == 0 and gpu_owners.get(row) == gen: del gpu_owners[row]
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
            if failure:
                if (row,gen,epoch) != failed_key and costs[row][2]: gpu_owners[row] = gen
                if row == 0 and (row,gen,epoch) != failed_key:
                    assert failed_key in destroyed_at and tick >= destroyed_at[failed_key] and gen > failed_key[1]
                    retry_generation = gen
        info, gpu = frame['info'], frame['gpu']
        assert len(info) == 28 and len(gpu) == (20 if hud else 19)
        if hud:
            assert gpu[19] == 0
        live_gpu = set(gpu_owners) if failure else {row for row in generation if costs[row][2]}
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
    assert set(outcomes) == attempts and all(kinds == ({0,1} if key == failed_key else {0,1,2}) for key,kinds in outcomes.items())
    if failure:
        assert len(attempts) == 19 and not gpu_owners and retry_generation is not None
        for key in attempts:
            expected = {i for i,n in enumerate((costs[key[0]][0],costs[key[0]][1],costs[key[0]][3],costs[key[0]][4],costs[key[0]][5])) if n > 0}
            if key == failed_key and failure_case < 5: expected = {i for i in expected if i < failure_case}
            assert allocated_fields[key] == expected
        assert completed_at[failed_key] < destroyed_at[failed_key]
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
    if len(report['route']) == 3200:
        assert set(k[0] for k in attempts) == set(range(64))
        def position(frame):
            return struct.unpack('<f', struct.pack('<I', frame['positionBits'][2]))[0]
        assert position(report['route'][1599]) > 704 and position(report['route'][-1]) < 64
        assert len(report['routeCells']) == 2
        for leg in report['routeCells']:
            assert [c[3] for c in leg] == list(range(1, 13))
        assert report['routeCells'][1][0][2] > report['routeCells'][0][0][2] > 0
    pressure_stats = inspect_category(scene_dir, output, report, costs) if pressure else None
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
            'source_sha256': retained_at_recovery, 'final_zero_owners': True,
            **({'category_pressure': pressure_stats} if pressure else {}),
            **({'load_failure': {'case':failure_case,'failed_key':failed_key,'allocated_fields':sorted(allocated_fields[failed_key]),
                 'completed_tick':completed_at[failed_key],'destroyed_tick':destroyed_at[failed_key],'retry_generation':retry_generation,
                 'continued_paid_steps':8,'physical_os_exhaustion_qualified':False}} if failure else {})}


def fault_checks(scene_dir, output, program):
    report = json.loads((output / 'receipt.json').read_bytes())
    before, after = (output / 'before.rgba').read_bytes(), (output / 'after.rgba').read_bytes()
    faults = ('image_bit', 'short_image', 'state_hash', 'source_hash', 'cpu_bytes', 'gpu_count', 'gpu_bytes',
              'allocation_size', 'allocation_generation', 'missing_allocation', 'duplicate_free', 'missing_completion',
              'ready_key', 'scene_identity', 'shader_identity', 'missing_step', 'direction', 'position', 'tick',
              'final_count', 'schema', 'negative_time')
    hud = report['schema'] != 'aethyr.stream-walk-native-gpu.v1'
    pressure = report['schema'].endswith('.v4')
    failure = report['schema'].endswith('.v5')
    if failure:
        faults += tuple('load_effect_' + str(i) for i in range(10)) + ('load_case','load_provenance','load_status','load_ready','load_source','load_missing_stage','load_missing_retirement')
    if pressure: faults += tuple('pressure_word_' + str(i) for i in range(13)) + ('pressure_source','pressure_state','pressure_owner','pressure_image','pressure_tick','pressure_no_continue') + tuple('pressure_cost_' + str(i) for i in range(13))
    if hud:
        faults += ('hud_shader_identity', 'present_shader_identity', 'hud_draw_count', 'hud_early_release',
                   'close_violation', 'close_claim', 'changed_hud', 'display_schema')
    with tempfile.TemporaryDirectory(prefix='aethyr-native-oracle-') as temporary:
        target = Path(temporary)
        if hud:
            for file in ('before-hud.f32', 'after-hud.f32'):
                (target / file).write_bytes((output / file).read_bytes())
        for fault in faults:
            changed = copy.deepcopy(report)
            image = after
            if pressure:
                for file in ('pressure-before.rgba','pressure-after.rgba','pressure.rcm'):
                    (target / file).write_bytes((output / file).read_bytes())
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
            elif fault.startswith('load_effect_'): changed['loadFailure']['effect'][int(fault.rsplit('_',1)[1])] += 1
            elif fault == 'load_case': changed['loadFailure']['case'] += 1
            elif fault == 'load_provenance': changed['loadFailure']['qualification'] = 'physical OS exhaustion'
            elif fault.startswith('load_'):
                key = tuple(report['loadFailure']['effect'][4:7])
                records = [r for f in changed['warmup'] for r in f['records'] if tuple(r[2:5]) == key]
                completion = next(r for r in records if r[1] == 0)
                if fault == 'load_status': completion[5] = 0
                elif fault == 'load_ready': completion[1] = 2
                elif fault == 'load_source': completion[15] ^= 1
                elif fault == 'load_missing_stage':
                    for f in changed['warmup']: f['stages'] = [s for s in f['stages'] if tuple(s[:3]) != key]
                elif fault == 'load_missing_retirement':
                    for f in changed['warmup']: f['records'] = [r for r in f['records'] if not (r[1] == 1 and tuple(r[2:5]) == key)]
            elif fault.startswith('pressure_word_'):
                changed['categoryPressure']['words'][int(fault.rsplit('_',1)[1])] += 1
            elif fault == 'pressure_source': changed['categoryPressure']['sourceSHA256'] = 'f' * 64
            elif fault == 'pressure_state': changed['categoryPressure']['stateNormalized'] = 'f' * 64
            elif fault == 'pressure_owner': changed['categoryPressure']['gpuOwners'] += 1
            elif fault == 'pressure_tick': changed['categoryPressure']['before'][0] += 1
            elif fault == 'pressure_no_continue': changed['route'] = []
            elif fault == 'pressure_image':
                pixels = (output / 'pressure-after.rgba').read_bytes()
                (target / 'pressure-after.rgba').write_bytes(bytes([pixels[0] ^ 1]) + pixels[1:])
            elif fault.startswith('pressure_cost_'):
                companion = bytearray((output / 'pressure.rcm').read_bytes()); companion[92 + int(fault.rsplit('_',1)[1]) * 8] ^= 1
                (target / 'pressure.rcm').write_bytes(companion)
                changed['categoryPressure']['manifestSHA256'] = hashlib.sha256(companion).hexdigest()
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


def inspect_cpu_replay(scene_dir, output, replay_path):
    """Independently bind the CPU observation stream to an actual native history.

    The CPU executable must also reject changed input; a plausible summary
    digest alone cannot establish that it evaluated the per-step states.
    """
    source_bytes = (output / 'receipt.json').read_bytes()
    source = json.loads(source_bytes)
    report = json.loads(replay_path.read_bytes())
    assert source['schema'] in ('aethyr.stream-walk-native-gpu.v1', 'aethyr.stream-walk-native-gpu.v2', 'aethyr.stream-walk-native-gpu.v3', 'aethyr.stream-walk-native-gpu.v4', 'aethyr.stream-walk-native-gpu.v5', 'aethyr.stream-walk-native-gpu.v6')
    assert report['schema'] == 'aethyr.stream-walk-native-replay.v3'
    assert report['backend'] == source['backend'] == 1 and report['tier'] == source['tier']
    assert report['input_sha256'] == hashlib.sha256(source_bytes).hexdigest()
    assert report['state_schema'] == 'aethyr.stream-walk-state.v1'
    assert report['source_schema'] == '' and report['captured_sources'] == 0
    frames = source['warmup'] + source['route'] + source['cleanup']
    observed = bytearray()
    previous = 0
    for frame in frames:
        digests = frame['stepDigests']
        assert frame['tick'] - previous == len(digests) and 0 <= len(digests) <= 8
        if not digests:
            assert source['schema'] == 'aethyr.stream-walk-native-gpu.v3' and frame['frameNs'] == 0
        previous = frame['tick']
        for digest in digests:
            assert len(digest) == 64
            bytes.fromhex(digest)
            observed.extend(digest.encode('ascii'))
        for record in frame['records']:
            assert len(record) == 23
            observed.extend(struct.pack('<23q', *map(int, record)))
        observed.extend(struct.pack('<3q', *frame['positionBits']))
    assert report['observation_sha256'] == hashlib.sha256(observed).hexdigest()
    assert report['ticks'] == previous and report['frames'] == len(source['warmup']) + len(source['route'])
    assert report['attempts'] == source['finalInfo'][20] == source['finalInfo'][21]
    # CPU replay keeps effects until their recorded accepted boundary.
    # The original synchronous Vulkan face may finish/free between ticks.
    # Reconstruct replay's actual retention ordering instead of asserting
    # that these different physical effect schedules have equal peaks.
    live, current, peak = {}, 0, 0
    for frame in frames:
        for record in frame['records']:
            kind, row, generation, epoch = map(int, record[1:5])
            if kind not in (0, 1):
                continue
            for key in list(live):
                if key[:3] == (row, generation, epoch) and (kind == 1 or key[3] in (3, 4)):
                    current -= live.pop(key)
        for freed, row, generation, epoch, field, extent in frame['allocations']:
            if freed:
                continue
            key = row, generation, epoch, field
            assert key not in live and extent > 0
            live[key] = extent
            current += extent
            peak = max(peak, current)
    assert not live and current == 0
    assert report['cpu_peak'] == peak <= 32768
    faults = ('first_step', 'interior_step', 'missing_step', 'tick', 'position', 'completion_generation',
              'completion_digest', 'missing_completion', 'backend', 'manifest', 'scene', 'actor_state')
    if source['schema'].endswith('.v4'):
        faults += ('pressure_missing','pressure_schema','pressure_clock','pressure_serial')
        faults += tuple('pressure_word_' + str(i) for i in range(13))
        faults += tuple('pressure_digest_' + key for key in ('manifestSHA256','sourceSHA256','stateBefore','stateAfter','stateNormalized'))
    if source['schema'].endswith('.v5'):
        faults += ('load_missing','load_schema','load_case','load_provenance','load_status') + tuple('load_effect_' + str(i) for i in range(10))
    if source['schema'].endswith('.v6'):
        faults += ('sdk_missing','sdk_schema','sdk_case','sdk_provenance') + tuple('sdk_word_' + str(i) for i in range(26))
        if source['sdkFault']['stop']:
            faults += tuple('sdk_stop_' + str(i) for i in range(24)) + ('sdk_sourceBefore','sdk_sourceAfter','sdk_both_sources','sdk_stopped')
    with tempfile.TemporaryDirectory(prefix='aethyr-native-cpu-replay-') as temporary:
        target = Path(temporary)
        for fault in faults:
            changed = copy.deepcopy(source)
            if fault in ('first_step', 'interior_step'):
                changed['warmup'][0]['stepDigests'][0 if fault == 'first_step' else 1] = 'f' * 64
            elif fault == 'missing_step':
                changed['warmup'][0]['stepDigests'].pop()
            elif fault == 'tick':
                changed['warmup'][0]['tick'] += 1
            elif fault == 'position':
                changed['warmup'][0]['positionBits'][2] ^= 1
            elif fault in ('completion_generation', 'completion_digest', 'missing_completion'):
                frame = next(f for f in changed['warmup'] + changed['route'] + changed['cleanup'] if f['records'])
                if fault == 'missing_completion':
                    frame['records'].pop(0)
                else:
                    frame['records'][0][3 if fault == 'completion_generation' else 15] ^= 1
            elif fault == 'backend':
                changed['backend'] = 0
            elif fault == 'manifest':
                changed['manifestSHA256'] = 'f' * 64
            elif fault == 'scene':
                changed['sceneSHA256'] = 'f' * 64
            elif fault == 'actor_state':
                changed['routeCells'][1][0][2] += 1
            elif fault == 'load_missing': del changed['loadFailure']
            elif fault == 'load_schema': changed['schema'] = 'aethyr.stream-walk-native-gpu.v3'
            elif fault == 'load_case': changed['loadFailure']['case'] += 1
            elif fault == 'load_provenance': changed['loadFailure']['qualification'] = 'physical OS exhaustion'
            elif fault == 'load_status': next(r for f in changed['warmup'] for r in f['records'] if r[6] == 6)[5] = 0
            elif fault.startswith('load_effect_'): changed['loadFailure']['effect'][int(fault.rsplit('_',1)[1])] += 1
            elif fault == 'sdk_missing': del changed['sdkFault']
            elif fault == 'sdk_schema': changed['schema'] = 'aethyr.stream-walk-native-gpu.v5'
            elif fault == 'sdk_case': changed['sdkFault']['case'] = -1
            elif fault == 'sdk_provenance': changed['sdkFault']['qualification'] = 'physical GPU reset'
            elif fault.startswith('sdk_word_'):
                i = int(fault.rsplit('_',1)[1]);q = changed['sdkFault']['words']
                q[i] = 0 if i in (20,21,24) or (q[1] == 4 and i in (10,11)) else q[i] + 1
            elif fault.startswith('sdk_stop_'):
                i = int(fault.rsplit('_',1)[1]);q = changed['sdkFault']['stop'];q[i] = 0 if i in (14,15,16) else q[i] + 1
            elif fault in ('sdk_sourceBefore','sdk_sourceAfter'): changed['sdkFault'][fault[4:]] = '0' * 64
            elif fault == 'sdk_both_sources':
                for key in ('sourceBefore','sourceAfter'): changed['sdkFault'][key] = '0' * 64
            elif fault == 'sdk_stopped': changed['sdkFault']['stoppedFrameRefused'] = False
            elif fault == 'pressure_missing': del changed['categoryPressure']
            elif fault == 'pressure_schema': changed['schema'] = 'aethyr.stream-walk-native-gpu.v3'
            elif fault in ('pressure_clock','pressure_serial'):
                field = 0 if fault == 'pressure_clock' else 3
                for key in ('before','after'): changed['categoryPressure'][key][field] += 1
            elif fault.startswith('pressure_word_'): changed['categoryPressure']['words'][int(fault.rsplit('_',1)[1])] += 1
            elif fault.startswith('pressure_digest_'): changed['categoryPressure'][fault[len('pressure_digest_'):]] = 'f' * 64
            input_path, result_path = target / (fault + '.json'), target / (fault + '-result.json')
            input_path.write_text(json.dumps(changed))
            env = dict(os.environ, ASAN_OPTIONS='detect_leaks=1:abort_on_error=1', UBSAN_OPTIONS='halt_on_error=1')
            result = subprocess.run([str(replay_path.parent / 'replay'), str(scene_dir), str(input_path), str(result_path)],
                                    capture_output=True, timeout=90, env=env)
            assert result.returncode != 0 and not result_path.exists(), ('tampered native CPU history accepted', fault)
            assert b'AddressSanitizer' not in result.stderr and b'runtime error:' not in result.stderr, (fault, result.stderr)
    return {'backend': 1, 'tier': source['tier'], 'canonical_paid_steps': previous,
            'observation_sha256': report['observation_sha256'], 'cpu_peak': report['cpu_peak'],
            'altered_native_histories_rejected': len(faults), 'no_gpu_replay_claim': True}


if __name__ == '__main__':
    if sys.argv[1] == '--compare-tiers':
        result = compare_tiers(Path(sys.argv[2]), Path(sys.argv[3]))
    elif sys.argv[1] == '--compare-presentation':
        result = compare_presentation(Path(sys.argv[2]), Path(sys.argv[3]))
    elif sys.argv[1] == '--cpu-replay':
        result = inspect_cpu_replay(*map(Path, sys.argv[2:5]))
    else:
        scene, output, program = map(Path, sys.argv[1:4])
        result = inspect(scene, output, program)
        if '--fault-checks' in sys.argv[4:]:
            result['tampered_histories_rejected'] = fault_checks(scene, output, program)
    print(json.dumps(result, sort_keys=True))
