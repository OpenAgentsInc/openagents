#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: a Python transliteration of Bitcoin Core's retargeting code decides each case outside Coax)
"""Bitcoin Core's difficulty retargeting, transliterated to Python.

Bitcoin From Scratch, chapter 48. From the pinned sources in
coax/tests/vectors/upstream/bitcoin-core/ at revision ddea9196:

- pow.cpp: GetNextWorkRequired (with testnet's minimum-difficulty rule),
  CalculateNextWorkRequired (with BIP-94's first-block nBits) and
  PermittedDifficultyTransition;
- arith_uint256.cpp: base_uint's operator*=(uint32_t), which wraps at
  2^256, on pow_reference.py's limbs;
- chain.cpp: CBlockIndex's GetAncestor, here a walk through pprev, and
  nChainWork as the sum of GetBlockProof (chapter 47's proof);
- chainparams.cpp: the retargeting fields of mainnet, testnet3, testnet4
  and regtest, and test/pow_tests.cpp's sanity_check_chainparams.

The chain is a linked list of Index objects, walked through pprev as Core
walks it. Nothing here shares code with Coax; the 256-bit arithmetic is
pow_reference.py's (chapter 47).
"""
from dataclasses import dataclass

import pow_reference as P

# The chapter's result codes (retarget.coax).
OK, BAD_DIFFBITS, NOT_PERMITTED, PERIOD, PARAMS = range(5)


@dataclass(frozen=True)
class Params:
    """The Consensus::Params fields retargeting reads."""
    pow_limit: int
    spacing: int          # nPowTargetSpacing
    timespan: int         # nPowTargetTimespan
    allow_min: bool       # fPowAllowMinDifficultyBlocks
    bip94: bool           # enforce_BIP94
    no_retargeting: bool  # fPowNoRetargeting

    def interval(self):
        """DifficultyAdjustmentInterval."""
        return self.timespan // self.spacing


TWO_WEEKS = 14 * 24 * 60 * 60
MAIN = Params(P.POW_LIMIT_MAIN, 600, TWO_WEEKS, False, False, False)
TESTNET3 = Params(P.POW_LIMIT_MAIN, 600, TWO_WEEKS, True, False, False)
TESTNET4 = Params(P.POW_LIMIT_MAIN, 600, TWO_WEEKS, True, True, False)
REGTEST = Params(P.POW_LIMIT_REGTEST, 600, 24 * 60 * 60, True, False, True)


class Index:
    """CBlockIndex: height, time, nBits and the previous block."""
    def __init__(self, height, time, bits, prev=None):
        self.nHeight, self.nTime, self.nBits, self.pprev = height, time, bits, prev

    def GetBlockTime(self):
        return self.nTime

    def GetAncestor(self, height):
        if height > self.nHeight or height < 0:
            return None
        p = self
        while p.nHeight > height:
            p = p.pprev
        return p


def chain(first_height, stamps):
    """Index objects for heights first_height.. with the given (time, bits),
    linked through pprev; returns the last."""
    prev = None
    for i, (t, b) in enumerate(stamps):
        prev = Index(first_height + i, t, b, prev)
    return prev


def mul32(v, b32):
    """base_uint::operator*=(uint32_t): carries limb by limb, drops the top."""
    pn = P.limbs(v)
    out, carry = [0] * P.WIDTH, 0
    for i in range(P.WIDTH):
        n = carry + b32 * pn[i]
        out[i] = n & P.MASK32
        carry = n >> 32
    return P.value(out)


def divide(v, d):
    return P.value(P.div(P.limbs(v), P.limbs(d)))


def params_ok(p):
    """The checks this chapter makes on parameters before using them: the
    ones of test/pow_tests.cpp's sanity_check_chainparams that concern
    retargeting, and that 4 * timespan fits the uint32_t Core multiplies by."""
    if p.spacing <= 0 or p.timespan <= 0 or p.timespan % p.spacing != 0 or p.pow_limit == 0:
        return False
    if p.timespan * 4 > 0xFFFFFFFF:
        return False
    if not p.no_retargeting:
        targ_max = divide((1 << 256) - 1, p.timespan * 4)
        if not p.pow_limit < targ_max:
            return False
    return True


def calculate_next_work_required(pindexLast, nFirstBlockTime, params):
    """pow.cpp CalculateNextWorkRequired."""
    if params.no_retargeting:
        return pindexLast.nBits
    nActualTimespan = pindexLast.GetBlockTime() - nFirstBlockTime
    if nActualTimespan < params.timespan // 4:
        nActualTimespan = params.timespan // 4
    if nActualTimespan > params.timespan * 4:
        nActualTimespan = params.timespan * 4
    bnPowLimit = params.pow_limit
    if params.bip94:
        nHeightFirst = pindexLast.nHeight - (params.interval() - 1)
        pindexFirst = pindexLast.GetAncestor(nHeightFirst)
        bnNew = P.set_compact(pindexFirst.nBits)[0]
    else:
        bnNew = P.set_compact(pindexLast.nBits)[0]
    bnNew = mul32(bnNew, nActualTimespan & 0xFFFFFFFF)
    bnNew = divide(bnNew, params.timespan)
    if bnNew > bnPowLimit:
        bnNew = bnPowLimit
    return P.get_compact(bnNew)


def get_next_work_required(pindexLast, new_time, params):
    """pow.cpp GetNextWorkRequired; new_time is pblock->GetBlockTime()."""
    nProofOfWorkLimit = P.get_compact(params.pow_limit)
    if (pindexLast.nHeight + 1) % params.interval() != 0:
        if params.allow_min:
            if new_time > pindexLast.GetBlockTime() + params.spacing * 2:
                return nProofOfWorkLimit
            pindex = pindexLast
            while pindex.pprev and pindex.nHeight % params.interval() != 0 and pindex.nBits == nProofOfWorkLimit:
                pindex = pindex.pprev
            return pindex.nBits
        return pindexLast.nBits
    nHeightFirst = pindexLast.nHeight - (params.interval() - 1)
    assert nHeightFirst >= 0
    pindexFirst = pindexLast.GetAncestor(nHeightFirst)
    assert pindexFirst
    return calculate_next_work_required(pindexLast, pindexFirst.GetBlockTime(), params)


def permitted(params, height, old_nbits, new_nbits):
    """pow.cpp PermittedDifficultyTransition."""
    if params.allow_min:
        return True
    if height % params.interval() == 0:
        smallest_timespan = params.timespan // 4
        largest_timespan = params.timespan * 4
        pow_limit = params.pow_limit
        observed_new_target = P.set_compact(new_nbits)[0]
        largest = divide(mul32(P.set_compact(old_nbits)[0], largest_timespan), params.timespan)
        if largest > pow_limit:
            largest = pow_limit
        maximum_new_target = P.set_compact(P.get_compact(largest))[0]
        if maximum_new_target < observed_new_target:
            return False
        smallest = divide(mul32(P.set_compact(old_nbits)[0], smallest_timespan), params.timespan)
        if smallest > pow_limit:
            smallest = pow_limit
        minimum_new_target = P.set_compact(P.get_compact(smallest))[0]
        if minimum_new_target > observed_new_target:
            return False
    elif old_nbits != new_nbits:
        return False
    return True


def chain_work(stamps):
    """nChainWork of the last block: the sum of GetBlockProof from the first."""
    return sum(P.proof(b) for _, b in stamps)


# The chapter's procedures, as the Coax code exposes them: a view of the
# current period (the blocks from the last multiple of the interval to the
# last block), with its refusals.
def next_bits(params, period, last_height, new_time):
    if not params_ok(params):
        return PARAMS, 0
    if last_height < 0 or len(period) != last_height % params.interval() + 1:
        return PERIOD, 0
    first = last_height - (len(period) - 1)
    # A block before the period, so that pprev exists past its first block.
    prefix = [(0, 0)] if first > 0 else []
    last = chain(first - len(prefix), prefix + list(period))
    return OK, get_next_work_required(last, new_time, params)


def check(params, period, last_height, new_time, new_bits):
    code, bits = next_bits(params, period, last_height, new_time)
    if code != OK:
        return code
    return OK if bits == new_bits else BAD_DIFFBITS


def permitted_code(params, height, old_nbits, new_nbits):
    if not params_ok(params):
        return PARAMS
    if height < 0:
        return PERIOD
    return OK if permitted(params, height, old_nbits, new_nbits) else NOT_PERMITTED
