#!/usr/bin/env python3
"""#11816: observe running Firecracker guests on the maintained worker (read-only).

Prints boot mode (cold boot config vs snapshot load), machine configuration,
and host process facts for each running guest. Prints no guest contents.
"""
import json, pathlib, socket, sys

def machine_config(pid, args):
    if '--api-sock' not in args: return None
    api = pathlib.Path(args[args.index('--api-sock') + 1])
    proc = pathlib.Path('/proc') / str(pid)
    path = proc / 'root' / str(api).lstrip('/') if api.is_absolute() else proc / 'cwd' / api
    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as c:
        c.settimeout(2); c.connect(str(path))
        c.sendall(b'GET /machine-config HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n')
        raw = bytearray()
        while len(raw) < 65536:
            chunk = c.recv(4096)
            if not chunk: break
            raw.extend(chunk)
            if b'\r\n\r\n' in raw:
                head, body = bytes(raw).split(b'\r\n\r\n', 1)
                length = next((int(l.split(b':', 1)[1]) for l in head.split(b'\r\n') if l.lower().startswith(b'content-length:')), None)
                if length is not None and len(body) >= length: break
    head, body = bytes(raw).split(b'\r\n\r\n', 1)
    if b' 200 ' not in head.split(b'\r\n', 1)[0] + b' ': return {'status': head.split(b'\r\n', 1)[0].decode()}
    cfg = json.loads(body)
    return {'vcpu_count': cfg.get('vcpu_count'), 'mem_size_mib': cfg.get('mem_size_mib'), 'smt': cfg.get('smt'), 'track_dirty_pages': cfg.get('track_dirty_pages')}

out = []
for p in pathlib.Path('/proc').iterdir():
    if not p.name.isdecimal(): continue
    try: args = [a.decode() for a in p.joinpath('cmdline').read_bytes().rstrip(b'\0').split(b'\0')]
    except (OSError, UnicodeError): continue
    if not args or pathlib.Path(args[0]).name != 'firecracker' or '--id' not in args: continue
    entry = {'sandbox_id': args[args.index('--id') + 1], 'host_pid': int(p.name),
             'boot_mode': 'cold' if '--config-file' in args else 'snapshot'}
    try: entry['machine_config'] = machine_config(int(p.name), args)
    except Exception as e: entry['machine_config_error'] = type(e).__name__
    out.append(entry)
print(json.dumps({'guests': out, 'host': socket.gethostname().split('.')[0]}))
