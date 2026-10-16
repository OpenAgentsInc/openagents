#!/usr/bin/env python3
"""Measure the real Computers reconciler on a disposable, local SPACE host.

The management proxy records timing and IDs only. The data proxy records the
WebSocket handshake and process start/exit events; it never saves tokens or
command output. Every sample includes host boot evidence, including failures.
"""

import argparse
import hashlib
import http.server
import json
import os
from pathlib import Path
import secrets
import selectors
import socket
import socketserver
import subprocess
import threading
import time
import urllib.error
import urllib.parse
import urllib.request

LIMIT = 4 * 1024 * 1024
HTTP = urllib.request.build_opener(urllib.request.ProxyHandler({}),
                                 type('NoRedirect', (urllib.request.HTTPRedirectHandler,),
                                      {'redirect_request': lambda *args, **kwargs: None})())
EVENTS = []
LOCK = threading.Lock()


def event(kind, ident=None):
    with LOCK:
        assert len(EVENTS) < 50000, 'Timing event limit reached'
        EVENTS.append({'kind': kind, 'id': ident, 'at': time.perf_counter()})


def origin(value):
    parsed = urllib.parse.urlsplit(value)
    assert parsed.scheme == 'http' and parsed.hostname in ('127.0.0.1', 'localhost')
    assert parsed.port and parsed.path in ('', '/') and not parsed.query and not parsed.fragment
    assert not parsed.username and not parsed.password
    return value.rstrip('/')


def request(base, path, token, method='GET', body=None, account=False, key=None):
    headers = {'Authorization': 'Bearer ' + token, 'Content-Type': 'application/json'}
    if account:
        headers['X-OpenAgents-Account'] = 'space-benchmark-11810'
    if key is not None:
        headers['Idempotency-Key'] = key
    data = None if body is None else json.dumps(body).encode()
    if method == 'POST' and data is None:
        data = b''
    req = urllib.request.Request(base + path, method=method, data=data, headers=headers)
    try:
        response = HTTP.open(req, timeout=90)
    except urllib.error.HTTPError as error:
        response = error
    with response:
        raw = response.read(LIMIT + 1)
        assert len(raw) <= LIMIT
        return response.status, json.loads(raw) if raw else {}


class Management(http.server.BaseHTTPRequestHandler):
    protocol_version = 'HTTP/1.1'

    def log_message(self, *args):
        pass

    def do_GET(self):
        self.forward()

    def do_POST(self):
        self.forward()

    def do_DELETE(self):
        self.forward()

    def forward(self):
        length = int(self.headers.get('Content-Length', '0'))
        assert 0 <= length <= LIMIT
        body = self.rfile.read(length) if length else None
        assert not self.path.startswith('//') and self.path.startswith('/v1/')
        ident = self.path.split('/')[3] if self.path.startswith('/v1/sandboxes/') else None
        create = self.command == 'POST' and self.path == '/v1/sandboxes'
        if create:
            event('reconcile_create_dispatch')
        headers = {key: value for key, value in self.headers.items()
                   if key.lower() in ('authorization', 'content-type', 'idempotency-key')}
        req = urllib.request.Request(self.server.upstream + self.path, method=self.command,
                                     data=body, headers=headers)
        try:
            response = HTTP.open(req, timeout=90)
        except urllib.error.HTTPError as error:
            response = error
        except Exception:
            self.send_error(502, 'The local measurement proxy could not complete the request.')
            return
        with response:
            raw = response.read(LIMIT + 1)
            assert len(raw) <= LIMIT
            status, content_type = response.status, response.headers.get('Content-Type', 'application/json')
        try:
            value = json.loads(raw)
        except ValueError:
            value = {}
        if create:
            event('provider_create_http_' + str(status))
            if 200 <= status < 300:
                event('provider_create_accepted', value.get('sandbox_id'))
                with LOCK:
                    self.server.creates[value['sandbox_id']] = (self.headers.get('Idempotency-Key'), body)
                if value.get('operation_id'):
                    event('provider_create_operation', value['operation_id'])
        if self.command == 'GET' and ident and value.get('state') == 'running':
            event('provider_running_observed', ident)
        if self.command == 'POST' and self.path.endswith('/commands') and 200 <= status < 300:
            event('command_session_created', value.get('session_id'))
            # The driver still verifies the configured attachment origin. A
            # trusted absolute upstream URL must name this proxy's path instead.
            expected = '/v1/sessions/' + value['session_id'] + '/attach'
            supplied = value['attach']['url']
            allowed = [expected, self.server.data_upstream + expected,
                       self.server.data_upstream.replace('http://', 'ws://', 1) + expected]
            assert supplied in allowed, 'Unexpected upstream attachment origin'
            value['attach']['url'] = expected
            raw = json.dumps(value).encode()
        self.send_response(status)
        self.send_header('Content-Type', content_type)
        self.send_header('Content-Length', str(len(raw)))
        self.end_headers()
        self.wfile.write(raw)


class Data(socketserver.BaseRequestHandler):
    def handle(self):
        client = self.request
        client.settimeout(30)
        header = bytearray()
        while b'\r\n\r\n' not in header:
            block = client.recv(4096)
            if not block:
                return
            header.extend(block)
            assert len(header) <= 65536
        line = bytes(header).split(b'\r\n', 1)[0].decode('ascii')
        path = line.split(' ')[1]
        assert path.startswith('/v1/sessions/') and path.endswith('/attach')
        ident = path.split('/')[3]
        with socket.create_connection(self.server.upstream, timeout=30) as upstream:
            upstream.sendall(header)
            reply = bytearray()
            while b'\r\n\r\n' not in reply:
                block = upstream.recv(4096)
                if not block:
                    return
                reply.extend(block)
                assert len(reply) <= 65536
            client.sendall(reply)
            if bytes(reply).split(b'\r\n', 1)[0].split(b' ')[1] != b'101':
                return
            event('attach_connected', ident)
            frames = bytearray(bytes(reply).split(b'\r\n\r\n', 1)[1])
            observe_frames(frames, ident)
            with selectors.DefaultSelector() as poll:
                poll.register(client, selectors.EVENT_READ, upstream)
                poll.register(upstream, selectors.EVENT_READ, client)
                while True:
                    selected = poll.select(30)
                    if not selected:
                        return
                    for key, _ in selected:
                        block = key.fileobj.recv(65536)
                        if not block:
                            return
                        key.data.sendall(block)
                        if key.fileobj is upstream:
                            frames.extend(block)
                            observe_frames(frames, ident)


def observe_frames(frames, ident):
    while len(frames) >= 2:
        length, offset = frames[1] & 127, 2
        if length in (126, 127):
            width = 2 if length == 126 else 8
            if len(frames) < 2 + width:
                break
            length, offset = int.from_bytes(frames[2:2+width], 'big'), 2 + width
        assert length <= 2 * 1024 * 1024
        assert frames[1] & 128 == 0, 'Unexpected masked server frame'
        if len(frames) < offset + length:
            break
        payload = bytes(frames[offset:offset+length])
        opcode = frames[0] & 15
        del frames[:offset+length]
        if opcode == 1:
            try:
                value = json.loads(payload)
            except ValueError:
                continue
            if value.get('type') in ('process_started', 'process_exited'):
                event(value['type'], ident)


class DataServer(socketserver.ThreadingTCPServer):
    allow_reuse_address = True
    daemon_threads = True

    def handle_error(self, *args):
        event('data_proxy_failed')


def save(path, value):
    pending = path.with_suffix('.pending')
    pending.write_text(json.dumps(value, indent=2) + '\n')
    pending.replace(path)


def wait(base, path, token, predicate, timeout=120, diagnostic=None):
    until = time.monotonic() + timeout
    while time.monotonic() < until:
        status, value = request(base, path, token, account=True)
        assert status == 200, 'Computer observation failed'
        if predicate(value):
            return value
        if diagnostic is not None:
            diagnostic['last_state'] = value.get('state')
            diagnostic['last_error'] = value.get('last_error')
            if value.get('state') == 'failed':
                raise RuntimeError('The computer reported a failed start')
        time.sleep(.02)
    raise RuntimeError('Computer did not reach the requested state')


def main():
    assert socket.gethostname().split('.')[0] == 'oa-space-spike-11810'
    assert Path.home() == Path('/var/lib/space-spike/home')
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('arm', choices=['cold', 'snapshot'])
    parser.add_argument('evidence', type=Path)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--observer', type=Path, required=True)
    parser.add_argument('--vcpu', type=int, default=2)
    parser.add_argument('--memory-mib', type=int, default=1024)
    parser.add_argument('--size', choices=['small', 'medium', 'large'], default='small')
    parser.add_argument('--expect-refusal', action='store_true')
    parser.add_argument('--scenario', choices=['normal', 'missing-snapshot', 'incompatible-shape', 'corrupt-snapshot'], default='normal')
    parser.add_argument('--count', type=int, default=20)
    args = parser.parse_args()
    assert 1 <= args.count <= 100
    assert args.evidence.parent.is_dir() and not args.evidence.exists(), 'Keep earlier evidence'
    api, data = origin(os.environ['SPACE_API_URL']), origin(os.environ['SPACE_DATA_PLANE_URL'])
    template = os.environ['SPACE_TEMPLATE']
    assert '@sha256:' in template and len(template.rsplit('@sha256:', 1)[1]) == 64
    management = http.server.ThreadingHTTPServer(('127.0.0.1', 0), Management)
    management.creates = {}
    management.upstream = api
    management.data_upstream = data
    dataplane = DataServer(('127.0.0.1', 0), Data)
    target = urllib.parse.urlsplit(data)
    dataplane.upstream = (target.hostname, target.port)
    for server in (management, dataplane):
        threading.Thread(target=server.serve_forever, daemon=True).start()
    report = {'schema': 'openagents.space_computers_benchmark.v1', 'arm': args.arm,
              'upstream_commit': 'de87b436c35dac056564bdd5b2a113c9046e189b',
              'service_binary_sha256': hashlib.sha256(args.binary.read_bytes()).hexdigest(),
              'template': template, 'size': args.size, 'scenario': args.scenario,
              'expected_refusal': args.expect_refusal,
              'requested_shape': {'vcpu': args.vcpu, 'memory_mib': args.memory_mib},
              'count': args.count, 'reconcile_poll_ms': 25, 'driver_poll_ms': 25,
              'client_poll_ms': 20, 'operation_poll_ms': 100, 'reconcile_revisit_ms': 30000,
              'concurrency': 1, 'samples': [], 'cleanup_errors': [],
              'conditions': 'Local loopback, warm template cache, no warm pool; boot observation overhead included'}
    token = secrets.token_urlsafe(32)
    container = process = None
    log = None
    try:
        container = subprocess.check_output(['docker', 'run', '--detach', '--rm',
            '--label', 'openagents-issue=11810', '--memory=2g', '-p', '127.0.0.1::5432',
            '-e', 'POSTGRES_HOST_AUTH_METHOD=trust',
            'postgres@sha256:b0f9560a2de083e2cc7382e75f808c7381a32852a7ec49117deedb300e552b24'], text=True).strip()
        for _ in range(60):
            if subprocess.run(['docker', 'exec', container, 'pg_isready', '-U', 'postgres'],
                              stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL).returncode == 0:
                break
            time.sleep(1)
        else:
            raise RuntimeError('The disposable database did not become ready')
        inspected = json.loads(subprocess.check_output(['docker', 'inspect', container]))[0]
        dbport = inspected['NetworkSettings']['Ports']['5432/tcp'][0]['HostPort']
        with socket.socket() as selected:
            selected.bind(('127.0.0.1', 0))
            port = selected.getsockname()[1]
        registry = args.evidence.with_suffix('.registry.json')
        save(registry, {'default': 'space-direct-11810', 'policy': 'snapshot-spike-v1', 'deployments': [{
            'id': 'space-direct-11810', 'kind': 'space',
            'api_url': 'http://127.0.0.1:' + str(management.server_port),
            'data_plane_url': 'http://127.0.0.1:' + str(dataplane.server_address[1]),
            'credential_env': 'SPACE_API_TOKEN', 'template': template,
            'shapes': ({'small': {'vcpus': args.vcpu, 'memory_mib': args.memory_mib}}
                       if args.size == 'small' else {}), 'capacity': 1}]})
        env = dict(os.environ, OA_COMPUTERS_REGISTRY_FILE=str(registry), OA_COMPUTERS_TOKEN=token,
                   OA_COMPUTERS_DATABASE_URL=f'host=127.0.0.1 port={dbport} user=postgres',
                   OA_COMPUTERS_LISTEN=f'127.0.0.1:{port}', OA_COMPUTERS_POLL_MS='25',
                   OA_COMPUTERS_DRIVER_POLL_MS='25')
        base = f'http://127.0.0.1:{port}'
        log = args.evidence.with_suffix('.service.log').open('wb')
        process = subprocess.Popen([str(args.binary.resolve())], env=env, stdout=log, stderr=log)
        for _ in range(150):
            assert process.poll() is None, 'The isolated Computers service exited'
            try:
                if request(base, '/v1/capabilities', token, account=True)[0] == 200:
                    break
            except OSError:
                pass
            time.sleep(.1)
        else:
            raise RuntimeError('The isolated Computers service did not become ready')
        for index in range(args.count):
            started = time.perf_counter()
            row = {'index': index, 'idempotency_key': 'space-11810-' + secrets.token_hex(16),
                   'passed': False, 'computer': None, 'error': None}
            report['samples'].append(row)
            body = {'size': args.size, 'idempotency_key': row['idempotency_key']}
            save(args.evidence, report)
            try:
                status, value = request(base, '/v1/computers', token, 'POST', body, True)
                assert status in (200, 202), 'Computer create was refused'
                row['computer'] = value['id']
                row['api_accept_ms'] = (time.perf_counter() - started) * 1000
                save(args.evidence, report)
                path = '/v1/computers/' + row['computer']
                if args.expect_refusal:
                    assert args.scenario != 'normal'
                    deadline = time.monotonic() + 60
                    while True:
                        with LOCK:
                            ids = {v['id'] for v in EVENTS if v['at'] >= started and v['kind'] == 'provider_create_accepted'}
                            ops = {v['id'] for v in EVENTS if v['at'] >= started and v['kind'] == 'provider_create_operation'}
                        assert len(ids) <= 1 and len(ops) <= 1, 'Create retry allocated another sandbox'
                        if ids and ops:
                            ident, operation_id = next(iter(ids)), next(iter(ops))
                            status, operation = request(api, '/v1/operations/' + operation_id, os.environ['SPACE_API_TOKEN'])
                            assert status == 200
                            status, observed = request(api, '/v1/sandboxes/' + ident, os.environ['SPACE_API_TOKEN'])
                            terminal = operation.get('state') in ('failed', 'canceled', 'expired')
                            refused = observed.get('state') == 'failed' or (status == 404 and terminal)
                            if refused:
                                row['provider_id'], row['provider_create_operation'] = ident, operation_id
                                row['refusal'] = {'operation_state': operation.get('state'),
                                                  'sandbox_status': status, 'sandbox_state': observed.get('state')}
                                break
                        assert time.monotonic() < deadline, 'The corrupt snapshot was not refused'
                        time.sleep(.02)
                    status, retry = request(base, '/v1/computers', token, 'POST', body, True)
                    assert status == 200 and retry['id'] == row['computer'], 'Retry changed the computer'
                    with LOCK:
                        create_key, create_body = management.creates[row['provider_id']]
                    assert create_key
                    status, retry = request(api, '/v1/sandboxes', os.environ['SPACE_API_TOKEN'],
                                            'POST', json.loads(create_body), key=create_key)
                    assert status in (200, 202) and retry['sandbox_id'] == row['provider_id'], 'Provider retry changed identity'
                    row['retry_same_computer'] = row['retry_same_provider'] = True
                    row['passed'] = True
                else:
                    awake = wait(base, path, token, lambda value: value.get('state') == 'awake', diagnostic=row)
                    row['awake_ms'] = (time.perf_counter() - started) * 1000
                    row['provider_id'] = awake['resource']
                    with LOCK:
                        operation_ids = [value['id'] for value in EVENTS
                                         if value['at'] >= started and value['kind'] == 'provider_create_operation']
                    assert operation_ids, 'The provider did not report its create operation'
                    row['provider_create_operation'] = operation_ids[0]
                    deadline = time.monotonic() + 30
                    while True:
                        status, operation = request(api, '/v1/operations/' + operation_ids[0],
                                                    os.environ['SPACE_API_TOKEN'])
                        assert status == 200, 'The accepted create operation could not be observed'
                        if operation.get('state') == 'succeeded':
                            row['operation_succeeded_ms'] = (time.perf_counter() - started) * 1000
                            break
                        assert operation.get('state') in ('queued', 'running'), 'The provider create operation failed'
                        assert time.monotonic() < deadline, 'The provider create operation did not finish'
                        time.sleep(.02)
                    proof = subprocess.run([str(args.observer.resolve()), row['provider_id'], '--machine-config'],
                                           capture_output=True, timeout=10)
                    if proof.returncode:
                        row['boot_observer_stderr'] = proof.stderr.decode(errors='replace')[:2048]
                    assert proof.returncode == 0, 'Host boot observation failed'
                    row['boot_evidence'] = json.loads(proof.stdout)
                    assert row['boot_evidence']['boot_mode'] == args.arm, 'Actual boot mode differs'
                    assert row['boot_evidence']['actual_shape'] == report['requested_shape'], 'Actual shape differs'
                    status, retry = request(base, '/v1/computers', token, 'POST', body, True)
                    assert status == 200 and retry['id'] == row['computer'], 'Retry changed the computer'
                    row['retry_same_computer'] = True
                    status, turn = request(base, path + '/turns', token, 'POST', {'command': 'true'}, True)
                    assert status == 202, 'Command was refused'
                    row['turn_id'] = turn['id']
                    save(args.evidence, report)
                    result = wait(base, path + '/turns/' + turn['id'], token,
                                  lambda value: value.get('state') in ('done', 'failed'))
                    row['first_command_ms'] = (time.perf_counter() - started) * 1000
                    assert result['state'] == 'done' and result['exit_code'] == 0, 'Command failed'
                    row['passed'] = True
            except Exception as error:
                row['error'] = type(error).__name__
                if isinstance(error, AssertionError):
                    row['assertion'] = str(error)
                save(args.evidence, report)
            finally:
                if row['computer'] is None:
                    try:
                        status, value = request(base, '/v1/computers', token, 'POST', body, True)
                        if status in (200, 202):
                            row['computer'] = value['id']
                    except Exception:
                        pass
                if row['computer']:
                    try:
                        path = '/v1/computers/' + row['computer']
                        assert request(base, path, token, 'DELETE', account=True)[0] == 202
                        if args.expect_refusal:
                            # The provider failure remains unknown to the lifecycle.
                            # Confirm provider cleanup before dropping this run's
                            # isolated database; never rewrite a production record.
                            ident = row['provider_id']
                            status, _ = request(api, '/v1/sandboxes/' + ident, os.environ['SPACE_API_TOKEN'])
                            if status != 404:
                                status, _ = request(api, '/v1/sandboxes/' + ident, os.environ['SPACE_API_TOKEN'],
                                                    'DELETE', key='cleanup-11810-' + secrets.token_hex(16))
                                assert status in (200, 202, 204, 404)
                            deadline = time.monotonic() + 120
                            while True:
                                status, _ = request(api, '/v1/sandboxes/' + ident, os.environ['SPACE_API_TOKEN'])
                                if status == 404:
                                    break
                                assert time.monotonic() < deadline, 'Provider cleanup did not finish'
                                time.sleep(.1)
                            row['provider_deleted'] = True
                            row['computer_fixture_cleanup'] = 'The isolated database is removed after provider deletion'
                        else:
                            wait(base, path, token, lambda value: value.get('state') == 'deleted')
                            row['deleted'] = True
                    except Exception:
                        report['cleanup_errors'].append(index)
                else:
                    report['cleanup_errors'].append(index)
                with LOCK:
                    row['boundaries'] = [{'kind': value['kind'], 'id': value['id'],
                                          'ms': (value['at'] - started) * 1000}
                                         for value in EVENTS if value['at'] >= started]
                row['elapsed_ms'] = (time.perf_counter() - started) * 1000
                required = {'reconcile_create_dispatch', 'provider_create_accepted', 'provider_create_operation', 'provider_running_observed',
                            'command_session_created', 'attach_connected', 'process_started', 'process_exited'}
                row['missing_boundaries'] = sorted(required - {value['kind'] for value in row['boundaries']})
                if row['passed'] and row['missing_boundaries'] and not args.expect_refusal:
                    row['passed'], row['error'] = False, 'IncompleteTimingEvidence'
                save(args.evidence, report)
                print(json.dumps({key: row.get(key) for key in
                    ('index', 'passed', 'first_command_ms', 'deleted', 'error')}), flush=True)
            if report['cleanup_errors']:
                break
        values = sorted(row['first_command_ms'] for row in report['samples'] if row.get('passed') and 'first_command_ms' in row)
        report['failures'] = sum(not row['passed'] for row in report['samples'])
        report['completed'] = len(report['samples']) == args.count
        report['latency_population'] = 'All successful attempts; failed attempts remain in samples and fail qualification'
        report['percentile_method'] = 'nearest rank'
        if values:
            report['p50_ms'] = values[(len(values) - 1) // 2]
            report['p90_ms'] = values[(9 * len(values) + 9) // 10 - 1]
        report['passed'] = report['completed'] and not report['failures'] and not report['cleanup_errors']
        if args.arm == 'snapshot' and not args.expect_refusal:
            report['passed'] = report['passed'] and report.get('p50_ms', float('inf')) < 2000
        save(args.evidence, report)
    finally:
        if process:
            process.terminate()
            try:
                process.wait(timeout=10)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()
        if log:
            log.close()
        if container:
            subprocess.run(['docker', 'stop', '--time', '10', container],
                           check=True, stdout=subprocess.DEVNULL)
            report['database_fixture_removed'] = True
            save(args.evidence, report)
        for server in (management, dataplane):
            server.shutdown()
            server.server_close()
    raise SystemExit(0 if report.get('passed') else 1)


if __name__ == '__main__':
    main()
