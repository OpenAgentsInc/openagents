#!/usr/bin/env python3
"""Bounded loopback webhook receiver for the signet experiment's test account."""
import argparse
import fcntl
import hashlib
from http.server import BaseHTTPRequestHandler, HTTPServer
import json
import os
from pathlib import Path
import time
from urllib.parse import urlsplit


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--directory', type=Path, required=True)
    parser.add_argument('--until', type=int, required=True)
    parser.add_argument('--network', choices=['signet', 'regtest'], default='signet')
    args = parser.parse_args()
    if not time.time() < args.until <= time.time() + 9 * 86400:
        parser.error('Choose an end time within the next nine days.')
    args.directory.mkdir(parents=True, exist_ok=True)
    lock = (args.directory / 'receiver.lock').open('a')
    fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
    path = args.directory / 'received.jsonl'
    path.touch(exist_ok=True)
    saved = {}
    # Only a complete, synced receipt can have received a successful response.
    with path.open('r+b') as recovery:
        raw = recovery.read(128 * 1024 * 1024 + 1)
        if len(raw) > 128 * 1024 * 1024:
            parser.error('The receiver log exceeds 128 MiB.')
        end = raw.rfind(b'\n') + 1
        recovery.truncate(end)
        for line in raw[:end].splitlines():
            row = json.loads(line)
            saved[row['id']] = row
    log = path.open('ab')

    class Receiver(BaseHTTPRequestHandler):
        timeout = 5

        def log_message(self, _format, *_args):
            pass

        def do_POST(self):
            try:
                size = int(self.headers.get('Content-Length', '0'))
                if not 0 < size <= 32768 or self.path != '/alarms':
                    raise ValueError('Invalid alarm request.')
                body = self.rfile.read(size)
                row = json.loads(body)
                identity = row['id']
                if (row['schema'] != 'layertree.follower.observation.v1' or
                        row['kind'] != 'light_client' or row['network'] != args.network or
                        row['result'] == 'ok' or self.headers.get('Idempotency-Key') != identity or
                        len(bytes.fromhex(identity)) != 32):
                    raise ValueError('Invalid test alarm.')
                if identity in saved and saved[identity] != row:
                    self.send_error(409)
                    return
                if identity not in saved:
                    log.write(json.dumps(row, separators=(',', ':')).encode() + b'\n')
                    log.flush()
                    os.fsync(log.fileno())
                    saved[identity] = row
                    print(json.dumps({'received_at': int(time.time()), 'id': identity,
                                      'member': row['member'], 'result': row['result'],
                                      'height': row['height']}), flush=True)
                self.send_response(204)
                self.end_headers()
            except (ValueError, KeyError, TypeError, TimeoutError):
                self.send_error(400)

        def do_GET(self):
            body = json.dumps({'alarms_received': len(saved)}).encode()
            self.send_response(200)
            self.send_header('Content-Type', 'application/json')
            self.send_header('Content-Length', str(len(body)))
            self.end_headers()
            self.wfile.write(body)

    ready_path = args.directory / 'receiver.json'
    port = 0
    if ready_path.exists():
        previous = json.loads(ready_path.read_text())
        endpoint = urlsplit(previous['url'])
        if endpoint.hostname != '127.0.0.1':
            parser.error('The saved receiver endpoint is not loopback.')
        port = endpoint.port
    with HTTPServer(('127.0.0.1', port), Receiver) as server:
        server.timeout = 1
        ready = {'schema': 'layertree.alarm.receiver.v1', 'pid': os.getpid(),
                 'url': f'http://127.0.0.1:{server.server_port}/alarms', 'until': args.until, 'network': args.network,
                 'script_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest()}
        ready_path.write_text(json.dumps(ready, indent=2) + '\n')
        while time.time() < args.until:
            server.handle_request()


if __name__ == '__main__':
    main()
