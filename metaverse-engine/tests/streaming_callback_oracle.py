# Firm reason: 2 (independent actual-callback lifetime, manifest, source, motion and pixel oracle).
import base64
from collections import Counter, defaultdict
import copy
import hashlib
import json
from pathlib import Path
import struct
import sys

from streaming_walk_oracle import f32

SCENE = 'ceefa05a481f8c8562646be74be333607075ae874fa3a076982ba23a81d00810'
QUALIFICATION = 'held actual response; old callback redelivery after physical join and newer generation; no simultaneous reuse'


def inspect(scene_dir, output, report=None):
    scene_bytes = (scene_dir / 'scene.json').read_bytes()
    scene = json.loads(scene_bytes)
    r = report if report is not None else json.loads((output / 'history.json').read_bytes())
    assert r['schema'] == 'aethyr.stream-walk-callback-history.v1'
    assert hashlib.sha256(scene_bytes).hexdigest() == r['profile']['sceneSHA'] == SCENE
    tier = r['profile']['tier'];assert tier in (0,1)
    descriptor = next(m for m in scene['manifests'] if m['tier'] == tier and m['backend'] == 0)
    manifest = (scene_dir / (descriptor['sha256'] + '.rcm')).read_bytes()
    assert hashlib.sha256(manifest).hexdigest() == r['profile']['manifestSHA'] == descriptor['sha256']
    assert r['profile']['stateSchema'] == 'aethyr.stream-walk-state.v1' and r['profile']['presentationSchema'] == 'aethyr.stream-walk-display.v1'
    costs = [struct.unpack_from('<13q',manifest,48 + row * 440 + 44) for row in range(64)]
    categories = [manifest[48 + row * 440 + 5] for row in range(64)]
    sources = []
    for row,item in enumerate(scene['resources']):
        container = (scene_dir / (item['container_sha256'] + ('.raw' if item['raw'] else '.vpk'))).read_bytes()
        data = container[item['offset']:item['offset'] + item['bytes']]
        assert hashlib.sha256(container).hexdigest() == item['container_sha256']
        assert hashlib.sha256(data).hexdigest() == item['sha256'] and len(data) == costs[row][0]
        sources.append(data)
    p = r['callbacks'];assert p['schema'] == 'aethyr.stream-walk-callbacks.v1' and p['qualification'] == QUALIFICATION
    q = p['words'];assert len(q) == 36 and all(type(x) is int for x in q)
    row,gen,epoch = q[:3];old = row,gen,epoch
    assert row in (4,8) and gen > 0 and epoch == 1
    assert 0 < q[3] <= q[4] == q[5] == q[6] < q[7] < q[8] <= q[10] and q[9] > gen
    sizes = (costs[row][0],costs[row][1],costs[row][3],costs[row][4],costs[row][5])
    mask = sum(1 << i for i,n in enumerate(sizes) if n)
    assert q[11:15] == [1,0,gen,mask]
    assert q[15:] == [1,1,5,6,0,1,1,0,0,3,0,0,0,3,0,1,q[10],q[10],1,1,1]
    assert p['guards'] == [0,0,3,0,0,0,3,0]
    pending,before,after = p['pending'],p['probeBefore'],p['probeAfter']
    assert pending['tick'] == q[4] and pending['row'][8:11] == [1,0,gen]
    assert before == after and before['tick'] == q[10] and before['row'][8:11] == [3,1,q[9]]
    assert before['canonical'] == after['canonical'] and len(bytes.fromhex(before['canonical'])) == 32
    assert before['trace'] == after['trace'] == 0 and before['info'][0] == q[10]
    assert all(before['ptrs'][i] > 0 for i in (0,4))
    assert pending['bytes'][0] == bytes(len(sources[row])).hex()
    assert before['bytes'][0] == sources[row].hex() and hashlib.sha256(bytes.fromhex(after['bytes'][0])).hexdigest() == p['currentSourceSHA']
    held = p['held'];item = scene['resources'][row]
    assert held['url'].endswith(item['container_sha256'] + '.vpk') and held['tick'] == q[3]
    assert held['byteLength'] == (scene_dir / (item['container_sha256'] + '.vpk')).stat().st_size
    assert held['aborted'] is True and held['released'] is True
    ptr_mask = sum(1 << i for i,pointer in enumerate((pending['ptrs'][0],pending['ptrs'][1],pending['ptrs'][4],pending['ptrs'][2],pending['ptrs'][3])) if pointer)
    assert ptr_mask == mask and all(n > 0 for n in (pending['ptrs'][0],pending['ptrs'][4]))
    assert p['retired']['row'][8:11] == [0,0,gen]
    assert r['sourceSnapshotError'] is None and r['sourceSchema'] == 'aethyr.recorded-source.v1'
    snapshots = {(s['owner'],int(s['generation']),int(s['epoch'])):bytes.fromhex(s['hex']) for s in r['sourceSnapshots']}
    assert len(snapshots) == len(r['sourceSnapshots']) == 1 and snapshots[old] == bytes(len(sources[row]))
    frames = r['frames'];closing = r['cleanup']['closingFrames']
    records = [list(map(int,v)) for f in frames for v in f[13]] + [list(map(int,v)) for f in closing for v in f['records']]
    live,attempts,all_events = {},set(),[]
    total = peak = event_count = 0
    owner = Counter();owner_peak = Counter();category = Counter();category_peak = Counter()
    groups = [(g['tick'],g['events']) for g in r['allocations']] + [(g['tick'],g['allocations']) for g in closing]
    held_live = {};cpu_at_tick = {}
    for tick,events in groups:
        for freed,resource,g,e,field,size in events:
            key = resource,g,e;full = key + (field,)
            assert 0 <= resource < 64 and g > 0 and e == 1 and 0 <= field < 5
            c = costs[resource];assert size == (c[0],c[1],c[3],c[4],c[5])[field] > 0
            all_events.append((tick,freed,*full,size))
            if freed: assert freed == 1 and live.pop(full) == size
            else:
                assert freed == 0 and full not in live
                assert not any(k[0] == resource and k[:3] != key for k in live)
                live[full] = size;attempts.add(key)
            sign = -1 if freed else 1;total += sign * size;cat = 6 if field >= 3 else categories[resource]
            owner[resource] += sign * size;category[cat] += sign * size
            assert 0 <= total <= 32768 and all(0 <= category[i] <= (8192,512,4096,2048,8192,4096,2048)[i] for i in range(7))
            peak = max(peak,total);owner_peak[resource] = max(owner_peak[resource],owner[resource]);category_peak[cat] = max(category_peak[cat],category[cat]);event_count += 1
        if tick <= q[4]: held_live = dict(live)
        cpu_at_tick[tick] = total
    assert not live and total == 0 and peak == r['cleanup']['info'][15]
    assert pending['info'][14] == sum(held_live.values())
    assert {k[3]:n for k,n in held_live.items() if k[:3] == old} == {i:n for i,n in enumerate(sizes) if n}
    old_frees = [e for e in all_events if e[1] == 1 and e[2:5] == old]
    assert len(old_frees) == mask.bit_count() and all(q[5] < e[0] <= q[8] + 4 for e in old_frees)
    assert all(e[0] >= q[8] for e in all_events if e[1] == 0 and e[2] == row and e[3] != gen)
    states,ready = defaultdict(dict),{}
    for rec in records:
        tick,event,resource,g,e,decoded,status = rec[:7];key = resource,g,e
        assert key in attempts and event in (0,1,2) and event not in states[key]
        assert struct.pack('>8I',*rec[7:15]).hex() == scene['resources'][resource]['sha256']
        payload = snapshots.get(key,sources[resource]) if event == 0 else b''
        assert struct.pack('>8I',*rec[15:23]).hex() == hashlib.sha256(payload).hexdigest()
        if event == 0: assert (decoded,status) == ((5,6) if key == old else (0,0))
        elif event == 2:
            assert key != old and status == decoded == 0 and 0 in states[key]
            at = 48 + resource * 440
            for i in range(struct.unpack_from('<I',manifest,at + 148)[0]): assert struct.unpack_from('<I',manifest,at + 152 + i * 36)[0] in ready
            ready[resource] = g
        else:
            assert status == decoded == 0 and 0 in states[key] and tick > states[key][0][0]
            if ready.get(resource) == g: ready.pop(resource)
        states[key][event] = tick,status
    assert set(states) == attempts and not ready
    assert all(set(s) == ({0,1} if k == old else {0,1,2}) for k,s in states.items())
    assert states[old][0] == (q[7],6) and states[old][1] == (q[8],0)
    assert (row,q[9],epoch) in states and states[row,q[9],epoch][2][0] <= q[10]
    callbacks = p['actualCallbacks']
    assert len(callbacks) == len(attempts) and len({tuple(c[:3]) for c in callbacks}) == len(callbacks)
    old_callback = next(c for c in callbacks if c[:3] == [row,str(gen),str(epoch)])
    assert old_callback[3:] == [1,1,q[6]] and p['oldCallback'] == [str(row),str(gen),str(epoch),'1']
    assert all(c[4] == 1 for c in callbacks)
    stage_keys = {(s['owner'],int(s['generation']),int(s['epoch'])) for s in r['stages']}
    assert len(r['stages']) == len(stage_keys) and stage_keys == attempts
    for s in r['stages']:
        key = s['owner'],int(s['generation']),int(s['epoch'])
        assert s['status'] == (1 if key == old else 0) and s['source'] == scene['resources'][key[0]]['sha256']
        times = [s.get(k,0) for k in ('allocationMs','sourceMs','decodeMs','uploadMs')]
        assert all(t >= 0 for t in times) and sum(times) <= s['totalMs'] + 1e-5
    z,velocity,tick = f32(56),f32(0),0;times=[];current_cpu=0
    for f in frames:
        ns,direction,observed,x,y,position,active,drawn,updated,declared,cpu,gpu,elapsed,rec,marks = f
        assert ns in (15625000,62500000) and direction in (-1,0,1)
        for _ in range(ns // 15625000):
            difference = f32(f32(direction * 7.5) - velocity);change = min(abs(difference),.9375)
            velocity = f32(velocity + (change if difference > 0 else -change if difference < 0 else 0));z = f32(z + f32(velocity * .015625));tick += 1
        assert [x,y,position,observed] == [32,0,z,tick]
        current_cpu = cpu_at_tick.get(tick,current_cpu);assert cpu == current_cpu
        assert len(marks) == ns // 15625000 * 64 and len(bytes.fromhex(marks)) * 2 == len(marks)
        assert 0 <= updated <= active <= 4 and 0 <= drawn <= active and 0 <= cpu + gpu <= declared <= 32768 and elapsed >= 0
        times.append(elapsed)
    for f in closing:
        tick += f['frameNs'] // 15625000
        assert f['position'] == [32,0,z] and f['tick'] == tick and len(f['stepDigests']) == f['frameNs'] // 15625000
        assert f['cpu'] == cpu_at_tick[tick]
    final = r['cleanup']['info'];assert final[0] == tick and final[20:22] == [len(attempts)] * 2 and final[24] == 1
    assert all(final[i] == 0 for i in (4,5,6,7,8,10,11,12,13,14,22,23))
    assert r['cleanup']['loadFailures'] == 1 and not r['cleanup']['gpuErrors'] and not r['before']['runtimeMessages']
    assert all(r['cleanup']['transport'][k] == 0 for k in ('owners','loading','published','readers','sdkPending','copied'))
    assert r['cleanup']['transport']['storage'][:2] == [1,8388608]
    assert len(r['legs']) == 3 and all([c[3] for c in leg['cells']] == list(range(1,13)) for leg in r['legs'])
    before_image = json.loads((output / 'before.json').read_bytes());after_image = json.loads((output / 'after.json').read_bytes())
    assert before_image == after_image and before_image['width'] == 320 and before_image['height'] == 180 and before_image['rowBytes'] == 1280
    pixels = base64.b64decode(before_image['data'],validate=True)
    assert len(pixels) == 230400 and len(set(pixels)) > 1
    assert hashlib.sha256(pixels).hexdigest() == p['beforeSHA'] == p['afterSHA']
    return {'tier':tier,'owner':row,'old_generation':gen,'new_generation':q[9],'frames':len(frames),'paid_steps':tick,'attempts':len(attempts),
            'physical_events':event_count,'cpu_peak':peak,'owner_cpu_peaks':dict(owner_peak),'category_cpu_peaks':dict(category_peak),
            'callback_guard_refusals':8,'image_channels_equal':len(pixels),'source_bytes_retained_until_join':len(sources[row]),'final_zero_owners':True,
            'frame_ms':{k:sorted(times)[int((len(times)-1)*p)] for k,p in [('p50',.5),('p95',.95),('p99',.99)]}}


def fault_checks(scene,output):
    original = json.loads((output / 'history.json').read_bytes());variants=[]
    for i in range(36):
        bad = copy.deepcopy(original);bad['callbacks']['words'][i] += 1;variants.append(bad)
    for key in ('schema','qualification','beforeSHA','afterSHA','currentSourceSHA'):
        bad = copy.deepcopy(original);bad['callbacks'][key] = 'invalid';variants.append(bad)
    for key in ('pending','probeBefore','probeAfter'):
        bad = copy.deepcopy(original);bad['callbacks'][key]['bytes'][0] = 'ff';variants.append(bad)
    for i in range(8):
        bad = copy.deepcopy(original);bad['callbacks']['guards'][i] += 1;variants.append(bad)
    bad = copy.deepcopy(original);bad['callbacks']['held']['aborted'] = False;variants.append(bad)
    bad = copy.deepcopy(original);bad['callbacks']['actualCallbacks'][0][4] = 0;variants.append(bad)
    for bad in variants:
        try: inspect(scene,output,bad)
        except (AssertionError,KeyError,IndexError,ValueError,StopIteration): pass
        else: raise AssertionError('altered callback evidence accepted')
    return len(variants)


if __name__ == '__main__':
    scene,output = map(Path,sys.argv[1:3]);result=inspect(scene,output)
    if '--fault-checks' in sys.argv[3:]: result['altered_callback_receipts_rejected'] = fault_checks(scene,output)
    print(json.dumps(result,sort_keys=True))
