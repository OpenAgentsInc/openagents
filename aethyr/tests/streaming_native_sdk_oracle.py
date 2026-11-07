# Firm reason: 2 (independent SDK-prefix, resource lifetime, source, motion, stop/drain and image reconstruction).
import copy
from collections import Counter, defaultdict
import hashlib
import json
from pathlib import Path
import struct
import sys
from streaming_native_oracle import f32

SCENE = 'ceefa05a481f8c8562646be74be333607075ae874fa3a076982ba23a81d00810'


def inspect(scene_dir, output, program, report=None, backing_statics=0):
    r = report if report is not None else json.loads((output / 'receipt.json').read_bytes())
    assert r['schema'] == 'aethyr.stream-walk-native-gpu.v6' and r['passed'] is True
    assert r['backend'] == 1 and r['tier'] in (0,1)
    raw = (scene_dir / 'scene.json').read_bytes();scene = json.loads(raw)
    assert hashlib.sha256(raw).hexdigest() == r['sceneSHA256'] == SCENE
    descriptor = next(m for m in scene['manifests'] if m['backend'] == 1 and m['tier'] == r['tier'])
    manifest = (scene_dir / (descriptor['sha256'] + '.rcm')).read_bytes()
    assert hashlib.sha256(manifest).hexdigest() == descriptor['sha256'] == r['manifestSHA256']
    for field, path in [('shaderSHA256',program),('hudShaderSHA256',program.parent / 'stream-hud.spv'),('presentShaderSHA256',program.parent / 'stream-present.spv')]:
        assert r[field] == hashlib.sha256(path.read_bytes()).hexdigest()
    assert r['presentationSchema'] == 'aethyr.stream-walk-display.v1'
    costs = [struct.unpack_from('<13q',manifest,48 + row * 440 + 44) for row in range(64)]
    categories = [manifest[48 + row * 440 + 5] for row in range(64)]
    kind = lambda row: 2 if manifest[48 + row * 440 + 4] == 5 else 1
    sources, dependencies = [], []
    for row, item in enumerate(scene['resources']):
        container = (scene_dir / (item['container_sha256'] + ('.raw' if item['raw'] else '.vpk'))).read_bytes()
        data = container[item['offset']:item['offset'] + item['bytes']]
        assert hashlib.sha256(container).hexdigest() == item['container_sha256'] and hashlib.sha256(data).hexdigest() == item['sha256'] and len(data) == costs[row][0]
        sources.append(data);at = 48 + row * 440
        dependencies.append([struct.unpack_from('<I',manifest,at + 152 + i * 36)[0] for i in range(struct.unpack_from('<I',manifest,at + 148)[0])])
    p = r['sdkFault'];assert p['schema'] == 'aethyr.stream-native-sdk-fault.v1'
    assert p['qualification'] == 'scoped SDK error inputs over real Vulkan effects; no physical exhaustion or reset'
    case = p['case'];assert type(case) is int and 0 <= case < 34
    sub = case % 17;domain = 1 if sub < 4 else 2 if sub < 8 else 3 if sub < 14 else 4
    phase = sub - (0,0,4,8,14)[domain] + 1;stopped = domain == 4;lost = stopped and phase == 2
    q = p['words'];assert len(q) == 26 and all(type(x) is int for x in q)
    assert q[:5] == [case // 17 + 1,domain,phase,1,1] and 0 <= q[5] < 64 and q[6] > 0 and q[7] == 1
    target = tuple(q[5:8]);code = -1 if not stopped or phase == 1 else -4 if lost else 1
    assert q[8:10] == [code,-2147483648 if not stopped and phase == 1 else 0]
    assert (q[10] >= 1 and q[11] >= 1) if stopped else q[10:12] == [phase - 1,phase]
    peak = 0 if stopped or phase == 1 else (1 if phase == 2 else 2 if domain != 3 or phase < 6 else 3)
    assert q[12:20] == [0] * 8 and q[20] > 0 and q[21] >= q[20] and q[22:] == [peak,0,280,int(stopped)]
    if stopped and phase == 3: assert q[21] - q[20] >= 10_100_000_000
    reason = 2 if stopped and phase == 3 else 3 if stopped else 0
    assert p['policy'] == [0,int(stopped),0,0,0,reason,-1] and p['stoppedFrameRefused'] is stopped
    assert len(r['warmup']) == (1 if stopped else 32) and len(r['route']) == (0 if stopped else 8)
    all_frames = r['warmup'] + r['route'] + r['cleanup'];all_stages = [s for f in all_frames for s in f['stages']]
    keys = [tuple(s[:3]) for s in all_stages];assert len(keys) == len(set(keys))
    assert target == next(tuple(s[:3]) for s in all_stages if costs[s[0]][2] > 0 and kind(s[0]) == q[0])
    expected, after, physical_gpu = {}, False, {}
    for s in all_stages:
        key = tuple(s[:3]);is_target = key == target
        expected[key] = (3 if domain == 1 else 5) if is_target else 5 if stopped and after and costs[key[0]][2] else 0
        after |= is_target
    allocated, fields, outcomes, ready, done, gone = {}, defaultdict(set), defaultdict(set), {}, {}, {}
    cpu = cpu_peak = tick = events = 0;category = Counter();category_peak = Counter();owner = Counter();owner_peak = Counter()
    z, velocity = f32(56), f32(0);caps = (8192,512,4096,2048,8192,4096,2048);times = [];attempts = set()
    for frame_index, f in enumerate(all_frames):
        ns, direction = f['frameNs'], f['direction'];assert ns in (15625000,62500000) and direction in (0,1)
        for _ in range(ns // 15625000):
            if frame_index < len(r['warmup']) + len(r['route']):
                delta = f32(f32(direction * 7.5) - velocity);change = min(abs(delta),.9375);velocity = f32(velocity + (change if delta > 0 else -change if delta < 0 else 0));z = f32(z + f32(velocity * .015625))
            tick += 1
        assert f['tick'] == tick and len(f['stepDigests']) == ns // 15625000
        assert all(len(x) == 64 and len(bytes.fromhex(x)) == 32 for x in f['stepDigests'])
        assert f['positionBits'] == [struct.unpack('<I',struct.pack('<f',x))[0] for x in (32,0,z)]
        for rec in f['records']:
            assert len(rec) == 23;at,event,row,g,e,decoded,status = rec[:7];key = row,g,e
            assert key in attempts and 0 < at <= tick and event in (0,1,2) and event not in outcomes[key]
            failure = expected[key] if event == 0 else 0
            assert decoded == failure and status == (6 if failure else 5 if stopped and event == 0 else 0)
            assert rec[7:15] == list(struct.unpack('>8I',bytes.fromhex(scene['resources'][row]['sha256'])))
            data = (bytes(len(sources[row])) if domain == 1 and key == target else sources[row]) if event == 0 else b''
            assert rec[15:23] == list(struct.unpack('>8I',hashlib.sha256(data).digest()))
            if event == 0: done[key] = at
            elif event == 1:
                assert 0 in outcomes[key];gone[key] = at;ready.pop(row,None);physical_gpu.pop(row,None)
            else:
                assert not stopped and expected[key] == 0 and 0 in outcomes[key] and all(d in ready for d in dependencies[row]);ready[row] = g
            outcomes[key].add(event)
        for free,row,g,e,field,size in f['allocations']:
            assert free in (0,1) and 0 <= row < 64 and g > 0 and e == 1 and 0 <= field < 5
            key = row,g,e;full = key + (field,);sizes = (costs[row][0],costs[row][1],costs[row][3],costs[row][4],costs[row][5])
            assert size == sizes[field] > 0
            if free:
                assert allocated.pop(full) == size
                if field == 0 and physical_gpu.get(row) == key: physical_gpu.pop(row)
            else:
                assert full not in allocated;allocated[full] = size;fields[key].add(field);attempts.add(key)
                if domain == 1 and key == target: assert field < 4
            sign = -1 if free else 1;cpu += sign * size;cat = 6 if field >= 3 else categories[row]
            category[cat] += sign * size;owner[row] += sign * size;cpu_peak = max(cpu_peak,cpu);category_peak[cat] = max(category_peak[cat],category[cat]);owner_peak[row] = max(owner_peak[row],owner[row]);events += 1
            assert cpu >= 0 and all(0 <= category[c] <= caps[c] for c in range(7))
        for s in f['stages']:
            key = tuple(s[:3]);row = key[0];assert key in attempts and len(s) == 8 and all(t >= 0 for t in s[3:]) and sum(s[3:7]) <= s[7]
            if costs[row][2] and (expected[key] == 0 or (key == target and stopped and not lost)): physical_gpu[row] = key
        info,gpu = f['info'],f['gpu'];assert len(info) == 28 and len(gpu) == 20
        assert info[14] == cpu <= info[10] and gpu[:2] == [len(physical_gpu),sum(costs[row][2] for row in physical_gpu)]
        assert cpu + gpu[1] <= info[8] <= 32768 and gpu[2:4] == [0,0] and gpu[9:12] == [0,0,1] and gpu[19] == 0
        assert 0 <= info[6] <= info[4] <= 4 and 0 <= info[5] <= info[4] and info[7] <= 2
        for c in range(7): assert category[c] + sum(costs[row][2] for row in physical_gpu if categories[row] == c) <= caps[c]
        if stopped: assert gpu[7:9] == [0,0] and f['hud'][:2] == [0,0]
        else: assert 600 < f['hud'][0] < 2048 and f['hud'][0] % 6 == 0 and f['hud'][1:] == [gpu[7],98304,1,1]
        assert f['elapsedNs'] >= 0;times.append(f['elapsedNs'] / 1e6)
    assert attempts == set(expected) == set(outcomes) and not allocated and not ready and not physical_gpu and cpu == 0
    for key in attempts:
        assert outcomes[key] == ({0,1} if stopped or expected[key] else {0,1,2}) and done[key] < gone[key]
        sizes = (costs[key[0]][0],costs[key[0]][1],costs[key[0]][3],costs[key[0]][4],costs[key[0]][5]);wanted = {i for i,n in enumerate(sizes) if n > 0}
        if key == target and domain == 1: wanted.discard(4)
        assert fields[key] == wanted
    final,gpu = r['finalInfo'],r['finalGPU'];assert final[20:22] == [len(attempts)] * 2 and final[15] == cpu_peak and final[24] == 1
    assert all(final[i] == 0 for i in (4,5,6,7,8,10,11,12,13,14,22,23)) and gpu[:7] == [0,0,0,0,0,1 if stopped else 2,1 if stopped else 2]
    assert gpu[9:12] == [0,0,1] and gpu[18:] == [23046 + q[24] + backing_statics,0] and final[27] == 874009 and r['fixedGrantsRetainedThroughDestruction'] is True
    assert r['finalHUD'][1:] == [gpu[7],98304,0,0] and r['observations'] == ([0] * 15 + [1] if stopped else [1] * 11 + [0] * 4 + [1])
    if stopped:
        t = p['stop'];assert len(t) == 24 and t[:3] == list(target) and t[3:5] == [r['warmup'][0]['tick']] * 2
        f = r['warmup'][0];prefix_end = next(i + 1 for i,a in enumerate(f['allocations']) if a[:5] == [0,*target,4])
        live = {}
        for free,row,g,e,field,size in f['allocations'][:prefix_end]:
            if free: assert live.pop((row,g,e,field)) == size
            else: live[row,g,e,field] = size
        assert t[5:7] == [sum(live.values())] * 2 and t[21:23] == [prefix_end] * 2
        index = next(i for i,s in enumerate(f['stages']) if tuple(s[:3]) == target)
        payload = sum(costs[s[0]][2] for s in f['stages'][:index + (0 if lost else 1)])
        assert t[7:9] == [payload] * 2 and t[9:14] == [0 if lost else 1,0 if lost else 1,0,0,reason]
        assert 0 < t[14] <= t[15] <= t[16] and t[14] <= q[20] <= q[21] <= t[16] and t[16] - t[14] <= f['stages'][index][6]
        assert t[17:21] == [code,-4 if lost else 0,1,1] and t[23] == 1
        if phase == 3: assert t[15] - t[14] >= 10_000_000_000 and t[16] - t[14] >= 10_100_000_000
        live_rows = {row for row,g,e,field in live if field == 0}
        source = b''.join(struct.pack('<qq',row,len(sources[row]) if row in live_rows else 0) + (sources[row] if row in live_rows else b'') for row in range(64))
        assert p['sourceBefore'] == p['sourceAfter'] == hashlib.sha256(source).hexdigest() and r['recovery'] == {}
        assert not (output / 'before.rgba').exists() and len(attempts) == 4 and tick == 7
    else:
        assert p['stop'] == [] and p['sourceBefore'] == p['sourceAfter'] == ''
        assert len(attempts) == 19 and tick == 139
        retry = next(k for k in keys if k[0] == target[0] and k != target);assert retry[1] > target[1]
        retry_frame = next(f for f in all_frames if any(tuple(s[:3]) == retry for s in f['stages']));assert retry_frame['tick'] >= gone[target]
        image = (output / 'before.rgba').read_bytes();assert image == (output / 'after.rgba').read_bytes() and len(image) == 230400 and len(set(image)) > 1
        assert (output / 'before-hud.f32').read_bytes() == (output / 'after-hud.f32').read_bytes()
        assert r['recovery']['stateSHA256'] == r['recovery']['stateAfterSHA256'] and r['recovery']['sourceSHA256'] == r['recovery']['sourceAfterSHA256']
    return {'case':case,'kind':q[0],'domain':domain,'phase':phase,'device':r['device'],'tier':r['tier'],'ticks':tick,'attempts':len(attempts),'allocation_events':events,
            'cpu_peak':cpu_peak,'category_cpu_peaks':dict(category_peak),'owner_cpu_peaks':dict(owner_peak),'sdk_memory_peak':gpu[17],'sdk_static_bytes':gpu[18],
            'frame_ms':{k:sorted(times)[int((len(times)-1)*p)] for k,p in [('p50',.5),('p95',.95),('p99',.99)]},
            'stopped':stopped,'stop_reason':reason,'scope_ns':q[21] - q[20],'retired_private_prefix':True,'final_zero_owners':True,'physical_fault_qualified':False,
            'image_channels_equal':0 if stopped else 230400}


def fault_checks(scene, output, program):
    original = json.loads((output / 'receipt.json').read_bytes());variants = []
    for i in range(26):
        p = copy.deepcopy(original);q = p['sdkFault']['words'];q[i] = 0 if i in (10,11,20,21,24) and original['sdkFault']['words'][1] == 4 else 0 if i in (20,21,24) else q[i] + 1;variants.append(p)
    for field in ('schema','qualification'):
        p = copy.deepcopy(original);p['sdkFault'][field] += '.invalid';variants.append(p)
    for field in ('sceneSHA256','manifestSHA256','shaderSHA256'):
        p = copy.deepcopy(original);p[field] = '0' * 64;variants.append(p)
    p = copy.deepcopy(original);p['passed'] = False;variants.append(p)
    if original['sdkFault']['stop']:
        for i in range(24):
            p = copy.deepcopy(original);p['sdkFault']['stop'][i] = 0 if i in (14,15,16) else p['sdkFault']['stop'][i] + 1;variants.append(p)
        for field in ('sourceBefore','sourceAfter'):
            p = copy.deepcopy(original);p['sdkFault'][field] = '0' * 64;variants.append(p)
    else:
        p = copy.deepcopy(original);p['sdkFault']['stop'] = [0] * 24;variants.append(p)
    for p in variants:
        try: inspect(scene,output,program,p)
        except (AssertionError,KeyError,IndexError,StopIteration,ValueError): pass
        else: raise AssertionError('altered SDK observation accepted')
    return len(variants)


if __name__ == '__main__':
    args = list(map(Path,sys.argv[1:4]));result = inspect(*args)
    if '--fault-checks' in sys.argv[4:]: result['altered_sdk_receipts_rejected'] = fault_checks(*args)
    print(json.dumps(result,sort_keys=True))
