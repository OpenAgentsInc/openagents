#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: Tor's rend-spec, I2P's SAM v3 specification, RFC 1928 and RFC 1929, and Bitcoin Core's pinned netbase.cpp, netaddress.cpp, i2p.cpp and init.cpp decide each case outside Coax)
"""The cross-check of Bitcoin chapter 75. It computes every line the
chapter's example prints, in Python that shares no code with Coax, and
compares the transcript with the fixture:

  python3 -I coax/tests/bitcoin/proxy.py                     # compare with the fixture
  python3 -I coax/tests/bitcoin/proxy.py --coax coax/bin/coax   # also run the example

Base32, base64 and the hashes are Python's own (base64.b32encode,
base64.b64decode with I2P's altchars, hashlib.sha3_256 and sha256). The
onion address follows the pinned rend-spec text; SetTor, SetI2P and
DecodeBase32's padding rules follow netaddress.cpp and strencodings.cpp.
The SOCKS5 client is Socks5 (netbase.cpp), byte for byte against a
proxy written from RFC 1928 and RFC 1929; the SAM exchanges follow
i2p.cpp; reachability follows AppInitMain (init.cpp) and ConnectNode
(net.cpp). The script checks the pinned sources say what it assumes.
"""
import base64
import hashlib
import json
import re
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
VECTORS = HERE.parent / 'vectors'
FIXTURE = REPO / 'bitcoin/fixtures/proxy.out'
EXAMPLE = 'bitcoin/examples/proxy.coax'
LIB = REPO / 'coax/lib/bitcoin/proxy.coax'


def pinned(path):
    manifest = json.loads((VECTORS / 'manifest.json').read_text())
    entry = next(f for f in manifest['files'] if f['path'] == path)
    data = (VECTORS / path).read_bytes()
    assert hashlib.sha256(data).hexdigest() == entry['sha256'], f'{path} differs from its pin'
    return data.decode()


SPEC = pinned('upstream/tor/encoding-onion-addresses.md')
SAM_SPEC = pinned('upstream/i2p/samv3.html')
NETBASE = pinned('upstream/bitcoin-core/netbase.cpp')
NETADDR = pinned('upstream/bitcoin-core/netaddress.cpp')
I2P_CPP = pinned('upstream/bitcoin-core/i2p.cpp')
I2P_TESTS = pinned('upstream/bitcoin-core/i2p_tests.cpp')
INIT = pinned('upstream/bitcoin-core/init.cpp')
NET_TESTS = pinned('upstream/bitcoin-core/net_tests.cpp')
RFC1928 = pinned('upstream/rfc/rfc1928.txt')
RFC1929 = pinned('upstream/rfc/rfc1929.txt')
assert 'CHECKSUM = SHA3_256(".onion checksum" | PUBKEY | VERSION)[:2]' in SPEC
assert "VERSION is a one byte version field (default value '\\x03')" in SPEC
EXAMPLES = re.findall(r'^\s+([a-z2-7]{56}\.onion)$', SPEC, re.M)
assert len(EXAMPLES) == 3
for line in ['HELLO VERSION MIN=3.1 MAX=3.1', 'DEST GENERATE SIGNATURE_TYPE=7', 'NAMING LOOKUP NAME=%s',
             'STREAM CONNECT ID=%s DESTINATION=%s SILENT=false', 'STREAM ACCEPT ID=%s SILENT=false',
             'SESSION CREATE STYLE=STREAM ID=%s DESTINATION=TRANSIENT SIGNATURE_TYPE=7 "\n                      "i2cp.leaseSetEncType=6,4 inbound.quantity=1 outbound.quantity=1',
             'static constexpr size_t DEST_LEN_BASE = 387;', 'static constexpr size_t CERT_LEN_POS = 385;',
             'result == "CANT_REACH_PEER" || result == "TIMEOUT"', 'result == "INVALID_ID"', 'RESULT=I2P_ERROR']:
    assert line in I2P_CPP, line
assert 'HELLO REPLY RESULT=OK VERSION=3.1' in SAM_SPEC and 'STREAM STATUS' in SAM_SPEC
assert "o  X'02' USERNAME/PASSWORD" in RFC1928 and "DOMAINNAME: X'03'" in RFC1928
assert 'VER | ULEN |  UNAME   | PLEN |  PASSWD' in RFC1929
assert 'auth.username = auth.password = strprintf("%s%i", m_prefix, m_counter);' in NETBASE
assert 'return HexStr(prefix_bytes) + "-";' in NETBASE
# Socks5ErrorString's table, read from netbase.cpp: the enum's values, then
# each case's text.
ENUM = dict(re.findall(r'(\w+) = (0x[0-9a-f]{2}),\s+//!< (?:RFC1928|Tor)', NETBASE))
REPLY_TEXT = {int(ENUM[name], 16): text for name, text in re.findall(r'case SOCKS5Reply::(\w+):\s*return "([^"]+)";', NETBASE)}
assert REPLY_TEXT[0xf0] == 'onion service descriptor can not be found' and len(REPLY_TEXT) == 16
# The test destinations, from i2p_tests.cpp, as the Coax library quotes them.
PUB = re.search(r'DEST REPLY PUB=([A-Za-z0-9~=-]+) PRIV=', I2P_TESTS).group(1)
PRIV = re.search(r' PRIV=([A-Za-z0-9~=-]+)\\n', I2P_TESTS).group(1)
PEER = re.search(r'// "([A-Za-z0-9~=-]{500,})\\n"', I2P_TESTS).group(1)
lib = LIB.read_text()
for name, value in (('I2P_TEST_PUB', PUB), ('I2P_TEST_PRIV', PRIV), ('I2P_TEST_PEER', PEER)):
    assert f'{name} :: "{value}";' in lib, name


# ---- Core's base32 (strencodings.cpp) over Python's
def b32(data, pad=True):
    s = base64.b32encode(data).decode().lower()
    return s if pad else s.rstrip('=')


def decode_b32(s):
    """DecodeBase32: strip 1, 3, 4 or 6 '=', then ConvertBits<5, 8, false>."""
    if len(s) % 8:
        return None
    for strip in ('=', '==', '=', '=='):
        if s.endswith(strip):
            s = s[:-len(strip)]
    alphabet = 'abcdefghijklmnopqrstuvwxyz234567'
    if any(c not in alphabet for c in s.lower()):
        return None
    if len(s) * 5 % 8 >= 5:
        return None
    full = base64.b32decode(s.upper() + '=' * (-len(s) % 8))
    # ConvertBits also refuses left-over bits that aren't zero.
    acc = 0
    for c in s.lower():
        acc = (acc << 5) | alphabet.index(c)
    if acc & ((1 << (len(s) * 5 % 8)) - 1):
        return None
    return full


def checksum(pub):
    return hashlib.sha3_256(b'.onion checksum' + pub + b'\x03').digest()[:2]


def onion_text(pub):
    return b32(pub + checksum(pub) + b'\x03') + '.onion'


def set_special(s):
    if '\0' in s:
        return None
    if s.endswith('.onion'):
        b = decode_b32(s[:-6])
        if b and len(b) == 35 and b[34] == 3 and checksum(b[:32]) == b[32:34]:
            return ('onion', b[:32])
        return None
    if len(s) == 60 and s[52:].lower() == '.b32.i2p':
        b = decode_b32(s[:52] + '====')
        if b and len(b) == 32:
            return ('i2p', b)
    return None


def text(a):
    return onion_text(a[1]) if a[0] == 'onion' else b32(a[1], False) + '.b32.i2p'


def i2p_b64(s):
    return base64.b64decode(s, altchars=b'-~', validate=True)


def i2p_addr(dest):
    return ('i2p', hashlib.sha256(dest).digest())


out = []
p = out.append

p('Tor v3 onion addresses (rend-spec-v3)')
for s in EXAMPLES:
    a = set_special(s)
    assert a and text(a) == s
    p(f'  {s}')
    p(f'    key {a[1].hex()}')
    p(f'    checksum {checksum(a[1]).hex()}, version 3')
    p(f'    addrv2 0420{a[1].hex()}')
bad = [('pg6mmjiyjmcrsslvykfwnntlaru7p5svn6y2ymmju6nubxndf4pscsad.onion', 'wrong checksum'),
       ('pg6mmjiyjmcrsslvykfwnntlaru7p5svn6y2ymmju6nubxndf4pscrye.onion', 'wrong version'),
       ('6hzph5hv6337r6p2.onion', 'Tor v2'),
       ('PG6MMJIYJMCRSSLVYKFWNNTLARU7P5SVN6Y2YMMJU6NUBXNDF4PSCRYD.onion', 'upper case'),
       ('pg6mmjiyjmcrsslvykfwnntlaru7p5svn6y2ymmju6nubxndf4pscryd.ONION', 'upper-case suffix')]
for s, why in bad:
    if why != 'upper case':
        assert s in NET_TESTS or why == 'upper-case suffix'
    p(f'  {s} ({why}): {"read" if set_special(s) else "refused"}')

p('I2P addresses')
pub, priv = i2p_b64(PUB), i2p_b64(PRIV)
cert = int.from_bytes(pub[385:387], 'big')
dest_len = 387 + cert
assert priv[:dest_len] == pub
a = i2p_addr(pub)
p(f'  destination: {len(pub)} bytes, certificate type {pub[384]} of {cert} bytes')
p(f'  private key: {len(priv)} bytes, of which the first {dest_len} are the destination')
p(f'  address {text(a)}')
p(f'    SHA-256 of the destination {a[1].hex()}')
p(f'    addrv2 0520{a[1].hex()}')
up = 'UDHDrtrcetjm5sxzskjyr5ztpeszydbh4dpl3pl4utgqqw2v4jna.b32.I2P'
assert up in NET_TESTS
p(f'  {up} reads as {text(set_special(up))}')
p(f'  56-character name: {"read" if set_special("pg6mmjiyjmcrsslvykfwnntlaru7p5svn6y2ymmju6nubxndf4pscsad.b32.i2p") else "refused"}')


# ---- SOCKS5: Core's client (netbase.cpp Socks5) and a proxy from RFC 1928
class Proxy:
    """Reads whole messages, prefers username/password, answers CONNECT."""
    def __init__(self):
        self.state, self.user, self.dest = 'greet', b'', b''

    def read(self, m, rep):
        if self.state == 'greet':
            assert m[0] == 5 and len(m) == 2 + m[1]
            method = 2 if 2 in m[2:] else 0 if 0 in m[2:] else 0xff
            self.state = 'auth' if method == 2 else 'request'
            return bytes([5, method])
        if self.state == 'auth':
            u = m[1]
            self.user = m[2:2 + u]
            assert m[0] == 1 and len(m) == 3 + u + m[2 + u]
            self.state = 'request'
            return bytes([1, 0])
        assert m[:4] == bytes([5, 1, 0, 3]) and len(m) == 7 + m[4]
        self.dest = m[5:5 + m[4]]
        return bytes([5, rep, 0, 1]) + bytes(6)


def socks5(dest, port, auth, proxy, rep):
    """Socks5 as netbase.cpp writes it, over the proxy. Returns (ok, reply, greeting)."""
    greeting = bytes([5, 2, 0, 2]) if auth else bytes([5, 1, 0])
    r = proxy.read(greeting, rep)
    assert r[0] == 5
    if r[1] == 2 and auth:
        a = bytes([1, len(auth)]) + auth + bytes([len(auth)]) + auth
        r = proxy.read(a, rep)
        assert r == bytes([1, 0])
    else:
        assert r[1] == 0
    req = bytes([5, 1, 0, 3, len(dest)]) + dest + bytes([port >> 8, port & 255])
    r = proxy.read(req, rep)
    assert r[0] == 5
    if r[1] != 0:
        return False, r[1], greeting
    assert r[2] == 0 and r[3] == 1
    return True, 0, greeting


p('SOCKS5 to Tor, with stream isolation')
prefix = bytes([0x9f, 0x31, 0x0c, 0x5e, 0xa2, 0x47, 0xd1, 0x08])
dests = [EXAMPLES[0], EXAMPLES[1], EXAMPLES[2], EXAMPLES[0]]
answers = [0, 0, 0xf0, 0]
circuits = []
for k, d in enumerate(dests):
    cred = (prefix.hex() + '-' + str(k)).encode()
    proxy = Proxy()
    ok, rep, greeting = socks5(d.encode(), 8333, cred, proxy, answers[k])
    if k == 0:
        p(f'  greeting {greeting.hex()}')
    if ok:
        if proxy.user not in circuits:
            circuits.append(proxy.user)
        p(f'  {d[:12]}...:8333 as {cred.decode()}: connected, circuit {circuits.index(proxy.user) + 1}')
    else:
        p(f'  {d[:12]}...:8333 as {cred.decode()}: failed, {REPLY_TEXT[rep]} (0x{rep:02x})')
proxy = Proxy()
ok, rep, greeting = socks5(b'1.2.3.4', 8333, None, proxy, 0)
p(f'  without isolation the greeting is {greeting.hex()}')
p(f'  1.2.3.4:8333: {"connected" if ok else "failed"}, credentials sent: {len(proxy.user)}')


# ---- SAM (i2p.cpp)
def short(s):
    return s if len(s) <= 60 else s[:60] + '...'


def get(line, key):
    keys = {}
    for kv in line.split(' '):
        k, eq, v = kv.partition('=')
        keys.setdefault(k, v if eq else None)
    return keys.get(key)


p('I2P through the SAM bridge')
sid = '2b7c0e91a4'
hello = 'HELLO REPLY RESULT=OK VERSION=3.1'
p('  > HELLO VERSION MIN=3.1 MAX=3.1')
p(f'  < {hello}')
p('  > ' + short(f'SESSION CREATE STYLE=STREAM ID={sid} DESTINATION=TRANSIENT SIGNATURE_TYPE=7 i2cp.leaseSetEncType=6,4 inbound.quantity=1 outbound.quantity=1'))
status = 'SESSION STATUS RESULT=OK DESTINATION=' + PRIV
p('  < ' + short(status))
key = i2p_b64(get(status, 'DESTINATION'))
me = i2p_addr(key[:387 + int.from_bytes(key[385:387], 'big')])
p(f'  session created, our address {text(me)}:0')
peer = i2p_addr(i2p_b64(PEER))
for result in ('STREAM STATUS RESULT=OK', 'STREAM STATUS RESULT=CANT_REACH_PEER'):
    p('  > ' + short(f'NAMING LOOKUP NAME={text(peer)}'))
    naming = f'NAMING REPLY RESULT=OK NAME={text(peer)} VALUE={PEER}'
    p('  < ' + short(naming))
    p('  > ' + short(f'STREAM CONNECT ID={sid} DESTINATION={get(naming, "VALUE")} SILENT=false'))
    p(f'  < {result}')
    r = get(result, 'RESULT')
    if r == 'OK':
        p('  connected')
    else:
        p(f'  failed; the proxy is to blame: {"no" if r in ("CANT_REACH_PEER", "TIMEOUT") else "yes"}')
p(f'  > STREAM ACCEPT ID={sid} SILENT=false')
p(f'  accepted a peer at {text(peer)}:0')
closed = 'STREAM STATUS RESULT=I2P_ERROR MESSAGE="Session was closed"'
try:
    i2p_b64(closed)
    gone = False
except ValueError:
    gone = 'RESULT=I2P_ERROR' in closed
p(f'  a closed session: make it again: {"yes" if gone else "no"}')


# ---- reachability (init.cpp AppInitMain) and routes (net.cpp ConnectNode)
for text_ in ['Unknown network specified in -onlynet', 'is not provided: none of -proxy, -onion or "\n                  "-listenonion is given',
              'reaching the Tor network is explicitly forbidden: -onion=0', '"-i2psam is not provided"',
              'Incompatible options: -dnsseed=1 was explicitly specified, but -onlynet forbids connections to IPv4/IPv6']:
    assert text_ in INIT, text_
NETS = ['ipv4', 'ipv6', 'onion', 'i2p', 'cjdns']


def configure(onlynet, proxies, onion, listenonion, i2psam, dnsseed):
    reach = set(NETS) | {'unroutable', 'internal'}
    if onlynet:
        reach = set()
        for n in onlynet:
            if n.lower() not in NETS:
                return 'Unknown network specified in -onlynet'
            reach.add(n.lower())
    if onlynet and 'cjdns' in reach:
        return 'Outbound connections restricted to CJDNS (-onlynet=cjdns) but -cjdnsreachable is not provided'
    reach.discard('cjdns')
    clearnet = not onlynet or 'ipv4' in reach or 'ipv6' in reach
    seeds = clearnet if dnsseed < 0 else dnsseed == 1
    if dnsseed == 1 and 'ipv4' not in reach and 'ipv6' not in reach:
        return 'Incompatible options: -dnsseed=1 was explicitly specified, but -onlynet forbids connections to IPv4/IPv6'
    prox = {}
    for s in proxies:
        value, eq, net = s.rpartition('=') if '=' in s else (s, '', '')
        on = value not in ('', '0')
        targets = {'': ['ipv4', 'ipv6', 'onion', 'cjdns'], 'ipv4': ['ipv4'], 'ipv6': ['ipv6'], 'onion': ['onion'], 'cjdns': ['cjdns']}[net]
        for t in targets:
            prox[t] = on
    only_onion = bool(onlynet) and 'onion' in reach
    if onion == '0':
        prox['onion'] = False
        if only_onion:
            return 'Outbound connections restricted to Tor (-onlynet=onion) but the proxy for reaching the Tor network is explicitly forbidden: -onion=0'
    elif onion:
        prox['onion'] = True
    later = False
    if not prox.get('onion'):
        if only_onion and not listenonion:
            return 'Outbound connections restricted to Tor (-onlynet=onion) but the proxy for reaching the Tor network is not provided: none of -proxy, -onion or -listenonion is given'
        reach.discard('onion')
        later = listenonion and (not onlynet or only_onion)
    if i2psam:
        prox['i2p'] = True
    else:
        if onlynet and 'i2p' in reach:
            return 'Outbound connections restricted to i2p (-onlynet=i2p) but -i2psam is not provided'
        reach.discard('i2p')
    return reach, prox, seeds, later


def route(cfg, net):
    reach, prox, _, _ = cfg
    if net not in reach:
        return 'unreachable'
    if net == 'i2p' and prox.get('i2p'):
        return 'SAM'
    return 'proxy' if prox.get(net) else 'direct'


p('Reachable networks')
tor = ['127.0.0.1:9050']
for label, args in [('no options', ([], [], '', True, False, -1)),
                    ('-proxy=127.0.0.1:9050', ([], tor, '', True, False, -1)),
                    ('-onion=127.0.0.1:9050 -i2psam=127.0.0.1:7656', ([], [], '127.0.0.1:9050', True, True, -1)),
                    ('-onlynet=onion -onlynet=i2p -proxy=127.0.0.1:9050 -i2psam=127.0.0.1:7656', (['onion', 'i2p'], tor, '', True, True, -1)),
                    ('-onlynet=onion -listenonion=0', (['onion'], [], '', False, False, -1)),
                    ('-onlynet=onion -onion=0', (['onion'], tor, '0', True, False, -1)),
                    ('-onlynet=onion -proxy=127.0.0.1:9050 -dnsseed=1', (['onion'], tor, '', True, False, 1)),
                    ('-onlynet=i2p', (['i2p'], [], '', True, False, -1)),
                    ('-onlynet=ipv4 -onion=127.0.0.1:9050', (['ipv4'], [], '127.0.0.1:9050', True, False, -1))]:
    p(f'  {label}')
    cfg = configure(*args)
    if isinstance(cfg, str):
        p(f'    error: {cfg}')
        continue
    reach, prox, seeds, later = cfg
    nets = ''.join(f' {n}' + (('(SAM)' if n == 'i2p' else '(proxy)') if prox.get(n) else '') for n in NETS if n in reach)
    p(f'    reachable:{nets}; DNS seeds {"on" if seeds else "off"}' + ('; onion may come from -listenonion' if later else ''))
    p(f'    1.2.3.4: {route(cfg, "ipv4")}, onion: {route(cfg, "onion")}, i2p: {route(cfg, "i2p")}')

want = '\n'.join(out) + '\n'


def main():
    if '--write' in sys.argv:
        FIXTURE.write_text(want)
    assert FIXTURE.read_text() == want, 'the fixture differs from the Python transcript'
    print(f'proxy fixture: {want.count(chr(10))} lines agree with rend-spec, SAM v3, RFC 1928 and Core\'s netbase.cpp, i2p.cpp and init.cpp')
    if '--coax' in sys.argv:
        coax = sys.argv[sys.argv.index('--coax') + 1]
        for mode in ([], ['--release']):
            run = subprocess.run([coax, 'run', EXAMPLE] + mode, cwd=REPO, capture_output=True, text=True)
            assert run.returncode == 0 and run.stdout == want, f'native {mode or "debug"} output differs'
            print(f'native {"release" if mode else "debug"}: agrees')


if __name__ == '__main__':
    main()
