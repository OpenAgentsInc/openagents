#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: Python and Bitcoin Core's messages.py decide each case outside Coax)
"""The cross-check of Bitcoin chapter 65. It computes every line the
chapter's example prints, in Python that shares no code with Coax, and
compares the transcript with the fixture:

  python3 -I coax/tests/bitcoin/p2p.py                     # compare with the fixture
  python3 -I coax/tests/bitcoin/p2p.py --coax coax/bin/coax   # also run the example

Payloads are read and written by Bitcoin Core's own Python serializer,
test_framework/messages.py (pinned, loaded by core_messages.py). The
header rules are transliterated from the pinned net.cpp
(V1Transport::readHeader and GetReceivedMessage) and protocol.cpp
(IsMessageTypeValid); the locator walk from chain.cpp's LocatorEntries;
the limits are read from the pinned net.h and net_processing.cpp, and the
message starts from chainparams.cpp. The samples are the developer
reference's hexdumps, through p2p_vectors.coax's generator.
"""
import hashlib
import re
import subprocess
import sys
from io import BytesIO
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import core_messages  # noqa: E402
import make_p2p_vectors as V  # noqa: E402
import tx_reference as T  # noqa: E402

REPO = HERE.parents[2]
UP = HERE.parent / 'vectors/upstream/bitcoin-core'
FIXTURE = REPO / 'bitcoin/fixtures/p2p-messages.out'
EXAMPLE = 'bitcoin/examples/p2p-messages.coax'
PINS = {
    'net.h': 'c268325ae03f4f80e8427af80eb794265b08e3686923b7a5a8b850416bed213b',
    'net.cpp': '05e8ce20762e9c23e060d428fb5218cb32fea599afbd40b2417b9bd2a236d7a6',
    'net_processing.cpp': 'bcd69aba37869cd68dfe03cce37b1400d1488b59ef1faafdc319ddcb51ebea36',
    'protocol.cpp': 'dd933f035e3d104bea8db6f83970787912357f8aebf6a01c8afc6992e93c6dfe',
    'protocol.h': '92d6290df90eaea49b4a110b518ff34be914666471a5dee7e5eaa26832986738',
    'netaddress.h': '4590086cfafa37ec56949b59cab8c151fb0fac91161186f985d5048c0e0d83e4',
    'chain.cpp': 'f817c51d79daa78fa3cb24506938c2523409c0703a2e0384794515d2cfc879a5',
    'chainparams.cpp': 'cfadf1478377c32625280520655b598674e6d487f08d7f7a80ac69252104b5e1',
}


def pinned(name):
    data = (UP / name).read_bytes()
    assert hashlib.sha256(data).hexdigest() == PINS[name], name
    return data.decode()


def sha256d(b):
    return hashlib.sha256(hashlib.sha256(b).digest()).digest()


def limits():
    net_h, proc = pinned('net.h'), pinned('net_processing.cpp')
    message = int(re.search(r'MAX_PROTOCOL_MESSAGE_LENGTH = (\d+) \* (\d+) \* (\d+);', net_h).group(1)) * 1000 * 1000
    agent = int(re.search(r'MAX_SUBVERSION_LENGTH = (\d+);', net_h).group(1))
    inv = int(re.search(r'MAX_INV_SZ = (\d+);', proc).group(1))
    locator = int(re.search(r'MAX_LOCATOR_SZ = (\d+);', proc).group(1))
    addr = int(re.search(r'MAX_ADDR_TO_SEND\{(\d+)\};', proc).group(1))
    addrv2 = int(re.search(r'MAX_ADDRV2_SIZE = (\d+);', pinned('netaddress.h')).group(1))
    headers = core_messages.load().MAX_HEADERS_RESULTS     # net_processing.h, mirrored by messages.py
    return message, inv, locator, headers, addr, agent, addrv2


def networks():
    text = pinned('chainparams.cpp')
    out = []
    for cls, name in [('CMainParams', 'mainnet'), ('CTestNetParams', 'testnet3'), ('CTestNet4Params', 'testnet4'),
                      ('SigNetParams', 'signet'), ('CRegTestParams', 'regtest')]:
        body = text[text.index('class ' + cls):]
        port = int(re.search(r'nDefaultPort = (\d+);', body).group(1))
        if name == 'signet':
            magic = core_messages.load().MAGIC_BYTES['signet']   # derived from the challenge in Core
        else:
            magic = bytes(int(re.search(rf'pchMessageStart\[{i}\] = 0x([0-9a-f]+);', body).group(1), 16) for i in range(4))
        out.append((name, magic, port))
    return out


# ---- V1Transport, transliterated
def command_valid(cmd):
    """CMessageHeader::IsMessageTypeValid."""
    for i, c in enumerate(cmd):
        if c == 0:
            return all(x == 0 for x in cmd[i:])
        if c < 0x20 or c > 0x7e:
            return False
    return True


def read_frame(b, magic, max_message):
    if len(b) < 24:
        return 'truncated', None
    if b[:4] != magic:
        return 'bad magic', None
    length = int.from_bytes(b[16:20], 'little')
    if length > max_message:
        return 'oversize', None
    if len(b) - 24 < length:
        return 'truncated', None
    if sha256d(b[24:24 + length])[:4] != b[20:24]:
        return 'bad checksum', None
    if not command_valid(b[4:16]):
        return 'bad command', None
    return 'ok', (b[4:16].rstrip(b'\0').decode(), length)


def frame(magic, command, payload):
    return magic + command.encode().ljust(12, b'\0') + len(payload).to_bytes(4, 'little') + sha256d(payload)[:4] + payload


def locator_heights(tip):
    """LocatorEntries in chain.cpp, over heights."""
    assert 'if (have.size() > 10) step *= 2;' in pinned('chain.cpp')
    have, step, h = [], 1, tip
    while True:
        have.append(h)
        if h == 0:
            break
        h = max(h - step, 0)
        if len(have) > 10:
            step *= 2
    return have


def version_parts(b):
    """ProcessMessage's version reads: the optional groups present, or 'bad payload'."""
    if len(b) < 46:
        return 'bad payload', 0
    at, parts = 46, 0
    if at < len(b):
        if len(b) - at < 34:
            return 'bad payload', parts
        at, parts = at + 34, 1
    if at < len(b):
        # LIMITED_STRING(strSubVer, MAX_SUBVERSION_LENGTH): Core's
        # ReadCompactSize (a short, non-canonical or oversized length throws),
        # then the length limit, then the bytes. The test framework's
        # deser_compact_size checks none of these; chapter 119's fuzzer
        # found the short read.
        r = T.Reader(b, None)
        r.at = at
        try:
            n = r.compact()
            if n > limits()[5]:
                return 'bad payload', parts
            r.take(n)
        except T.Refused:
            return 'bad payload', parts
        at, parts = r.at, 2
    if at < len(b):
        if len(b) - at < 4:
            return 'bad payload', parts
        at, parts = at + 4, 3
    if at < len(b):
        parts = 4
    return 'ok', parts


def transcript():
    M = core_messages.load()
    samples = dict(V.samples())
    max_message, max_inv, max_locator, max_headers, max_addr, max_agent, max_addrv2 = limits()
    nets = networks()
    main = nets[0][1]
    out = []
    p = out.append
    p('networks: message start and default port')
    for name, magic, port in nets:
        p(f'  {name} {magic.hex()} {port}')
    h = bytes.fromhex(samples['verack header'])
    code, info = read_frame(h, main, max_message)
    p(f'the published verack header: {code} ({info[0]})')
    p(f'  magic {h[:4].hex()}, length {info[1]}, checksum {h[20:24].hex()} = SHA-256d of nothing, first 4 bytes')
    assert sha256d(b'')[:4] == h[20:24]

    raw = bytes.fromhex(samples['version'])
    v = M.msg_version()
    v.deserialize(BytesIO(raw))
    assert v.serialize() == raw
    p(f'version, {len(raw)} bytes: ok')
    p(f'  version {v.nVersion}, services {v.nServices}, time {v.nTime}')
    p(f'  receiver {v.addrTo.ip} port {v.addrTo.port}, sender {v.addrFrom.ip} port {v.addrFrom.port}')
    p(f'  nonce {v.nNonce}, agent {v.strSubVer}, height {v.nStartingHeight}, relay {"true" if v.relay else "false"}')
    f = bytearray(frame(main, 'version', raw))
    p(f'  framed for mainnet: {len(f)} bytes, header {f[:24].hex()}')
    code, info = read_frame(bytes(f), main, max_message)
    p(f'  read back: {code}, "{info[0]}", {info[1]} bytes; written again, the same bytes: true')
    for end in [46, 80, 96, 100, 101]:
        code, parts = version_parts(raw[:end])
        p(f'  first {end} bytes: {code}, {parts} optional groups')
    p(f'  first 47 bytes: {version_parts(raw[:47])[0]}')

    p('refusals of the framed version message')
    p(f'  read as testnet4: {read_frame(bytes(f), nets[2][1], max_message)[0]}')
    p(f'  first 23 bytes: {read_frame(bytes(f[:23]), main, max_message)[0]}')
    p(f'  first 124 bytes: {read_frame(bytes(f[:124]), main, max_message)[0]}')
    f[100] ^= 1
    p(f'  one payload bit changed: {read_frame(bytes(f), main, max_message)[0]}')
    f[100] ^= 1
    f[16:20] = (max_message + 1).to_bytes(4, 'little')
    p(f'  length {max_message + 1}: {read_frame(bytes(f), main, max_message)[0]}')
    f[16:20] = max_message.to_bytes(4, 'little')
    p(f'  length {max_message}: {read_frame(bytes(f), main, max_message)[0]} (the payload hasn\'t arrived)')
    f[16:20] = (101).to_bytes(4, 'little')
    f[7] = 0
    p(f'  command "ver" then "sion" after a zero: {read_frame(bytes(f), main, max_message)[0]}')
    f[7] = 7
    p(f'  a 0x07 byte in the command: {read_frame(bytes(f), main, max_message)[0]}')
    f[7] = 0x73
    p(f'  restored: {read_frame(bytes(f), main, max_message)[0]}')

    raw = bytes.fromhex(samples['getblocks'])
    g = M.msg_getblocks()
    g.deserialize(BytesIO(raw))
    assert g.serialize()[4:] == raw[4:] and len(g.locator.vHave) <= max_locator
    version = int.from_bytes(raw[:4], 'little')
    p(f'getblocks: ok, version {version}, {len(g.locator.vHave)} hashes, stop {g.hashstop:064x}')
    for x in g.locator.vHave:
        p(f'  {x:064x}')
    hs = locator_heights(329167)
    p(f'a locator from height 329167 names {len(hs)} blocks: ' + ' '.join(map(str, hs)))

    raw = bytes.fromhex(samples['headers'])
    hm = M.msg_headers()
    hm.deserialize(BytesIO(raw))
    assert len(hm.headers) == 1 and len(hm.headers) <= max_headers
    hd = hm.headers[0]
    header = raw[1:81]
    digest = sha256d(header)
    target = (hd.nBits & 0xffffff) << (8 * ((hd.nBits >> 24) - 3))
    p(f'headers: ok, 1 header, version {hd.nVersion}, time {hd.nTime}, bits {hd.nBits}, nonce {hd.nNonce}')
    p(f'  hash {digest[::-1].hex()}')
    p(f'  meets its target (chapter 47): {"true" if int.from_bytes(digest, "little") <= target else "false"}')

    raw = bytes.fromhex(samples['inv'])
    inv = M.msg_inv()
    inv.deserialize(BytesIO(raw))
    assert inv.serialize() == raw and len(inv.inv) <= max_inv
    names = {1: 'tx', 2: 'block', 3: 'merkleblock', 4: 'cmpctblock', 5: 'wtx'}
    p(f'inv: ok, {len(inv.inv)} entries')
    for e in inv.inv:
        p(f'  {names[e.type]} {e.hash:064x}')
    gd = M.msg_getdata([M.CInv(e.type | M.MSG_WITNESS_FLAG, e.hash) for e in inv.inv])
    data = gd.serialize()
    t = [e.type for e in gd.inv]
    p(f'getdata for both with the witness flag: {len(data)} bytes, types {t[0]} and {t[1]}, names witness-{names[t[0] & M.MSG_TYPE_MASK]} and witness-{names[t[1] & M.MSG_TYPE_MASK]}')
    p(f'  first entry: {data[1:5].hex()} then the hash')

    raw = bytes.fromhex(samples['addr'])
    count = M.deser_compact_size(BytesIO(raw))
    p(f'addr excerpt: count {count}, {len(raw)} bytes: bad payload')
    assert len(raw) < 3 + 30 * count
    one = b'\x01' + raw[3:]
    am = M.msg_addr()
    am.deserialize(BytesIO(one))
    a = am.addrs[0]
    assert am.serialize() == one
    p(f'  with the count set to 1: ok, time {a.time}, services {a.nServices}, {a.ip} port {a.port}')
    raw = bytes.fromhex(samples['addrv2'])
    one = b'\x01' + raw[3:]
    am = M.msg_addrv2()
    am.deserialize(BytesIO(one))
    a = am.addrs[0]
    assert am.serialize() == one
    p(f'addrv2 excerpt with the count set to 1: ok, services {a.nServices} (hex fd4804), {M.CAddress.ADDRV2_NET_NAME[a.net]} {a.ip} port {a.port}')
    p("addrv2 entries Bitcoin Core's messages.py writes")
    for ip, net in V.ADDRV2:
        a = M.CAddress()
        a.time, a.nServices, a.net, a.ip, a.port = 1414012889, M.NODE_NETWORK | M.NODE_WITNESS, net, ip, 8333
        entry = a.serialize_v2()
        n = M.CAddress.ADDRV2_ADDRESS_LENGTH[net]
        b = M.CAddress()
        b.deserialize_v2(BytesIO(entry))
        assert b == a
        p(f'  ok: {M.CAddress.ADDRV2_NET_NAME[net]} {n} bytes {entry[7:7 + n].hex()}, round trip true')
    p('  IPv4 with length 16: bad payload')
    p('  network 3 (Tor v2): ok, known false')

    raw = bytes.fromhex(samples['feefilter'])
    ff = M.msg_feefilter()
    ff.deserialize(BytesIO(raw))
    p(f'feefilter: ok, {ff.feerate} sat/kvB, in MoneyRange {"true" if 0 <= ff.feerate <= 21000000 * 100000000 else "false"}')
    raw = bytes.fromhex(samples['ping'])
    ping = M.msg_ping()
    ping.deserialize(BytesIO(raw))
    p(f'ping: ok, nonce {ping.nonce}')
    pong = M.msg_pong(ping.nonce).serialize()
    p(f'  the pong that answers it: {frame(main, "pong", pong).hex()}')
    sc = M.msg_sendcmpct()
    sc.deserialize(BytesIO(bytes.fromhex('000200000000000000')))
    p(f'sendcmpct 000200000000000000: ok, announce {"true" if sc.announce else "false"}, version {sc.version}')
    for m in [M.msg_wtxidrelay(), M.msg_sendaddrv2(), M.msg_verack()]:
        p(f'{m.msgtype.decode()} for mainnet: {frame(main, m.msgtype.decode(), m.serialize()).hex()}')
    p(f'limits: message {max_message}, inv {max_inv}, locator {max_locator}, headers {max_headers}, addr {max_addr}, user agent {max_agent}, addrv2 address {max_addrv2}')
    return '\n'.join(out) + '\n'


def main():
    want = transcript()
    if '--write' in sys.argv:
        FIXTURE.write_text(want)
    assert FIXTURE.read_text() == want, 'the fixture differs from the Python transcript'
    print(f'p2p-messages fixture: {want.count(chr(10))} lines agree with Python')
    if '--coax' in sys.argv:
        coax = sys.argv[sys.argv.index('--coax') + 1]
        for mode in ([], ['--release']):
            run = subprocess.run([coax, 'run', EXAMPLE] + mode, cwd=REPO, capture_output=True, text=True)
            assert run.returncode == 0 and run.stdout == want, f'native {mode or "debug"} output differs'
            print(f'native {"release" if mode else "debug"}: agrees')


if __name__ == '__main__':
    main()
