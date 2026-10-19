#!/usr/bin/env python3
"""#11816 qualification driver for the private SPACE Computers candidate.

Runs on oa-space-service-control as root. Uses synthetic owners only, prints no
credentials, and keeps every request identity in a private state directory.
"""
import argparse, hashlib, http.client, json, os, secrets, subprocess, sys, time, threading
from pathlib import Path
sys.path.insert(0, '/space/config/deployment-tools')
from cluster_secrets import identity, secret

ROOT = Path('/space/qualification-11816')
HOST = 'oa-space-service-computers-ezxz4mgdsq-uc.a.run.app'
NATIVE = 'oa-space-service-api-ezxz4mgdsq-uc.a.run.app'
PREFIX = b'openagents.web.chat.account.v1\0'
# Synthetic account ids: never a real account. The owner is the web's digest form.
SYNTHETIC = {'a': 'acct_space_qual_11816_a', 'b': 'acct_space_qual_11816_b'}
def owner(name):
    return 'account:' + hashlib.sha256(PREFIX + SYNTHETIC[name].encode()).hexdigest()

_fixture = None
def fixture():
    global _fixture
    if _fixture is None:
        _fixture = secret('qualification', identity())
    return _fixture

def log(event):
    event = {'t': round(time.time(), 3), **event}
    with open(ROOT / 'events.jsonl', 'a') as f:
        f.write(json.dumps(event, sort_keys=True) + '\n')
    return event

def call(method, path, body=None, who='a', timeout=60, headers=None):
    conn = http.client.HTTPSConnection(HOST, timeout=timeout)
    h = {'Authorization': 'Bearer ' + fixture()['computers_token'], 'Content-Type': 'application/json',
         'X-OpenAgents-Test': '1'}
    if who is not None:
        h['X-OpenAgents-Account'] = owner(who) if who in SYNTHETIC else who
    h.update(headers or {})
    started = time.monotonic()
    try:
        conn.request(method, path, body=json.dumps(body).encode() if body is not None else (b'' if method in ('POST', 'PUT') else None), headers=h)
        r = conn.getresponse(); raw = r.read(1048577)
        try: value = json.loads(raw) if raw else None
        except json.JSONDecodeError: value = {'non_json_bytes': len(raw)}
        return r.status, value, round((time.monotonic() - started) * 1000, 1)
    finally:
        conn.close()

def native(method, path, body=None, admin=False, timeout=30):
    f = fixture()
    if admin:
        ip = subprocess.run(['/usr/local/bin/k3s','kubectl','-n','space','get','service','space-admin-api','-o','jsonpath={.spec.clusterIP}'], capture_output=True, check=True, timeout=10).stdout.decode()
        conn = http.client.HTTPConnection(ip, 8080, timeout=timeout); h = {'Authorization': 'Bearer ' + f['admin_session']}
    else:
        conn = http.client.HTTPSConnection(NATIVE, timeout=timeout); h = {'X-API-Key': f['api_key'], 'Authorization': 'Bearer ' + f['api_key']}
    h['Content-Type'] = 'application/json'
    try:
        conn.request(method, path, body=json.dumps(body).encode() if body is not None else None, headers=h)
        r = conn.getresponse(); raw = r.read(4 * 1048576)
        try: value = json.loads(raw) if raw else None
        except json.JSONDecodeError: value = {'non_json': raw[:200].decode(errors='replace')}
        return r.status, value
    finally:
        conn.close()

def sql(query, db='space_computers'):
    from database_check import credential
    value = credential()
    if db == 'space_computers':
        user, password = 'space_computers', secret('computers-database', identity())['password']
    else:
        user, password = value['bootstrap_login'], value['bootstrap_password']
    env = {**os.environ, 'PGHOST': value['private_ip'], 'PGDATABASE': db, 'PGUSER': user, 'PGPASSWORD': password,
           'PGSSLMODE': 'verify-ca', 'PGSSLROOTCERT': '/space/config/postgres-ca.crt', 'PGCONNECT_TIMEOUT': '5', 'PSQL_HISTORY': '/dev/null'}
    r = subprocess.run(['psql', '--no-psqlrc', '--no-password', '--tuples-only', '--no-align', '--set=ON_ERROR_STOP=1', '--command', query], env=env, capture_output=True, timeout=30)
    if r.returncode: raise RuntimeError('SQL failed: ' + r.stderr.decode()[:400])
    return r.stdout.decode()


def useful(cid, who, nonce, extra=''):
    """Run a command whose output is checked here, not trusted from the exit code."""
    cmd = f"printf %s {nonce} | sha256sum | cut -d' ' -f1; nproc; awk '/MemTotal/{{print $2}}' /proc/meminfo; cat /proc/sys/kernel/random/boot_id" + extra
    tid, r = turn(cid, cmd, who=who)
    lines = (r.get('stdout') or '').split('\n')
    ok = r.get('exit_code') == 0 and lines and lines[0] == hashlib.sha256(nonce.encode()).hexdigest()
    return ok, tid, r, lines

def create_trial(kind, cache, conc, label, index, who, size):
    key = f'q16-{label}-{index}-{secrets.token_hex(4)}'
    rec = {'kind': kind, 'cache': cache, 'concurrency': conc, 'label': label, 'index': index, 'request_key': key, 'size': size, 'owner': who}
    t0 = time.monotonic(); cid = None
    try:
        s, v, ms = call('POST', '/v1/computers', {'size': size, 'idempotency_key': key}, who=who)
        rec['create_status'] = s; rec['create_ms'] = round((time.monotonic() - t0) * 1000)
        if s not in (200, 201, 202): raise RuntimeError(f'create {s}: {json.dumps(v)[:300]}')
        cid = v['id']; rec['computer'] = cid
        v = wait_state(cid, 'awake', 480, who=who, poll=0.1)
        rec['resource'] = v.get('resource'); rec['readiness_ms'] = round((time.monotonic() - t0) * 1000)
        nonce = secrets.token_hex(16)
        ok, tid, r, lines = useful(cid, who, nonce)
        rec['first_useful_ms'] = round((time.monotonic() - t0) * 1000); rec['turn'] = tid
        rec['observed'] = {'vcpus': int(lines[1]) if len(lines) > 1 and lines[1].isdigit() else None,
                           'memtotal_kib': int(lines[2]) if len(lines) > 2 and lines[2].isdigit() else None,
                           'boot_id': lines[3] if len(lines) > 3 else None}
        rec['succeeded'] = bool(ok)
        if not ok: rec['error'] = 'useful output check failed: ' + json.dumps(brief(r))[:300]
    except Exception as e:
        rec['succeeded'] = False; rec['error'] = f'{type(e).__name__}: {e}'[:600]
    finally:
        if cid:
            d0 = time.monotonic()
            try:
                s, v, _ = call('DELETE', '/v1/computers/' + cid, who=who)
                wait_state(cid, 'deleted', 600, who=who, poll=0.5)
                rec['cleanup_ms'] = round((time.monotonic() - d0) * 1000); rec['deleted'] = True
            except Exception as e:
                rec['deleted'] = False; rec['cleanup_error'] = f'{type(e).__name__}: {e}'[:400]
    return rec

def wake_trial(cid, kind_cache, conc, label, index, who, canary):
    rec = {'kind': 'wake', 'cache': kind_cache, 'concurrency': conc, 'label': label, 'index': index, 'computer': cid, 'owner': who}
    try:
        wait_state(cid, 'stopped', 900, who=who, poll=0.5)
        t0 = time.monotonic()
        s, v, ms = call('POST', f'/v1/computers/{cid}/wake', {}, who=who)
        rec['wake_status'] = s; rec['create_ms'] = round((time.monotonic() - t0) * 1000)
        if s not in (200, 202): raise RuntimeError(f'wake {s}: {json.dumps(v)[:300]}')
        wait_state(cid, 'awake', 600, who=who, poll=0.1)
        rec['readiness_ms'] = round((time.monotonic() - t0) * 1000)
        nonce = secrets.token_hex(16)
        extra = "; p=$(cat /home/user/q16.pid); echo $p; awk '{print $22}' /proc/$p/stat; tr '\\0' '\\n' </proc/$p/environ | sed -n 's/^Q16_CANARY=//p' | tr -d '\\n' | sha256sum | cut -d' ' -f1; sha256sum /home/user/q16-workspace.bin | cut -d' ' -f1"
        ok, tid, r, lines = useful(cid, who, nonce, extra)
        rec['first_useful_ms'] = round((time.monotonic() - t0) * 1000); rec['turn'] = tid
        got = {'boot_id': lines[3] if len(lines) > 3 else None, 'pid': lines[4] if len(lines) > 4 else None,
               'start_ticks': lines[5] if len(lines) > 5 else None, 'canary_sha256': lines[6] if len(lines) > 6 else None,
               'workspace_sha256': lines[7] if len(lines) > 7 else None}
        rec['observed'] = got
        same = all(got.get(k) == canary.get(k) for k in ('boot_id', 'pid', 'start_ticks', 'canary_sha256', 'workspace_sha256'))
        rec['memory_preserved'] = same
        rec['succeeded'] = bool(ok and same)
        if not rec['succeeded']: rec['error'] = 'useful output or memory check failed: ' + json.dumps(brief(r))[:300]
    except Exception as e:
        rec['succeeded'] = False; rec['error'] = f'{type(e).__name__}: {e}'[:600]
    return rec

def seed_memory(cid, who):
    """Start a long-lived canary process and a workspace file; return their identities."""
    canary = secrets.token_hex(32)
    cmd = ("set -eu; head -c 1048576 /dev/urandom > /home/user/q16-workspace.bin; "
           f"env Q16_CANARY={canary} setsid sh -c 'while :; do sleep 3600; done' </dev/null >/dev/null 2>&1 & p=$!; echo $p > /home/user/q16.pid; "
           "cat /proc/sys/kernel/random/boot_id; echo $p; awk '{print $22}' /proc/$p/stat; "
           "tr '\\0' '\\n' </proc/$p/environ | sed -n 's/^Q16_CANARY=//p' | tr -d '\\n' | sha256sum | cut -d' ' -f1; sha256sum /home/user/q16-workspace.bin | cut -d' ' -f1")
    tid, r = turn(cid, cmd, who=who)
    lines = (r.get('stdout') or '').split('\n')
    if r.get('exit_code') != 0 or len(lines) < 5 or lines[3] != hashlib.sha256(canary.encode()).hexdigest():
        raise RuntimeError('seed failed: ' + json.dumps(brief(r))[:400])
    return {'boot_id': lines[0], 'pid': lines[1], 'start_ticks': lines[2], 'canary_sha256': lines[3], 'workspace_sha256': lines[4], 'turn': tid}

def run_parallel(fns):
    out = [None] * len(fns); barrier = threading.Barrier(len(fns))
    def go(i):
        barrier.wait(); out[i] = fns[i]()
    th = [threading.Thread(target=go, args=(i,)) for i in range(len(fns))]
    [t.start() for t in th]; [t.join() for t in th]
    return out

def record(label, rec):
    rec['t'] = round(time.time(), 3)
    with open(ROOT / f'trials-{label}.jsonl', 'a') as f: f.write(json.dumps(rec, sort_keys=True) + '\n')
    print(json.dumps({k: rec.get(k) for k in ('kind','cache','concurrency','index','succeeded','create_ms','readiness_ms','first_useful_ms','cleanup_ms','error')}), flush=True)


def send_and_drop(method, path, body, who='a'):
    """Send a complete request and close before reading any reply byte."""
    import ssl, socket
    data = json.dumps(body).encode() if body is not None else b''
    h = (f'{method} {path} HTTP/1.1\r\nHost: {HOST}\r\nAuthorization: Bearer ' + fixture()['computers_token'] +
         f'\r\nX-OpenAgents-Account: {owner(who)}\r\nX-OpenAgents-Test: 1\r\nContent-Type: application/json\r\nContent-Length: {len(data)}\r\nConnection: close\r\n\r\n').encode()
    ctx = ssl.create_default_context()
    with socket.create_connection((HOST, 443), timeout=10) as raw:
        with ctx.wrap_socket(raw, server_hostname=HOST) as tls:
            tls.sendall(h + data)
            time.sleep(0.05)
    return {'sent_bytes': len(h) + len(data), 'reply_read': False}

def native_sandboxes_for(cid):
    """Native sandboxes whose metadata names this computer (admin view, all states)."""
    s, v = native('GET', '/admin/api/v1/sandboxes?limit=200', admin=True)
    items = v if isinstance(v, list) else (v.get('items') or v.get('sandboxes') or v.get('data') or [])
    return s, [{'id': i.get('id'), 'lifecycle': i.get('lifecycle'), 'status': i.get('status'), 'node': i.get('active_node_id'), 'created_at': i.get('created_at')}
               for i in items if (i.get('metadata') or {}).get('computer') == cid]


def isolation(victim, victim_turn, attacker='b'):
    """The attacker owner tries every route on the victim's computer and turn."""
    checks = []
    for method, path, body in [('GET', f'/v1/computers/{victim}', None), ('GET', '/v1/computers', None),
                               ('GET', f'/v1/computers/{victim}/turns/{victim_turn}', None),
                               ('POST', f'/v1/computers/{victim}/turns', {'command': 'id', 'timeout_seconds': 5}),
                               ('POST', f'/v1/computers/{victim}/turns/{victim_turn}/stop', {}),
                               ('POST', f'/v1/computers/{victim}/commands', {'command': 'id', 'idempotency_key': 'q16-iso'}),
                               ('GET', f'/v1/computers/{victim}/files?path=q16-workspace.bin', None),
                               ('POST', f'/v1/computers/{victim}/stop', {}), ('POST', f'/v1/computers/{victim}/wake', {}),
                               ('DELETE', f'/v1/computers/{victim}', None), ('GET', '/v1/usage', None)]:
        st, v, _ = call(method, path, body, who=attacker)
        text = json.dumps(v)
        leaked = victim in text or victim_turn in text
        if path == '/v1/computers': ok = st == 200 and not leaked
        elif path == '/v1/usage': ok = st == 200 and not leaked
        else: ok = st in (403, 404) and not leaked
        checks.append({'method': method, 'path': path.replace(victim, '{victim}').replace(victim_turn, '{turn}'), 'status': st, 'victim_identifiers_in_response': leaked, 'refused_or_empty': ok,
                       'error': (v or {}).get('error') if isinstance(v, dict) else None})
    st, v, _ = call('GET', f'/v1/computers/{victim}', who='a')
    return {'checks': checks, 'all_refused': all(c['refused_or_empty'] for c in checks), 'victim_still_owned_by_a': st == 200 and v.get('desired') != 'deleted'}

SUMMARY = ('id','state','desired','size','image','placement','deployment','last_error','resource','boots','error','turns','exit_code','attempts','shape')
def brief(v):
    if not isinstance(v, dict): return v
    return {k: v[k] for k in SUMMARY if k in v}

def wait_state(cid, want, seconds, who='a', poll=0.25):
    deadline = time.monotonic() + seconds
    while True:
        s, v, _ = call('GET', '/v1/computers/' + cid, who=who)
        state = (v or {}).get('state')
        if s == 200 and state == want: return v
        if s == 200 and state in ('failed',) and want != 'failed': raise RuntimeError(f'{cid} failed: {json.dumps(brief(v))[:600]}')
        if s == 404 and want == 'deleted': return {'state': 'deleted', 'http_status': 404}
        if s == 200 and want != 'deleted' and "couldn't confirm" in str((v or {}).get('last_error') or ''):
            raise RuntimeError(f'{cid} is stranded by an uncertain turn (#12141)')
        if time.monotonic() > deadline: raise TimeoutError(f'{cid} did not reach {want}; last {s} {json.dumps(brief(v))[:400]}')
        time.sleep(poll)

def turn(cid, command, who='a', seconds=90, timeout=45, poll=0.2):
    s, v, ms = call('POST', f'/v1/computers/{cid}/turns', {'command': command, 'timeout_seconds': timeout}, who=who)
    if s not in (200, 201, 202) or not isinstance(v, dict) or not v.get('id'):
        raise RuntimeError(f'turn submit {s}: {json.dumps(v)[:500]}')
    tid = v['id']; deadline = time.monotonic() + seconds
    while True:
        s, r, _ = call('GET', f'/v1/computers/{cid}/turns/{tid}', who=who)
        if s == 200 and r.get('exit_code') is not None: return tid, r
        if s == 200 and r.get('state') in ('failed', 'cancelled', 'stopped', 'error'): return tid, r
        if time.monotonic() > deadline: raise TimeoutError(f'turn {tid} unresolved: {json.dumps(brief(r))[:400]}')
        time.sleep(poll)

def main():
    ap = argparse.ArgumentParser(); sub = ap.add_subparsers(dest='cmd', required=True)
    c = sub.add_parser('call'); c.add_argument('method'); c.add_argument('path'); c.add_argument('body', nargs='?'); c.add_argument('--who', default='a')
    n = sub.add_parser('native'); n.add_argument('method'); n.add_argument('path'); n.add_argument('body', nargs='?'); n.add_argument('--admin', action='store_true')
    sub.add_parser('owners')
    iso = sub.add_parser('isolation'); iso.add_argument('victim'); iso.add_argument('turn')
    dr = sub.add_parser('drop'); dr.add_argument('method'); dr.add_argument('path'); dr.add_argument('body', nargs='?'); dr.add_argument('--who', default='a')
    ns = sub.add_parser('natives'); ns.add_argument('cid')
    tr = sub.add_parser('trials'); tr.add_argument('kind', choices=['cold','snapshot','wake']); tr.add_argument('label'); tr.add_argument('--count', type=int, default=1)
    tr.add_argument('--concurrency', type=int, default=1); tr.add_argument('--cache', default='warm'); tr.add_argument('--size', default='small')
    tr.add_argument('--computers', help='comma-separated computers for wake trials'); tr.add_argument('--start', type=int, default=0); tr.add_argument('--owners', default='a,b')
    sd = sub.add_parser('seed'); sd.add_argument('cid'); sd.add_argument('--who', default='a')
    q = sub.add_parser('sql'); q.add_argument('query'); q.add_argument('--db', default='space_computers')
    t = sub.add_parser('turn'); t.add_argument('cid'); t.add_argument('command'); t.add_argument('--who', default='a')
    w = sub.add_parser('wait'); w.add_argument('cid'); w.add_argument('state'); w.add_argument('--seconds', type=float, default=600); w.add_argument('--who', default='a')
    args = ap.parse_args()
    if os.getuid() != 0: raise SystemExit('Run as root on the control node.')
    ROOT.mkdir(mode=0o700, exist_ok=True)
    if args.cmd == 'seed':
        v = seed_memory(args.cid, args.who); (ROOT / f'canary-{args.cid}.json').write_text(json.dumps(v)); print(json.dumps(v)); return
    if args.cmd == 'trials':
        whos = args.owners.split(',')
        for i in range(args.start, args.start + args.count):
            if args.kind == 'wake':
                cids = args.computers.split(',')[:args.concurrency]
                fns = [(lambda c=c, w=whos[j % 2]: wake_trial(c, args.cache, args.concurrency, args.label, i, w, json.loads((ROOT / f'canary-{c}.json').read_text()))) for j, c in enumerate(cids)]
            else:
                fns = [(lambda j=j: create_trial(args.kind, args.cache, args.concurrency, args.label, i * 10 + j, whos[j % 2], args.size)) for j in range(args.concurrency)]
            for rec in run_parallel(fns): record(args.label, rec)
        return
    if args.cmd == 'drop':
        v = send_and_drop(args.method, args.path, json.loads(args.body) if args.body else None, args.who); log({'drop': args.method, 'path': args.path, **v}); print(json.dumps(v)); return
    if args.cmd == 'natives':
        s, v = native_sandboxes_for(args.cid); print(json.dumps({'status': s, 'sandboxes': v})); return
    if args.cmd == 'isolation':
        v = isolation(args.victim, args.turn); (ROOT / 'isolation.json').write_text(json.dumps(v, indent=1)); print(json.dumps(v)); return
    if args.cmd == 'sql':
        print(sql(args.query, args.db)); return
    if args.cmd == 'owners':
        print(json.dumps({k: owner(k) for k in SYNTHETIC})); return
    if args.cmd == 'call':
        s, v, ms = call(args.method, args.path, json.loads(args.body) if args.body else None, who=None if args.who == 'none' else args.who)
        log({'call': args.method, 'path': args.path, 'status': s, 'ms': ms, 'value': brief(v)})
        print(json.dumps({'status': s, 'ms': ms, 'value': v}))
    elif args.cmd == 'native':
        s, v = native(args.method, args.path, json.loads(args.body) if args.body else None, admin=args.admin)
        print(json.dumps({'status': s, 'value': v}))
    elif args.cmd == 'turn':
        tid, r = turn(args.cid, args.command, who=args.who)
        log({'turn': tid, 'computer': args.cid, 'value': r})
        print(json.dumps(r))
    elif args.cmd == 'wait':
        v = wait_state(args.cid, args.state, args.seconds, who=args.who)
        print(json.dumps(brief(v)))

if __name__ == '__main__':
    main()
