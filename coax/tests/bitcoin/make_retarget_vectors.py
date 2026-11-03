#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: copies published headers and computes every retarget and the chainwork outside Coax)
"""Writes coax/tests/bitcoin/retarget_vectors.coax for Bitcoin chapter 48
from files pinned in coax/tests/vectors/upstream/:

  python3 -I coax/tests/bitcoin/make_retarget_vectors.py

- mainnet/retarget-boundaries.txt: mainnet headers from mempool.space, one
  per line as height, block hash and the 80 bytes in hex. It holds the
  genesis block, both sides of every retarget boundary from 2,016 to
  973,728 (heights 2016k - 1 and 2016k) and block 966,143. Each header
  hashes to its hash and meets its target, each block at 2016k names the
  block at 2016k - 1 as its previous block, and a period's first and last
  blocks have the same nBits.
- Bitcoin Core's chainparams.cpp: nMinimumChainWork and defaultAssumeValid
  (block 966,143). Core's release process sets nMinimumChainWork to the
  chainwork getblockheader prints for that block, so the work of every
  header up to it must add up to that number.
- Bitcoin Core's pow_tests.cpp: the CalculateNextWorkRequired cases, each
  copied here must appear in the pinned file, character for character.

Retargets come from retarget_reference.py, a transliteration of Core
that shares no code with Coax.
"""
import hashlib, json
from pathlib import Path
import sys

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import pow_reference as P  # noqa: E402
import retarget_reference as R  # noqa: E402

ROOT = HERE.parent / 'vectors' / 'upstream'
GENESIS = '000000000019d6689c085ae165831e934ff763ae46a2a6c172b3f1b60a8ce26f'
TOP = 483              # the last boundary pinned is 2016 * 483 = 973,728
ASSUME_VALID = 966143
PINS = ['mainnet/retarget-boundaries.txt', 'bitcoin-core/chainparams.cpp', 'bitcoin-core/pow_tests.cpp',
        'bitcoin-core/pow.cpp', 'bitcoin-core/consensus.h']
MANIFEST = json.loads((HERE.parent / 'vectors' / 'manifest.json').read_text())
PINNED = {e['path'][len('upstream/'):]: e['sha256'] for e in MANIFEST['files']}
src = {}
for name in PINS:
    data = (ROOT / name).read_bytes()
    assert hashlib.sha256(data).hexdigest() == PINNED[name], name
    src[name] = data.decode()

# The headers: each hashes to its hash and meets its target.
headers = {}
for line in src['mainnet/retarget-boundaries.txt'].splitlines():
    height, h, hexed = line.split(' ')
    raw = bytes.fromhex(hexed)
    assert len(raw) == 80 and P.sha256d(raw)[::-1].hex() == h, height
    assert P.check_header(raw, P.POW_LIMIT_MAIN) == P.OK, height
    headers[int(height)] = (h, raw)
want = {0, ASSUME_VALID} | {x for k in range(1, TOP + 1) for x in (2016 * k - 1, 2016 * k)}
assert set(headers) == want, sorted(set(headers) ^ want)[:5]
assert headers[0][0] == GENESIS


def field(height, name):
    return P.read_header(headers[height][1])[name]


# Both sides of each boundary link, and a period's ends share nBits.
for k in range(1, TOP + 1):
    assert headers[2016 * k][1][4:36] == P.sha256d(headers[2016 * k - 1][1]), k
    assert field(2016 * (k - 1), 'bits') == field(2016 * k - 1, 'bits'), k
first_time = [field(2016 * k, 'time') for k in range(TOP)]
last_time = [field(2016 * k + 2015, 'time') for k in range(TOP)]
bits = [field(2016 * k, 'bits') for k in range(TOP + 1)]
assert field(ASSUME_VALID, 'bits') == bits[ASSUME_VALID // 2016]

# Every retarget, through CalculateNextWorkRequired and through
# GetNextWorkRequired on a whole period; each is permitted, and nBits is
# canonical.
up = down = None
clamped, capped = [], []
for k in range(1, TOP + 1):
    last = R.Index(2016 * k - 1, last_time[k - 1], bits[k - 1])
    assert R.calculate_next_work_required(last, first_time[k - 1], R.MAIN) == bits[k], k
    period = [(first_time[k - 1], bits[k - 1])] + [(last_time[k - 1], bits[k - 1])] * 2015
    assert R.next_bits(R.MAIN, period, 2016 * k - 1, 0) == (R.OK, bits[k]), k
    assert R.permitted(R.MAIN, 2016 * k, bits[k - 1], bits[k]), k
    assert P.get_compact(P.set_compact(bits[k])[0]) == bits[k]
    actual = last_time[k - 1] - first_time[k - 1]
    if not R.TWO_WEEKS // 4 <= actual <= R.TWO_WEEKS * 4:
        clamped.append(k)
    raw_target = P.set_compact(bits[k - 1])[0] * actual // R.TWO_WEEKS
    if raw_target > P.POW_LIMIT_MAIN:
        capped.append(k)
    ratio = P.set_compact(bits[k - 1])[0] / P.set_compact(bits[k])[0]
    if up is None or ratio > up[1]:
        up = (k, ratio)
    if down is None or ratio < down[1]:
        down = (k, ratio)
# One mainnet period has hit the clamp: 66,528 to 68,543 took 3.35 days in
# July 2010, under the quarter, so the target fell by exactly four.
assert clamped == [34], clamped
print('largest increase of difficulty at', 2016 * up[0], up[1], '; largest decrease at', 2016 * down[0], down[1])
print('periods capped at powLimit:', capped)

# Chainwork to block 966,143 is Core's nMinimumChainWork.
chainparams = src['bitcoin-core/chainparams.cpp']
av_hash = headers[ASSUME_VALID][0]
assert f'consensus.defaultAssumeValid = uint256{{"{av_hash}"}}; // {ASSUME_VALID}' in chainparams
MIN_WORK = '000000000000000000000000000000000000000145ec036acc5ba740052af1a0'
assert f'consensus.nMinimumChainWork = uint256{{"{MIN_WORK}"}};' in chainparams
work = sum(2016 * P.proof(bits[k]) for k in range(ASSUME_VALID // 2016))
work += (ASSUME_VALID % 2016 + 1) * P.proof(bits[ASSUME_VALID // 2016])
assert work == int(MIN_WORK, 16), hex(work)
# The parameters are chainparams.cpp's, and Core's.
for line in ('consensus.nPowTargetTimespan = 14 * 24 * 60 * 60; // two weeks', 'consensus.nPowTargetSpacing = 10 * 60;',
             'consensus.fPowAllowMinDifficultyBlocks = false;', 'consensus.enforce_BIP94 = true;',
             'consensus.nPowTargetTimespan = 24 * 60 * 60; // one day', 'consensus.fPowNoRetargeting = true;'):
    assert line in chainparams, line
assert 'inline constexpr int64_t MAX_TIMEWARP = 600;' in src['bitcoin-core/consensus.h']
assert 'bnNew *= nActualTimespan;' in src['bitcoin-core/pow.cpp']
assert 'int nHeightFirst = pindexLast->nHeight - (params.DifficultyAdjustmentInterval()-1);' in src['bitcoin-core/pow.cpp']

# pow_tests.cpp: (test, last height, last time, last nBits, first time, expected).
T = R.TWO_WEEKS
CORE = [
    ('get_next_work', 32255, 1262152739, 0x1d00ffff, 1261130161, 0x1d00d86a),
    ('get_next_work_pow_limit', 2015, 1233061996, 0x1d00ffff, 1231006505, 0x1d00ffff),
    ('get_next_work_lower_limit_actual', 68543, 1279297671, 0x1c05a3f4, 1279008237, 0x1c0168fd),
    ('get_next_work_lower_limit_rounding', 68543, 1279297671, 0x1c05a3f5, 1279297671 - T // 4, 0x1c0168fd),
    ('get_next_work_negative_actual', 68543, 1279008237, 0x1c05a3f4, 1279297671, 0x1c0168fd),
    ('get_next_work_upper_limit_actual', 46367, 1269211443, 0x1c387f6f, 1263163443, 0x1d00e1fd),
]
tests = src['bitcoin-core/pow_tests.cpp']
for name, height, t, b, first, expected in CORE:
    body = tests[tests.index(f'BOOST_AUTO_TEST_CASE({name})'):]
    body = body[:body.index('\n}\n')]
    assert f'pindexLast.nHeight = {height};' in body and f'pindexLast.nBits = {b:#010x};' in body, name
    assert f'expected_nbits = {expected:#010x}U;' in body, name
    assert f'pindexLast.nTime = {t};' in body or (name == 'get_next_work_lower_limit_rounding' and 'pindexLast.nTime = 1279297671;' in body)
    assert (str(first) in body or 'nLastRetargetTime = pindexLast.nTime - consensus.nPowTargetTimespan/4;' in body), name
    assert R.calculate_next_work_required(R.Index(height, t, b), first, R.MAIN) == expected, name
    assert R.permitted(R.MAIN, height + 1, b, expected), name
# Core's times for blocks 0, 2,015, 30,240, 32,255 and 46,367, and the
# window 66,528 to 68,543, are the real headers' (get_next_work_upper_limit_actual
# says its first time is not a block's).
assert (first_time[0], last_time[0], bits[0]) == (1231006505, 1233061996, 0x1d00ffff)
assert (first_time[15], last_time[15], bits[15], bits[16]) == (1261130161, 1262152739, 0x1d00ffff, 0x1d00d86a)
assert (first_time[33], last_time[33], bits[33]) == (1279008237, 1279297671, 0x1c05a3f4)
assert (last_time[22], bits[22]) == (1269211443, 0x1c387f6f)
assert not R.permitted(R.MAIN, 68544, 0x1c05a3f4, 0x1c0168fd - 1) and not R.permitted(R.MAIN, 46368, 0x1c387f6f, 0x1d00e1fd + 1)
assert 'unsigned int expected_nbits = 0x1c05a33c;' in tests

# The boundaries the Coax tests check header by header: the powLimit cap
# at 2,016, the first change at 32,256, the clamp at 68,544, the largest
# increase and decrease
# of difficulty, the first boundary after block 900,000, and the last three.
BOUNDARIES = sorted({1, 16, 34, up[0], down[0], 900000 // 2016 + 1, TOP - 2, TOP - 1, TOP})
assert BOUNDARIES[0] == 1 and 16 in BOUNDARIES


def hexed(height):
    return headers[height][1].hex()


lines = [
    '// Generated by make_retarget_vectors.py from the files pinned in',
    '// coax/tests/vectors/upstream/ (see SOURCES.md there). Test data only.',
    '',
    '// pow_tests.cpp\'s CalculateNextWorkRequired cases: the last block\'s height,',
    '// time and nBits, the first block\'s time and the expected nBits.',
    f'RT_CORE_COUNT :: {len(CORE)};',
    f'RT_CORE_HEIGHT :: [{len(CORE)}] int.[{", ".join(str(c[1]) for c in CORE)}];',
    f'RT_CORE_LAST_TIME :: [{len(CORE)}] int.[{", ".join(str(c[2]) for c in CORE)}];',
    f'RT_CORE_LAST_BITS :: [{len(CORE)}] int.[{", ".join(f"{c[3]:#010x}" for c in CORE)}];',
    f'RT_CORE_FIRST_TIME :: [{len(CORE)}] int.[{", ".join(str(c[4]) for c in CORE)}];',
    f'RT_CORE_EXPECTED :: [{len(CORE)}] int.[{", ".join(f"{c[5]:#010x}" for c in CORE)}];',
    '// Every mainnet period from genesis: period k is blocks 2016k to 2016k + 2015.',
    '// The time of its first and its last block, and its nBits (mempool.space,',
    f'// retrieved 2026-11-02). RT_BITS has one more entry: period {TOP}, from block',
    f'// {2016 * TOP:,}.',
    f'RT_PERIODS :: {TOP};',
    f'RT_FIRST_TIME :: [{TOP}] int.[{", ".join(str(x) for x in first_time)}];',
    f'RT_LAST_TIME :: [{TOP}] int.[{", ".join(str(x) for x in last_time)}];',
    f'RT_BITS :: [{TOP + 1}] int.[{", ".join(f"{x:#010x}" for x in bits)}];',
    '// Whole headers at some boundaries: the first and last blocks of the period',
    '// before it and the block at the boundary, whose height is given, and its hash.',
    f'RT_BOUNDARY_COUNT :: {len(BOUNDARIES)};',
    f'RT_BOUNDARY_HEIGHT :: [{len(BOUNDARIES)}] int.[{", ".join(str(2016 * k) for k in BOUNDARIES)}];',
    f'RT_BOUNDARY_HEADERS :: [{len(BOUNDARIES)}] string.[{", ".join(chr(34) + hexed(2016 * (k - 1)) + hexed(2016 * k - 1) + hexed(2016 * k) + chr(34) for k in BOUNDARIES)}];',
    f'RT_BOUNDARY_NEXT_HASH :: [{len(BOUNDARIES)}] string.[{", ".join(chr(34) + headers[2016 * k][0] + chr(34) for k in BOUNDARIES)}];',
    '// Block 966,143, chainparams.cpp\'s defaultAssumeValid, and nMinimumChainWork:',
    '// the chainwork Core\'s getblockheader prints for it.',
    f'RT_ASSUME_VALID_HEIGHT :: {ASSUME_VALID};',
    f'RT_ASSUME_VALID_HEADER :: "{hexed(ASSUME_VALID)}";',
    f'RT_ASSUME_VALID_HASH :: "{av_hash}";',
    f'RT_MIN_CHAIN_WORK :: "{MIN_WORK}";',
]
(HERE / 'retarget_vectors.coax').write_text('\n'.join(lines) + '\n')
print('wrote retarget_vectors.coax:', TOP, 'retargets,', len(BOUNDARIES), 'boundaries,', len(CORE), 'Core cases')
