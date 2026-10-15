#!/usr/bin/env python3
"""Observe the running Firecracker launch on the disposable GCE host."""

import json
import pathlib
import sys
import uuid

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
print(json.dumps(matches[0]))
