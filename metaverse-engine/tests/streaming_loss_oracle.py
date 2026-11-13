# Firm reason: 2 (independent reconstruction of device-loss outcomes, absolute deadlines, canonical state preservation and complete GPU readbacks).
"""Inspect actual loss callbacks; this does not infer persistent rescue."""
import hashlib
import copy
import json
import math
from pathlib import Path
import sys
import tempfile

from streaming_hud_oracle import capture


def inspect(output):
    receipt = json.loads((output / 'receipt.json').read_bytes())
    assert receipt['schema'] == 'aethyr.stream-walk-loss.v1'
    mode = receipt['mode']
    assert mode in ('steady', 'repeated', 'restart', 'adapter-failure', 'deadline', 'late-device', 'frame-loss', 'route')
    before, after = receipt['before'], receipt['after']
    states, final = receipt['states'], receipt['cleanup']
    assert receipt['profile']['stateSchema'] == 'aethyr.stream-walk-state.v1'
    assert receipt['profile']['presentationSchema'] == 'aethyr.stream-walk-display.v1'
    assert not any(word in json.dumps(receipt['adapter']).lower() for word in ('swiftshader', 'llvmpipe', 'software'))
    assert len(before['digest']) == len(after['digest']) == 64
    bytes.fromhex(before['digest'])
    bytes.fromhex(after['digest'])
    assert all(final['info'][i] == 0 for i in (4, 5, 6, 7, 8, 10, 11, 12, 13, 14, 22, 23))
    assert final['hud']['info'][3:] == [0, 0] and not final['gpuErrors']
    assert all(final['transport'][key] == 0 for key in ('owners', 'loading', 'published', 'readers', 'sdkPending', 'copied'))
    # The page-wide pack reactor remains live; this consumer releases its
    # own 8 MiB engine arena and every per-transfer owner/ticket.
    assert final['transport']['storage'] == [1, 8388608, 268435456, 67108864]
    assert final['transportBeforeDispose']['storage'][0:2] == [2, 16777216]
    assert before['hud']['info'][2:] == [98304, 1, 1]
    for state in states:
        token = state['generation']
        assert token == str(int(token)) and 0 < int(token) <= (1 << 63) - 1
        if state['state'] == 'recovered':
            recovery = state['lastRecovery']
            assert state['reason'] == 0 and 0 <= recovery['ms'] < 10000
            assert recovery['granted'] == recovery['uploaded'] > 0
            assert all(recovery['before'][i] == recovery['after'][i] for i in (0, 4, 5, 6, 7, 8, 10, 11, 14, 20, 26))
    stopped = mode in ('repeated', 'adapter-failure', 'deadline', 'late-device')
    assert receipt['blocked'] == stopped
    assert receipt['faceBlocked'] is (True if stopped else None)
    assert after['status']['state'] == ('stopped' if stopped else 'recovered')
    expected_ticks = before['stats']['info'][0] + (4 if mode == 'frame-loss' else 12800 if mode == 'route' else 0)
    assert after['stats']['info'][0] == expected_ticks
    images_equal = None
    if mode not in ('frame-loss', 'route'):
        assert before['digest'] == after['digest'] and before['cells'] == after['cells']
        assert before['stats']['info'][14] == after['stats']['info'][14] > 0
    if mode in ('steady', 'restart'):
        a, b = capture(output / 'before.json'), capture(output / 'after.json')
        assert a == b and a[:2] == (320, 180)
        images_equal = len(a[2])
    if mode == 'repeated':
        assert len(states) == 4
        assert [int(s['generation']) for s in states] == [1, 2, 3, 3]
        assert [s['losses'] for s in states] == [1, 2, 3, 3]
        assert [s['state'] for s in states] == ['recovered'] * 3 + ['stopped']
        assert states[-1]['reason'] == 1 and states[-1]['last'] - states[0]['began'] < 60000
    elif mode in ('deadline', 'late-device'):
        assert json.loads((output / 'active-frame-gate.json').read_bytes()) == {'activeFaceBlocked': True}
        assert len(states) == 1 and states[0]['reason'] == 2
        assert 10000 <= receipt['stoppedAt'] - states[0]['began'] < 10100
        assert receipt['lateDeviceDestroyed'] == (mode == 'late-device')
    elif mode == 'adapter-failure':
        assert len(states) == 1 and states[0]['reason'] == 3
    else:
        assert len(states) == (2 if mode == 'route' else 1)
        assert all(s['state'] == 'recovered' and s['reason'] == 0 and s['lastRecovery']['ms'] < 10000 for s in states)
        assert int(states[-1]['generation']) == states[-1]['losses'] == (2 if mode in ('restart', 'route') else 1)
        assert states[-1]['lastRecovery']['epoch'] == (3 if mode in ('restart', 'route') else 2)
        assert all(s['lastRecovery']['granted'] == s['lastRecovery']['uploaded'] > 0 for s in states)
    if mode == 'route':
        assert len(receipt['legs']) == 2 and len(receipt['frames']) == 3233
        outward, returned = receipt['legs']
        assert outward['last'][5] > 704 and returned['last'][5] < 64
        assert outward['cells'][0][0] == 0
        assert returned['cells'][0][2] > outward['cells'][0][2] > 0
        for leg in receipt['legs']:
            assert [c[3] for c in leg['cells']] == list(range(1, 13))
            assert leg['hud']['info'][3:] == [1, 1]
    timings = sorted(frame[12] for frame in receipt['frames'])
    assert timings and all(isinstance(value, (int, float)) and math.isfinite(value) and value >= 0 for value in timings)
    return {'mode': mode, 'actual_losses': 4 if mode == 'repeated' else 2 if mode in ('restart', 'route') else 1,
            'complete_recovery_channels_equal': images_equal, 'canonical_state_equal': mode not in ('frame-loss', 'route'),
            'paused_paid_tick': expected_ticks, 'final_zero_dynamic_and_display_owners': True,
            'remaining_shared_transport_bytes': 8388608,
            'frame_ms': {name: timings[int((len(timings) - 1) * p)] for name, p in (('p50', .5), ('p95', .95), ('p99', .99))},
            'cpu_resource_peak': after['stats']['info'][15], 'core_static_bytes': after['stats']['info'][27],
            'active_drawn_updated_peaks': [max(frame[i] for frame in receipt['frames']) for i in (6, 7, 8)],
            'blocked_audio_refused_cells': after['stats']['info'][16:20],
            'receipt_sha256': hashlib.sha256((output / 'receipt.json').read_bytes()).hexdigest(),
            'persistent_rescue_qualified': False}


def fault_checks(output):
    receipt = json.loads((output / 'receipt.json').read_bytes())
    faults = ['unknown_mode', 'retained_owner', 'retained_transport', 'missing_pause', 'paid_tick', 'generation']
    if receipt['faceBlocked']:
        faults.append('sdk_pause_bypassed')
    if receipt['mode'] not in ('frame-loss', 'route'):
        faults.append('canonical_digest')
    if receipt['mode'] in ('deadline', 'late-device'):
        faults += ['early_deadline', 'late_deadline']
    if any(s['state'] == 'recovered' for s in receipt['states']):
        faults += ['missing_upload', 'clock_advanced_during_rebuild']
    with tempfile.TemporaryDirectory(prefix='aethyr-loss-oracle-') as temporary:
        target = Path(temporary)
        for name in ('before.json', 'after.json', 'active-frame-gate.json'):
            if (output / name).exists():
                (target / name).write_bytes((output / name).read_bytes())
        for fault in faults:
            changed = copy.deepcopy(receipt)
            if fault == 'unknown_mode':
                changed['mode'] = 'unsupported'
            elif fault == 'generation':
                changed['states'][-1]['generation'] = '0'
            elif fault == 'retained_owner':
                changed['cleanup']['info'][12] = 1
            elif fault == 'retained_transport':
                changed['cleanup']['transport']['storage'][1] += 1
            elif fault == 'missing_pause':
                changed['blocked'] = not changed['blocked']
            elif fault == 'sdk_pause_bypassed':
                changed['faceBlocked'] = False
            elif fault == 'paid_tick':
                changed['after']['stats']['info'][0] += 1
            elif fault == 'canonical_digest':
                changed['after']['digest'] = 'f' * 64
            elif fault in ('early_deadline', 'late_deadline'):
                changed['stoppedAt'] = changed['states'][0]['began'] + (9999 if fault == 'early_deadline' else 10100)
            else:
                recovery = next(s['lastRecovery'] for s in changed['states'] if s['state'] == 'recovered')
                if fault == 'missing_upload':
                    recovery['uploaded'] -= 1
                else:
                    recovery['after'][0] += 1
            (target / 'receipt.json').write_text(json.dumps(changed))
            try:
                inspect(target)
            except AssertionError:
                continue
            raise AssertionError('altered loss history accepted: ' + fault)
    return len(faults)


def inspect_live(path):
    receipt = json.loads(path.read_bytes())
    assert receipt['schema'] == 'aethyr.stream-walk-live-loss.v1'
    held, following, status = receipt['held'], receipt['next'], receipt['status']
    assert held['before'] == 'recovering' and following['before'] == 'recovered'
    assert held['ns'] == 0 and held['ended'] - held['entered'] >= 900
    excess = following['ns'] / 1e6 - (following['entered'] - held['ended'])
    assert abs(excess - receipt['excessMs']) < 1e-6 and excess <= 100
    assert status['state'] == 'recovered' and status['serial'] == status['losses'] == 1 and status['reason'] == 0
    assert 1000 <= status['lastRecovery']['ms'] < 10000
    assert status['lastRecovery']['before'][0] == status['lastRecovery']['after'][0]
    assert not any(word in json.dumps(status['lastRecovery']['adapter']).lower() for word in ('swiftshader', 'llvmpipe', 'software'))
    final = receipt['cleanup']
    assert all(final['info'][i] == 0 for i in (8, 10, 11, 12, 13, 14, 22, 23))
    assert not final['gpuErrors'] and final['transport']['storage'][0:2] == [1, 8388608]
    return {'held_step_ns': held['ns'], 'sdk_wait_ms': held['ended'] - held['entered'],
            'following_step_ns': following['ns'], 'excess_ms': excess,
            'physical_device_recovery_ms': status['lastRecovery']['ms'],
            'final_zero_dynamic_and_display_owners': True}


if __name__ == '__main__':
    if sys.argv[1] == '--live':
        print(json.dumps(inspect_live(Path(sys.argv[2])), sort_keys=True))
        sys.exit(0)
    output = Path(sys.argv[1])
    result = inspect(output)
    if '--fault-checks' in sys.argv[2:]:
        result['altered_loss_receipts_rejected'] = fault_checks(output)
    print(json.dumps(result, sort_keys=True))
