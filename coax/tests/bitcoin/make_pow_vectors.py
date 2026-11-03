#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: copies published headers and computes their targets and work outside Coax)
"""Writes coax/tests/bitcoin/pow_vectors.coax for Bitcoin chapter 47 from
files pinned in coax/tests/vectors/upstream/:

  python3 -I coax/tests/bitcoin/make_pow_vectors.py

- Mainnet headers from mempool.space's API, each pinned by its SHA-256 and
  named by its block hash: the genesis block, the eleven blocks 32,250 to
  32,260 (the first change of nBits is at 32,256), and block 900,000.
  Each header hashes to the hash in its file name, and each of the eleven
  names the one before it as its previous block.
- The headers of blocks 100,000 and 514,999, the first 80 bytes of chapter
  46's pinned blocks, whose JSON records give nBits, the nonce and the
  difficulty: the genesis target divided by the block's must equal it.
- Bitcoin Core's chainparams.cpp: the genesis header's fields and the
  mainnet powLimit.
- Bitcoin Core's arith_uint256_tests.cpp (bignum_SetCompact) and
  pow_tests.cpp (GetBitsProof_test): each case copied here must appear in
  the pinned file, character for character.

Targets and work come from pow_reference.py, a transliteration of Core that
shares no code with Coax, and the work of every header is checked against
Python's own big integers as well.
"""
import hashlib, json
from pathlib import Path
import sys

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import pow_reference as P  # noqa: E402

ROOT = HERE.parent / 'vectors' / 'upstream'
GENESIS = '000000000019d6689c085ae165831e934ff763ae46a2a6c172b3f1b60a8ce26f'
CHAIN = [
    '000000002931f8c7a9fe1a6f122dd5777d09d81dc107e051d7f78efef09c1719',
    '00000000360d826e18e72b3f5f3a05a43989131e512319a3b24aaa24b1010aa9',
    '000000003d5eb2328f71644f2540cf6acc343f90d1594b2ef916ad4d4a7f5d2d',
    '000000000e27ee2a3899aec79a8aa08f138e320a66fa7fa3f2e4b9fd05e42a67',
    '000000006baebaa74cecde6c6787c26ee0a616a3c333261bff36653babdac149',
    '00000000984f962134a7291e3693075ae03e521f0ee33378ec30a334d860034b',
    '000000004f2886a170adb7204cb0c7a824217dd24d11a74423d564c4e0904967',
    '0000000078bb40a0a5aae645f86e144cab79209726b98d74e2f311c6d31aa52e',
    '000000006e35246abda2e8f2ab945780ea7619c74a3ee5db5ca6ff0f4d5b88c6',
    '000000008a5b32a0610b2b0eeb5390e30e157324bf28c09ab83ccbb99184c38b',
    '0000000007bfbe7ec5510ccf80c5a291bf8fc603dd79f5d6a69fae6039f8a35e',
]
B900000 = '000000000000000000010538edbfd2d5b809a33dd83f284aeea41c6d0d96968a'
B100000 = '000000000003ba27aa200b1cecaad478d2b00432346c3f1f3986da1afd33e506'
B514999 = '00000000000000000008fda0e5bb3105dab72605332085684c42dceb0ea61097'
PINS = {
    f'mainnet/{GENESIS}.header': '67991e515cf93949369ff472bba7fd07686e8587dc877149e5a25019609285fb',
    f'mainnet/{CHAIN[0]}.header': '1a85eb8f9f82c9359f42d548e586692754f90aaf339e19cfcd49660f4ac51635',
    f'mainnet/{CHAIN[1]}.header': 'a0fa0ce865f13539961d93a4f9959fe12a458a09b41561ae794f1507b0b3229c',
    f'mainnet/{CHAIN[2]}.header': '02ca7bd9c850d875508586de55c1a884dad17577dd9401dac8b3e69f4209ce0b',
    f'mainnet/{CHAIN[3]}.header': 'd250c2d84d158c866b0bae8f59fb7c99fc5cb8c8f85ae3c35e84c3d36948730c',
    f'mainnet/{CHAIN[4]}.header': '48403dd4a894635c26bc0f53e02dd41935d6696472b179e6048416d4044ad91a',
    f'mainnet/{CHAIN[5]}.header': '89ca08988ec28cc68f54e77972e8afb157c8ddc5d06a12ef545c5135a0d963cf',
    f'mainnet/{CHAIN[6]}.header': 'dd3a54aca0c6a10ca16a5700c10fb42b4e28447a403711b18140d27947864f9a',
    f'mainnet/{CHAIN[7]}.header': 'e741166721df7d1f4d28a4ee16e933189214266b1a437f1b6e8fa0153d93a5e4',
    f'mainnet/{CHAIN[8]}.header': '3c76ec1316163eed9b338f63b2bb985a0acbdd3c95b38d401fb6c0d99c8ab50d',
    f'mainnet/{CHAIN[9]}.header': '1c9f0331e9623fee87ab7e5686d39b06af48736f202dea3951cc0ffbc20f0e40',
    f'mainnet/{CHAIN[10]}.header': '19b17e9e8cb9c29a3590d288696f7b1cef72a57f18f47bc5d76778ab6dc29471',
    f'mainnet/{B900000}.header': '79abf5bbdf58748d3d51d7a96fbb7afb6720f78f98a4ccb3ec67d131ec9930bf',
    f'mainnet/{B100000}.raw': 'b02256f05ce2fc0d152394b5cd2fa28fe4ab61100844c389581c4cffad88a2b1',
    f'mainnet/{B514999}.raw': '7c0d1862284dcd240a63b5bcb75e956c3645ed5d7453799abb721ce06e6b2902',
    f'mainnet/{B100000}.json': 'a40baaad74146e536439774ac3966f471c6745a942423db503789547cfe2b45e',
    f'mainnet/{B514999}.json': '81682b48567be9bafe33b712930567fea33f9a18cbabc56eb89b82666d144d64',
    'bitcoin-core/chainparams.cpp': 'cfadf1478377c32625280520655b598674e6d487f08d7f7a80ac69252104b5e1',
    'bitcoin-core/arith_uint256_tests.cpp': None,
    'bitcoin-core/pow_tests.cpp': None,
}
MANIFEST = json.loads((HERE.parent / 'vectors' / 'manifest.json').read_text())
PINNED = {e['path'][len('upstream/'):]: e['sha256'] for e in MANIFEST['files']}
src = {}
for name, pin in PINS.items():
    data = (ROOT / name).read_bytes()
    assert hashlib.sha256(data).hexdigest() == (pin or PINNED[name]), name
    src[name] = data


def header(h):
    raw = bytes.fromhex(src[f'mainnet/{h}.header'].decode())
    assert len(raw) == 80 and P.sha256d(raw)[::-1].hex() == h, 'the header hashes to its name'
    return raw


def shown(v):
    return format(v, '064x')


# Genesis: chainparams.cpp's fields and powLimit.
chainparams = src['bitcoin-core/chainparams.cpp'].decode()
assert 'genesis = CreateGenesisBlock(1231006505, 2083236893, 0x1d00ffff, 1, 50 * COIN);' in chainparams
assert f'consensus.hashGenesisBlock == uint256{{"{GENESIS}"}}' in chainparams
assert 'consensus.powLimit = uint256{"00000000ffffffffffffffffffffffffffffffffffffffffffffffffffffffff"};' in chainparams
assert 'consensus.powLimit = uint256{"7fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"};' in chainparams
genesis = header(GENESIS)
g = P.read_header(genesis)
assert (g['version'], g['time'], g['bits'], g['nonce'], g['prev']) == (1, 1231006505, 0x1d00ffff, 2083236893, bytes(32))
assert g['merkle_root'][::-1].hex() == '4a5e1e4baab89f3a32518a88c31bc87f618f76673e2cc77ab2127b7afdeda33b'
assert f'0x{g["merkle_root"][::-1].hex()}' in chainparams.replace('uint256{"', '0x')

# The chain 32,250 to 32,260: each links to the one before.
chain = [header(h) for h in CHAIN]
for prev, cur in zip(chain, chain[1:]):
    assert cur[4:36] == P.sha256d(prev), 'each header names the one before'
bits_seen = [P.read_header(h)['bits'] for h in chain]
assert bits_seen[:6] == [0x1d00ffff] * 6 and bits_seen[6:] == [0x1d00d86a] * 5, bits_seen
others = {B100000: src[f'mainnet/{B100000}.raw'][:80], B514999: src[f'mainnet/{B514999}.raw'][:80], B900000: header(B900000)}
for h, raw in others.items():
    assert P.sha256d(raw)[::-1].hex() == h

# Every header meets its target and the mainnet limit, and the work agrees.
everything = [genesis] + chain + list(others.values())
for raw in everything:
    b = P.read_header(raw)['bits']
    assert P.check_header(raw, P.POW_LIMIT_MAIN) == P.OK
    assert P.proof(b) == P.proof_direct(b) > 0
    assert P.get_compact(P.set_compact(b)[0]) == b, 'mainnet nBits are canonical'
assert P.proof(0x1d00ffff) == 0x100010001
# The difficulty mempool.space publishes is the genesis target over the
# block's target; it must agree with ours to the double's precision.
for h in (B100000, B514999):
    record = json.loads(src[f'mainnet/{h}.json'])
    b = P.read_header(others[h])['bits']
    assert record['bits'] == b and record['nonce'] == P.read_header(others[h])['nonce']
    ratio = P.set_compact(0x1d00ffff)[0] / P.set_compact(b)[0]
    assert abs(ratio - record['difficulty']) <= 1e-12 * record['difficulty'], (ratio, record['difficulty'])
chain_work = sum(P.proof(P.read_header(h)['bits']) for h in chain)

# Core's bignum_SetCompact: (compact, target, negative, overflow, GetCompact
# with the case's sign, or -1 when the test does not encode).
arith = src['bitcoin-core/arith_uint256_tests.cpp'].decode()
CORE = [
    (0x00000000, 0, False, False, 0x00000000), (0x00123456, 0, False, False, 0x00000000),
    (0x01003456, 0, False, False, 0x00000000), (0x02000056, 0, False, False, 0x00000000),
    (0x03000000, 0, False, False, 0x00000000), (0x04000000, 0, False, False, 0x00000000),
    (0x00923456, 0, False, False, 0x00000000), (0x01803456, 0, False, False, 0x00000000),
    (0x02800056, 0, False, False, 0x00000000), (0x03800000, 0, False, False, 0x00000000),
    (0x04800000, 0, False, False, 0x00000000), (0x01123456, 0x12, False, False, 0x01120000),
    (0x01fedcba, 0x7e, True, False, 0x01fe0000), (0x02123456, 0x1234, False, False, 0x02123400),
    (0x03123456, 0x123456, False, False, 0x03123456), (0x04123456, 0x12345600, False, False, 0x04123456),
    (0x04923456, 0x12345600, True, False, 0x04923456), (0x05009234, 0x92340000, False, False, 0x05009234),
    (0x20123456, 0x123456 << 232, False, False, 0x20123456), (0xff123456, None, False, True, -1),
]
for compact, target, negative, overflow, encoded in CORE:
    assert f'num.SetCompact({compact:#010x}, &fNegative, &fOverflow);' in arith or (compact == 0 and 'num.SetCompact(0, &fNegative' in arith), hex(compact)
    t, n, o = P.set_compact(compact)
    assert (n, o) == (negative, overflow) and (target is None or t == target)
    if target is not None:
        assert f'BOOST_CHECK_EQUAL(num.GetHex(), "{shown(target)}");' in arith
        assert P.get_compact(t, negative) == encoded
        assert f'num.GetCompact({"true" if negative else ""}), {encoded:#010x}U);' in arith or encoded == 0
assert P.get_compact(0x80) == 0x02008000 and 'BOOST_CHECK_EQUAL(num.GetCompact(), 0x02008000U);' in arith

# pow_tests.cpp GetBitsProof_test: nBits that encode no work.
pow_tests = src['bitcoin-core/pow_tests.cpp'].decode()
ZERO_WORK = [0x00000000, 0x01003456, 0x04923456, 0xff123456, 0x80010203, 0x21012345]
for b in ZERO_WORK:
    assert f'BOOST_CHECK_EQUAL(GetBitsProof({b:#010x}), arith_uint256{{0}});' in pow_tests, hex(b)
    assert P.proof(b) == 0
assert 'BOOST_CHECK_EQUAL(GetBitsProof(0x1d00ffff), arith_uint256{0x100010001});' in pow_tests

lines = [
    '// Generated by make_pow_vectors.py from the files pinned in',
    '// coax/tests/vectors/upstream/ (see SOURCES.md there). Test data only.',
    '',
    '// The genesis header, from mempool.space; its fields are chainparams.cpp\'s.',
    f'PW_GENESIS :: "{genesis.hex()}";',
    f'PW_GENESIS_HASH :: "{GENESIS}";',
    '// Mainnet headers 32,250 to 32,260, one after another (mempool.space).',
    '// The first change of nBits, from 1d00ffff to 1d00d86a, is at 32,256.',
    f'PW_CHAIN :: "{"".join(h.hex() for h in chain)}";',
    f'PW_CHAIN_HASHES :: "{"".join(CHAIN)}";',
    f'PW_CHAIN_COUNT :: {len(chain)};',
    f'PW_CHAIN_FIRST :: 32250;',
    '// Their total work, 64 hex digits (computed by pow_reference.py).',
    f'PW_CHAIN_WORK :: "{shown(chain_work)}";',
    '// The headers of blocks 100,000 and 514,999 (chapter 46\'s pinned blocks)',
    '// and of block 900,000 (mempool.space), with their hashes.',
    f'PW_HEADER_100000 :: "{others[B100000].hex()}";',
    f'PW_HASH_100000 :: "{B100000}";',
    f'PW_HEADER_514999 :: "{others[B514999].hex()}";',
    f'PW_HASH_514999 :: "{B514999}";',
    f'PW_HEADER_900000 :: "{others[B900000].hex()}";',
    f'PW_HASH_900000 :: "{B900000}";',
    '// The target and work of each, 64 hex digits (computed by pow_reference.py).',
]
for name, raw in [('GENESIS', genesis), ('32256', chain[6]), ('100000', others[B100000]),
                  ('514999', others[B514999]), ('900000', others[B900000])]:
    b = P.read_header(raw)['bits']
    lines.append(f'PW_TARGET_{name} :: "{shown(P.set_compact(b)[0])}";')
    lines.append(f'PW_WORK_{name} :: "{shown(P.proof(b))}";')
lines += [
    '// arith_uint256_tests.cpp bignum_SetCompact: each compact, its target (64',
    '// hex digits), its flags (1 negative, 2 overflow) and GetCompact with the',
    '// same sign, or -1 where the test does not encode it.',
    f'PW_CORE_COUNT :: {len(CORE)};',
    f'PW_CORE_COMPACT :: [{len(CORE)}] int.[{", ".join(f"{c[0]:#010x}" for c in CORE)}];',
    f'PW_CORE_TARGET :: [{len(CORE)}] string.[{", ".join(chr(34) + (shown(c[1]) if c[1] is not None else "") + chr(34) for c in CORE)}];',
    f'PW_CORE_FLAGS :: [{len(CORE)}] int.[{", ".join(str(int(c[2]) + 2 * int(c[3])) for c in CORE)}];',
    f'PW_CORE_ENCODE :: [{len(CORE)}] int.[{", ".join(f"{c[4]:#010x}" if c[4] >= 0 else "-1" for c in CORE)}];',
    '// pow_tests.cpp GetBitsProof_test: nBits whose work is zero.',
    f'PW_CORE_ZERO_WORK :: [{len(ZERO_WORK)}] int.[{", ".join(f"{b:#010x}" for b in ZERO_WORK)}];',
]
(HERE / 'pow_vectors.coax').write_text('\n'.join(lines) + '\n')
print('wrote pow_vectors.coax:', len(everything), 'headers,', len(CORE), 'compact cases')
