#!/usr/bin/env python3
"""Run scratch qualification with orphan reaping when container init cannot reap."""
import ctypes
import os
import subprocess
import sys
import time

if len(sys.argv) < 2:
    raise SystemExit("Usage: reap-orphans.py COMMAND [ARG ...]")
if sys.platform != "linux":
    raise SystemExit(subprocess.call(sys.argv[1:]))
# PR_SET_CHILD_SUBREAPER makes orphaned scratch sandbox helpers our children.
# This affects only this qualification process and its descendants.
libc = ctypes.CDLL(None, use_errno=True)
if libc.prctl(36, 1, 0, 0, 0) != 0:
    raise OSError(ctypes.get_errno(), "Cannot install the qualification child reaper")
child = subprocess.Popen(sys.argv[1:])
while True:
    try:
        pid, status = os.waitpid(-1, os.WNOHANG)
    except ChildProcessError:
        raise SystemExit("Qualification child exited without a retained wait status")
    if pid == child.pid:
        child.returncode = os.waitstatus_to_exitcode(status)
        raise SystemExit(child.returncode if child.returncode >= 0 else 128 - child.returncode)
    if pid == 0:
        time.sleep(0.01)
