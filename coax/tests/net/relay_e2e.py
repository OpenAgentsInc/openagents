#!/usr/bin/env python3
"""The native relay client end to end, under ASan and UBSan, in seconds.

Builds coax/tests/net/relay_e2e.coax (the driver, over
coax/lib/net/relay_conn.coax) with socket-native.c, tls-native.c (OpenSSL
from Nix through coax/platform/tls-env.sh, built with -DCXTLS_TEST_TRUST),
entropy-native.c (through entropy-foreign.c) and process-native.c (for
$RX_PORT and $RX_TLS), starts the local fixture relay (relay_fixture.py,
127.0.0.1 only, standard library), runs the driver against it and checks
both sides: the driver's own checks and what the relay saw on the wire.

It runs the script twice: over ws://, then over wss:// with the same fixture
behind Python's ssl module and a test CA and localhost leaf that openssl
generates into the temp dir for this run (none is committed, and the driver
trusts only that CA through CXTLS_TEST_CA). A third run leaves CXTLS_TEST_CA
unset, so the system trust store applies, and checks that every connection is
refused before the relay sees a WebSocket handshake. Tooling only; nothing
here ships.

  python3 coax/tests/net/relay_e2e.py          # ws:// and wss://
  python3 coax/tests/net/relay_e2e.py --ws     # ws:// only
  COAX=... COAX_CC=... override the compiler and the C compiler (default gcc)
"""
import os
import ssl
import subprocess
import sys
import socket
import tempfile
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
sys.path.insert(0, str(HERE))
from relay_fixture import FixtureRelay  # noqa: E402

COAX = os.environ.get('COAX', str(ROOT / 'coax/bin/coax'))
CC = os.environ.get('COAX_CC', 'gcc')
PLATFORM = ROOT / 'coax/platform'


def tls_flags():
    """CXTLS_CFLAGS and CXTLS_LIBS from tls-env.sh (OpenSSL from Nix on Linux)."""
    out = subprocess.run(['bash', '-c', '. "$1" && printf "%s\\n%s\\n" "$CXTLS_CFLAGS" "$CXTLS_LIBS"', 'tls-env',
                          str(PLATFORM / 'tls-env.sh')], check=True, capture_output=True, text=True).stdout
    cflags, libs = out.split('\n')[:2]
    return cflags.split(), libs.split()


def certs(d):
    """A one-day test CA and a localhost leaf it signs, in d. Returns (ca, cert, key)."""
    def run(*args):
        subprocess.run(['openssl', *args], check=True, capture_output=True)
    run('req', '-x509', '-newkey', 'ec', '-pkeyopt', 'ec_paramgen_curve:prime256v1', '-nodes', '-days', '1',
        '-keyout', str(d / 'ca.key'), '-out', str(d / 'ca.pem'), '-subj', '/CN=Coax relay e2e test CA',
        '-addext', 'basicConstraints=critical,CA:TRUE', '-addext', 'keyUsage=critical,keyCertSign,cRLSign')
    run('req', '-newkey', 'ec', '-pkeyopt', 'ec_paramgen_curve:prime256v1', '-nodes',
        '-keyout', str(d / 'leaf.key'), '-out', str(d / 'leaf.csr'), '-subj', '/CN=localhost')
    (d / 'leaf.ext').write_text('subjectAltName=DNS:localhost\nbasicConstraints=critical,CA:FALSE\n'
                                'keyUsage=critical,digitalSignature\nextendedKeyUsage=serverAuth\n')
    run('x509', '-req', '-in', str(d / 'leaf.csr'), '-CA', str(d / 'ca.pem'), '-CAkey', str(d / 'ca.key'),
        '-set_serial', '0x' + os.urandom(8).hex(), '-days', '1', '-extfile', str(d / 'leaf.ext'),
        '-out', str(d / 'leaf.pem'))
    return d / 'ca.pem', d / 'leaf.pem', d / 'leaf.key'


class ClosingSSLSocket(ssl.SSLSocket):
    """The fixture ends connections with shutdown(SHUT_RDWR). Over TLS that is
    a TCP close without close_notify, which the client rightly reports as a
    failure, not an end of stream; send close_notify first, as a TLS relay
    closing a connection does."""
    def shutdown(self, how):
        try:
            self.unwrap()
        except (OSError, ValueError):
            pass
        try:
            socket.socket.shutdown(self, how)
        except OSError:
            pass


def build(d):
    # entropy-native.c includes the policy's header; the policy's code itself
    # is compiled into the driver (relay_e2e.coax loads entropy.coax).
    subprocess.run([COAX, 'c', str(ROOT / 'coax/lib/crypto/entropy.coax'), '--library',
                    '--header', str(d / 'entropy-policy.h')], check=True, stdout=subprocess.DEVNULL)
    with open(d / 'driver.c', 'wb') as out:
        subprocess.run([COAX, 'c', str(HERE / 'relay_e2e.coax')], check=True, stdout=out)
    exe = d / 'driver'
    cflags, libs = tls_flags()
    subprocess.run([CC, '-std=c11', '-O1', '-g', '-fsanitize=address,undefined', '-fno-sanitize-recover=all',
                    '-fno-omit-frame-pointer', '-DCXTLS_TEST_TRUST', *cflags, '-I' + str(d), '-I' + str(PLATFORM),
                    str(d / 'driver.c'), str(PLATFORM / 'socket-native.c'), str(PLATFORM / 'tls-native.c'),
                    str(PLATFORM / 'entropy-native.c'), str(PLATFORM / 'entropy-foreign.c'),
                    str(PLATFORM / 'process-native.c'), *libs, '-lm', '-o', str(exe)],
                   check=True)
    return exe


def untrusted(exe):
    """wss:// with the system trust store, which does not hold the test CA:
    relay_conn_open must refuse every connection before any WebSocket byte."""
    relay = FixtureRelay()
    ca, cert, key = certs(exe.parent)
    ctx = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    ctx.load_cert_chain(str(cert), str(key))
    relay.server = ctx.wrap_socket(relay.server, server_side=True)
    env = {**os.environ, 'RX_PORT': str(relay.port), 'RX_TLS': '1'}
    env.pop('CXTLS_TEST_CA', None)
    relay.start()
    try:
        run = subprocess.run([str(exe)], capture_output=True, text=True, timeout=60, env=env)
    finally:
        relay.stop()
    hs = len([e for e in relay.log if e['what'] == 'handshake'])
    refused = 'relay e2e: FAIL connect, handshake, REQ, stored events, EOSE' in run.stdout.splitlines()
    print('--- wss:// without the test CA: connection refused: %s, WebSocket handshakes the relay saw: %d' % (refused, hs))
    return [] if refused and hs == 0 else ['untrusted wss: refused=%s handshakes=%d' % (refused, hs)]


def run_once(exe, tls):
    """One run of the script, ws:// or wss://. Returns the list of problems."""
    label = 'wss' if tls else 'ws'
    relay = FixtureRelay()
    env = {**os.environ, 'RX_PORT': str(relay.port), 'RX_TLS': '1' if tls else '0',
           'ASAN_OPTIONS': 'abort_on_error=0:halt_on_error=1',
           'UBSAN_OPTIONS': 'print_stacktrace=1:halt_on_error=1'}
    env.pop('CXTLS_TEST_CA', None)
    if tls:
        ca, cert, key = certs(exe.parent)
        ctx = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        ctx.minimum_version = ssl.TLSVersion.TLSv1_2
        ctx.load_cert_chain(str(cert), str(key))
        ctx.sslsocket_class = ClosingSSLSocket
        relay.server = ctx.wrap_socket(relay.server, server_side=True)
        env['CXTLS_TEST_CA'] = str(ca)
    relay.start()
    t1 = time.monotonic()
    try:
        run = subprocess.run([str(exe)], capture_output=True, text=True, timeout=120, env=env)
    finally:
        relay.stop()
    ran = time.monotonic() - t1
    print('--- %s://' % label)
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
    print('%s run %.1f s (ASan+UBSan)' % (label, ran))
    return ['%s: %s' % (label, p) for p in problems]


def main():
    modes = [False] if '--ws' in sys.argv[1:] else [False, True]
    with tempfile.TemporaryDirectory(prefix='coax-relay-e2e-') as tmp:
        d = Path(tmp)
        t0 = time.monotonic()
        exe = build(d)
        print('build %.1f s' % (time.monotonic() - t0))
        problems = []
        for tls in modes:
            problems += run_once(exe, tls)
        if True in modes:
            problems += untrusted(exe)
        for p in problems:
            print('FAIL ' + p)
        print('relay_e2e: ' + ('ok' if not problems else '%d problems' % len(problems)))
        return 0 if not problems else 1


if __name__ == '__main__':
    sys.exit(main())
