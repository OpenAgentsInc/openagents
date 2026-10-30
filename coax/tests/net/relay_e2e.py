#!/usr/bin/env python3
"""The native relay client end to end, under ASan and UBSan, in seconds.

Builds coax/tests/net/relay_e2e.coax with socket-native.c, entropy-native.c
(through entropy-foreign.c) and process-native.c (for $RX_PORT), starts the
local fixture relay (relay_fixture.py, 127.0.0.1 only, standard library),
runs the driver against it and checks both sides: the driver's own checks
and what the relay saw on the wire. Tooling only; nothing here ships.

  python3 coax/tests/net/relay_e2e.py        # COAX=... and COAX_CC=... override
"""
import os
import subprocess
import sys
import tempfile
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
sys.path.insert(0, str(HERE))
from relay_fixture import FixtureRelay  # noqa: E402

COAX = os.environ.get('COAX', str(ROOT / 'coax/bin/coax'))
CC = os.environ.get('COAX_CC', 'cc')
PLATFORM = ROOT / 'coax/platform'


def build(d):
    # entropy-native.c includes the policy's header; the policy's code itself
    # is compiled into the driver (relay_e2e.coax loads entropy.coax).
    subprocess.run([COAX, 'c', str(ROOT / 'coax/lib/crypto/entropy.coax'), '--library',
                    '--header', str(d / 'entropy-policy.h')], check=True, stdout=subprocess.DEVNULL)
    with open(d / 'driver.c', 'wb') as out:
        subprocess.run([COAX, 'c', str(HERE / 'relay_e2e.coax')], check=True, stdout=out)
    exe = d / 'driver'
    subprocess.run([CC, '-std=c11', '-O1', '-g', '-fsanitize=address,undefined', '-fno-sanitize-recover=all',
                    '-fno-omit-frame-pointer', '-I' + str(d), '-I' + str(PLATFORM),
                    str(d / 'driver.c'), str(PLATFORM / 'socket-native.c'), str(PLATFORM / 'entropy-native.c'),
                    str(PLATFORM / 'entropy-foreign.c'), str(PLATFORM / 'process-native.c'), '-lm', '-o', str(exe)],
                   check=True)
    return exe


def main():
    with tempfile.TemporaryDirectory(prefix='coax-relay-e2e-') as tmp:
        d = Path(tmp)
        t0 = time.monotonic()
        exe = build(d)
        built = time.monotonic() - t0
        relay = FixtureRelay().start()
        t1 = time.monotonic()
        try:
            run = subprocess.run([str(exe)], capture_output=True, text=True, timeout=120,
                                 env={**os.environ, 'RX_PORT': str(relay.port),
                                      'ASAN_OPTIONS': 'abort_on_error=0:halt_on_error=1',
                                      'UBSAN_OPTIONS': 'print_stacktrace=1:halt_on_error=1'})
        finally:
            relay.stop()
        ran = time.monotonic() - t1
        sys.stdout.write(run.stdout)
        if run.stderr:
            sys.stderr.write(run.stderr)
        problems = []
        if run.returncode != 0:
            problems.append('driver exited %d' % run.returncode)
        if 'relay e2e: ok' not in run.stdout.splitlines():
            problems.append('driver did not print "relay e2e: ok"')
        if 'runtime error' in run.stderr or 'AddressSanitizer' in run.stderr or 'LeakSanitizer' in run.stderr:
            problems.append('sanitizer report on stderr')
        problems += ['relay: ' + p for p in relay.failures()]
        masks = len([e for e in relay.log if e['what'] == 'mask'])
        print('relay saw %d handshakes, %d masked frames (all mask keys distinct: %s)' % (
            len([e for e in relay.log if e['what'] == 'handshake']), masks,
            not any('mask key repeated' in p for p in problems)))
        if relay.strays:
            print('relay ignored %d stray non-WebSocket connection(s) from other local processes' % relay.strays)
        print('build %.1f s, run %.1f s (ASan+UBSan)' % (built, ran))
        for p in problems:
            print('FAIL ' + p)
        print('relay_e2e: ' + ('ok' if not problems else '%d problems' % len(problems)))
        return 0 if not problems else 1


if __name__ == '__main__':
    sys.exit(main())
