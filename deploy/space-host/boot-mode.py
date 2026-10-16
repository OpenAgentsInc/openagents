#!/usr/bin/env python3
"""Observe the running Firecracker launch on the disposable GCE host."""

import json
import os
import pathlib
import socket
import sys
import uuid

shape = sys.argv[2:] == ['--machine-config']
if shape and os.geteuid() != 0:
    os.execvp('sudo', ['sudo', '-n', sys.executable, __file__, *sys.argv[1:]])

sandbox = str(uuid.UUID(sys.argv[1].removeprefix('sbx_')))
matches = []
for process in pathlib.Path('/proc').iterdir():
    if not process.name.isdecimal():
        continue
    try:
        args = process.joinpath('cmdline').read_bytes().rstrip(b'\0').split(b'\0')
        args = [arg.decode() for arg in args]
    except (OSError, UnicodeError):
        continue
    if not args or pathlib.Path(args[0]).name != 'firecracker':
        continue
    if '--id' not in args or args[args.index('--id') + 1] != sandbox:
        continue
    # At pinned SPACE, cold starts supply a boot config. Snapshot restores
    # start Firecracker idle, then load its saved state through the API.
    matches.append({'sandbox_id': sandbox, 'host_pid': int(process.name),
                    'boot_mode': 'cold' if '--config-file' in args else 'snapshot',
                    'firecracker_argv': args})
if len(matches) != 1:
    sys.exit('Expected exactly one running Firecracker process for this sandbox.')
if shape:
    observation = matches[0]
    args = observation['firecracker_argv']
    if '--api-sock' not in args:
        sys.exit('The running Firecracker process has no API socket argument.')
    api = pathlib.Path(args[args.index('--api-sock') + 1])
    process = pathlib.Path('/proc') / str(observation['host_pid'])
    path = process / 'root' / str(api).lstrip('/') if api.is_absolute() else process / 'cwd' / api
    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as client:
        client.settimeout(2)
        client.connect(str(path))
        client.sendall(b'GET /machine-config HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n')
        raw = bytearray()
        while True:
            chunk = client.recv(4096)
            if not chunk:
                break
            raw.extend(chunk)
            if len(raw) > 65536:
                sys.exit('The Firecracker machine configuration exceeds the observation bound.')
            if b'\r\n\r\n' in raw:
                headers, body = bytes(raw).split(b'\r\n\r\n', 1)
                length = next((int(line.split(b':', 1)[1].strip()) for line in headers.split(b'\r\n')
                               if line.lower().startswith(b'content-length:')), None)
                if length is not None and len(body) >= length:
                    break
    headers, body = bytes(raw).split(b'\r\n\r\n', 1)
    if headers.split(b'\r\n', 1)[0].split(b' ')[1] != b'200':
        sys.exit('The Firecracker machine configuration was refused.')
    config = json.loads(body)
    observation['actual_shape'] = {'vcpu': config['vcpu_count'], 'memory_mib': config['mem_size_mib']}
print(json.dumps(matches[0]))
