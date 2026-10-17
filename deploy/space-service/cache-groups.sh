#!/usr/bin/env bash
# Repair cache group inheritance on an owned node with no running guests.
set -euo pipefail
[[ $(id -u) == 0 ]] && mountpoint -q /space
case $(hostname -s) in oa-space-service-worker|oa-space-service-builder) ;; *) exit 2 ;; esac
python3 - <<'PY'
from pathlib import Path
import os,stat
for fd in Path('/proc').glob('[0-9]*/fd/*'):
    try:
        if 'kvm-vm' in os.readlink(fd): raise RuntimeError('Drain guests before repairing cache group inheritance.')
    except (FileNotFoundError,PermissionError): pass
root=Path('/space/cache/firecracker/templates')
for path in [root,*root.rglob('*')]:
    metadata=path.lstat()
    if stat.S_ISLNK(metadata.st_mode): continue
    if metadata.st_uid not in (0,950,955): raise RuntimeError('An unexpected cache owner needs review.')
    os.chown(path,-1,952,follow_symlinks=False)
    if stat.S_ISDIR(metadata.st_mode): os.chmod(path,stat.S_IMODE(metadata.st_mode)|stat.S_ISGID)
print('Owned template directories now preserve the shared cache group; artifact bytes are unchanged.')
PY
