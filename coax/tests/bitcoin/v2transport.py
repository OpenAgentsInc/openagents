#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: BIP-324's reference.py and Bitcoin Core's pinned sources decide each case outside Coax)
"""The cross-check of Bitcoin chapter 67. It computes every line the
chapter's example prints and compares the transcript with the fixture:

  python3 -I coax/tests/bitcoin/v2transport.py                     # compare with the fixture
  python3 -I coax/tests/bitcoin/v2transport.py --coax coax/bin/coax   # also run the example

The keys, both rekeying ciphers and the packets are BIP-324's reference.py
(initialize_v2_transport, FSChaCha20, FSChaCha20Poly1305, v2_enc_packet),
pinned, sharing no code with Coax. The receiving side, which reference.py
writes against a socket, is its decryption steps called directly. The
message-ID table is read from the pinned net.cpp and protocol.h, the limits
from net.h, net.cpp and serialize.h, and the garbage search and message-type
rules are transliterated from net.cpp's ProcessReceivedGarbageBytes and
GetMessageType.

BIP-324's reference ChaCha20 is plain Python, so the 4 MB and 16 MiB rows
take a minute or two.
"""
import hashlib
import re
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import ellswift as E  # noqa: E402
import make_ellswift_vectors as V  # noqa: E402

REPO = HERE.parents[2]
CORE = HERE.parent / 'vectors/upstream/bitcoin-core'
FIXTURE = REPO / 'bitcoin/fixtures/v2-transport.out'
EXAMPLE = 'bitcoin/examples/v2-transport.coax'
STACK_KIB = 24576
PINS = {
    'net.h': 'c268325ae03f4f80e8427af80eb794265b08e3686923b7a5a8b850416bed213b',
    'net.cpp': '05e8ce20762e9c23e060d428fb5218cb32fea599afbd40b2417b9bd2a236d7a6',
    'protocol.h': '92d6290df90eaea49b4a110b518ff34be914666471a5dee7e5eaa26832986738',
    'serialize.h': '9acb4bab2fe63ef351429b19d6ed75a9e719705060999c4e5dba78ce7c08db6f',
}
MAGIC = bytes.fromhex('f9beb4d9')
CODES = ['ok', 'truncated', 'no terminator', 'too large', 'bad tag', 'bad type', 'capacity', 'keys']


def core(name):
    data = (CORE / name).read_bytes()
    assert hashlib.sha256(data).hexdigest() == PINS[name], f'{name} differs from its pin'
    return data.decode()


def limits():
    net_h, net_cpp, serialize = core('net.h'), core('net.cpp'), core('serialize.h')
    garbage = int(re.search(r'MAX_GARBAGE_LEN = (\d+);', net_h).group(1))
    product = re.search(r'MAX_PROTOCOL_MESSAGE_LENGTH = ([\d *]+);', net_h).group(1)
    protocol = 1
    for factor in product.split('*'):
        protocol *= int(factor)
    max_size = int(re.search(r'MAX_SIZE = (0x[0-9a-fA-F]+);', serialize).group(1), 16)
    assert 'MAX_CONTENTS_LEN =\n        1 + CMessageHeader::MESSAGE_TYPE_SIZE +' in net_cpp
    return garbage, 1 + 12 + min(max_size, protocol)


def message_ids():
    """V2_MESSAGE_IDS from net.cpp, with each NetMsgType name read from protocol.h."""
    net_cpp, protocol_h = core('net.cpp'), core('protocol.h')
    names = dict(re.findall(r'inline constexpr const char\* (\w+)\{"([^"]*)"\};', protocol_h))
    body = net_cpp.split('V2_MESSAGE_IDS = {')[1].split('};')[0]
    ids = []
    for line in body.split('\n'):
        line = line.split('//')[0]
        for token in re.findall(r'NetMsgType::\w+|""', line):
            ids.append('' if token == '""' else names[token.split('::')[1]])
    return ids


def message_type(contents, ids):
    """GetMessageType: (command, payload start) or None."""
    if not contents:
        return None
    if contents[0] != 0:
        return (ids[contents[0]], 1) if contents[0] < len(ids) else None
    if len(contents) < 13:
        return None
    n = 0
    while n < 12 and contents[1 + n] != 0:
        if not 0x20 <= contents[1 + n] <= 0x7e:
            return None
        n += 1
    if any(contents[1 + i] for i in range(n, 12)):
        return None
    return contents[1:1 + n].decode(), 13


def garbage_search(received, terminator, max_garbage):
    """ProcessReceivedGarbageBytes, one byte at a time: (code, garbage length)."""
    for end in range(16, len(received) + 1):
        if received[end - 16:end] == terminator:
            return 'ok', end - 16
        if end == max_garbage + 16:
            return 'no terminator', 0
    return 'truncated', 0


def main_transcript():
    R = E.load_reference()
    max_garbage, max_contents = limits()
    ids = message_ids()
    lines = []
    out = lines.append

    def receive(peer, aad, packet):
        """Decrypt one packet: (code, length, ignore, contents)."""
        n = int.from_bytes(peer['recv_L'].crypt(packet[:3]), 'little')
        if n > max_contents:
            return 'too large', n, False, b''
        if n + 20 > len(packet):
            return 'truncated', n, False, b''
        plain = peer['recv_P'].decrypt(aad, packet[3:n + 20])
        if plain is None:
            return 'bad tag', n, False, b''
        return 'ok', n, bool(plain[0] & 0x80), plain[1:]

    out('packet vectors')
    for k, r in enumerate(V.rows('packet_encoding_test_vectors.csv')):
        priv, ours, theirs = (bytes.fromhex(r[c]) for c in ('in_priv_ours', 'in_ellswift_ours', 'in_ellswift_theirs'))
        initiating = r['in_initiating'] == '1'
        secret = R.v2_ecdh(priv, theirs, ours, initiating)
        c = R.initialize_v2_transport(secret, initiating)
        d = R.initialize_v2_transport(secret, not initiating)
        keys = (c['session_id'].hex() == r['out_session_id']
                and c['send_garbage_terminator'].hex() == r['mid_send_garbage_terminator']
                and c['recv_garbage_terminator'].hex() == r['mid_recv_garbage_terminator'])
        for _ in range(int(r['in_idx'])):
            code, _, ignore, _ = receive(d, b'', R.v2_enc_packet(c, b'', ignore=True))
            keys = keys and code == 'ok' and ignore
        contents = bytes.fromhex(r['in_contents']) * int(r['in_multiply'])
        aad, ignore = bytes.fromhex(r['in_aad']), r['in_ignore'] == '1'
        packet = R.v2_enc_packet(c, contents, aad=aad, ignore=ignore)
        whole = r['out_ciphertext'] != ''
        match = packet.hex() == r['out_ciphertext'] if whole else packet[-128:].hex() == r['out_ciphertext_endswith']
        code, n, got_ignore, got = receive(d, aad, packet)
        if code == 'ok' and (got != contents or got_ignore != ignore):
            code = 'bad type'
        size = len(contents)
        out(f'  row {k}: packet {r["in_idx"]} from the {"initiator" if initiating else "responder"}, '
            f'{size} byte{"" if size == 1 else "s"} {"(a decoy)" if ignore else "(data)"}: '
            f'keys {"true" if keys else "false"}, {"ciphertext" if whole else "last 128 bytes"} '
            f'{"true" if match else "false"}; opened: {code}')

    out('a handshake')
    ka, kb = (1).to_bytes(32, 'big'), (2).to_bytes(32, 'big')
    ea = E.encode(R, (1 * R.SECP256K1_G).x, bytes(range(32)))
    eb = E.encode(R, (2 * R.SECP256K1_G).x, bytes(range(32, 64)))
    garbage_a = b'garbage'
    wire = ea + garbage_a
    out(f'  initiator sends {len(wire)} bytes: its key and {len(garbage_a)} bytes of garbage')
    v1 = wire[:16] == MAGIC + b'version' + bytes(5)
    out(f'  the responder\'s v1 check on the first 16 bytes: {"true" if v1 else "false"}')
    pb = R.initialize_v2_transport(R.v2_ecdh(kb, wire[:64], eb, False), False)
    back = eb + pb['send_garbage_terminator'] + R.v2_enc_packet(pb, b'', aad=b'')
    out(f'  responder sends {len(back)} bytes: key, terminator, version packet')
    pa = R.initialize_v2_transport(R.v2_ecdh(ka, back[:64], ea, True), True)
    code, g = garbage_search(back[64:], pa['recv_garbage_terminator'], max_garbage)
    out(f'  initiator finds the terminator: {code}, after {g} bytes of garbage')
    code, n, _, _ = receive(pa, back[64:64 + g], back[64 + g + 16:])
    out(f'  and reads the version packet: {code}, {n} bytes of contents')
    same = pa['session_id'] == pb['session_id']
    out(f'  both sides have session id {pa["session_id"].hex()}: {"true" if same else "false"}')
    decoy = R.v2_enc_packet(pa, bytes(10), aad=garbage_a, ignore=True)
    version = R.v2_enc_packet(pa, b'')
    wire += pa['send_garbage_terminator'] + decoy + version
    code, g = garbage_search(wire[64:], pb['recv_garbage_terminator'], max_garbage)
    out(f'  responder finds the terminator after {g} bytes of garbage')
    code, n, ignore, _ = receive(pb, wire[64:64 + g], decoy)
    out(f'  a decoy: {code}, {n} bytes, ignore {"true" if ignore else "false"}')
    code, n, ignore, _ = receive(pb, b'', version)
    out(f'  the version packet: {code}, ignore {"true" if ignore else "false"}')

    out('messages')
    nonce = bytes(range(1, 9))
    ping = bytes([ids.index('ping')]) + nonce
    packet = R.v2_enc_packet(pa, ping)
    out(f'  ping: short ID {ping[0]}, {len(ping)} bytes of contents, {len(packet)} bytes on the wire (v1: {24 + len(nonce)})')
    code, n, _, got = receive(pb, b'', packet)
    command, at = message_type(got, ids)
    out(f'  received: {code}, message {command}, payload {got[at:].hex()}')
    assert 'sendaddrv2' not in ids
    contents = b'\x00' + b'sendaddrv2'.ljust(12, b'\x00')
    packet = R.v2_enc_packet(pa, contents)
    out(f'  sendaddrv2 has no short ID: {len(contents)} bytes of contents, {len(packet)} on the wire')
    code, n, _, got = receive(pb, b'', packet)
    out(f'  received: {code}, message {message_type(got, ids)[0]}')
    out(f'  contents with ID 200: {"ok" if message_type(bytes([200]), ids) else "bad type"}')

    out('rekeying')
    key, lkey, sent = pa['send_P'].key, pa['send_L'].key, pa['send_P'].packet_counter
    while pa['send_P'].packet_counter % 224:
        receive(pb, b'', R.v2_enc_packet(pa, b'', ignore=True))
    now = pa['send_P'].packet_counter
    out(f'  the initiator had sent {sent} packets; {now - sent} decoys later it has sent {now}')
    out(f'  the packet key changed: {"true" if key != pa["send_P"].key else "false"}, '
        f'the length key changed: {"true" if lkey != pa["send_L"].key else "false"}')
    packet = R.v2_enc_packet(pa, ping)
    out(f'  packet 224, the first with the new keys: {packet.hex()}')
    code, n, _, got = receive(pb, b'', packet)
    out(f'  the responder opens it: {code}, message {ids[got[0]]}')
    code, n, _, _ = receive(pb, b'', packet)
    out(f'  the same packet replayed: {code}, its length reads as {n}')
    return '\n'.join(lines) + '\n'


def main():
    want = main_transcript()
    if '--write' in sys.argv:
        FIXTURE.write_text(want)
    assert FIXTURE.read_text() == want, 'the fixture differs from the Python transcript'
    print(f'v2-transport fixture: {want.count(chr(10))} lines agree with BIP-324\'s reference.py and Core\'s sources')
    if '--coax' in sys.argv:
        coax = sys.argv[sys.argv.index('--coax') + 1]
        # The example holds a 16 MiB packet on its stack, as examples.json's stack_kib says.
        script = 'ulimit -S -s "$1"; shift; exec "$@"'
        for mode in ([], ['--release']):
            cmd = ['/bin/sh', '-c', script, 'sh', str(STACK_KIB), coax, 'run', EXAMPLE] + mode
            run = subprocess.run(cmd, cwd=REPO, capture_output=True, text=True)
            assert run.returncode == 0 and run.stdout == want, f'native {mode or "debug"} output differs'
            print(f'native {"release" if mode else "debug"}: agrees')


if __name__ == '__main__':
    main()
