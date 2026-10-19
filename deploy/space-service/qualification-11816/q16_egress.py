import json, shlex, sys, importlib.util
spec = importlib.util.spec_from_file_location('q16', '/space/config/deployment-tools/q16.py'); q = importlib.util.module_from_spec(spec); spec.loader.exec_module(q)
cid, who = sys.argv[1], sys.argv[2]
src = open('/space/qualification-11816/egress_probe.py').read()
out = []
for group in ('1', '2'):
    st, v, _ = q.call('GET', f'/v1/computers/{cid}', who=who)
    if v.get('state') == 'stopped':
        q.call('POST', f'/v1/computers/{cid}/wake', {}, who=who); q.wait_state(cid, 'awake', 300, who=who)
    elif v.get('state') != 'awake':
        q.wait_state(cid, 'stopped', 300, who=who); q.call('POST', f'/v1/computers/{cid}/wake', {}, who=who); q.wait_state(cid, 'awake', 300, who=who)
    tid, r = q.turn(cid, 'RES_OPTIONS="timeout:1 attempts:1" python3 -c ' + shlex.quote(src) + ' ' + group, who=who, seconds=120, timeout=58)
    out.append({'group': group, 'turn': tid, 'exit_code': r.get('exit_code'), 'state': r.get('state'), 'probes': json.loads(r['stdout']) if r.get('stdout', '').strip().startswith('[') else None, 'stderr_bytes': len(r.get('stderr') or '')})
open('/space/qualification-11816/egress.json', 'w').write(json.dumps(out, indent=1)); print(json.dumps(out))
