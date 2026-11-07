# Firm reason: 2 (independent manifest-to-Vulkan plan and physical allocation lifetime reconstruction).
"""Check recorded device requirements and every allocation, without SDK or Coax.

Requirements are device observations, not CPU-derived Vulkan layout claims.
The oracle derives descriptors from immutable manifest costs, checks the
observed type/alignment bounds, then reconstructs admission and live memory.
"""
import copy
import hashlib
import json
from pathlib import Path
import struct
import subprocess
import sys
import tempfile
from streaming_native_oracle import inspect as inspect_native
from streaming_native_oracle import inspect_cpu_replay
from streaming_native_loss_oracle import inspect as inspect_loss
from streaming_native_sdk_oracle import inspect as inspect_sdk
from streaming_native_transfer_oracle import inspect as inspect_transfer

CAPS = [262144,65536,0,16384,0,0,524288,1572864]
STATIC = 25272 + 803432 + 2056  # native ledger, bounded observer, memory/lease map and active pointer (LP64)


def inspect(scene_dir, output, program, report=None):
    r = report if report is not None else json.loads((output / 'receipt.json').read_bytes())
    assert r['schema'] == 'aethyr.stream-walk-native-gpu.v7' and r['passed'] is True
    original = r['observationSchema']
    assert original in [f'aethyr.stream-walk-native-gpu.v{i}' for i in range(2,7)]
    b = r['backing'];assert b['schema'] == 'aethyr.stream-native-backing.v1'
    scene = json.loads((scene_dir / 'scene.json').read_bytes())
    desc = next(m for m in scene['manifests'] if m['backend'] == 1 and m['tier'] == r['tier'])
    raw = (scene_dir / (desc['sha256'] + '.rcm')).read_bytes()
    assert hashlib.sha256(raw).hexdigest() == r['manifestSHA256'] == desc['sha256']
    specs = {}
    for row in range(64):
        at = 48 + 440 * row;cost = struct.unpack_from('<13q',raw,at + 44)
        if not cost[2]: continue
        image = raw[at + 4] == 5
        specs[row*2] = [row*2,row,0,6,0,cost[2],0,0,0]
        specs[row*2+1] = [row*2+1,row,1,raw[at+5],int(image),cost[2],*(cost[9:12] if image else (0,0,0))]
    for i,(kind,size) in enumerate(zip([0,0,0,1,2,3,0],[4096,98304,256,230400,460800,230400,230400])):
        specs[128+i] = [128+i,64+i,4,6 if i == 6 else 7,kind,size,*((320,180,1) if kind else (0,0,0))]
    devices,plans,events = b['devices'],b['plans'],b['events']
    assert 1 <= len(devices) <= 8 and len(plans) == len(devices) and 1 <= len(events) <= 4096
    for n,(d,rows) in enumerate(zip(devices,plans)):
        assert len(d) == 18 and d[:6] == [n+1,*r['finalGPU'][12:16],devices[0][5]] and d[5] >= 64
        assert all(type(x) is int and 0 <= x <= 4294967295 for x in d[1:6]) and d[3] >= 4198400
        assert d[6] in (0,1) and d[7:9] == [2097152,1048576] and d[17] == len(rows) == len(specs)
        assert [p[0] for p in rows] == sorted(specs)
        for p in rows:
            assert len(p) == 13 and all(type(x) is int for x in p) and p[:9] == specs[p[0]]
            size,alignment,types,selected = p[9:]
            assert p[5] <= size <= 8388608 and 1 <= alignment <= 8388608 and alignment & (alignment-1) == 0
            assert 1 <= types <= 4294967295 and 0 <= selected < 32 and types & (1 << selected)
        cap = CAPS.copy()
        if d[6]: cap[1] = next(p[9] for p in rows if p[0] == 1)
        assert d[9:17] == cap
    known = {(v[1],v[2],v[3]) for f in r['warmup']+r['route']+r['cleanup'] for v in f['allocations']}
    live = {};actual = set();used = [0]*8;reserved = allocated = peak = actual_peak = refused = 0
    epoch = serial = tick = 0;current = None;counts = [0]*7;category_peaks = [0]*8
    for e in events:
        assert len(e) == 20 and all(type(x) is int for x in e)
        tag,t,index,row,gen,world,part,category,logical,size,alignment,types,selected,slot,seq,era,status,rbytes,abytes,count = e
        assert 0 <= tag <= 6 and tick <= t <= r['finalInfo'][0];tick = t;counts[tag] += 1
        if tag in (5,6):
            assert e[2:15] == [-1,-1,0,0,-1,-1,0,0,0,0,0,-1,0] and status == 0 and not live and not actual
            if tag == 5:
                assert current is None and era == epoch+1 and era <= len(devices)
                epoch = era;serial = 0;current = {p[0]:p for p in plans[epoch-1]};caps = devices[epoch-1][9:17]
            else:
                assert current is not None and era == epoch;current = None
        else:
            assert current is not None and index in current and (era == epoch if tag != 4 else era == 0)
            p = current[index];assert [index,row,category,logical,size,alignment,types,selected] == [p[i] for i in (0,1,3,5,9,10,11,12)]
            assert part == 4 if row >= 64 else part in ((0,2) if index % 2 == 0 else (1,3))
            assert (gen,world) == (0,0) if row >= 64 else (row,gen,world) in known
            key = slot,seq,era;owner = index,gen,world,part
            if tag in (0,4):
                assert owner not in live.values()
                expected = 1 if size > 1048576 else 2 if size > caps[category]-used[category] else 3 if size > 2097152-reserved else 4 if len(live) == 64 else 0
                assert status == expected and (tag == 0) == (status == 0)
                if status:
                    assert (slot,seq,era) == (-1,0,0)
                    refused += status != 1
                else:
                    assert slot == next(i for i in range(64) if all(k[0] != i for k in live)) and seq == serial+1
                    serial = seq;live[key] = owner;reserved += size;used[category] += size
                    peak = max(peak,reserved);category_peaks[category] = max(category_peaks[category],used[category])
            else:
                assert key in live and live[key] == owner and status == 0
                if tag == 1:
                    assert key not in actual;actual.add(key);allocated += size;actual_peak = max(actual_peak,allocated)
                elif tag == 2:
                    assert key in actual;actual.remove(key);allocated -= size
                else:
                    assert tag == 3 and key not in actual;del live[key];reserved -= size;used[category] -= size
            assert 0 <= allocated <= reserved <= 2097152 and all(0 <= x <= c for x,c in zip(used,caps))
        assert [rbytes,abytes,count] == [reserved,allocated,len(live)]
    assert current is None and epoch == len(devices) and not live and not actual and used == [0]*8
    assert b['summary'] == [0,0,0,0,peak,actual_peak,refused,0,epoch+1,25272,803432,len(events),len(devices)]
    assert r['finalGPU'][17:19] == [actual_peak,23326 + STATIC]
    assert all(d[6] == devices[0][6] for d in devices)
    assert ('backingPressure' in r) == bool(devices[0][6])
    if devices[0][6]:
        p = r['backingPressure'];assert p['schema'] == 'aethyr.stream-native-backing-pressure.v1'
        rejected = [e for e in events if e[0] == 4];assert len(rejected) == refused == 1
        e = rejected[0];assert e[2:4] == [1,0] and e[6:8] == [3,1] and e[16] == 2
        assert p['words'] == [e[1],e[1],e[17],e[17],e[18],e[18],-1000069000]
        assert p['sourceBefore'] == p['sourceAfter'] == r['recovery']['sourceSHA256']
        assert p['stateBefore'] == p['stateAfter'] == r['recovery']['stateSHA256']
        assert (output / 'backing-before.rgba').read_bytes() == (output / 'backing-after.rgba').read_bytes() == (output / 'before.rgba').read_bytes()
        assert len((output / 'backing-before.rgba').read_bytes()) == 230400
    normalized = copy.deepcopy(r);normalized['schema'] = original
    base = inspect_sdk(scene_dir,output,program,normalized,STATIC) if original.endswith('.v6') else inspect_loss(scene_dir,output,program,normalized) if original.endswith('.v3') else inspect_native(scene_dir,output,program,normalized)
    if (output / 'upload-probe.json').exists(): base = inspect_transfer(scene_dir,output,program,report=normalized,stats=base)
    return {'backing_events':len(events),'plans_per_device':len(specs),'devices':epoch,'reservations':counts[0],
            'allocations':counts[1],'frees':counts[2],'releases':counts[3],'refusals':refused,
            'reserved_peak':peak,'allocated_peak':actual_peak,'category_peaks':category_peaks,'base':base}


def negative(scene, output, program, replay):
    """Alter independent receipt facts; both verifiers must refuse each one."""
    original = json.loads((output / 'receipt.json').read_bytes());cases = []
    def changed(path, value):
        r = copy.deepcopy(original);node = r
        for key in path[:-1]: node = node[key]
        node[path[-1]] = value;cases.append(r)
    changed(['schema'],'unknown');changed(['observationSchema'],'unknown');changed(['backing','schema'],'unknown')
    for i,v in enumerate(original['backing']['summary']): changed(['backing','summary',i],v+1)
    for i,v in enumerate(original['backing']['devices'][0]): changed(['backing','devices',0,i],v+1)
    chosen = {};seen = {}
    for i,p in enumerate(original['backing']['plans'][0]): chosen.setdefault((p[2],p[4]),i)
    for p in chosen.values():
        for i,v in enumerate(original['backing']['plans'][0][p]): changed(['backing','plans',0,p,i],v+1)
    for e,v in enumerate(original['backing']['events']): seen.setdefault(v[0],e)
    for e in seen.values():
        for i,v in enumerate(original['backing']['events'][e]):
            changed(['backing','events',e,i],original['finalInfo'][0]+1 if i == 1 else v+1)
    changed(['backing','events'],original['backing']['events'][:-1])
    changed(['backing','plans',0],original['backing']['plans'][0][1:])
    changed(['backing','devices'],original['backing']['devices'][:-1])
    if 'backingPressure' in original:
        changed(['backingPressure','schema'],'unknown')
        for i,v in enumerate(original['backingPressure']['words']): changed(['backingPressure','words',i],v+1)
        for field in ('sourceBefore','sourceAfter','stateBefore','stateAfter'): changed(['backingPressure',field],'0'*64)
    env = dict(__import__('os').environ,ASAN_OPTIONS='detect_leaks=1:abort_on_error=1',UBSAN_OPTIONS='halt_on_error=1')
    with tempfile.TemporaryDirectory(prefix='backing-oracle-') as tmp:
        path = Path(tmp)
        for i,r in enumerate(cases):
            try: inspect(scene,output,program,r)
            except (AssertionError,KeyError,IndexError,ValueError,StopIteration): pass
            else: raise AssertionError(f'independent oracle accepted altered case {i}')
            receipt = path / 'input.json';receipt.write_text(json.dumps(r,separators=(',',':')))
            result = subprocess.run([str(replay),str(scene),str(receipt),str(path / 'out.json')],stdout=subprocess.PIPE,stderr=subprocess.PIPE,env=env,timeout=30)
            assert result.returncode > 0, (i,result.returncode,result.stdout,result.stderr)
            assert b'AddressSanitizer' not in result.stderr and b'runtime error:' not in result.stderr, (i,result.stderr)
    return len(cases)


if __name__ == '__main__':
    args = list(map(Path,sys.argv[1:4]));result = inspect(*args)
    if (args[1] / 'replay.json').exists(): result['cpu_replay'] = inspect_cpu_replay(args[0],args[1],args[1] / 'replay.json')
    if len(sys.argv) == 5: result['altered_receipts_refused_by_both'] = negative(*args,Path(sys.argv[4]))
    print(json.dumps(result,sort_keys=True))
