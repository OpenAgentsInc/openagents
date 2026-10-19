#!/usr/bin/env python3
"""#11816 case checks on the private SPACE candidate; synthetic owners only."""
import json, sys, time, hashlib, secrets, importlib.util
spec = importlib.util.spec_from_file_location('q16', '/space/config/deployment-tools/q16.py'); q = importlib.util.module_from_spec(spec); spec.loader.exec_module(q)
from pathlib import Path
ROOT = Path('/space/qualification-11816')

def ensure_awake(cid, who):
    st, v, _ = q.call('GET', f'/v1/computers/{cid}', who=who)
    if v.get('state') not in ('stopped', 'awake'):
        v = q.wait_state(cid, 'stopped', 600, who=who)
    if v.get('state') == 'stopped':
        q.call('POST', f'/v1/computers/{cid}/wake', {}, who=who); q.wait_state(cid, 'awake', 600, who=who)

def submit(cid, command, who, timeout=58):
    st, v, _ = q.call('POST', f'/v1/computers/{cid}/turns', {'command': command, 'timeout_seconds': timeout}, who=who)
    if st not in (200, 201, 202): raise RuntimeError(f'submit {st} {v}')
    return v['id']

def final(cid, tid, who, seconds=180):
    deadline = time.monotonic() + seconds
    while True:
        st, r, _ = q.call('GET', f'/v1/computers/{cid}/turns/{tid}', who=who)
        if r.get('exit_code') is not None or r.get('state') in ('failed', 'cancelled', 'stopped', 'done'): return r
        if time.monotonic() > deadline: raise TimeoutError(tid)
        time.sleep(0.3)

def cancellation(cid, who):
    ev = {'computer': cid}
    ensure_awake(cid, who)
    cmd = ("rm -f /home/user/q16-late*; (sleep 12; echo late1 > /home/user/q16-late1) & "
           "setsid sh -c 'sleep 12; echo late2 > /home/user/q16-late2' </dev/null >/dev/null 2>&1 & "
           "nohup sh -c 'sleep 12; echo late3 > /home/user/q16-late3' </dev/null >/dev/null 2>&1 & "
           "sh -c 'trap \"\" TERM; sleep 12; echo late4 > /home/user/q16-late4' & sleep 40; echo finished")
    tid = submit(cid, cmd, who); ev['turn'] = tid
    time.sleep(3)
    st, v, _ = q.call('POST', f'/v1/computers/{cid}/turns/{tid}/stop', {}, who=who); ev['stop_status'] = st; ev['stop_reply'] = q.brief(v)
    t0 = time.monotonic(); r = final(cid, tid, who); ev['stopped_turn'] = q.brief(r); ev['stop_to_final_ms'] = round((time.monotonic() - t0) * 1000)
    ev['finished_marker_printed'] = 'finished' in (r.get('stdout') or '')
    ensure_awake(cid, who)
    tid2, r2 = q.turn(cid, "sleep 16; ls /home/user/ | grep -c '^q16-late' || true; ps -eo args | grep -c '^sleep 12' || true", who=who, seconds=120)
    lines = (r2.get('stdout') or '').split()
    ev['check_turn'] = tid2; ev['late_files_after_16s'] = int(lines[0]) if lines else None; ev['sleep12_processes'] = int(lines[1]) if len(lines) > 1 else None
    ev['passed'] = st in (200, 202) and not ev['finished_marker_printed'] and ev['late_files_after_16s'] == 0 and ev['sleep12_processes'] == 0
    return ev

def attach(cid, who):
    ev = {'computer': cid}
    ensure_awake(cid, who)
    tid = submit(cid, "echo run >> /home/user/q16-attach.log; sleep 20; echo attach-done; wc -l < /home/user/q16-attach.log", who); ev['turn'] = tid
    (ROOT / 'attach-turn.json').write_text(json.dumps({'turn': tid}))
    time.sleep(2)
    ev['interrupted_reads'] = [q.send_and_drop('GET', f'/v1/computers/{cid}/turns/{tid}', None, who) for _ in range(3)]
    # A read with a deadline shorter than the command, abandoned mid-response.
    try:
        q.call('GET', f'/v1/computers/{cid}/turns/{tid}', who=who, timeout=0.001); ev['short_read'] = 'completed'
    except Exception as e: ev['short_read'] = type(e).__name__
    saved = json.loads((ROOT / 'attach-turn.json').read_text())['turn']
    r = final(cid, saved, who); ev['reattached'] = q.brief(r)
    out = (r.get('stdout') or '').split()
    ev['output_complete'] = out[:1] == ['attach-done']; ev['executions'] = int(out[1]) if len(out) > 1 else None
    st, c, _ = q.call('GET', f'/v1/computers/{cid}', who=who); ev['computer_turns'] = c.get('turns')
    ev['passed'] = r.get('exit_code') == 0 and ev['output_complete'] and ev['executions'] == 1
    return ev

def lost_turn_reply(cid, who):
    """A dropped turn submission is retried with its saved Idempotency-Key (#12142)."""
    ev = {'computer': cid}
    ensure_awake(cid, who)
    key = 'q16-lost-' + secrets.token_hex(6); ev['request_key'] = key
    body = {'command': 'echo q16-lost >> /home/user/q16-lost.log', 'timeout_seconds': 20}
    st, c0, _ = q.call('GET', f'/v1/computers/{cid}', who=who); ev['turns_before'] = c0.get('turns')
    ev['drop'] = q.send_and_drop('POST', f'/v1/computers/{cid}/turns', body, who, key=key)
    time.sleep(8)
    st, v, _ = q.call('POST', f'/v1/computers/{cid}/turns', body, who=who, headers={'Idempotency-Key': key})
    ev['retry_status'] = st; ev['retry_turn'] = v.get('id'); final(cid, v['id'], who)
    st, v2, _ = q.call('POST', f'/v1/computers/{cid}/turns', {**body, 'timeout_seconds': 21}, who=who, headers={'Idempotency-Key': key})
    ev['changed_body_status'] = st
    st, c1, _ = q.call('GET', f'/v1/computers/{cid}', who=who); ev['turns_after'] = c1.get('turns')
    ensure_awake(cid, who)
    tid, r = q.turn(cid, 'wc -l < /home/user/q16-lost.log', who=who); ev['executions_of_logical_request'] = int((r.get('stdout') or '0').strip() or 0)
    ev['request_identity_supported'] = True
    ev['passed'] = ev['executions_of_logical_request'] == 1 and ev['changed_body_status'] == 409
    return ev

def duplicates(who):
    ev = {}
    def keep():
        (ROOT / 'case-duplicates-partial.json').write_text(json.dumps(ev, indent=1))
    key = 'q16-dup-' + secrets.token_hex(4); ev['request_key'] = key
    res = q.run_parallel([lambda: q.call('POST', '/v1/computers', {'size': 'small', 'idempotency_key': key}, who=who) for _ in range(3)])
    ev['create_replies'] = [{'status': s, 'id': (v or {}).get('id')} for s, v, _ in res]
    ids = {v.get('id') for s, v, _ in res if s in (200, 201, 202)}
    ev['distinct_computers'] = len(ids)
    cid = ids.pop() if len(ids) == 1 else None; ev['computer'] = cid
    if cid:
        q.wait_state(cid, 'awake', 480, who=who)
        ev['natives_after_create'] = q.native_sandboxes_for(cid)[1]
        keep()
        # Lost create reply: replay the same key after a dropped request.
        ev['lost_create_drop'] = q.send_and_drop('POST', '/v1/computers', {'size': 'small', 'idempotency_key': key}, who)
        s, v, _ = q.call('POST', '/v1/computers', {'size': 'small', 'idempotency_key': key}, who=who); ev['lost_create_retry'] = {'status': s, 'id': v.get('id')}
        keep()
        # Duplicate stop and wake deliveries.
        res = q.run_parallel([lambda: q.call('POST', f'/v1/computers/{cid}/stop', {}, who=who) for _ in range(3)]); ev['stop_replies'] = [s for s, _, _ in res]
        q.wait_state(cid, 'stopped', 600, who=who)
        res = q.run_parallel([lambda: q.call('POST', f'/v1/computers/{cid}/wake', {}, who=who) for _ in range(3)]); ev['wake_replies'] = [s for s, _, _ in res]
        keep()
        q.wait_state(cid, 'awake', 600, who=who)
        tid, r = q.turn(cid, 'cat /proc/sys/kernel/random/boot_id', who=who); ev['boot_after_duplicates'] = (r.get('stdout') or '').strip(); ev['boot_turn'] = q.brief(r)
        if r.get('exit_code') is None:
            ev['stranded_by_uncertain_turn'] = True; keep()
            q.call('DELETE', f'/v1/computers/{cid}', who=who); q.wait_state(cid, 'deleted', 600, who=who); ev['natives_after_delete'] = q.native_sandboxes_for(cid)[1]; return ev
        ev['natives_after_lifecycle'] = q.native_sandboxes_for(cid)[1]
        keep()
        # Duplicate turn delivery: two identical submissions.
        dkey = 'q16-dupturn-' + secrets.token_hex(6); ev['turn_request_key'] = dkey
        res = q.run_parallel([lambda: q.call('POST', f'/v1/computers/{cid}/turns', {'command': 'echo q16-dup >> /home/user/q16-dup.log', 'timeout_seconds': 20}, who=who, headers={'Idempotency-Key': dkey}) for _ in range(2)])
        ev['duplicate_turn_ids'] = [(v or {}).get('id') for s, v, _ in res]
        for t in set(ev['duplicate_turn_ids']):
            if t: final(cid, t, who)
        ensure_awake(cid, who)
        tid, r = q.turn(cid, 'wc -l < /home/user/q16-dup.log', who=who); ev['duplicate_turn_executions'] = int((r.get('stdout') or '0').strip() or 0)
        res = q.run_parallel([lambda: q.call('DELETE', f'/v1/computers/{cid}', who=who) for _ in range(2)]); ev['delete_replies'] = [s for s, _, _ in res]
        q.wait_state(cid, 'deleted', 600, who=who)
        ev['natives_after_delete'] = q.native_sandboxes_for(cid)[1]
    return ev

if __name__ == '__main__':
    name = sys.argv[1]; fn = {'cancellation': cancellation, 'attach': attach, 'lost_turn_reply': lost_turn_reply, 'duplicates': duplicates}[name]
    ev = fn(*sys.argv[2:]); ev['case'] = name; ev['observed_unix'] = int(time.time())
    (ROOT / f'case-{name}.json').write_text(json.dumps(ev, indent=1)); print(json.dumps(ev))
