#!/usr/bin/env python3
"""Check a disposable terminal chat continued through desktop and phone controls."""
import argparse
import json
import os
import signal
from pathlib import Path
import shlex
import shutil
import subprocess
import threading
import time
import urllib.error
import urllib.request
import uuid
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

ROOT = Path(__file__).resolve().parents[2]
EDGE = 'https://coder-sync-157437760789.us-central1.run.app'


def main():
    def interrupted(_signal, _frame):
        raise KeyboardInterrupt
    signal.signal(signal.SIGTERM, interrupted)
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--dedicated-account', required=True, action='store_true')
    parser.add_argument('--desktop-binary', required=True, type=Path)
    parser.add_argument('--target-dir', required=True, type=Path)
    parser.add_argument('--scratch', required=True, type=Path)
    parser.add_argument('--api', default='https://api.openagents.com')
    parser.add_argument('--website', default='https://openagents.com')
    parser.add_argument('--simulator', default='8010197D-517D-43DA-9559-46742EFABC65')
    args = parser.parse_args()
    args.scratch.mkdir(parents=True, exist_ok=False)
    token = os.environ['OPENAGENTS_TOKEN']
    account = os.environ['OPENAGENTS_TEST_ACCOUNT']
    assert token.startswith('oa_sk_') and args.api.startswith('https://')
    env = dict(os.environ, SSL_CERT_FILE='/etc/ssl/cert.pem')

    def call(method, path, data=None, origin=None):
        request = urllib.request.Request((origin or args.api) + path, method=method,
            headers={'Authorization': 'Bearer ' + token, 'Content-Type': 'application/json'},
            data=json.dumps(data).encode() if data is not None else None)
        try:
            response = urllib.request.urlopen(request, timeout=30)
        except urllib.error.HTTPError as error:
            response = error
        with response:
            raw = response.read(2 * 1024 * 1024)
            return response.status, json.loads(raw) if raw else None

    assert call('GET', '/v1/account')[1]['account']['id'] == account
    assert '/v1/threads/{id}/continue' in call('GET', '/v1/openapi.json')[1]['paths']
    subprocess.run(['openagents', 'capacity', 'check', 'claude'], check=True)
    # Read an existing provider credential without logging or modifying the Keychain.
    oauth = os.environ.get('CLAUDE_CODE_OAUTH_TOKEN')
    if not oauth and not os.environ.get('ANTHROPIC_API_KEY'):
        secret = subprocess.run(['/usr/bin/security', 'find-generic-password', '-a', os.environ['USER'], '-s', 'Claude Code-credentials', '-w'], capture_output=True)
        assert secret.returncode == 0, 'An isolated Claude credential is required'
        oauth = json.loads(secret.stdout)['claudeAiOauth']['accessToken']
    home = args.scratch / 'provider-home'
    home.mkdir(mode=0o700)
    config = home / '.claude'
    config.mkdir(mode=0o700)
    claude = shutil.which('claude')
    assert claude
    wrapper = args.scratch / 'claude-without-history'
    wrapper.write_text('#!/bin/sh\nexec ' + shlex.quote(claude) + ' --no-session-persistence "$@"\n')
    wrapper.chmod(0o700)
    native = args.scratch / 'desktop'
    captures = native / 'captures'
    session = 'continuation-' + uuid.uuid4().hex
    flower = 'coral orchid'
    title = 'Continuation test ' + session[-8:]
    status, source = call('PUT', '/v1/threads/synced/' + session, {
        'computer': 'Disposable terminal', 'title': title,
        'messages': [{'role': 'user', 'text': 'For this reading test, the fictional flower name is ' + flower + '.'},
                     {'role': 'tool', 'text': 'Do not copy tool output into the new context'},
                     {'role': 'assistant', 'text': 'I will remember the fictional flower name.'}]})
    assert status == 200
    chat = source['chat']
    env.update(SDK_FIXTURE_CHAT_IDS=json.dumps([chat]), SDK_FIXTURE_API=args.api + '/v1',
        SDK_FIXTURE_WEBSITE=args.website, SDK_FIXTURE_EDGE=EDGE, SDK_FIXTURE_CLAUDE=str(wrapper),
        SDK_CHAT_PROMPT='What fictional flower name did we choose earlier? Reply with only that name. Do not use tools.',
        SDK_CHAT_EXPECTED=flower, SDK_FIXTURE_WAIT_PHONE='1',
        HOME=str(home), CLAUDE_CONFIG_DIR=str(config), XDG_CONFIG_HOME=str(home / 'config'),
        XDG_CACHE_HOME=str(home / 'cache'), XDG_DATA_HOME=str(home / 'share'),
        TMPDIR=str(args.scratch), NO_OPEN_BROWSER='1', BROWSER='/usr/bin/true')
    if oauth:
        env['CLAUDE_CODE_OAUTH_TOKEN'] = oauth
    process = None
    server = None
    target = None
    try:
        with (args.scratch / 'desktop-driver.log').open('w') as log:
            process = subprocess.Popen(['python3', str(ROOT / 'apps/openagents/scripts/check-account-fixture.py'), '--continue-chat', str(args.desktop_binary), str(native)], cwd=ROOT, env=env, stdout=log, stderr=subprocess.STDOUT)
        deadline = time.monotonic() + 190
        while not (captures / 'continued-chat').exists():
            assert process.poll() is None, 'Desktop check exited; inspect its log'
            assert time.monotonic() < deadline, 'Desktop continuation timed out'
            time.sleep(0.2)
        target = (captures / 'continued-chat').read_text()
        status, old = call('GET', '/v1/threads/' + chat)
        assert status == 200 and old['thread']['continued_by']['id'] == target
        assert call('POST', '/v1/threads/' + chat + '/messages', {'request_id': str(uuid.uuid4()), 'text': 'Should refuse'})[0] == 409

        class Handler(BaseHTTPRequestHandler):
            def log_message(self, *_args):
                pass
            def do_GET(self):
                if self.path != '/credential':
                    self.send_error(404)
                    return
                data = json.dumps({'credential': token}).encode()
                self.send_response(200)
                self.send_header('Content-Type', 'application/json')
                self.end_headers()
                self.wfile.write(data)
            def do_POST(self):
                if self.path != '/phone-done':
                    self.send_error(404)
                    return
                (captures / 'phone-done').write_text('done')
                self.send_response(200)
                self.end_headers()

        server = ThreadingHTTPServer(('127.0.0.1', 0), Handler)
        threading.Thread(target=server.serve_forever, daemon=True).start()
        control = 'http://127.0.0.1:' + str(server.server_port)
        test_env = dict(os.environ, OPENAGENTS_SKIP_CORE='1', OPENAGENTS_ACCOUNT_FIXTURE='1', CARGO_TARGET_DIR=str(args.target_dir),
            TEST_RUNNER_CODER_MOBILE_SYNC_CONFIG=json.dumps({'control': control, 'credential_url': control + '/credential', 'sync': EDGE, 'title': title, 'chat': target, 'flower': flower}))
        with (args.scratch / 'phone-uitest.log').open('w') as log:
            subprocess.run(['xcodebuild', 'test', '-project', 'apps/ios/OpenAgents.xcodeproj', '-scheme', 'OpenAgents', '-configuration', 'Debug',
                '-destination', 'id=' + args.simulator, '-derivedDataPath', str(args.target_dir / 'ios-derived'),
                'CODE_SIGNING_ALLOWED=YES', 'CODE_SIGN_IDENTITY=-', 'ARCHS=arm64', 'SWIFT_ACTIVE_COMPILATION_CONDITIONS=DEBUG CODER_ACCOUNT_FIXTURE',
                '-resultBundlePath', str(args.scratch / 'phone.xcresult'), '-only-testing:OpenAgentsUITests/OpenAgentsLiveSyncTests/testContinuedComputerSession'],
                cwd=ROOT / 'apps/openagents', env=test_env, stdout=log, stderr=subprocess.STDOUT, check=True)
        assert process.wait(timeout=40) == 0, 'Desktop cleanup or sign-out failed'
        assert (captures / 'phone-steer-confirmed').exists()
        (args.scratch / 'result.json').write_text(json.dumps({'desktop_context': True, 'phone_observation_and_steer': True, 'source_read_only': True, 'chat': target, 'source': chat}, indent=2) + '\n')
        print('PASS desktop continuation, earlier context, phone observation and steer')
    finally:
        if process and process.poll() is None:
            process.terminate()
            process.wait(timeout=15)
        if server:
            server.shutdown()
        # Delete test registry rows even when a native assertion fails.
        profile = captures / 'engine'
        device_files = list(profile.rglob('device-id')) if profile.exists() else []
        devices = [p.read_text().strip() for p in device_files]
        if (captures / 'fixture-device').exists():
            devices.append((captures / 'fixture-device').read_text().strip())
        devices = sorted(set(devices))
        ids = [('devices', d) for d in devices]
        if (captures / 'fixture-space').exists():
            ids.append(('spaces', (captures / 'fixture-space').read_text().strip()))
        if not target and (captures / 'pending-chat').exists():
            target = (captures / 'pending-chat').read_text().strip()
        if target:
            ids += [('chats', target), ('sessions', target)]
        ops = [{'kind': kind, 'id': id_, 'op': 'delete', 'hlc': f'{int(time.time()*1000)+10000:013}-000000-continuation-fixture'} for kind, id_ in ids]
        if ops:
            assert call('POST', '/registry/' + account + '/push', {'batch': str(uuid.uuid4()), 'ops': ops}, EDGE)[0] == 200
        assert call('DELETE', '/v1/threads/synced/' + session)[0] in (200, 204, 404)
        print('Cleanup: disposable source and session rows removed')


if __name__ == '__main__':
    main()
