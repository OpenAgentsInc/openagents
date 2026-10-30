#!/usr/bin/env python3
"""relay_fixture.py's scripted relay behind TLS, for coax/tests/net/tls.sh.

Test tooling only. Imports the plain-ws fixture unchanged and wraps its
listening socket with Python's ssl module, so the client's first connection
gets the same script over wss://: REQ -> the stored events, EOSE, a ping.
Prints "PORT <n>" and "KINDS <k,...>" (the kinds of connection 1's events),
then waits for the client's close frame and prints "RELAY ok" or the
fixture's errors.
  tls_relay.py CERT KEY
"""
import ssl
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import relay_fixture  # noqa: E402


def main():
    cert, key = sys.argv[1], sys.argv[2]
    relay = relay_fixture.FixtureRelay()
    ctx = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    ctx.load_cert_chain(cert, key)
    relay.server = ctx.wrap_socket(relay.server, server_side=True)
    kinds = sorted({ev['kind'] for _, ev in relay.events[:3]})
    count = sum(1 for _, ev in relay.events[:3] if ev['kind'] in kinds)
    print('PORT %d' % relay.port, flush=True)
    print('KINDS %s %d' % (','.join(map(str, kinds)), count), flush=True)
    relay.start()
    end = time.time() + 30
    while time.time() < end and not any(e['what'] == 'close' for e in relay.log):
        time.sleep(0.05)
    relay.stop()
    whats = [e['what'] for e in relay.log]
    if relay.errors or 'req' not in whats or 'close' not in whats:
        print('RELAY failed: errors=%r log=%r' % (relay.errors, whats), flush=True)
        sys.exit(1)
    print('RELAY ok: %s' % ' '.join(whats), flush=True)


if __name__ == '__main__':
    main()
