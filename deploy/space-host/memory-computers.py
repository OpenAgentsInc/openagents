#!/usr/bin/env python3
"""Check process memory through the Computers API on the isolated #11812 host."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import secrets
import shlex
import socket
import subprocess
import time
import urllib.error
import urllib.request

HTTP = urllib.request.build_opener(urllib.request.ProxyHandler({}))
OWNER = 'space-memory-11812'
MAX_BODY = 4 * 1024 * 1024


def call(base, token, path, method='GET', body=None, account=True):
    headers = {'Authorization': 'Bearer ' + token, 'Content-Type': 'application/json'}
    if account:
        headers['X-OpenAgents-Account'] = OWNER
    request = urllib.request.Request(base + path, method=method, headers=headers,
        data=None if body is None else json.dumps(body).encode())
    try:
        response = HTTP.open(request, timeout=90)
    except urllib.error.HTTPError as error:
        response = error
    with response:
        raw = response.read(MAX_BODY + 1)
        assert len(raw) <= MAX_BODY
        return response.status, json.loads(raw) if raw else {}


def save(path, value):
    with path.open('w') as out:
        json.dump(value, out, indent=2)
        out.write('\n')


def wait(base, token, path, predicate, timeout=240):
    deadline = time.monotonic() + timeout
    while True:
        status, value = call(base, token, path)
        assert status == 200, f'Could not read {path}: {status}'
        if predicate(value):
            return value
        assert time.monotonic() < deadline, f'Timed out at {path}: {value.get("state")}'
        time.sleep(.05)


def turn(base, token, path, command):
    status, value = call(base, token, path + '/turns', 'POST',
                         {'command': command, 'timeout_seconds': 180})
    assert status == 202, f'Turn refused: {status}'
    result = wait(base, token, path + '/turns/' + value['id'],
                  lambda row: row.get('state') in ('done', 'failed', 'stopped'))
    assert result['state'] == 'done' and result['exit_code'] == 0, result
    return result


def proof(result):
    # Read the live process's answer, never a hash saved in the guest filesystem.
    lines = result.get('stdout', '').strip().splitlines()
    value = json.loads(lines[-1])
    guest_unix_ms = value.pop('guest_unix_ms')
    skew_ms = time.time() * 1000 - guest_unix_ms
    assert abs(skew_ms) < 10000, f'The restored guest clock differs by {skew_ms}ms'
    assert value['bytes'] == 1024**3 and len(value['sha256']) == 64
    assert value['pid'] > 1 and value['start_ticks'] > 0
    return value, skew_ms


def main():
    assert socket.gethostname().split('.')[0] == 'oa-space-spike-11812'
    assert Path.home() == Path('/var/lib/space-spike/home')
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--observer', type=Path, required=True)
    parser.add_argument('--evidence', type=Path, required=True)
    parser.add_argument('--commit', required=True)
    args = parser.parse_args()
    assert args.binary.is_file() and not args.evidence.exists()
    assert len(args.commit) == 40 and all(c in '0123456789abcdef' for c in args.commit)
    assert os.environ['SPACE_API_URL'] == 'http://127.0.0.1:8080'
    assert os.environ['SPACE_DATA_PLANE_URL'] == 'http://127.0.0.1:8090'
    assert '@sha256:' in os.environ['SPACE_TEMPLATE']
    report = {'schema': 'openagents.space.memory_computers.v1', 'commit': args.commit,
              'binary_sha256': hashlib.sha256(args.binary.read_bytes()).hexdigest(),
              'upstream_commit': 'de87b436c35dac056564bdd5b2a113c9046e189b',
              'buffer_bytes': 1024**3, 'samples': [], 'cleanup_errors': [],
              'conditions': 'Isolated loopback Computers API, one guest at a time; no model or customer data'}
    token = secrets.token_urlsafe(32)
    container = process = log = None
    base = None
    try:
        container = subprocess.check_output(['docker', 'run', '--detach', '--rm',
            '--label', 'openagents-issue=11812', '--memory=2g', '-p', '127.0.0.1::5432',
            '-e', 'POSTGRES_HOST_AUTH_METHOD=trust',
            'postgres@sha256:b0f9560a2de083e2cc7382e75f808c7381a32852a7ec49117deedb300e552b24'], text=True).strip()
        for _ in range(60):
            if subprocess.run(['docker', 'exec', container, 'pg_isready', '-U', 'postgres'],
                stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL).returncode == 0:
                break
            time.sleep(1)
        else:
            raise RuntimeError('The isolated database did not become ready')
        inspected = json.loads(subprocess.check_output(['docker', 'inspect', container]))[0]
        dbport = inspected['NetworkSettings']['Ports']['5432/tcp'][0]['HostPort']
        with socket.socket() as selected:
            selected.bind(('127.0.0.1', 0))
            port = selected.getsockname()[1]
        registry = args.evidence.with_suffix('.registry.json')
        save(registry, {'default': 'space-memory-11812', 'policy': 'memory-qualification-v1',
            'deployments': [{'id': 'space-memory-11812', 'kind': 'space',
                'api_url': os.environ['SPACE_API_URL'],
                'data_plane_url': os.environ['SPACE_DATA_PLANE_URL'],
                'credential_env': 'SPACE_API_TOKEN', 'template': os.environ['SPACE_TEMPLATE'],
                'capacity': 1}]})
        env = dict(os.environ, OA_COMPUTERS_REGISTRY_FILE=str(registry), OA_COMPUTERS_TOKEN=token,
            OA_COMPUTERS_DATABASE_URL=f'host=127.0.0.1 port={dbport} user=postgres',
            OA_COMPUTERS_LISTEN=f'127.0.0.1:{port}', OA_COMPUTERS_POLL_MS='50',
            OA_COMPUTERS_DRIVER_POLL_MS='50')
        base = f'http://127.0.0.1:{port}'
        log = args.evidence.with_suffix('.service.log').open('wb')

        def start_service():
            service = subprocess.Popen([str(args.binary.resolve())], env=env, stdout=log, stderr=log)
            for _ in range(150):
                assert service.poll() is None, 'The isolated Computers service exited'
                try:
                    if call(base, token, '/v1/capabilities')[0] == 200:
                        return service
                except OSError:
                    pass
                time.sleep(.1)
            raise RuntimeError('The isolated Computers service did not become ready')

        process = start_service()
        report['capabilities'] = call(base, token, '/v1/capabilities')[1]
        save(args.evidence, report)
        for size, memory_mib in [('medium', 4096), ('large', 8192)]:
            row = {'size': size, 'memory_mib': memory_mib, 'cycles': [], 'passed': False}
            report['samples'].append(row)
            key = 'memory-11812-' + secrets.token_hex(16)
            status, value = call(base, token, '/v1/computers', 'POST',
                {'size': size, 'idempotency_key': key, 'needs': {'memory_suspend': True}})
            assert status == 202, f'Computer creation refused: {status}'
            row['computer'] = value['id']
            path = '/v1/computers/' + value['id']
            try:
                awake = wait(base, token, path, lambda item: item.get('state') == 'awake')
                row['resource'] = awake['resource']
                assert awake['binding']['shape']['memory_mib'] == memory_mib, awake['binding']
                observed = subprocess.run([str(args.observer.resolve()), row['resource'],
                    '--machine-config'], capture_output=True, text=True, timeout=30, check=True)
                row['boot_evidence'] = json.loads(observed.stdout)
                assert row['boot_evidence']['actual_shape']['memory_mib'] == memory_mib
                sock = '/tmp/oa-memory-11812-' + secrets.token_hex(8) + '.sock'
                daemon = f'''import hashlib,json,os,socket,time
data=os.urandom(1024**3)
s=socket.socket(socket.AF_UNIX)
s.bind({sock!r})
os.chmod({sock!r},0o600)
s.listen(2)
while True:
 c,_=s.accept()
 c.recv(16)
 value={{'pid':os.getpid(),'start_ticks':int(open('/proc/self/stat').read().split()[21]),'bytes':len(data),'sha256':hashlib.sha256(data).hexdigest(),'boot_id':open('/proc/sys/kernel/random/boot_id').read().strip(),'guest_unix_ms':time.time()*1000}}
 c.sendall(json.dumps(value).encode())
 c.close()
'''
                query = f'''import socket
s=socket.socket(socket.AF_UNIX)
s.connect({sock!r})
s.sendall(b'check')
print(s.recv(4096).decode())
'''
                ask = 'python3 -c ' + shlex.quote(query)
                start = ('set -eu; command -v python3 >/dev/null || '
                    '(sudo apt-get update >/dev/null && sudo DEBIAN_FRONTEND=noninteractive apt-get install -y python3 >/dev/null); '
                    'nohup python3 -c ' + shlex.quote(daemon)
                    + ' >/tmp/oa-memory-11812.log 2>&1 </dev/null & '
                    + 'for i in $(seq 1 120); do test -S ' + shlex.quote(sock)
                    + ' && break; sleep 1; done; ' + ask)
                row['before'], row['before_clock_skew_ms'] = proof(turn(base, token, path, start))
                wait(base, token, path, lambda item: item.get('state') == 'stopped')
                row['stopped_after_turn'] = True
                for cycle in range(2):
                    # Rebuild the API/worker process against its same durable store.
                    # Reconciliation must leave the stopped policy effective.
                    process.kill()
                    process.wait(timeout=15)
                    process = start_service()
                    time.sleep(15)
                    stopped = call(base, token, path)[1]
                    assert stopped['state'] == 'stopped' and stopped['resource'] == row['resource']
                    started = time.perf_counter()
                    assert call(base, token, path + '/wake', 'POST', {})[0] == 202
                    awake = wait(base, token, path, lambda item: item.get('state') == 'awake')
                    ready_ms = (time.perf_counter() - started) * 1000
                    after, clock_skew_ms = proof(turn(base, token, path, ask))
                    assert awake['resource'] == row['resource'] and after == row['before']
                    row['cycles'].append({'cycle': cycle, 'wake_ready_ms': ready_ms,
                        'wake_to_answer_ms': (time.perf_counter() - started) * 1000,
                        'same_computer_process_memory': True, 'after': after,
                        'guest_clock_skew_ms': clock_skew_ms,
                        'api_process_restarted': True, 'stopped_policy_survived_restart': True})
                    wait(base, token, path, lambda item: item.get('state') == 'stopped')
                    save(args.evidence, report)
                row['passed'] = True
            finally:
                assert call(base, token, path, 'DELETE')[0] == 202
                wait(base, token, path, lambda item: item.get('state') == 'deleted')
                row['deleted'] = True
                provider_status, _ = call(os.environ['SPACE_API_URL'], os.environ['SPACE_API_TOKEN'],
                                          '/v1/sandboxes/' + row['resource'], account=False)
                assert provider_status == 404, 'The private checkpoint still exists'
                row['provider_absent_after_delete'] = True
                save(args.evidence, report)
        report['passed'] = all(row['passed'] and row.get('deleted') for row in report['samples'])
    finally:
        if process is not None:
            process.terminate()
            try:
                process.wait(timeout=15)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait(timeout=15)
        if log is not None:
            log.close()
        if container is not None:
            subprocess.run(['docker', 'rm', '--force', container], check=True,
                           stdout=subprocess.DEVNULL)
        save(args.evidence, report)
    assert report.get('passed'), 'Memory qualification did not complete'
    print(json.dumps({'passed': True, 'memory_sizes_mib': [4096, 8192], 'bytes': 1024**3}))


if __name__ == '__main__':
    main()
