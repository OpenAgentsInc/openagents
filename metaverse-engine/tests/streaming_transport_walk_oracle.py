# Firm reason: 2 (independent reconstruction of actual per-owner HTP1 transport costs, original tickets and allocation/join histories).
import copy
import hashlib
import json
from pathlib import Path
import sys
from streaming_assets_oracle import pack

PAGE = 65536
INITIAL = 8388608
MODEL = b'HTP1/whole-vpk1-vpk2-1/raw-exact-1/hpk-initial8m-page64k-1'


def inspect(scene_dir, transport_dir, receipt, stages, incomplete=False):
    scene_bytes = (scene_dir / 'scene.json').read_bytes()
    scene = json.loads(scene_bytes)
    binding = receipt['transportBinding']
    assert binding['schema'] in ('aethyr.stream-transport-observation.v1', 'aethyr.stream-transport-observation.v2')
    assert binding['phase'] == 3 and 1 <= binding['capacity'] <= 2048
    assert binding['traceBytes'] == binding['capacity'] * 24 * 8
    trace_reserved = (binding['traceBytes']+PAGE-1)//PAGE*PAGE
    if binding['schema'].endswith('.v2'):
        assert binding['traceReservedBytes'] == trace_reserved
        if incomplete:
            assert not binding['complete'] and binding['exhausted'] and 0 < int(binding['dropped']) <= 2**63-1
            assert len(binding['events']) == binding['capacity']
        else:
            assert binding['complete'] and not binding['exhausted'] and int(binding['dropped']) == 0
    else:
        assert not incomplete and binding['capacity'] == 2048
    assert 0 < binding['staticBytes'] < INITIAL
    index = json.loads((transport_dir / 'transport.json').read_bytes())
    companion = next(x for x in index['companions'] if x['sha256'] == binding['root'])
    assert companion['manifestSHA256'] == receipt['profile']['manifestSHA']
    assert companion['backend'] == 0 and companion['tier'] == receipt['profile']['tier']
    assert hashlib.sha256(scene_bytes).hexdigest() == receipt['profile']['sceneSHA'] == index['sceneSHA256']
    raw = (transport_dir / (binding['root'] + '.htp')).read_bytes()
    assert len(raw) == 6256 and hashlib.sha256(raw).hexdigest() == binding['root']
    def digest(at):
        return b''.join(raw[i:i+4][::-1] for i in range(at, at+32, 4)).hex()
    assert raw[:16] == b'HTP1\x01\x00\x00\x00\x40\x00\x00\x00\x60\x00\x00\x00'
    assert digest(16) == index['sceneSHA256'] and digest(48) == companion['manifestSHA256']
    assert digest(80) == hashlib.sha256(MODEL).hexdigest()
    extents = {}
    for i, r in enumerate(scene['resources']):
        data = (scene_dir / (r['container_sha256'] + ('.raw' if r['raw'] else '.vpk'))).read_bytes()
        assert hashlib.sha256(data).hexdigest() == r['container_sha256']
        payload = data[r['offset']:r['offset']+r['bytes']]
        assert hashlib.sha256(payload).hexdigest() == r['sha256']
        assert 0 < len(data) <= 32768
        if r['raw']:
            assert payload == data and r['offset'] == 0
        else:
            assert any(at == r['offset'] and len(b) >= r['bytes'] for _, at, b in pack(data))
        at = 112+i*96
        assert int.from_bytes(raw[at:at+8], 'little') == int(r['raw'])
        assert digest(at+8) == r['container_sha256'] and digest(at+40) == r['sha256']
        assert [int.from_bytes(raw[j:j+8], 'little') for j in range(at+72, at+96, 8)] == [len(data), r['offset'], r['bytes']]
        extents[i] = (len(data), r['offset'], r['bytes'], int(r['raw']))
    extents[-1] = (len(scene_bytes), 0, len(scene_bytes), 1)
    extents[-2] = (28208, 0, 28208, 1)
    extents[-3] = (6256, 0, 6256, 1)
    active, tickets, ended, begins = {}, {}, {}, set()
    owner_peaks = {}
    baseline_reserved = INITIAL*2 + trace_reserved  # master, world arena, trace
    reserved = baseline_reserved
    peak_reserved = reserved
    physical = 0
    peak_physical = peak_operations = 0
    plans = checked = released = 0
    events = binding['events']
    assert 0 < len(events) <= binding['capacity']
    for wire in events:
        assert len(wire) == 24
        e = list(map(int, wire));kind, owner, generation, epoch = e[:4]
        key = (owner, generation, epoch)
        assert owner in extents
        if owner < 0:
            assert generation == epoch == 0
        else:
            assert generation > 0 and epoch > 0
        if kind == 1:
            assert owner >= 0 and key not in begins and key not in active
            assert tuple(e[4:8]) == extents[owner]
            assert all(k[0] != owner for k in active)
            begins.add(key);active[key] = {'plan': False, 'source': False, 'freed': False, 'reactor': 0, 'external': 0}
        elif kind == 2:
            if owner < 0:
                assert key not in active and key not in ended
                active[key] = {'plan': False, 'source': False, 'freed': False, 'reactor': 0, 'external': 0}
            a = active[key]
            assert not a['plan'] and not a['freed']
            size, _, _, is_raw = extents[owner]
            reactor, external, cost, base, initial, grow = e[4:10]
            assert initial == INITIAL and 0 <= base <= initial
            expected = INITIAL if is_raw else max(INITIAL, (((base+15)//16*16+size+PAGE-1)//PAGE)*PAGE)
            assert reactor == expected and external == (size if is_raw else 0)
            assert grow == int(expected > initial) and cost == reactor + (external+PAGE-1)//PAGE*PAGE
            count = 1 + int(bool(external));assert e[10] == count
            owned = [tuple(e[11+i*3:14+i*3]) for i in range(count)]
            assert len(set(owned)) == count and all(t not in tickets and 0 <= t[0] < 64 and t[1] > 0 and t[2] > 0 for t in owned)
            assert all(t[0] != old[0] for t in owned for old in tickets)
            for t in owned:
                tickets[t] = key
            reserved += cost;peak_reserved = max(peak_reserved, reserved)
            assert e[17:21] == [3+len(tickets), reserved, 268435456, 67108864]
            a.update(plan=True, cost=cost, max_reactor=reactor, max_external=external, tickets=owned)
            plans += 1;peak_operations = max(peak_operations, len(tickets))
        elif kind == 3:
            a = active[key];assert a['plan'] and not a['freed']
            reactor, external = e[4:6]
            assert (reactor > 0) != (external > 0)
            if reactor:
                assert external == 0 and reactor == (INITIAL if not a['reactor'] else a['max_reactor']-a['reactor'])
            else:
                assert a['reactor'] > 0 and a['external'] == 0 and external == a['max_external']
            a['reactor'] += reactor;a['external'] += external
            assert a['reactor'] <= a['max_reactor'] and a['external'] <= a['max_external']
            physical += reactor+external;peak_physical = max(peak_physical, physical)
            owner_peaks[str(owner)] = max(owner_peaks.get(str(owner), 0), a['reactor']+a['external'])
        elif kind == 4:
            a = active[key]
            assert owner >= 0 and a['plan'] and not a['source'] and not a['freed'] and e[4] == extents[owner][0]
            assert a['reactor'] == a['max_reactor'] and a['external'] == a['max_external']
            a['source'] = True;checked += 1
        elif kind in (5, 7):
            a = active[key];assert a['plan'] and not a['freed']
            assert e[4:6] == [a['reactor'], a['external']]
            assert e[10] == len(a['tickets'])
            assert [tuple(e[11+i*3:14+i*3]) for i in range(e[10])] == a['tickets']
            for t in a['tickets']:
                assert tickets.pop(t) == key
            reserved -= a['cost'];physical -= a['reactor']+a['external']
            assert e[17:21] == [3+len(tickets), reserved, 268435456, 67108864]
            a['freed'] = True;released += 1
            if kind == 7:
                assert a['external'] == 0 and not a['source']
            if owner < 0:
                ended[key] = 0;del active[key]
        elif kind == 6:
            a = active[key]
            assert owner >= 0 and e[4] in (0, 1) and a['freed']
            assert e[4] != 0 or a['source']
            ended[key] = e[4];del active[key]
        else:
            raise AssertionError(('unexpected transport event', e))
    if not incomplete:
        assert not active and not tickets and physical == 0
        assert plans == released and reserved == baseline_reserved
    stage_outcomes = {(s['owner'], int(s['generation']), int(s['epoch'])): s['status'] for s in stages}
    if incomplete:
        assert begins <= stage_outcomes.keys()
        assert all(stage_outcomes[k] == status for k, status in ended.items() if k[0] >= 0)
    else:
        assert begins == stage_outcomes.keys()
        assert all(ended[k] == status for k, status in stage_outcomes.items())
    cleanup = receipt['cleanup']
    assert cleanup['transport']['storage'][:2] == [1, INITIAL]
    assert cleanup['transportBeforeDispose']['storage'][:2] == [2, INITIAL*2]
    assert all(cleanup['info'][i] == 0 for i in (8, 10, 11, 12, 13, 14, 22, 23))
    return {'schema': 'aethyr.stream-transport-oracle.v2', 'complete_history': not incomplete, 'events': len(events), 'attempts': len(begins), 'plans': plans, 'source_checks': checked, 'releases': released, 'prefix_live_operations': len(active), 'prefix_live_tickets': len(tickets), 'dropped_events': int(binding.get('dropped', 0)), 'peak_operation_tickets': peak_operations, 'peak_reserved_with_master_world_trace': peak_reserved, 'peak_transport_allocated': peak_physical, 'peak_allocated_by_resource_owner': owner_peaks, 'trace_bytes': binding['traceBytes'], 'carrier_static_bytes': binding['staticBytes'], 'opaque_sdk_gc_rss': 'outside explicit engine ownership; not measured'}


def negatives(scene, transport, receipt, stages):
    count = 0
    selectors = [
        (2, 4, 1),  # predicted reactor extent
        (2, 11, 1),  # original ticket
        (3, 4, 1),  # observed allocation
        (4, 4, 1),  # complete source extent
        (5, 12, 1),  # released generation
        (6, 3, 1),  # joined world epoch
    ]
    for kind, field, delta in selectors:
        bad = copy.deepcopy(receipt)
        event = next(e for e in bad['transportBinding']['events'] if int(e[0]) == kind and int(e[1]) >= 0)
        event[field] = str(int(event[field])+delta)
        try:
            inspect(scene, transport, bad, stages)
        except (AssertionError, KeyError, StopIteration):
            count += 1
        else:
            raise AssertionError(('altered history accepted', kind, field))
    if receipt['transportBinding']['schema'].endswith('.v2'):
        for field, value in [('complete', False), ('exhausted', True), ('dropped', '1'), ('traceReservedBytes', 1)]:
            bad = copy.deepcopy(receipt);bad['transportBinding'][field] = value
            try:
                inspect(scene, transport, bad, stages)
            except AssertionError:
                count += 1
            else:
                raise AssertionError(('altered trace status accepted', field))
    return count


if __name__ == '__main__':
    incomplete = sys.argv[-1] == '--incomplete'
    scene, transport, source = map(Path, sys.argv[1:-1] if incomplete else sys.argv[1:])
    receipt = json.loads(source.read_text())
    receipt['transportBinding'] = json.loads(Path(str(source)+'.transport.json').read_text())
    sidecar = Path(str(source)+'.stages.json')
    stages = json.loads(sidecar.read_text()) if sidecar.exists() else receipt['stages'] + [s for f in receipt['cleanup']['closingFrames'] for s in f['stages']]
    result = inspect(scene, transport, receipt, stages, incomplete)
    if not incomplete:
        result['negative_histories_rejected'] = negatives(scene, transport, receipt, stages)
    else:
        try:
            inspect(scene, transport, receipt, stages)
        except AssertionError:
            result['complete_verifier_refused'] = True
        else:
            raise AssertionError('incomplete history accepted as complete')
    print(json.dumps(result, indent=2))
