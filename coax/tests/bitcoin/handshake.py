#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: Bitcoin Core's pinned sources, messages.py and BIP-324's reference.py decide each case outside Coax)
"""The cross-check of Bitcoin chapter 68. It computes every line the
chapter's example prints, in Python that shares no code with Coax, and
compares the transcript with the fixture:

  python3 -I coax/tests/bitcoin/handshake.py                     # compare with the fixture
  python3 -I coax/tests/bitcoin/handshake.py --coax coax/bin/coax   # also run the example

The handshake is Bitcoin Core's ProcessMessage (net_processing.cpp, VERSION
through SENDTXRCNCL), CheckIncomingNonce and InactivityCheck (net.cpp)
transliterated, with every constant read from the pinned
protocol_version.h, protocol.h, net.h and net_processing.cpp. Payloads are
make_handshake_vectors.py's, from Core's messages.py. The v2 side is
BIP-324's reference.py (keys, packets and their decryption) with the
receive states of net.cpp's V2Transport transliterated: the v1 prefix,
the key, the garbage search and the first packet's AAD.
"""
import hashlib
import re
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import ellswift as E  # noqa: E402
import make_handshake_vectors as H  # noqa: E402
import p2p  # noqa: E402
import v2transport as T  # noqa: E402

REPO = HERE.parents[2]
FIXTURE = REPO / 'bitcoin/fixtures/handshake.out'
EXAMPLE = 'bitcoin/examples/handshake.coax'
VERSION_PIN = 'ce87f506fc0db48f9cdb74e58a7ee396261b543fe9a7f423ec8eab1f2f82aa7a'
MAGIC = bytes.fromhex('f9beb4d9')
M = H.M


def constants():
    text = (p2p.UP / 'protocol_version.h').read_bytes()
    assert hashlib.sha256(text).hexdigest() == VERSION_PIN, 'protocol_version.h differs from its pin'
    c = {k: int(v) for k, v in re.findall(r'inline constexpr int (\w+) = (\d+);', text.decode())}
    proto = p2p.pinned('protocol.h')
    for name in ['NODE_NETWORK', 'NODE_BLOOM', 'NODE_WITNESS', 'NODE_COMPACT_FILTERS', 'NODE_NETWORK_LIMITED', 'NODE_P2P_V2']:
        c[name] = 1 << int(re.search(rf'{name} = \(1 << (\d+)\)', proto).group(1))
    for name in ['MAX_FEATUREID_LENGTH', 'MAX_FEATUREDATA_LENGTH']:
        c[name] = int(re.search(rf'{name}\{{(\d+)\}}', proto).group(1))
    net_h, proc = p2p.pinned('net.h'), p2p.pinned('net_processing.cpp')
    c['TIMEOUT_INTERVAL'] = 60 * int(re.search(r'TIMEOUT_INTERVAL\{(\d+)\};', net_h).group(1))
    c['CONNECT_TIMEOUT'] = int(re.search(r'DEFAULT_PEER_CONNECT_TIMEOUT = (\d+);', net_h).group(1))
    c['LIMITED_DEPTH'] = int(re.search(r'NODE_NETWORK_LIMITED_ALLOW_CONN_BLOCKS = (\d+);', proc).group(1))
    assert 'if (greatest_common_version >= 70016) {' in proc and 'if (greatest_common_version <= 70012) {' in proc
    c['ADDRV2'], c['ALERT'] = 70016, 70012
    return c


C = constants()
INBOUND, OUTBOUND, BLOCK_RELAY, MANUAL, FEELER, ADDR_FETCH = range(6)
WITNESS_FULL = C['NODE_NETWORK'] | C['NODE_WITNESS']
OURS = C['NODE_NETWORK'] | C['NODE_WITNESS'] | C['NODE_P2P_V2']
CORE = dict(H.vectors())


def their_version(version, services):
    p = bytearray(CORE['their version'])
    p[0:4] = version.to_bytes(4, 'little')
    p[4:12] = services.to_bytes(8, 'little')
    return bytes(p)


class Peer:
    """One connection's CNode and Peer, as ProcessMessage changes them."""

    def __init__(self, conn, nonce, services, now):
        self.conn, self.nonce, self.services = conn, nonce, services
        self.peer_services = 0 if conn == INBOUND else WITNESS_FULL
        self.version = 0
        self.common = 0
        self.connected = self.wtxid = self.addrv2 = self.cmpct = self.high = self.headers = False
        self.ping = 0
        self.opened, self.time = now, now
        self.last_send = -1 if conn == INBOUND else now
        self.last_recv = -1

    def open(self):
        return [] if self.conn == INBOUND else ['version']

    def payload(self, command):
        if command == 'version':
            return H.our_version(dict(version=C['PROTOCOL_VERSION'], services=self.services, time=self.time,
                                      peer_services=self.peer_services, nonce=self.nonce, agent='/Coax:0.68/',
                                      height=900000, relay=1))
        if command == 'sendcmpct':
            return CORE['sendcmpct']
        if command == 'pong':
            return self.ping.to_bytes(8, 'little')
        if command == 'alert':
            return CORE['alert']
        return b''

    def desirable(self, services):
        if services & C['NODE_NETWORK_LIMITED'] and 0 < C['LIMITED_DEPTH']:   # the node is at the tip
            return C['NODE_NETWORK_LIMITED'] | C['NODE_WITNESS']
        return WITNESS_FULL

    def receive(self, command, payload, now, pending=()):
        """(verdict, reason, sends)."""
        self.last_recv = now
        if command == 'version':
            if self.version != 0:
                return 'ignored', 'redundant version', []
            if len(payload) < 46:
                return 'ignored', "payload doesn't read", []
            version = int.from_bytes(payload[0:4], 'little', signed=True)
            services = int.from_bytes(payload[4:12], 'little')
            if self.conn in (OUTBOUND, BLOCK_RELAY, ADDR_FETCH) and self.desirable(services) & ~services:
                return 'disconnect', 'no expected services', []
            if version < C['MIN_PEER_PROTO_VERSION']:
                return 'disconnect', 'obsolete version', []
            v = M.msg_version()
            from io import BytesIO
            v.deserialize(BytesIO(payload))
            nonce = v.nNonce if len(payload) > 46 else 1
            if self.conn == INBOUND and nonce in pending:
                return 'disconnect', 'connected to self', []
            sends = ['version'] if self.conn == INBOUND else []
            self.common = min(version, C['PROTOCOL_VERSION'])
            self.version = version
            if self.common >= C['WTXID_RELAY_VERSION']:
                sends.append('wtxidrelay')
            if self.common >= C['ADDRV2']:
                sends.append('sendaddrv2')
            sends.append('verack')
            if self.conn not in (INBOUND, BLOCK_RELAY, FEELER):
                sends.append('getaddr')
            if self.common <= C['ALERT']:
                sends.append('alert')
            self.last_send = now
            if self.conn == FEELER:
                return 'disconnect', 'feeler done', sends
            return 'done', '', sends
        if self.version == 0:
            return 'ignored', 'no version yet', []
        if command == 'verack':
            if self.connected:
                return 'ignored', 'redundant verack', []
            self.connected = True
            return 'done', '', ['sendcmpct'] if self.common >= C['SHORT_IDS_BLOCKS_VERSION'] else []
        if command == 'sendheaders':
            self.headers = True
            return 'done', '', []
        if command == 'sendcmpct':
            if len(payload) < 9:
                return 'ignored', "payload doesn't read", []
            if payload[0] > 1:
                return 'disconnect', 'bad announce field', []
            if int.from_bytes(payload[1:9], 'little') != 2:
                return 'ignored', 'not version 2', []
            self.cmpct, self.high = True, payload[0] == 1
            return 'done', '', []
        if command == 'wtxidrelay':
            if self.connected:
                return 'disconnect', 'after verack', []
            if self.common < C['WTXID_RELAY_VERSION']:
                return 'ignored', 'old common version', []
            if self.wtxid:
                return 'ignored', 'duplicate', []
            self.wtxid = True
            return 'done', '', []
        if command == 'sendaddrv2':
            if self.connected:
                return 'disconnect', 'after verack', []
            self.addrv2 = True
            return 'done', '', []
        if command == 'feature':
            if self.connected:
                return 'disconnect', 'after verack', []
            if self.common < C['FEATURE_VERSION']:
                return 'disconnect', 'feature before version 70017', []
            from io import BytesIO
            f = BytesIO(payload)
            try:
                feature = M.deser_string(f)
                data = M.deser_string(f)
                ok = 4 <= len(feature) <= C['MAX_FEATUREID_LENGTH'] and len(data) <= C['MAX_FEATUREDATA_LENGTH'] and not f.read()
            except Exception:
                ok = False
            return ('ignored', 'unknown feature', []) if ok else ('disconnect', 'bad feature payload', [])
        if command == 'sendtxrcncl':
            return 'ignored', 'reconciliation off', []
        if not self.connected:
            return 'ignored', 'before verack', []
        if command == 'ping':
            self.ping = int.from_bytes(payload[:8], 'little')
            if self.common > C['BIP0031_VERSION']:
                self.last_send = now
                return 'done', '', ['pong']
            return 'done', '', []
        return 'deliver', '', []

    def tick(self, now):
        if now <= self.opened + C['CONNECT_TIMEOUT']:
            return 'done', ''
        if self.last_recv < 0 or self.last_send < 0:
            return 'disconnect', 'no message in 60 s'
        if now > self.last_send + C['TIMEOUT_INTERVAL']:
            return 'disconnect', 'send timeout'
        if now > self.last_recv + C['TIMEOUT_INTERVAL']:
            return 'disconnect', 'receive timeout'
        if not self.connected:
            return 'disconnect', 'version handshake timeout'
        return 'done', ''


class Wire:
    """V2Transport's receive states, or V1Transport's frames."""

    def __init__(self, R, initiating, v2, key, rnd, garbage):
        self.R, self.initiating, self.garbage = R, initiating, garbage
        self.state = 'v1' if not v2 else ('key' if initiating else 'maybe_v1')
        self.sent = 0
        if v2:
            self.key = key
            self.ours = E.encode(R, (int.from_bytes(key, 'big') * R.SECP256K1_G).x, rnd)

    def key_bytes(self):
        out = self.ours + self.garbage
        self.sent += len(out)
        return out

    def ready_bytes(self):
        out = self.cipher['send_garbage_terminator'] + self.R.v2_enc_packet(self.cipher, b'', aad=self.garbage)
        self.sent += len(out)
        return out

    def frame(self, command, payload):
        if self.state == 'v1':
            out = MAGIC + command.encode().ljust(12, b'\0') + len(payload).to_bytes(4, 'little') + p2p.sha256d(payload)[:4] + payload
        else:
            ids = T.message_ids()
            contents = (bytes([ids.index(command)]) if command in ids else b'\0' + command.encode().ljust(12, b'\0')) + payload
            out = self.R.v2_enc_packet(self.cipher, contents)
        self.sent += len(out)
        return out

    def read(self, b):
        """(event, used, command, payload)."""
        if self.state == 'maybe_v1':
            prefix = MAGIC + b'version' + bytes(5)
            n = min(len(b), 16)
            if b[:n] != prefix[:n]:
                self.state = 'key'
                return 'send key', 0, None, None
            if n < 16:
                return 'wait', 0, None, None
            self.state = 'v1'
            return 'v1', 0, None, None
        if self.state == 'v1':
            if len(b) < 24:
                return 'wait', 0, None, None
            if b[:4] != MAGIC:
                return 'refuse bad magic', 0, None, None
            n = int.from_bytes(b[16:20], 'little')
            if len(b) < 24 + n:
                return 'wait', 0, None, None
            assert p2p.sha256d(b[24:24 + n])[:4] == b[20:24]
            return 'message', 24 + n, b[4:16].rstrip(b'\0').decode(), b[24:24 + n]
        if self.state == 'key':
            if len(b) < 64:
                return 'wait', 0, None, None
            secret = self.R.v2_ecdh(self.key, b[:64], self.ours, self.initiating)
            self.cipher = self.R.initialize_v2_transport(secret, self.initiating)
            self.state = 'garbage'
            return 'keys', 64, None, None
        skip, aad = 0, b''
        if self.state == 'garbage':
            code, g = T.garbage_search(b, self.cipher['recv_garbage_terminator'], 4095)
            if code == 'truncated':
                return 'wait', 0, None, None
            assert code == 'ok'
            self.state, self.their_garbage = 'version', g
        if self.state == 'version':
            skip, aad = self.their_garbage + 16, b[:self.their_garbage]
        p = b[skip:]
        if len(p) < 3:
            return 'wait', 0, None, None
        n = int.from_bytes(self.cipher['recv_L'].crypt(p[:3]), 'little') if not hasattr(self, 'pending') else self.pending
        if len(p) < n + 20:
            self.pending = n
            return 'wait', 0, None, None
        if hasattr(self, 'pending'):
            del self.pending
        plain = self.cipher['recv_P'].decrypt(aad, p[3:n + 20])
        assert plain is not None
        used = skip + n + 20
        if plain[0] & 0x80:
            return 'decoy', used, None, None
        if self.state == 'version':
            self.state = 'app'
            return 'ready', used, None, None
        command, at = T.message_type(plain[1:], T.message_ids())
        return 'message', used, command, plain[1 + at:]


class Side:
    def __init__(self, name):
        self.name, self.inbox, self.got, self.queued, self.closed = name, b'', 0, [], False


def transcript():
    R = E.load_reference()
    lines = []
    out = lines.append

    def opened(a_v2, b_v2):
        a, b = Side('A'), Side('B')
        a.wire = Wire(R, True, a_v2, (1).to_bytes(32, 'big'), bytes(range(32)), b'hello')
        b.wire = Wire(R, False, b_v2, (2).to_bytes(32, 'big'), bytes(range(32, 64)), bytes([1, 2, 3]))
        b.peer = Peer(INBOUND, 0x1112131415161718, OURS, 1000)
        a.peer = Peer(OUTBOUND, 0x0102030405060708, OURS, 1000)
        if a_v2:
            k = a.wire.key_bytes()
            b.inbox += k
            b.got += len(k)
            out(f'  A sends its key and 5 bytes of garbage: {len(k)} bytes')
        send(a, b, a.peer.open())
        return a, b

    def send(src, dst, sends):
        if src.wire.state in ('key', 'maybe_v1'):
            src.queued += sends
            return
        for command in sends:
            frame = src.wire.frame(command, src.peer.payload(command))
            dst.inbox += frame
            dst.got += len(frame)

    def pump(me, them):
        progress = False
        while not me.closed:
            event, used, command, payload = me.wire.read(me.inbox)
            if event == 'wait':
                break
            progress = True
            if event == 'v1':
                out(f'  {me.name}: the first 16 bytes are a v1 version, so v1 it is')
            elif event == 'send key':
                k = me.wire.key_bytes()
                them.inbox += k
                them.got += len(k)
                out(f'  {me.name}: not a v1 version; sends its key and garbage: {len(k)} bytes')
            elif event == 'keys':
                k = me.wire.ready_bytes()
                them.inbox += k
                them.got += len(k)
                out(f'  {me.name}: has the peer\'s key; sends its terminator and version packet: {len(k)} bytes')
                queued, me.queued = me.queued, []
                send(me, them, queued)
            elif event == 'ready':
                out(f'  {me.name}: the peer\'s version packet, after its garbage: v2 is up')
            elif event == 'message':
                verdict, reason, sends = me.peer.receive(command, payload, 1001)
                out(f'  {me.name} <- {command}, {used} bytes: {verdict}' + (f'; sends {" ".join(sends)}' if sends else ''))
                send(me, them, sends)
                if verdict == 'disconnect':
                    me.closed = True
            else:
                out(f'  {me.name}: disconnects ({event.split(" ", 1)[1]})')
                me.closed = True
            me.inbox = me.inbox[used:]
        return progress

    def run(a, b):
        for _ in range(20):
            x = pump(b, a)
            y = pump(a, b)
            if not x and not y:
                break

    def summary(s):
        p = s.peer
        t = lambda v: 'true' if v else 'false'  # noqa: E731
        out(f'  {s.name}: connected {t(p.connected)}, common version {p.common}, wtxidrelay {t(p.wtxid)}, '
            f'addrv2 {t(p.addrv2)}, compact blocks {t(p.cmpct)} (high bandwidth {t(p.high)}), {s.got} bytes received')

    out('v1: outbound A, inbound B')
    a, b = opened(False, False)
    run(a, b)
    summary(a)
    summary(b)
    out('v2: both sides speak BIP-324')
    a, b = opened(True, True)
    run(a, b)
    summary(a)
    summary(b)
    same = a.wire.cipher['session_id'] == b.wire.cipher['session_id']
    out(f'  session id {a.wire.cipher["session_id"].hex()}, the same on both sides: {"true" if same else "false"}')
    out('v1 A, v2 B: B detects v1')
    a, b = opened(False, True)
    run(a, b)
    summary(a)
    summary(b)
    out('v2 A, v1-only B: A reconnects in v1')
    a, b = opened(True, False)
    run(a, b)
    # ShouldReconnectV1: initiating, still awaiting the key, nothing received, >= 24 bytes sent.
    again = a.wire.initiating and a.wire.state == 'key' and a.got == 0 and a.wire.sent >= 24
    out(f'  A sent {a.wire.sent}, received {a.got}: reconnect in v1 {"true" if again else "false"}')
    a, b = opened(False, False)
    run(a, b)
    summary(a)
    summary(b)

    out("the version checks, in Core's order")

    def check(label, conn, version, services):
        p = Peer(conn, 1, WITNESS_FULL, 1000)
        verdict, reason, sends = p.receive('version', their_version(version, services), 1001)
        out(f'  {label}: {verdict}' + (f' ({reason})' if reason else '') + (f'; sends {" ".join(sends)}' if sends else ''))

    N, W, L = C['NODE_NETWORK'], C['NODE_WITNESS'], C['NODE_NETWORK_LIMITED']
    check('outbound, 70017, NETWORK|WITNESS', OUTBOUND, 70017, N | W)
    check('outbound, 70017, NETWORK only', OUTBOUND, 70017, N)
    check('outbound, 31799, NETWORK|WITNESS', OUTBOUND, 31799, N | W)
    check('outbound, 31799, NETWORK only', OUTBOUND, 31799, N)
    check('outbound, 70017, NETWORK_LIMITED|WITNESS', OUTBOUND, 70017, L | W)
    check('inbound, 70017, no services', INBOUND, 70017, 0)
    check('block-relay-only, 70017, NETWORK|WITNESS', BLOCK_RELAY, 70017, N | W)
    check('feeler, 70017, NETWORK|WITNESS', FEELER, 70017, N | W)
    check('manual, 70015', MANUAL, 70015, 0)
    check('manual, 70012', MANUAL, 70012, 0)
    ours = Peer(OUTBOUND, 0x0102030405060708, WITNESS_FULL, 1000).payload('version')
    p = Peer(INBOUND, 99, WITNESS_FULL, 1000)
    verdict, reason, _ = p.receive('version', ours, 1001, pending=(0x0102030405060708,))
    out(f'  inbound, carrying our pending nonce: {verdict} ({reason})')

    out('messages around verack')

    def after(label, verack, command, payload):
        p = Peer(OUTBOUND, 1, WITNESS_FULL, 1000)
        p.receive('version', their_version(70017, N | W), 1001)
        if verack:
            p.receive('verack', b'', 1001)
        verdict, reason, sends = p.receive(command, payload, 1002)
        out(f'  {label}: {verdict}' + (f' ({reason})' if reason else '') + (f'; sends {" ".join(sends)}' if sends else ''))

    ping = ours[:8]
    verdict, reason, _ = Peer(OUTBOUND, 1, 0, 1000).receive('ping', ping, 1001)
    out(f'  ping with no version yet: {verdict} ({reason})')
    after('ping before verack', False, 'ping', ping)
    after('ping after verack', True, 'ping', ping)
    after('wtxidrelay before verack', False, 'wtxidrelay', b'')
    after('wtxidrelay after verack', True, 'wtxidrelay', b'')
    after('sendaddrv2 after verack', True, 'sendaddrv2', b'')
    after('sendheaders before verack', False, 'sendheaders', b'')
    after('sendcmpct 1, version 2, after verack', True, 'sendcmpct', M.msg_sendcmpct(True, 2).serialize())
    after('sendcmpct 1, version 1', True, 'sendcmpct', M.msg_sendcmpct(True, 1).serialize())
    after('sendcmpct 2, version 2', True, 'sendcmpct', bytes([2]) + (2).to_bytes(8, 'little'))
    feature = M.msg_feature('test', b'\x00').serialize()
    after('feature "test" before verack', False, 'feature', feature)
    after('feature "test" after verack', True, 'feature', feature)
    after('sendtxrcncl before verack', False, 'sendtxrcncl', ours[:12])
    after('headers after verack', True, 'headers', b'')
    after('version again', True, 'version', their_version(70017, N | W))

    out('timeouts')
    for t, stage in [(1060, 0), (1061, 0), (1061, 1), (1061, 2), (2201, 2), (2202, 2)]:
        p = Peer(OUTBOUND, 1, 0, 1000)
        if stage >= 1:
            p.receive('version', their_version(70017, N | W), 1001)
        if stage >= 2:
            p.receive('verack', b'', 1001)
        verdict, reason = p.tick(t)
        label = ['nothing received', 'version only', 'connected'][stage]
        out(f'  {label}, at {t - 1000} s: {verdict}' + (f' ({reason})' if reason else ''))
    return '\n'.join(lines) + '\n'


def main():
    want = transcript()
    if '--write' in sys.argv:
        FIXTURE.write_text(want)
    assert FIXTURE.read_text() == want, 'the fixture differs from the Python transcript'
    print(f'handshake fixture: {want.count(chr(10))} lines agree with Core\'s sources, messages.py and BIP-324\'s reference.py')
    if '--coax' in sys.argv:
        coax = sys.argv[sys.argv.index('--coax') + 1]
        for mode in ([], ['--release']):
            run = subprocess.run([coax, 'run', EXAMPLE] + mode, cwd=REPO, capture_output=True, text=True)
            assert run.returncode == 0 and run.stdout == want, f'native {mode or "debug"} output differs'
            print(f'native {"release" if mode else "debug"}: agrees')


if __name__ == '__main__':
    main()
