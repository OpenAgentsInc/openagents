#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: rules and values read from the pinned Bitcoin Core sources and Python transliterations of them decide each case outside Coax)
"""The cross-check of Bitcoin chapter 51. It reads the header rules from
the pinned Bitcoin Core sources, checks the constants of
coax/lib/bitcoin/header.coax against them, replays the chapter's example
with Python transliterations of ContextualCheckBlockHeader,
GetMedianTimePast and versionbits.cpp's GetStateFor (Core's own backward
walk with its cache), and writes the transcript the example must print:

  python3 -I coax/tests/bitcoin/header.py                     # compare with the fixture
  python3 -I coax/tests/bitcoin/header.py --coax coax/bin/coax   # also run the example

The sources at revision ddea9196: validation.cpp (the rules and their
order), chain.h (MAX_FUTURE_BLOCK_TIME, GetMedianTimePast), consensus.h
(MAX_TIMEWARP), params.h (BIP9Deployment's special values),
versionbits.h, versionbits_impl.h and versionbits.cpp, and
kernel/chainparams.cpp (the buried heights and each network's test
deployment). Taproot's Speedy Trial is read from chainparams.cpp at v22.0.
The headers are mempool.space's bytes, pinned by sha256. Retargeting is
chapter 48's transliteration, retarget_reference.py.
"""
import hashlib, re, subprocess, sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import retarget_reference as R  # noqa: E402

REPO = HERE.parents[2]
UP = HERE.parent / 'vectors' / 'upstream'
FIXTURE = REPO / 'bitcoin/fixtures/header-rules.out'
EXAMPLE = 'bitcoin/examples/header-rules.coax'
LIB = REPO / 'coax/lib/bitcoin/header.coax'
VECTORS = HERE / 'header_vectors.coax'
PINS = {
    'bitcoin-core/validation.cpp': '65e88786ccaaf4dd0ebd14e7a0b82d7fbe6b86a8f0b2f9de4cc2db7b983283d2',
    'bitcoin-core/chain.h': 'a1b863764bc6e40ed79a447805843eb7b18a09730eb3a849bf9fe43f17274aba',
    'bitcoin-core/consensus.h': 'aa55254b379a2cc5f1835b506e248f80b4f020120782d2780ce52ff7c8cb827d',
    'bitcoin-core/params.h': '21bc0a1421ca71b057325fac96ef1a7297b737a47c3231a6c7c7ff8e0598e2f0',
    'bitcoin-core/versionbits.h': 'babd3b786f7d787f88b47da1effd47f631b3e37cba95c1d501f1a553c960f100',
    'bitcoin-core/versionbits_impl.h': '6c7b5afcaff8d99b2e9e1e62d11bc76adb6164dcce8add41aabde51bcbe7a9ce',
    'bitcoin-core/versionbits.cpp': '4394f6fdf7656e0e6e4eb90a11c44875dab506345bde9a54d84ed9337d524ec8',
    'bitcoin-core/chainparams.cpp': 'cfadf1478377c32625280520655b598674e6d487f08d7f7a80ac69252104b5e1',
    'bitcoin-core/chainparams_v22.cpp': '55406041319840c962c4ca5a792111bcac6a30c88aca3dde812b324a15f9220c',
    'mainnet/mainnet-840102-840113.headers': '5fce735a8802701ea89ca98442496c48c70ffd81b654355d2b03a3c9f483adf1',
    'testnet4/testnet4-0-2005-2016.headers': 'a1fef921c3eea5d33e21dc6bcc92ca4405c450e3661d72a1cc1f3e56bce6b2c4',
}
CLASSES = [('main', 'CMainParams'), ('test', 'CTestNetParams'), ('testnet4', 'CTestNet4Params'),
           ('signet', 'SigNetParams'), ('regtest', 'CRegTestParams')]


def pinned(name):
    data = (UP / name).read_bytes()
    assert hashlib.sha256(data).hexdigest() == PINS[name], name
    return data


def text(name):
    return pinned(name).decode()


def one(pattern, s):
    found = re.findall(pattern, s)
    assert len(found) == 1, (pattern, found)
    return found[0]


def cpp_int(expr):
    """A C++ integer expression of digits, *, + and int64 limits."""
    expr = expr.strip().replace('UL', '')
    if expr == 'std::numeric_limits<int64_t>::max()':
        return (1 << 63) - 1
    assert re.fullmatch(r'[0-9xa-fA-F *+\-()]+', expr), expr
    return eval(expr, {'__builtins__': {}})


# ---- the rules, from Core's text
def rules():
    v = text('bitcoin-core/validation.cpp')
    body = v[v.index('static bool ContextualCheckBlockHeader'):v.index('static bool ContextualCheckBlock(')]
    reasons = re.findall(r'BlockValidationResult::\w+, (?:strprintf\()?"([a-z-]+)', body)
    assert reasons == ['bad-diffbits', 'time-too-old', 'time-timewarp-attack', 'time-too-new', 'bad-version'], reasons
    versions = re.findall(r'block\.nVersion < (\d) && DeploymentActiveAfter\(pindexPrev, chainman, Consensus::DEPLOYMENT_(\w+)\)', body)
    assert versions == [('2', 'HEIGHTINCB'), ('3', 'DERSIG'), ('4', 'CLTV')], versions
    assert 'nHeight % consensusParams.DifficultyAdjustmentInterval() == 0' in body
    assert 'block.GetBlockTime() < pindexPrev->GetBlockTime() - MAX_TIMEWARP' in body
    assert 'block.GetBlockTime() <= pindexPrev->GetMedianTimePast()' in body
    chain_h = text('bitcoin-core/chain.h')
    future = cpp_int(one(r'MAX_FUTURE_BLOCK_TIME = ([^;]+);', chain_h))
    span = cpp_int(one(r'static constexpr int nMedianTimeSpan = ([^;]+);', chain_h))
    assert 'return pbegin[(pend - pbegin) / 2];' in chain_h
    timewarp = cpp_int(one(r'MAX_TIMEWARP = ([^;]+);', text('bitcoin-core/consensus.h')))
    vb = text('bitcoin-core/versionbits.h')
    top_bits = cpp_int(one(r'VERSIONBITS_TOP_BITS = ([^;]+);', vb))
    top_mask = cpp_int(one(r'VERSIONBITS_TOP_MASK = ([^;]+);', vb))
    params = text('bitcoin-core/params.h')
    special = dict(always=cpp_int(one(r'ALWAYS_ACTIVE = ([^;]+);', params)),
                   never=cpp_int(one(r'NEVER_ACTIVE = ([^;]+);', params)),
                   no_timeout=cpp_int(one(r'NO_TIMEOUT = ([^;]+);', params)))
    impl = text('bitcoin-core/versionbits_impl.h')
    assert '(((nVersion & VERSIONBITS_TOP_MASK) == VERSIONBITS_TOP_BITS) && (nVersion & Mask()) != 0)' in impl
    names = re.findall(r'case (\w+): return "(\w+)";', text('bitcoin-core/versionbits.cpp'))
    assert [n for _, n in names] == ['defined', 'started', 'locked_in', 'active', 'failed'], names
    return dict(reasons=reasons, future=future, span=span, timewarp=timewarp, top_bits=top_bits,
                top_mask=top_mask, special=special, states=[n for _, n in names])


def networks(special):
    cpp = text('bitcoin-core/chainparams.cpp')
    starts = [cpp.index(f'class {c} ') for _, c in CLASSES] + [len(cpp)]
    nets = {}
    for k, (name, _) in enumerate(CLASSES):
        body = cpp[starts[k]:starts[k + 1]]
        c = lambda field: one(rf'consensus\.{field} = ([^;]+);', body)
        d = lambda field: c(rf'vDeployments\[Consensus::DEPLOYMENT_TESTDUMMY\]\.{field}')
        start = d('nStartTime')
        start = special['never'] if start.endswith('NEVER_ACTIVE') else cpp_int(start)
        timeout = d('nTimeout')
        assert timeout.endswith('NO_TIMEOUT')
        nets[name] = dict(
            heights={'HEIGHTINCB': cpp_int(c('BIP34Height')), 'DERSIG': cpp_int(c('BIP66Height')), 'CLTV': cpp_int(c('BIP65Height'))},
            bip94=c('enforce_BIP94') == 'true',
            dummy=dict(bit=cpp_int(d('bit')), start=start, timeout=special['no_timeout'],
                       min_height=cpp_int(d('min_activation_height')), period=cpp_int(d('period')),
                       threshold=cpp_int(d('threshold'))))
    v22 = text('bitcoin-core/chainparams_v22.cpp')
    main22 = v22[v22.index('class CMainParams '):v22.index('class CTestNetParams ')]
    t = lambda field: cpp_int(one(rf'vDeployments\[Consensus::DEPLOYMENT_TAPROOT\]\.{field} = ([^;]+);', main22))
    taproot = dict(bit=t('bit'), start=t('nStartTime'), timeout=t('nTimeout'), min_height=t('min_activation_height'),
                   period=cpp_int(one(r'nMinerConfirmationWindow = ([^;]+);', main22)),
                   threshold=cpp_int(one(r'nRuleChangeActivationThreshold = ([^;]+);', main22)))
    return nets, taproot


# ---- the transliterations
def mtp(times):
    """CBlockIndex::GetMedianTimePast over the given times."""
    s = sorted(times)
    return s[len(s) // 2]


def min_version(net, height):
    m = 1
    for v, dep in ((2, 'HEIGHTINCB'), (3, 'DERSIG'), (4, 'CLTV')):
        if height >= net['heights'][dep]:
            m = max(m, v)
    return m


def check(R_, net, prev_height, prev_times, required_bits, h, now):
    """ContextualCheckBlockHeader, written from Core's text in Core's order."""
    height = prev_height + 1
    if h['bits'] != required_bits:
        return 'bad-diffbits'
    if h['time'] <= mtp(prev_times):
        return 'time-too-old'
    if net['bip94'] and height % 2016 == 0 and h['time'] < prev_times[-1] - R_['timewarp']:
        return 'time-timewarp-attack'
    if h['time'] > now + R_['future']:
        return 'time-too-new'
    for v, dep in ((2, 'HEIGHTINCB'), (3, 'DERSIG'), (4, 'CLTV')):
        if h['version'] < v and height >= net['heights'][dep]:
            return 'bad-version'
    return 'ok'


def signals(R_, d, version):
    return (version & R_['top_mask']) == R_['top_bits'] and (version & (1 << d['bit'])) != 0


def get_state_for(R_, d, periods):
    """versionbits.cpp GetStateFor for the block after the last period, with
    its backward walk and cache; periods[k] = (mtp at its last block, count)."""
    if d['start'] == R_['special']['always']:
        return 'active'
    if d['start'] == R_['special']['never']:
        return 'failed'
    P = d['period']
    prev = len(periods) * P - 1 if periods else None    # height of pindexPrev
    cache, todo = {}, []
    while prev not in cache:
        if prev is None:
            cache[None] = 'defined'
            break
        k = prev // P
        if periods[k][0] < d['start']:
            cache[prev] = 'defined'
            break
        todo.append(prev)
        prev = prev - P if prev - P >= 0 else None
    state = cache[prev]
    while todo:
        prev = todo.pop()
        mt, count = periods[prev // P]
        nxt = state
        if state == 'defined' and mt >= d['start']:
            nxt = 'started'
        elif state == 'started':
            if count >= d['threshold']:
                nxt = 'locked_in'
            elif mt >= d['timeout']:
                nxt = 'failed'
        elif state == 'locked_in' and prev + 1 >= d['min_height']:
            nxt = 'active'
        cache[prev] = state = nxt
    return state


def compute_version(R_, d, state):
    v = R_['top_bits']
    if state in ('started', 'locked_in'):
        v |= 1 << d['bit']
    return v


# ---- headers
def headers(name):
    raw = pinned(name)
    out = []
    for i in range(0, len(raw), 80):
        b = raw[i:i + 80]
        v = int.from_bytes(b[0:4], 'little')
        out.append(dict(raw=b, version=v - (1 << 32) if v >= 1 << 31 else v, prev=b[4:36],
                        time=int.from_bytes(b[68:72], 'little'), bits=int.from_bytes(b[72:76], 'little'),
                        hash=hashlib.sha256(hashlib.sha256(b).digest()).digest()))
    return out


def h32(v):
    return f'0x{v & 0xffffffff:08x}'


def transcript():
    R_ = rules()
    nets, taproot = networks(R_['special'])
    main, t4, t3 = nets['main'], nets['testnet4'], nets['test']
    out = [f'limits: {R_["future"]} seconds ahead of the clock, {R_["timewarp"]} seconds of timewarp']
    hs = headers('mainnet/mainnet-840102-840113.headers')
    for i in range(1, 12):
        assert hs[i]['prev'] == hs[i - 1]['hash']
    out.append('mainnet:')
    for i, h in enumerate(hs):
        out.append(f'  {840102 + i} time {h["time"]} version {h32(h["version"])}')
    times = [h['time'] for h in hs[:11]]
    h = hs[11]
    m = mtp(times)
    out.append(f'  median time past {m} ; 840113 is {h["time"] - m} s after it and {times[10] - h["time"]} s before its parent')
    bits, now = hs[10]['bits'], h['time']
    out.append(f'  840113: {check(R_, main, 840112, times, bits, h, now)}')
    out.append(f'  time at the median: {check(R_, main, 840112, times, bits, dict(h, time=m), now)}')
    out.append(f'  one second later: {check(R_, main, 840112, times, bits, dict(h, time=m + 1), now)}')
    out.append(f'  clock two hours behind: {check(R_, main, 840112, times, bits, h, now - R_["future"])}')
    out.append(f'  and one second more: {check(R_, main, 840112, times, bits, h, now - R_["future"] - 1)}')
    out.append(f'  other nBits: {check(R_, main, 840112, times, bits ^ 1, h, now)}')
    out.append(f'  version 3: {check(R_, main, 840112, times, bits, dict(h, version=3), now)}')
    out.append('minimum versions:')
    for height in (0, 227930, 227931, 363724, 363725, 388380, 388381, 840113):
        out.append(f'  mainnet {height} : {min_version(main, height)}')
    out.append(f'  testnet3 21111 : {min_version(t3, 21111)}, 330776 : {min_version(t3, 330776)}, 581885 : {min_version(t3, 581885)}')
    out.append(f'  testnet4, signet and regtest from height 1 : {min_version(t4, 1)}, {min_version(nets["signet"], 1)}, {min_version(nets["regtest"], 1)}')

    ts = headers('testnet4/testnet4-0-2005-2016.headers')
    for i in range(2, 13):
        assert ts[i]['prev'] == ts[i - 1]['hash']
    assert ts[0]['hash'][::-1].hex() == '00000000da84f2bafbbc53dee25a72ae507ff4914b867c565be350b0da8bf043'
    times = [x['time'] for x in ts[1:12]]
    b = ts[12]
    last = R.Index(2015, ts[11]['time'], ts[11]['bits'], R.Index(0, ts[0]['time'], ts[0]['bits']))
    need = R.calculate_next_work_required(last, ts[0]['time'], R.TESTNET4)
    out.append('testnet4:')
    out.append(f'  2015 time {times[10]}, median time past {mtp(times)}; 2016 time {b["time"]}')
    out.append(f'  2016 needs nBits {h32(need)}, has {h32(b["bits"])}: {check(R_, t4, 2015, times, need, b, b["time"])}')
    times[10] += 3600
    out.append(f'  2015 an hour ahead, at {times[10]}')
    for back in (600, 601):
        e = dict(b, time=times[10] - back)
        out.append(f'  2016 {back} s before it: testnet4 {check(R_, t4, 2015, times, need, e, e["time"])}, mainnet\'s rules {check(R_, main, 2015, times, need, e, e["time"])}')
    e = dict(b, time=times[10] - 601)
    out.append(f'  the same header at height 2017: {check(R_, t4, 2016, times, need, e, e["time"])}')

    d = taproot
    out.append('version bits:')
    out.append(f'  Taproot: bit {d["bit"]}, start {d["start"]}, timeout {d["timeout"]}, minimum height {d["min_height"]}, {d["threshold"]} of {d["period"]}')
    rolled = dict(d, bit=13)
    n2 = sum(signals(R_, d, x['version']) for x in hs)
    n13 = sum(signals(R_, rolled, x['version']) for x in hs)
    out.append(f'  840102 to 840113 with bit 2 set: {n2} of 12; with bit 13 set: {n13} of 12')
    for v in (0x20000004, 0x20000000, 0x00000004, 0x60000004, 0x3ffffffc):
        out.append(f'  {h32(v)} signals Taproot: {"yes" if signals(R_, d, v) else "no"}')
    for s in R_['states']:
        out.append(f'  mined version while {s}: {h32(compute_version(R_, d, s))}')
    p = [(d['start'] - 100 if k < 338 else d['start'] + (k - 337) * 1000, d['threshold'] if k == 340 else d['threshold'] - 1)
         for k in range(353)]
    for k in (338, 339, 340, 341, 351, 352, 353):
        out.append(f'  period {k} (from height {k * d["period"]}): {get_state_for(R_, d, p[:k])}')
    out.append(f'  without the minimum height, period 342: {get_state_for(R_, dict(d, min_height=0), p[:342])}')
    p[340] = (p[340][0], d['threshold'] - 1)
    p[345] = (d['timeout'], p[345][1])
    out.append(f'  never enough, timeout reached in period 345: period 346 {get_state_for(R_, d, p[:346])}')
    p[345] = (p[345][0], d['threshold'])
    out.append(f'  the threshold reached in that period: period 346 {get_state_for(R_, d, p[:346])}')
    for name, _ in CLASSES:
        td = nets[name]['dummy']
        out.append(f'  {name} testdummy: start {td["start"]}, {td["threshold"]} of {td["period"]}')
    check_lib(R_, nets, taproot)
    return '\n'.join(out) + '\n'


def check_lib(R_, nets, taproot):
    """The constants of header.coax are Core's."""
    lib = LIB.read_text()
    k = lambda name: cpp_int(one(rf'(?m)^{name} :: ([^;]+);', lib))
    assert k('HEADER_MAX_FUTURE') == R_['future'] and k('HEADER_MAX_TIMEWARP') == R_['timewarp']
    assert k('VERSIONBITS_TOP_BITS') == R_['top_bits'] and k('VERSIONBITS_TOP_MASK') == R_['top_mask']
    assert k('BIP9_ALWAYS_ACTIVE') == R_['special']['always'] and k('BIP9_NEVER_ACTIVE') == R_['special']['never']
    assert k('BIP9_NO_TIMEOUT') == R_['special']['no_timeout']
    reasons = re.findall(r'"([a-z_ -]+)"', one(r'HEADER_REASONS :: \[7\] string\.\[([^\]]+)\]', lib))
    assert reasons[1:6] == R_['reasons'] and reasons[0] == 'ok', reasons
    names = re.findall(r'"(\w+)"', one(r'BIP9_NAMES :: \[5\] string\.\[([^\]]+)\]', lib))
    assert names == R_['states']
    for i, s in enumerate(R_['states']):
        assert k(f'BIP9_{s.upper()}') == i
    tp = dict(re.findall(r'(\w+)=(\d+)', one(r'TAPROOT_SPEEDY_TRIAL :: Bip9Deployment\.\{([^}]+)\}', lib)))
    assert {k_: int(v) for k_, v in tp.items()} == dict(bit=taproot['bit'], start=taproot['start'], timeout=taproot['timeout'],
                                                       min_activation_height=taproot['min_height'],
                                                       period=taproot['period'], threshold=taproot['threshold']), tp
    vec = VECTORS.read_text()
    assert one(r'HEADERS_MAINNET_840102 :: "([0-9a-f]+)";', vec) == pinned('mainnet/mainnet-840102-840113.headers').hex()
    assert one(r'HEADERS_TESTNET4_2005 :: "([0-9a-f]+)";', vec) == pinned('testnet4/testnet4-0-2005-2016.headers').hex()
    # The chain-params heights the minimum versions read are chapter 50's,
    # which chainparams.py checks; here only that testnet4 alone enforces BIP-94.
    assert [n for n in nets if nets[n]['bip94']] == ['testnet4']


def main():
    want = transcript()
    assert FIXTURE.read_text() == want, 'the fixture differs from the Python transcript'
    print(f'header-rules fixture: {want.count(chr(10))} lines agree with Python')
    if '--coax' in sys.argv:
        coax = sys.argv[sys.argv.index('--coax') + 1]
        for mode in ([], ['--release']):
            run = subprocess.run([coax, 'run', EXAMPLE] + mode, cwd=REPO, capture_output=True, text=True)
            assert run.returncode == 0 and run.stdout == want, f'native {mode or "debug"} output differs'
            print(f'native {"release" if mode else "debug"}: agrees')


if __name__ == '__main__':
    main()
