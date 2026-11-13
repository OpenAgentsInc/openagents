# Firm reason: 2 (independent checkpoint identity, full GPU image, saved-state, single-use and owner-lifetime reconstruction).
"""Inspect actual reloads; materialize their unchanged accepted CPU history."""
import base64
import copy
import hashlib
import json
from pathlib import Path
import sys


def pixels(value):
    w, h, stride = (value[k] for k in ('width', 'height', 'rowBytes'))
    assert (w, h) == (320, 180) and stride >= w * 4 and stride % 256 == 0
    raw = base64.b64decode(value['data'], validate=True)
    assert len(raw) == stride * h and value['format'] in ('bgra8unorm', 'rgba8unorm')
    return b''.join(raw[y * stride:y * stride + w * 4] for y in range(h))


def inspect(data, mode, engine):
    before, stop, restored, continued, snapshot = (data[k] for k in ('before', 'stop', 'restored', 'continued', 'snapshot'))
    assert mode in ('trap', 'adapter-failure', 'pending', 'once', 'loop', 'engine', 'corrupt', 'extent', 'tier')
    assert before['rescue']['supported'] and not before['rescue']['restored']
    assert not any(x in json.dumps(before['adapter']).lower() for x in ('swiftshader', 'llvmpipe', 'software'))
    assert stop['rescue']['saved'] and not stop['rescue']['suppressed']
    assert stop['status']['state'] == 'stopped' and stop['status']['reason'] == 3
    assert stop['actions'] == ['Reload where you were']
    raw = base64.b64decode(snapshot['bytes'], validate=True)
    assert snapshot['engine'] == hashlib.sha256(engine).hexdigest()
    assert snapshot['scene'] == before['profile']['sceneSHA'] and snapshot['manifest'] == before['profile']['manifestSHA']
    assert snapshot['snapshot'] == hashlib.sha256(raw).hexdigest()
    assert snapshot['layout'] == 1 and snapshot['tier'] == 0
    assert len(raw) == snapshot['size'] == before['stats']['wasmBytes'] == 2097152
    assert snapshot['tick'] == stop['rescue']['heldTick']
    assert snapshot['digest'] == stop['rescue']['heldDigest']
    assert before['rescue']['heldBytes'] == len(raw)
    # Engine arena + page reactor + one complete checkpoint allocation.
    assert before['stats']['transport']['storage'][1] >= 16777216 + len(raw)
    rejected = mode in ('engine', 'corrupt', 'extent', 'tier')
    state = restored['rescue']
    assert state['restored'] == (not rejected)
    if rejected:
        assert state['refusal'] == {'engine': 6, 'corrupt': 6, 'extent': 4, 'tier': 5}[mode]
        assert restored['stats']['info'][0] == 0
    else:
        assert state['refusal'] is None and state['restoredFrames'] == 0
        assert state['restoredDigest'] == restored['digest'] == snapshot['digest']
        assert state['restoredTick'] == restored['stats']['info'][0] == snapshot['tick']
        if mode != 'adapter-failure':
            assert snapshot['digest'] == before['digest'] and restored['cells'] == before['cells']
        assert pixels(data['before-image']) == pixels(data['restored-image'])
    if mode == 'pending':
        assert before['stats']['transport']['loading'] > 0 and before['stats']['info'][14] > 0
        assert restored['stats']['info'][14] > 0
    if mode == 'loop':
        assert data['loop']['suppressed'] and data['loop']['saved'] is False
        assert data['loop']['restoredFrames'] == 1
        assert continued['rescue']['restored'] is False
    if mode == 'once':
        assert data['once']['rescue']['restored'] is False and data['once']['tick'] == 0
    frames = continued['frames']
    assert len(frames) == 32 and all(f[0] == 62500000 and f[1] == 1 for f in frames)
    tick = 0 if rejected or mode == 'loop' else snapshot['tick']
    for f in frames:
        assert f[2] == tick + 4 and len(f[14]) == 256
        tick = f[2]
    final = continued['cleanup']
    assert final['info'][0] >= tick and final['info'][20] == final['info'][21] > 0
    assert all(final['info'][i] == 0 for i in (4, 5, 6, 7, 8, 10, 11, 12, 13, 14, 22, 23))
    assert final['hud']['info'][3:] == [0, 0] and not final['gpuErrors']
    assert all(final['transport'][key] == 0 for key in ('owners', 'loading', 'published', 'readers', 'sdkPending', 'copied'))
    assert final['transport']['storage'] == [1, 8388608, 268435456, 67108864]
    return {'mode': mode, 'saved_bytes': len(raw), 'snapshot_sha256': snapshot['snapshot'],
            'held_tick': snapshot['tick'], 'restored': not rejected,
            'equal_rgba_channels': None if rejected else len(pixels(data['before-image'])),
            'continued_steps': 128, 'final_tick': final['info'][0], 'attempts': final['info'][20],
            'final_zero_dynamic_display_and_checkpoint_owners': True}


def fault_checks(data, mode, engine):
    faults = ['save', 'snapshot_hash', 'engine_hash', 'retained_size', 'cleanup', 'continuation', 'reload_action']
    if mode not in ('engine', 'corrupt', 'extent', 'tier'):
        faults += ['restored_digest', 'pixels']
    if mode == 'loop':
        faults += ['crash_loop']
    if mode == 'once':
        faults += ['single_use']
    for fault in faults:
        bad = copy.deepcopy(data)
        if fault == 'save': bad['stop']['rescue']['saved'] = False
        elif fault == 'snapshot_hash': bad['snapshot']['snapshot'] = '0' * 64
        elif fault == 'engine_hash': bad['snapshot']['engine'] = '0' * 64
        elif fault == 'retained_size': bad['snapshot']['size'] += 65536
        elif fault == 'cleanup': bad['continued']['cleanup']['info'][14] = 1
        elif fault == 'continuation': bad['continued']['frames'][1][2] += 1
        elif fault == 'reload_action': bad['stop']['actions'].append('Resume')
        elif fault == 'restored_digest': bad['restored']['rescue']['restoredDigest'] = '0' * 64
        elif fault == 'pixels':
            image = bad['restored-image']; raw = bytearray(base64.b64decode(image['data'])); raw[0] ^= 1; image['data'] = base64.b64encode(raw).decode()
        elif fault == 'crash_loop': bad['loop']['saved'] = True
        elif fault == 'single_use': bad['once']['rescue']['restored'] = True
        try:
            inspect(bad, mode, engine)
        except AssertionError:
            continue
        raise AssertionError('altered rescue receipt accepted: ' + fault)
    return len(faults)


def history(data, mode):
    before, continued, snapshot = (data[k] for k in ('before', 'continued', 'snapshot'))
    # The GPU-stop checkpoint may precede the most recently rendered frame.
    # Replay only the accepted prefix actually retained in that checkpoint.
    prefix = [] if mode in ('engine', 'corrupt', 'extent', 'tier', 'loop') else [f for f in before['frames'] if f[2] <= snapshot['tick']]
    return {'schema': 'aethyr.stream-walk-rescue-history.v1', 'profile': before['profile'],
            'frames': prefix + continued['frames'], 'legs': [{'cells': continued['cells']}],
            'cleanup': continued['cleanup']}


if __name__ == '__main__':
    output, mode, wasm = Path(sys.argv[1]), sys.argv[2], Path(sys.argv[3])
    data = {p.stem: json.loads(p.read_bytes()) for p in output.glob('*.json') if p.stem not in ('history', 'replay')}
    result = inspect(data, mode, wasm.read_bytes())
    if '--fault-checks' in sys.argv[4:]: result['altered_rescue_receipts_rejected'] = fault_checks(data, mode, wasm.read_bytes())
    if '--history' in sys.argv[4:]:
        path = output / 'history.json'
        assert not path.exists()
        path.write_text(json.dumps(history(data, mode), separators=(',', ':')))
    print(json.dumps(result, sort_keys=True))
