#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: copies the published BIP-67 and BIP-387 vectors and recomputes each in Python outside Coax)
"""Writes multisig_vectors.coax for Bitcoin chapter 100 from the pinned
BIP-67 and BIP-387 texts and Bitcoin Core's solver.cpp:

  python3 -I coax/tests/bitcoin/make_multisig_vectors.py          # check the file
  python3 -I coax/tests/bitcoin/make_multisig_vectors.py --write  # rewrite it

Each BIP-67 vector's sorted keys, script and P2SH address are recomputed
in Python. Each valid BIP-387 descriptor's keys are resolved (WIF, hex,
and BIP-32 derivation of xprv and xpub keys at each listed index), its
multi_a leaf, leaf hash and output key are recomputed, and every listed
scriptPubKey must come out. The match cases are decided by a Python
transliteration of Core's MatchMultisig and MatchMultiA, after the
pinned solver.cpp is checked to still read as transliterated.
"""
import hashlib, hmac, json, re, sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import der_reference as D  # noqa: E402
import psbt_reference as R  # noqa: E402

UP = HERE.parent / 'vectors' / 'upstream'
OUT = HERE / 'multisig_vectors.coax'
B58 = '123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz'


def pinned(name):
    b = (UP / name).read_bytes()
    m = json.loads((UP.parent / 'manifest.json').read_text())
    pin = next(e['sha256'] for e in m['files'] if e['path'] == 'upstream/' + name)
    assert hashlib.sha256(b).hexdigest() == pin, name
    return b.decode()


# ---- Base58Check and BIP-32
def b58decode(s):
    n = 0
    for c in s: n = n * 58 + B58.index(c)
    b = n.to_bytes((n.bit_length() + 7) // 8, 'big')
    b = b'\0' * (len(s) - len(s.lstrip('1'))) + b
    assert hashlib.sha256(hashlib.sha256(b[:-4]).digest()).digest()[:4] == b[-4:], s
    return b[:-4]


def b58encode(b):
    b = b + hashlib.sha256(hashlib.sha256(b).digest()).digest()[:4]
    n = int.from_bytes(b, 'big')
    out = ''
    while n: n, r = divmod(n, 58); out = B58[r] + out
    return '1' * (len(b) - len(b.lstrip(b'\0'))) + out


def point(secret):
    return D.mul(secret, D.G)


def ser(p):
    return R.compress(p)


def ckd(node, i):
    """BIP-32's child of (private or None, public point, chain code)."""
    k, P, c = node
    if i & 0x80000000:
        assert k is not None
        data = b'\0' + k.to_bytes(32, 'big') + i.to_bytes(4, 'big')
    else:
        data = ser(P) + i.to_bytes(4, 'big')
    h = hmac.new(c, data, hashlib.sha512).digest()
    t = int.from_bytes(h[:32], 'big')
    assert t < D.N
    if k is not None:
        k2 = (k + t) % D.N
        return k2, point(k2), h[32:]
    return None, D.add(point(t), P), h[32:]


def xkey(text):
    b = b58decode(text)
    assert len(b) == 78
    c, key = b[13:45], b[45:]
    if key[0] == 0:
        k = int.from_bytes(key[1:], 'big')
        return k, point(k), c
    return None, decompress(key), c


def decompress(key):
    x = int.from_bytes(key[1:], 'big')
    y = pow((x ** 3 + 7) % D.P, (D.P + 1) // 4, D.P)
    if y % 2 != key[0] % 2: y = D.P - y
    return (x, y)


def resolve(text, index):
    """A descriptor key expression's x-only key at a ranged index."""
    text = re.sub(r'^\[[^\]]*\]', '', text)
    if len(text) == 64 and all(c in '0123456789abcdef' for c in text): return bytes.fromhex(text)
    if len(text) == 66: return bytes.fromhex(text)[1:]
    if text[0] in 'KL5c9':
        b = b58decode(text)
        return ser(point(int.from_bytes(b[1:33], 'big')))[1:]
    parts = text.split('/')
    node = xkey(parts[0])
    for p in parts[1:]:
        hard = p.endswith("'") or p.endswith('h')
        p = p.rstrip("'h")
        i = index if p == '*' else int(p)
        node = ckd(node, i | (0x80000000 if hard else 0))
    return ser(node[1])[1:]


def args(s):
    out, depth, cur = [], 0, ''
    for ch in s:
        if ch == ',' and depth == 0: out.append(cur); cur = ''; continue
        depth += {'(': 1, ')': -1}.get(ch, 0)
        cur += ch
    return out + [cur]


# ---- the scripts, in Python
def number(n):
    if 1 <= n <= 16: return bytes([0x50 + n])
    b = n.to_bytes((n.bit_length() + 8) // 8, 'little')
    return bytes([len(b)]) + b


def multisig(m, keys, sort):
    keys = sorted(keys) if sort else keys
    return number(m) + b''.join(b'\x21' + k for k in keys) + number(len(keys)) + b'\xae'


def multi_a(m, keys, sort):
    keys = sorted(keys) if sort else keys
    return b''.join(b'\x20' + k + (b'\xac' if i == 0 else b'\xba') for i, k in enumerate(keys)) + number(m) + b'\x9c'


def tagged(tag, b):
    t = hashlib.sha256(tag.encode()).digest()
    return hashlib.sha256(t + t + b).digest()


def leaf_hash(script):
    return tagged('TapLeaf', bytes([0xc0, len(script)]) + script)


def output_key(internal, merkle):
    P = R.lift_x(int.from_bytes(internal, 'big'))
    t = int.from_bytes(tagged('TapTweak', internal + merkle), 'big')
    return D.add(P, point(t))[0].to_bytes(32, 'big')


def p2sh_address(script):
    h = hashlib.new('ripemd160', hashlib.sha256(script).digest()).digest()
    return b58encode(b'\x05' + h)


# ---- Core's MatchMultisig and MatchMultiA
def get_op(s, pc):
    """GetScriptOp: (ok, opcode, data, next)."""
    if pc >= len(s): return False, 0xff, b'', pc
    op = s[pc]; pc += 1
    if op > 0x4e: return True, op, b'', pc
    width = 0 if op < 0x4c else {0x4c: 1, 0x4d: 2, 0x4e: 4}[op]
    if width > len(s) - pc: return False, 0xff, b'', pc
    n = op if width == 0 else int.from_bytes(s[pc:pc + width], 'little')
    pc += width
    if n > len(s) - pc: return False, 0xff, b'', pc
    return True, op, s[pc:pc + n], pc + n


def minimal_push(data, op):
    n = len(data)
    if n == 0: return op == 0
    if n == 1 and 1 <= data[0] <= 16: return False
    if n == 1 and data[0] == 0x81: return False
    if n <= 75: return op == n
    if n <= 255: return op == 0x4c
    if n <= 65535: return op == 0x4d
    return True


def script_number(ok, op, data, lo, hi):
    if 0x51 <= op <= 0x60: v = op - 0x50
    elif ok and op <= 0x4e:
        if not minimal_push(data, op) or len(data) > 4: return None
        if data and data[-1] & 0x7f == 0 and (len(data) == 1 or data[-2] & 0x80 == 0): return None
        v = int.from_bytes(data, 'little') if data else 0
        if data and data[-1] & 0x80: v = -(v & ~(0x80 << (8 * (len(data) - 1))))
    else: return None
    return v if lo <= v <= hi else None


def valid_size(data):
    return len(data) > 0 and len(data) == {2: 33, 3: 33, 4: 65, 6: 65, 7: 65}.get(data[0], -1)


def match_multisig(s):
    if len(s) < 1 or s[-1] != 0xae: return None
    ok, op, data, pc = get_op(s, 0)
    m = script_number(ok, op, data, 1, 20)
    if m is None: return None
    keys = []
    while True:
        ok, op, data, pc = get_op(s, pc)
        if not (ok and valid_size(data)): break
        keys.append(data)
    n = script_number(ok, op, data, m, 20)
    if n is None or n != len(keys): return None
    return (m, n) if pc + 1 == len(s) else None


def match_multi_a(s):
    if len(s) == 0 or s[0] != 32 or s[-1] != 0x9c: return None
    it, n = 0, 0
    while len(s) - it >= 34:
        if s[it] != 32: return None
        n += 1; it += 33
        if s[it] != (0xac if n == 1 else 0xba): return None
        it += 1
    if n == 0 or n > 999: return None
    ok, op, data, it = get_op(s, it)
    if not ok or it >= len(s) or s[it] != 0x9c or it + 1 != len(s): return None
    m = script_number(ok, op, data, 1, n)
    return None if m is None else (m, n)


def check_solver():
    src = pinned('bitcoin-core/solver.cpp')
    for line in ('auto req_sigs = GetScriptNumber(opcode, data, 1, MAX_PUBKEYS_PER_MULTISIG);',
                 'while (script.GetOp(it, opcode, data) && CPubKey::ValidSize(data)) {',
                 'auto num_keys = GetScriptNumber(opcode, data, required_sigs, MAX_PUBKEYS_PER_MULTISIG);',
                 'return (it + 1 == script.end());',
                 'if (script.size() == 0 || script[0] != 32 || script.back() != OP_NUMEQUAL) return {};',
                 'if (*it != (keyspans.size() == 1 ? OP_CHECKSIG : OP_CHECKSIGADD)) return {};',
                 'if (keyspans.size() == 0 || keyspans.size() > MAX_PUBKEYS_PER_MULTI_A) return {};',
                 'auto threshold = GetScriptNumber(opcode, data, 1, (int)keyspans.size());',
                 'if (!CheckMinimalPush(data, opcode)) return {};',
                 'count = CScriptNum(data, /* fRequireMinimal = */ true).getint();'):
        assert line in src, line


# ---- the vectors
def bip67():
    text = pinned('bip67/bip-0067.mediawiki')
    text = text[text.index('==Test vectors=='):text.index('==Acknowledgements==')]
    out = []
    for block in re.split(r'\nVector \d', text)[1:]:
        lists = re.findall(r'\*\* ([0-9a-f]+)', block)
        keys = [bytes.fromhex(k) for k in lists if len(k) == 66]
        n = len(keys) // 2
        script = bytes.fromhex(next(k for k in lists if len(k) > 66))
        address = re.search(r'\*\* ([13][1-9A-HJ-NP-Za-km-z]+)\s*$', block.strip(), re.M).group(1)
        assert sorted(keys[:n]) == keys[n:] and multisig(2, keys[:n], True) == script and p2sh_address(script) == address
        out.append((b''.join(keys[:n]).hex(), script.hex(), address))
    assert len(out) == 4
    return out


def bip387():
    text = pinned('bip387/bip-0387.mediawiki')
    text = text[text.index('==Test Vectors=='):text.index('Invalid descriptors')]
    out = []
    for desc, spks in re.findall(r'\* <tt>(tr\(.*?\))</tt>\n((?:\*\* <tt>[0-9a-f]+</tt>\n)+)', text):
        spks = re.findall(r'<tt>([0-9a-f]+)</tt>', spks)
        internal_text, inner = args(desc[3:-1])
        name = inner[:inner.index('(')]
        a = args(inner[len(name) + 1:-1])
        m, key_texts = int(a[0]), a[1:]
        for index, spk in enumerate(spks):
            internal = resolve(internal_text, index)
            keys = [resolve(k, index) for k in key_texts]
            leaf = multi_a(m, keys, name == 'sortedmulti_a')
            assert match_multi_a(leaf) == (m, len(keys))
            assert (b'\x51\x20' + output_key(internal, leaf_hash(leaf))).hex() == spk, desc
            out.append((internal.hex(), b''.join(keys).hex(), m, name == 'sortedmulti_a', spk))
    assert len(out) == 10
    return out


def match_cases():
    k = [bytes.fromhex(x) for x in ('02fe6f0a5a297eb38c391581c4413e084773ea23954d93f7753db7dc0adc188b2f',
                                     '02ff12471208c14bd580709cb2358d98975247d8765f92bc25eab3b2763ed605f8')]
    u = b'\x04' + bytes(64)
    x = bytes(range(32))
    two = multisig(2, k, False)
    cases = [
        ('a 2-of-2', two),
        ('a 1-of-2', multisig(1, k, False)),
        ('n not the key count', two[:-2] + b'\x53\xae'),
        ('m above n', b'\x53' + two[1:]),
        ('m of 0', b'\x00' + two[1:]),
        ('an uncompressed key', b'\x51\x41' + u + b'\x51\xae'),
        ('a key of 32 bytes', b'\x51\x20' + bytes(32) + b'\x51\xae'),
        ('m pushed, not OP_1', b'\x01\x01' + two[1:-2] + b'\x52\xae'),
        ('17 of 20, pushed', number(17) + b''.join(b'\x21' + k[0] for _ in range(20)) + number(20) + b'\xae'),
        ('21 keys', number(1) + b''.join(b'\x21' + k[0] for _ in range(21)) + number(21) + b'\xae'),
        ('20 pushed as two bytes', number(1) + b''.join(b'\x21' + k[0] for _ in range(20)) + b'\x02\x14\x00\xae'),
        ('a byte after the count', two[:-1] + b'\x61\xae'),
        ('OP_CHECKMULTISIGVERIFY', two[:-1] + b'\xaf'),
        ('a 2-of-3 multi_a', multi_a(2, [x, x, x], False)),
        ('a 1-of-1 multi_a', multi_a(1, [x], False)),
        ('multi_a, 4 of 3', multi_a(3, [x, x, x], False)[:-2] + b'\x54\x9c'),
        ('multi_a, OP_CHECKSIG twice', multi_a(2, [x, x], False).replace(b'\xba', b'\xac')),
        ('multi_a, OP_NUMEQUALVERIFY', multi_a(1, [x], False)[:-1] + b'\x9d'),
        ('multi_a, 17 of 20', multi_a(17, [x] * 20, False)),
        ('multi_a, threshold pushed', multi_a(1, [x], False)[:-2] + b'\x01\x01\x9c'),
    ]
    out = []
    for name, s in cases:
        tap = s[0] == 32 or name.startswith('multi_a')
        r = (match_multi_a if tap else match_multisig)(s)
        out.append((name, s.hex(), tap, r[0] if r else 0, r[1] if r else 0))
    return out


def q(s):
    return '"' + s + '"'


def render():
    check_solver()
    lines = ['// Generated by make_multisig_vectors.py from the pinned BIP-67 and BIP-387',
             '// texts and Bitcoin Core\'s solver.cpp (coax/tests/vectors/upstream/, listed',
             '// in SOURCES.md). Published values, copied; each recomputed in Python.',
             '// BIP-67\'s vectors: the keys in their listed order (33 bytes each), the',
             '// 2-of-n script of the sorted keys, and its P2SH address.',
             'MsBip67 :: struct { keys: string; script: string; address: string; }',
             'MS_BIP67 :: [4] MsBip67.[']
    for k, s, a in bip67(): lines.append(f'  MsBip67.{{keys={q(k)}, script={q(s)}, address={q(a)}}},')
    lines.append('];')
    lines += ['// BIP-387\'s valid descriptors, one row per listed scriptPubKey: the',
              '// internal key and the multi_a keys (x-only, 32 bytes each), resolved',
              '// in Python from WIF, hex and BIP-32 keys at the row\'s index.',
              'MsBip387 :: struct { internal: string; keys: string; m: int; sorted: bool; spk: string; }']
    v = bip387()
    lines.append(f'MS_BIP387 :: [{len(v)}] MsBip387.[')
    for i, k, m, srt, spk in v:
        lines.append(f'  MsBip387.{{internal={q(i)}, keys={q(k)}, m={m}, sorted={"true" if srt else "false"}, spk={q(spk)}}},')
    lines.append('];')
    lines += ['// Scripts for Core\'s MatchMultisig (or MatchMultiA, for tapscript):',
              '// the m and n Python\'s transliteration finds, 0 and 0 for no match.',
              'MsMatch :: struct { name: string; script: string; tapscript: bool; m: int; n: int; }']
    c = match_cases()
    lines.append(f'MS_MATCH :: [{len(c)}] MsMatch.[')
    for name, s, tap, m, n in c:
        lines.append(f'  MsMatch.{{name={q(name)}, script={q(s)}, tapscript={"true" if tap else "false"}, m={m}, n={n}}},')
    lines.append('];')
    return '\n'.join(lines) + '\n'


if __name__ == '__main__':
    text = render()
    if '--write' in sys.argv:
        OUT.write_text(text)
    assert OUT.read_text() == text, 'multisig_vectors.coax differs; run with --write'
    print(f'multisig vectors: 4 BIP-67 vectors and {text.count("MsBip387.{")} BIP-387 scripts recomputed')
