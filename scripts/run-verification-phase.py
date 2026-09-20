#!/usr/bin/env python3
"""Run one verification phase with live output and periodic elapsed status."""
import argparse
import os
import signal
import subprocess
import sys
import time


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--heartbeat-seconds', type=float, default=30)
    parser.add_argument('phase')
    parser.add_argument('command', nargs=argparse.REMAINDER)
    args = parser.parse_args()
    command = args.command
    if command[:1] == ['--']:
        command = command[1:]
    if not command or args.heartbeat_seconds <= 0:
        parser.error('provide a command and a positive heartbeat interval')
    started = time.monotonic()
    print(f'PHASE START: {args.phase}', flush=True)
    child = None
    received = None

    def forward(signum, _frame):
        nonlocal received
        received = signum
        print(f'PHASE SIGNAL: {args.phase}; forwarding {signal.Signals(signum).name}', flush=True)
        if child is not None and child.poll() is None:
            try:
                os.killpg(child.pid, signum)
            except ProcessLookupError:
                pass

    for signum in (signal.SIGINT, signal.SIGTERM, signal.SIGHUP):
        signal.signal(signum, forward)
    try:
        # Inherit output descriptors: nothing is hidden behind a captured pipe.
        child = subprocess.Popen(command, start_new_session=True)
        if received is not None:
            forward(received, None)
        while True:
            try:
                code = child.wait(timeout=args.heartbeat_seconds)
                break
            except subprocess.TimeoutExpired:
                print(f'PHASE RUNNING: {args.phase}; elapsed {time.monotonic()-started:.1f}s', flush=True)
        code = 128-code if code < 0 else code
    except OSError as error:
        print(f'PHASE ERROR: {args.phase}; {error}', file=sys.stderr, flush=True)
        code = 127
    elapsed = time.monotonic()-started
    print(f'PHASE END: {args.phase}; elapsed {elapsed:.1f}s; exit {code}', flush=True)
    return code


if __name__ == '__main__':
    sys.exit(main())
