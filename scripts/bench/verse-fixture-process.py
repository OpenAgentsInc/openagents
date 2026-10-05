#!/usr/bin/env python3
"""Supervise one scratch process until it exits or the controlling stdin closes."""
import argparse
import os
import select
import signal
import subprocess
import sys


def supervise(command, home, control):
    child = subprocess.Popen(command, stdin=subprocess.DEVNULL,
                             env={**os.environ, 'HOME': home}, start_new_session=True)
    try:
        while child.poll() is None:
            ready, _, _ = select.select([control], [], [], 0.1)
            if ready and not os.read(control.fileno(), 4096):
                break
    finally:
        # The process group belongs exclusively to this fixture, including descendants.
        try:
            os.killpg(child.pid, signal.SIGTERM)
        except ProcessLookupError:
            pass
        try:
            child.wait(timeout=10)
        except subprocess.TimeoutExpired:
            try:
                os.killpg(child.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            child.wait()
        # A descendant can ignore TERM even when the direct child has already exited.
        try:
            os.killpg(child.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
    return child.returncode if child.returncode >= 0 else 128 - child.returncode


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--home', required=True)
    parser.add_argument('command', nargs=argparse.REMAINDER)
    args = parser.parse_args()
    command = args.command[1:] if args.command[:1] == ['--'] else args.command
    if not command or not os.path.isabs(args.home):
        parser.error('An absolute scratch home and a command are required')
    sys.exit(supervise(command, args.home, sys.stdin.buffer))
