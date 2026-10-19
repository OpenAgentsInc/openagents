#!/usr/bin/env python3
"""#11816 fault injection on the private SPACE deployment's own native services.

Every injected outage is restored in a finally block. Synthetic owners only.
"""
import json, subprocess, sys, time, threading, importlib.util
from pathlib import Path
spec = importlib.util.spec_from_file_location('q16', '/space/config/deployment-tools/q16.py'); q = importlib.util.module_from_spec(spec); spec.loader.exec_module(q)
spec2 = importlib.util.spec_from_file_location('cases', '/space/config/deployment-tools/q16_cases.py'); c = importlib.util.module_from_spec(spec2); spec2.loader.exec_module(c)
ROOT = Path('/space/qualification-11816')
K = ['/usr/local/bin/k3s', 'kubectl', '-n', 'space']
T0 = time.monotonic()
def ms(): return round((time.monotonic() - T0) * 1000)
def kube(*args, timeout=120):
    r = subprocess.run(K + list(args), capture_output=True, timeout=timeout)
    return {'args': ' '.join(args), 'rc': r.returncode, 'out': r.stdout.decode()[-300:], 'err': r.stderr.decode()[-300:], 'at_ms': ms()}
def scale(dep, n): return kube('scale', 'deployment/' + dep, f'--replicas={n}')
def ready(dep, seconds=300): return kube('rollout', 'status', 'deployment/' + dep, f'--timeout={seconds}s', timeout=seconds + 10)
def state(cid, who):
    s, v, _ = q.call('GET', f'/v1/computers/{cid}', who=who); return {'at_ms': ms(), 'status': s, 'state': (v or {}).get('state'), 'desired': (v or {}).get('desired'), 'attempts': (v or {}).get('attempts'), 'last_error': (v or {}).get('last_error')}
def watch(cid, who, seconds, ev, key, every=2):
    end = time.monotonic() + seconds; seen = []
    while time.monotonic() < end:
        st = state(cid, who)
        if not seen or (st['state'], st['desired'], st['last_error']) != (seen[-1]['state'], seen[-1]['desired'], seen[-1]['last_error']): seen.append(st)
        time.sleep(every)
    ev[key] = seen
def memory(cid, who):
    canary = json.loads((ROOT / f'canary-{cid}.json').read_text())
    tid, r = q.turn(cid, "p=$(cat /home/user/q16.pid); cat /proc/sys/kernel/random/boot_id; echo $p; awk '{print $22}' /proc/$p/stat; tr '\\0' '\\n' </proc/$p/environ | sed -n 's/^Q16_CANARY=//p' | tr -d '\\n' | sha256sum | cut -d' ' -f1; sha256sum /home/user/q16-workspace.bin | cut -d' ' -f1", who=who, seconds=300)
    l = (r.get('stdout') or '').split('\n')
    got = dict(zip(['boot_id', 'pid', 'start_ticks', 'canary_sha256', 'workspace_sha256'], l[:5]))
    return {'turn': tid, 'exit_code': r.get('exit_code'), 'error': r.get('error'), 'preserved': all(got.get(k) == canary[k] for k in got) and len(got) == 5}

def api_restart(cid, who):
    ev = {'computer': cid, 'fault': 'delete the native space-api pod during wake'}
    st = state(cid, who); ev['before'] = st
    if st['state'] != 'stopped': q.wait_state(cid, 'stopped', 600, who=who)
    s, v, _ = q.call('POST', f'/v1/computers/{cid}/wake', {}, who=who); ev['wake_status'] = s; ev['wake_at_ms'] = ms()
    ev['restart'] = kube('delete', 'pod', '-l', 'app.kubernetes.io/name=space-api', '--wait=false')
    t = threading.Thread(target=watch, args=(cid, who, 60, ev, 'states')); t.start()
    ev['api_ready'] = ready('space-api'); t.join()
    q.wait_state(cid, 'awake', 600, who=who); ev['awake_at_ms'] = ms()
    ev['memory'] = memory(cid, who)
    ev['natives'] = q.native_sandboxes_for(cid)[1]
    ev['passed'] = ev['memory']['preserved'] and len([n for n in ev['natives'] if n['lifecycle'] != 'deleted']) == 1
    return ev

def controller_restart(cid, who):
    ev = {'computer': cid, 'fault': 'delete the native cluster-controller pod during stop'}
    q.wait_state(cid, 'stopped', 600, who=who) if state(cid, who)['state'] not in ('awake', 'stopped') else None
    if state(cid, who)['state'] == 'stopped':
        q.call('POST', f'/v1/computers/{cid}/wake', {}, who=who); q.wait_state(cid, 'awake', 600, who=who)
    s, v, _ = q.call('POST', f'/v1/computers/{cid}/stop', {}, who=who); ev['stop_status'] = s; ev['stop_at_ms'] = ms()
    time.sleep(1)
    ev['restart'] = kube('delete', 'pod', '-l', 'app.kubernetes.io/name=space-cluster-controller', '--wait=false')
    t = threading.Thread(target=watch, args=(cid, who, 90, ev, 'states')); t.start()
    ev['controller_ready'] = ready('space-cluster-controller'); t.join()
    q.wait_state(cid, 'stopped', 900, who=who); ev['stopped_at_ms'] = ms()
    q.call('POST', f'/v1/computers/{cid}/wake', {}, who=who); q.wait_state(cid, 'awake', 600, who=who)
    ev['memory'] = memory(cid, who); ev['natives'] = q.native_sandboxes_for(cid)[1]
    ev['passed'] = ev['memory']['preserved'] and len(ev['natives']) == 1
    return ev

def delayed(cid, who):
    ev = {'computer': cid, 'fault': 'scale the native cluster-controller to zero, request stop, hold 60 s, restore'}
    if state(cid, who)['state'] != 'awake':
        q.wait_state(cid, 'stopped', 600, who=who); q.call('POST', f'/v1/computers/{cid}/wake', {}, who=who); q.wait_state(cid, 'awake', 600, who=who)
    try:
        ev['down'] = scale('space-cluster-controller', 0); time.sleep(5)
        s, v, _ = q.call('POST', f'/v1/computers/{cid}/stop', {}, who=who); ev['stop_status'] = s; ev['stop_at_ms'] = ms()
        watch(cid, who, 60, ev, 'states_while_delayed')
    finally:
        ev['up'] = scale('space-cluster-controller', 1); ev['controller_ready'] = ready('space-cluster-controller')
    q.wait_state(cid, 'stopped', 900, who=who); ev['stopped_at_ms'] = ms()
    q.call('POST', f'/v1/computers/{cid}/wake', {}, who=who); q.wait_state(cid, 'awake', 600, who=who)
    ev['memory'] = memory(cid, who); ev['natives'] = q.native_sandboxes_for(cid)[1]
    ev['stopped_while_delayed'] = any(x['state'] == 'stopped' for x in ev['states_while_delayed'])
    ev['passed'] = ev['memory']['preserved'] and len(ev['natives']) == 1 and not ev['stopped_while_delayed']
    return ev

def provider_outage(cid, who):
    ev = {'computer': cid, 'fault': 'scale the native space-api and data-plane to zero for 60 s while waking'}
    if state(cid, who)['state'] != 'stopped': q.wait_state(cid, 'stopped', 600, who=who)
    try:
        ev['down'] = [scale('space-api', 0), scale('space-data-plane', 0)]; time.sleep(5)
        s, v, _ = q.call('POST', f'/v1/computers/{cid}/wake', {}, who=who); ev['wake_status'] = s; ev['wake_at_ms'] = ms()
        watch(cid, who, 60, ev, 'states_during_outage')
    finally:
        ev['up'] = [scale('space-api', 1), scale('space-data-plane', 1)]; ev['ready'] = [ready('space-api'), ready('space-data-plane')]
    ev['restored_at_ms'] = ms()
    q.wait_state(cid, 'awake', 900, who=who); ev['awake_at_ms'] = ms()
    ev['memory'] = memory(cid, who); ev['natives'] = q.native_sandboxes_for(cid)[1]
    ev['identity_unchanged'] = state(cid, who)['status'] == 200
    ev['passed'] = ev['memory']['preserved'] and len(ev['natives']) == 1
    return ev


def storage_phase(cid, who, phase):
    """phase=prepare: make the computer awake. phase=stop: request stop and watch 150 s. phase=finish: wait, wake, check memory."""
    ev = {'computer': cid, 'phase': phase}
    if phase == 'prepare':
        st = state(cid, who)
        if st['state'] != 'stopped': q.wait_state(cid, 'stopped', 600, who=who)
        q.call('POST', f'/v1/computers/{cid}/wake', {}, who=who); q.wait_state(cid, 'awake', 600, who=who); ev['awake'] = True
    elif phase == 'stop':
        s, v, _ = q.call('POST', f'/v1/computers/{cid}/stop', {}, who=who); ev['stop_status'] = s; ev['stop_unix'] = time.time()
        watch(cid, who, 150, ev, 'states_during_storage_outage')
        ev['stopped_during_outage'] = any(x['state'] == 'stopped' for x in ev['states_during_storage_outage'])
    else:
        t = time.time(); q.wait_state(cid, 'stopped', 1200, who=who); ev['stopped_after_restore_s'] = round(time.time() - t, 1)
        q.call('POST', f'/v1/computers/{cid}/wake', {}, who=who); q.wait_state(cid, 'awake', 600, who=who)
        ev['memory'] = memory(cid, who); ev['natives'] = q.native_sandboxes_for(cid)[1]
    return ev

def worker_phase(z, w, phase):
    """prepare: W stopped, Z mid-turn. watch: record states after the worker reset, then recover both."""
    ev = {'awake_computer': z, 'stopped_computer': w, 'phase': phase}
    if phase == 'prepare':
        if state(w, 'b')['state'] != 'stopped': q.wait_state(w, 'stopped', 900, who='b')
        if state(z, 'a')['state'] != 'stopped': q.wait_state(z, 'stopped', 900, who='a')
        q.call('POST', f'/v1/computers/{z}/wake', {}, who='a'); q.wait_state(z, 'awake', 600, who='a')
        tid, r = q.turn(z, 'echo m1 > /home/user/q16-marker-checkpointed', who='a'); ev['marker1_turn'] = q.brief(r)
        q.wait_state(z, 'stopped', 900, who='a')
        q.call('POST', f'/v1/computers/{z}/wake', {}, who='a'); q.wait_state(z, 'awake', 600, who='a')
        s, v, _ = q.call('POST', f'/v1/computers/{z}/turns', {'command': 'echo m2 > /home/user/q16-marker-uncheckpointed; sleep 55; echo survived', 'timeout_seconds': 58}, who='a')
        ev['long_turn'] = v.get('id'); time.sleep(3); ev['z_state'] = state(z, 'a'); ev['w_state'] = state(w, 'b'); ev['ready_unix'] = time.time()
        (ROOT / 'worker-loss-long-turn.json').write_text(json.dumps({'turn': ev['long_turn']}))
    else:
        start = time.time(); seen = {z: [], w: []}
        while time.time() - start < 1200:
            for cid, who in ((z, 'a'), (w, 'b')):
                st = state(cid, who); st['t'] = round(time.time() - start, 1)
                if not seen[cid] or (st['state'], st['last_error']) != (seen[cid][-1]['state'], seen[cid][-1]['last_error']): seen[cid].append(st)
            nodes = subprocess.run(K[:2] + ['get', 'nodes', 'oa-space-service-worker', '-o', 'jsonpath={.status.conditions[?(@.type=="Ready")].status}'], capture_output=True, timeout=20).stdout.decode()
            if nodes == 'True' and 'worker_ready_s' not in ev: ev['worker_ready_s'] = round(time.time() - start, 1)
            if 'worker_ready_s' in ev and time.time() - start > ev['worker_ready_s'] + 240 and seen[z][-1]['state'] in ('stopped', 'awake', 'unknown', 'failed') and seen[w][-1]['state'] in ('stopped', 'awake', 'unknown', 'failed'):
                break
            time.sleep(5)
        ev['states'] = seen
        lt = json.loads((ROOT / 'worker-loss-long-turn.json').read_text())['turn']
        s, r, _ = q.call('GET', f'/v1/computers/{z}/turns/{lt}', who='a'); ev['long_turn_after'] = q.brief(r)
        ev['natives_z'] = q.native_sandboxes_for(z)[1]; ev['natives_w'] = q.native_sandboxes_for(w)[1]
        # Recover the checkpointed computer.
        t = time.time()
        try:
            if state(w, 'b')['state'] == 'stopped':
                q.call('POST', f'/v1/computers/{w}/wake', {}, who='b')
            q.wait_state(w, 'awake', 900, who='b'); ev['w_wake_s'] = round(time.time() - t, 1); ev['w_memory'] = memory(w, 'b')
        except Exception as e: ev['w_recovery_error'] = f'{type(e).__name__}: {e}'[:500]
        t = time.time()
        try:
            st = state(z, 'a')
            if st['state'] == 'stopped': q.call('POST', f'/v1/computers/{z}/wake', {}, who='a')
            q.wait_state(z, 'awake', 600, who='a'); ev['z_wake_s'] = round(time.time() - t, 1)
            tid, r = q.turn(z, 'cat /proc/sys/kernel/random/boot_id; ls /home/user | grep q16-marker', who='a'); ev['z_after'] = {'turn': q.brief(r), 'stdout': r.get('stdout')}
            ev['z_memory'] = memory(z, 'a')
        except Exception as e: ev['z_recovery_error'] = f'{type(e).__name__}: {e}'[:500]
        ev['natives_z_final'] = q.native_sandboxes_for(z)[1]; ev['natives_w_final'] = q.native_sandboxes_for(w)[1]
    return ev

def lifecycle_loop(cid, who, seconds):
    """Cycle stop and wake for a window (used during a Computers revision rollout)."""
    ev = {'computer': cid, 'cycles': []}
    end = time.time() + float(seconds)
    while time.time() < end:
        t = time.time(); st = state(cid, who)
        try:
            if st['state'] == 'awake':
                s, v, _ = q.call('POST', f'/v1/computers/{cid}/stop', {}, who=who); q.wait_state(cid, 'stopped', 900, who=who); ev['cycles'].append({'op': 'stop', 'status': s, 's': round(time.time() - t, 1), 'at': round(t)})
            elif st['state'] == 'stopped':
                s, v, _ = q.call('POST', f'/v1/computers/{cid}/wake', {}, who=who); q.wait_state(cid, 'awake', 900, who=who); ev['cycles'].append({'op': 'wake', 'status': s, 's': round(time.time() - t, 1), 'at': round(t)})
            else:
                ev['cycles'].append({'op': 'observe', 'state': st['state'], 'at': round(t)}); time.sleep(3)
        except Exception as e:
            ev['cycles'].append({'op': 'error', 'error': f'{type(e).__name__}: {e}'[:300], 'at': round(t)}); break
    if state(cid, who)['state'] == 'stopped':
        q.call('POST', f'/v1/computers/{cid}/wake', {}, who=who); q.wait_state(cid, 'awake', 900, who=who)
    ev['memory'] = memory(cid, who); ev['natives'] = q.native_sandboxes_for(cid)[1]
    ev['passed'] = ev['memory']['preserved'] and len(ev['natives']) == 1 and not any(c['op'] == 'error' for c in ev['cycles'])
    return ev

PARTIAL = {}
CANDIDATE = {'space-api': ('api', 'us-central1-docker.pkg.dev/openagentsgemini/oa-space-service/native/api-cleanup@sha256:0aee5f68945231fc28034cffd0d8f844fed7a4f18c6be674997b6a155dd6e060'),
             'space-admin-api': ('api', 'us-central1-docker.pkg.dev/openagentsgemini/oa-space-service/native/api-cleanup@sha256:0aee5f68945231fc28034cffd0d8f844fed7a4f18c6be674997b6a155dd6e060'),
             'space-cluster-controller': ('cluster-controller', 'us-central1-docker.pkg.dev/openagentsgemini/oa-space-service/native/cluster-controller-cleanup@sha256:f9fbc943591a5a7b7404a6fb18e39ad66ca49ac6e03527bd1c15a1970219906f')}
def images():
    out = {}
    for d in CANDIDATE:
        r = subprocess.run(K + ['get', 'deploy', d, '-o', 'jsonpath={.spec.template.spec.containers[0].image}'], capture_output=True, timeout=20)
        out[d] = r.stdout.decode()
    return out
def verify(existing, who_existing, tag):
    ev = {}
    t = time.time()
    if state(existing, who_existing)['state'] == 'stopped':
        q.call('POST', f'/v1/computers/{existing}/wake', {}, who=who_existing); q.wait_state(existing, 'awake', 900, who=who_existing)
    ev['existing_wake_s'] = round(time.time() - t, 1); ev['existing_memory'] = memory(existing, who_existing)
    q.wait_state(existing, 'stopped', 900, who=who_existing); ev['existing_suspended'] = True
    rec = q.create_trial('cold', 'warm', 1, 'upgrade-' + tag, 0, 'a', 'small'); ev['new_computer'] = {k: rec.get(k) for k in ('succeeded', 'first_useful_ms', 'deleted', 'error', 'computer')}
    return ev
def upgrade(existing, who_existing, ev=None):
    ev = PARTIAL; ev.update({'existing': existing, 'previous_images': images()})
    try:
        for d, (container, image) in CANDIDATE.items():
            ev.setdefault('set', []).append(kube('set', 'image', 'deployment/' + d, f'{container}={image}'))
        ev['upgrade_ready'] = [ready(d) for d in CANDIDATE]; ev['upgraded_images'] = images()
        ev['upgraded'] = verify(existing, who_existing, 'candidate')
    finally:
        ev['undo'] = [kube('rollout', 'undo', 'deployment/' + d) for d in CANDIDATE]
        ev['rollback_ready'] = [ready(d) for d in CANDIDATE]
    ev['rolled_back_images'] = images()
    ev['rolled_back'] = verify(existing, who_existing, 'rollback')
    ev['rollback_restored_previous'] = ev['rolled_back_images'] == ev['previous_images']
    ev['passed'] = (ev['rollback_restored_previous'] and ev['upgraded']['existing_memory']['preserved'] and ev['rolled_back']['existing_memory']['preserved']
                    and ev['upgraded']['new_computer']['succeeded'] and ev['rolled_back']['new_computer']['succeeded'])
    return ev

if __name__ == '__main__':
    name = sys.argv[1]
    fn = {'api_restart': api_restart, 'controller_restart': controller_restart, 'delayed': delayed, 'provider_outage': provider_outage, 'storage': storage_phase, 'worker': worker_phase, 'lifecycle_loop': lifecycle_loop, 'upgrade': upgrade}[name]
    ev = {'case': name, 'started_unix': int(time.time())}
    try: ev.update(fn(*sys.argv[2:]))
    except Exception as e: ev.update(PARTIAL); ev['exception'] = f'{type(e).__name__}: {e}'[:800]; ev['passed'] = False
    ev['observed_unix'] = int(time.time())
    (ROOT / (f'fault-{name}-' + sys.argv[4] + '.json' if name in ('storage', 'worker') else f'fault-{name}.json')).write_text(json.dumps(ev, indent=1)); print(json.dumps(ev))
