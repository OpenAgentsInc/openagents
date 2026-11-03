#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: values read from the pinned Bitcoin Core sources and a Python transliteration of signet.cpp decide each case outside Coax)
"""The cross-check of Bitcoin chapter 50. It reads every chain parameter
from the pinned Bitcoin Core sources, builds and hashes each genesis block,
checks signet's block solutions with a transliteration of signet.cpp, and
writes the transcript the chapter's example must print:

  python3 -I coax/tests/bitcoin/chainparams.py                     # compare with the fixture
  python3 -I coax/tests/bitcoin/chainparams.py --coax coax/bin/coax   # also run the example

The sources, all at revision ddea9196: kernel/chainparams.cpp (each
network's class), kernel/signet.h (the default challenge and
GetSignetMessageStart), signet.cpp (SignetTxs::Create and
CheckSignetBlockSolution), consensus/params.h (the buried deployments) and
validation.cpp (the BIP-30 blocks). The values are read with regular
expressions from Core's text, not copied, and then compared with every
field of the parameter constants in coax/lib/bitcoin/chainparams.coax and
chapter 48's retarget.coax. Transactions are read by tx_reference.py and
scripts run by script_reference.py, chapters 34 and 36's transliterations
of Core, which share no code with Coax.
"""
import hashlib, re, subprocess, sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import tx_reference as T  # noqa: E402
import script_reference as S  # noqa: E402

REPO = HERE.parents[2]
UP = HERE.parent / 'vectors' / 'upstream'
FIXTURE = REPO / 'bitcoin/fixtures/chain-params.out'
EXAMPLE = 'bitcoin/examples/chain-params.coax'
LIB = REPO / 'coax/lib/bitcoin/chainparams.coax'
RETARGET = REPO / 'coax/lib/bitcoin/retarget.coax'
VECTORS = HERE / 'chainparams_vectors.coax'
BLOCK_1 = '00000086d6b2636cb2a392d45edc4ec544a10024d30141c9adf4bfd9de533b53'
BLOCK_19990 = '0000008448acc419d5f54a777cf1fe519cb6a2e0998bedc9322978a276539466'
PINS = {
    'bitcoin-core/chainparams.cpp': 'cfadf1478377c32625280520655b598674e6d487f08d7f7a80ac69252104b5e1',
    'bitcoin-core/kernel_signet.h': '8a0b9207b16dfad04214f42ac1ccc70f252025ed2a6f7ad2a5163b2a1c2fb118',
    'bitcoin-core/signet.cpp': '58f92d2bbcae1f609016e05ddf4be0b5eec1b1e126e3910ed2c5c49e7e914908',
    'bitcoin-core/params.h': '21bc0a1421ca71b057325fac96ef1a7297b737a47c3231a6c7c7ff8e0598e2f0',
    'bitcoin-core/validation.cpp': '65e88786ccaaf4dd0ebd14e7a0b82d7fbe6b86a8f0b2f9de4cc2db7b983283d2',
    f'signet/{BLOCK_1}.raw': '85b2a673710c23cb88dd37c6b742900627bb0a7a4df65a2f44d0c91a2b45f50d',
    f'signet/{BLOCK_19990}.raw': '7f1098495673b86ddc983431f9e00c29a949b7cd733da0c514118852b7dfbb1f',
}
CLASSES = [('main', 'CMainParams'), ('test', 'CTestNetParams'), ('testnet4', 'CTestNet4Params'),
           ('signet', 'SigNetParams'), ('regtest', 'CRegTestParams')]
COIN = 100000000


def pinned(name):
    data = (UP / name).read_bytes()
    assert hashlib.sha256(data).hexdigest() == PINS[name], name
    return data


def sha256d(b):
    return hashlib.sha256(hashlib.sha256(b).digest()).digest()


def one(pattern, text, group=1):
    m = re.search(pattern, text)
    assert m, pattern
    return m.group(group)


def num(text):
    return int(text.replace("'", ''), 0)


# ---- the networks, read from chainparams.cpp
def networks():
    cpp = pinned('bitcoin-core/chainparams.cpp').decode()
    signet_h = pinned('bitcoin-core/kernel_signet.h').decode()
    assert 'checkpoint' not in cpp.lower(), 'the pinned Core has checkpoints again'
    default_challenge = bytes.fromhex(one(r'SIGNET_DEFAULT_CHALLENGE\{\s*"([0-9a-f]+)"_hex_v_u8', signet_h))
    starts = [cpp.index(f'class {c} ') for _, c in CLASSES] + [cpp.index('std::unique_ptr<const CChainParams> CChainParams::SigNet')]
    nets = []
    for k, (name, cls) in enumerate(CLASSES):
        body = cpp[starts[k]:starts[k + 1]]
        if name == 'signet':
            # The default-challenge branch: the custom one has no work or assumevalid.
            body_work = body[body.index('if (!options.challenge)'):body.index('} else {')]
        else:
            body_work = body
        c = lambda field: one(rf'consensus\.{field} = ([^;]+);', body)
        n = dict(name=name)
        if name == 'signet':
            n['challenge'] = default_challenge
            n['magic'] = sha256d(T.compact(len(default_challenge)) + default_challenge)[:4]
            assert 'pchMessageStart = kernel::GetSignetMessageStart(consensus.signet_challenge);' in body
            assert 'h << signet_challenge;' in signet_h and 'std::copy_n(hash.begin(), 4, msg_start.begin());' in signet_h
        else:
            n['challenge'] = b''
            n['magic'] = bytes(num(one(rf'pchMessageStart\[{i}\] = (0x[0-9a-f]+);', body)) for i in range(4))
        n['port'] = num(one(r'nDefaultPort = (\d+);', body))
        n['pubkey'] = num(one(r'base58Prefixes\[PUBKEY_ADDRESS\] = std::vector<unsigned char>\(1,(\d+)\);', body))
        n['script'] = num(one(r'base58Prefixes\[SCRIPT_ADDRESS\] = std::vector<unsigned char>\(1,(\d+)\);', body))
        n['secret'] = num(one(r'base58Prefixes\[SECRET_KEY\] = +std::vector<unsigned char>\(1,(\d+)\);', body))
        ext = lambda key: int(''.join(x[2:].lower().zfill(2) for x in one(rf'base58Prefixes\[{key}\] = \{{([^}}]+)\}};', body).replace(' ', '').split(',')), 16)
        n['xpub'], n['xprv'] = ext('EXT_PUBLIC_KEY'), ext('EXT_SECRET_KEY')
        n['hrp'] = one(r'bech32_hrp = "([a-z]+)";', body)
        n['halving'] = num(c('nSubsidyHalvingInterval'))
        for f in ['BIP34Height', 'BIP65Height', 'BIP66Height', 'CSVHeight', 'SegwitHeight', 'MinBIP9WarningHeight']:
            n[f] = num(one(rf'consensus\.{f} = (\d+);', body))
        h = c('BIP34Hash')
        n['BIP34Hash'] = one(r'"([0-9a-f]{64})"', h) if '"' in h else ''
        n['powLimit'] = int(one(r'"([0-9a-f]{64})"', c('powLimit')), 16)
        n['timespan'] = eval(c('nPowTargetTimespan'), {})
        n['spacing'] = eval(c('nPowTargetSpacing'), {})
        n['min_difficulty'] = c('fPowAllowMinDifficultyBlocks') == 'true'
        n['bip94'] = c('enforce_BIP94') == 'true'    # regtest's opts.enforce_bip94 is off by default
        n['no_retargeting'] = c('fPowNoRetargeting') == 'true'
        m = re.search(r'consensus\.nMinimumChainWork = uint256\{"([0-9a-f]{64})"\};', body_work)
        n['work'] = m.group(1) if m else ''
        m = re.search(r'consensus\.defaultAssumeValid = uint256\{"([0-9a-f]{64})"\}; // (\d+)', body_work)
        n['assume'], n['assume_height'] = (m.group(1), int(m.group(2))) if m else ('', 0)
        n['bip16'] = (re.search(r'// BIP16 exception\s+uint256\{"([0-9a-f]{64})"\}, SCRIPT_VERIFY_NONE', body) or [None, ''])[1]
        n['taproot'] = (re.search(r'// Taproot exception\s+uint256\{"([0-9a-f]{64})"\}, SCRIPT_VERIFY_P2SH \| SCRIPT_VERIFY_WITNESS', body) or [None, ''])[1]
        # The genesis block's arguments and the two asserted hashes.
        if name == 'testnet4':
            message = one(r'testnet4_genesis_msg = "([^"]+)";', body)
            key = bytes.fromhex(one(r'testnet4_genesis_script = CScript\(\) << "([0-9a-f]+)"_hex << OP_CHECKSIG;', body))
            args = one(r'genesis = CreateGenesisBlock\(testnet4_genesis_msg,\s*testnet4_genesis_script,\s*([^;]+)\);', body)
        else:
            message = one(r'pszTimestamp = "([^"]+)";', cpp)
            key = bytes.fromhex(one(r'genesisOutputScript = CScript\(\) << "([0-9a-f]+)"_hex << OP_CHECKSIG;', cpp))
            args = one(r'genesis = CreateGenesisBlock\(([^;]+)\);', body)
        a = [x.strip() for x in args.split(',')]
        n['genesis'] = dict(message=message.encode(), script=S.push_data(key) + b'\xac', time=num(a[0]), nonce=num(a[1]),
                            bits=num(a[2]), version=num(a[3]), reward=eval(a[4].replace('COIN', str(COIN)), {}),
                            hash=one(r'hashGenesisBlock == uint256\{"([0-9a-f]{64})"\}', body),
                            root=one(r'hashMerkleRoot == uint256\{"([0-9a-f]{64})"\}', body))
        nets.append(n)
    return nets


# ---- the genesis block, as CreateGenesisBlock builds it
def script_num_push(v):
    return S.push_data(S.num_serialize(v))


def genesis_block(g):
    script_sig = script_num_push(486604799) + script_num_push(4) + S.push_data(g['message'])
    tx = dict(version=(1).to_bytes(4, 'little'),
              ins=[dict(prevout=bytes(32) + b'\xff\xff\xff\xff', script=script_sig, sequence=b'\xff\xff\xff\xff', witness=[])],
              outs=[dict(value=g['reward'].to_bytes(8, 'little'), script=g['script'])],
              locktime=bytes(4), segwit=False)
    raw = T.serialize(tx, False)
    root = sha256d(raw)
    header = (g['version'].to_bytes(4, 'little') + bytes(32) + root + g['time'].to_bytes(4, 'little')
              + g['bits'].to_bytes(4, 'little') + g['nonce'].to_bytes(4, 'little'))
    return header + b'\x01' + raw


def target(bits):
    e, m = bits >> 24, bits & 0x7fffff
    return m >> (8 * (3 - e)) if e <= 3 else m << (8 * (e - 3))


def compact(t):
    size = (t.bit_length() + 7) // 8
    m = t >> (8 * (size - 3)) if size > 3 else t << (8 * (3 - size))
    if m & 0x00800000:
        m >>= 8
        size += 1
    return (size << 24) | m


# ---- signet's block solution: signet.cpp
FLAGS = S.F['P2SH'] | S.F['WITNESS'] | S.F['DERSIG'] | S.F['NULLDUMMY']
HEADER = bytes.fromhex('ecc7daa2')


def read_block(b):
    """The block's transactions: tx_reference reads one at a time."""
    count, at = b[80], 81
    txs = []
    for _ in range(count):
        end = at
        while True:
            end += 1
            try:
                tx = T.unserialize(b[at:end], True)
                break
            except T.Refused:
                assert end < len(b)
        txs.append(tx)
        at = end
    assert at == len(b)
    return txs


def commitment_index(cb):
    """GetWitnessCommitmentIndex."""
    found = None
    for o, out in enumerate(cb['outs']):
        s = out['script']
        if len(s) >= 38 and s[:6] == bytes.fromhex('6a24aa21a9ed'):
            found = o
    return found


def fetch_and_clear(s):
    """FetchAndClearCommitmentSection: (new script, solution or None)."""
    out, result, pc = b'', None, 0
    while pc < len(s):
        ok, op, data, pc = S.get_op(s, pc)
        if not ok: break
        if data:
            if result is None and len(data) > 4 and data[:4] == HEADER:
                result, data = data[4:], data[:4]
            out += S.push_data(data)
        else:
            out += bytes([op])
    return (out, result) if result is not None else (s, None)


def read_solution(sol):
    """SpanReader >> scriptSig >> witness stack, and nothing after."""
    class Short(Exception): pass
    at = 0
    def compact_size():
        nonlocal at
        if at >= len(sol): raise Short
        first = sol[at]
        width = {0xfd: 2, 0xfe: 4, 0xff: 8}.get(first, 0)
        if at + 1 + width > len(sol): raise Short
        v = first if not width else int.from_bytes(sol[at + 1:at + 1 + width], 'little')
        if width and v < (0xfd if width == 2 else 0x10000 if width == 4 else 1 << 32): raise Short
        if v > T.MAX_SIZE: raise Short
        at += 1 + width
        return v
    def take(n):
        nonlocal at
        if at + n > len(sol): raise Short
        at += n
        return sol[at - n:at]
    try:
        script_sig = take(compact_size())
        witness = [take(compact_size()) for _ in range(compact_size())]
    except Short:
        return None
    return (script_sig, witness) if at == len(sol) else None


def signet_check(b, challenge, genesis_hash):
    """CheckSignetBlockSolution: ('ok' | a refusal, script error name)."""
    if sha256d(b[:80])[::-1].hex() == genesis_hash: return 'ok', None
    txs = read_block(b)
    cb = dict(txs[0], outs=[dict(o) for o in txs[0]['outs']])
    cidx = commitment_index(cb)
    if cidx is None: return 'no witness commitment', None
    cb['outs'][cidx]['script'], solution = fetch_and_clear(cb['outs'][cidx]['script'])
    script_sig, witness = b'', []
    if solution is not None:
        got = read_solution(solution)
        if got is None: return 'solution does not read', None
        script_sig, witness = got
    leaves = [T.txid(cb)] + [T.txid(t) for t in txs[1:]]
    while len(leaves) > 1:
        if len(leaves) % 2: leaves.append(leaves[-1])
        leaves = [sha256d(leaves[i] + leaves[i + 1]) for i in range(0, len(leaves), 2)]
    data = b[:36] + leaves[0] + b[68:72]
    to_spend = dict(version=bytes(4), ins=[dict(prevout=bytes(32) + b'\xff\xff\xff\xff', script=b'\x00' + S.push_data(data),
                    sequence=bytes(4), witness=[])], outs=[dict(value=bytes(8), script=challenge)], locktime=bytes(4), segwit=False)
    to_sign = dict(version=bytes(4), ins=[dict(prevout=T.txid(to_spend) + bytes(4), script=script_sig, sequence=bytes(4),
                   witness=witness)], outs=[dict(value=bytes(8), script=b'\x6a')], locktime=bytes(4), segwit=bool(witness))
    error = S.verify_script(script_sig, challenge, witness, FLAGS, S.Checker(to_sign, 0, 0))
    return ('ok', None) if error == 0 else ('solution invalid', S.ERRORS[error])


# ---- the Coax constants, compared field by field
def coax_params():
    """Each CHAIN_*_PARAMS literal of chainparams.coax as a dict of text."""
    text = LIB.read_text()
    consts = dict(re.findall(r'^([A-Z0-9_]+) :: ("[^"]*");', text, re.M))
    out = {}
    for label, name in [('MAINNET', 'main'), ('TESTNET3', 'test'), ('TESTNET4', 'testnet4'), ('SIGNET', 'signet'), ('REGTEST', 'regtest')]:
        body = one(rf'CHAIN_{label}_PARAMS :: ChainParams\.\{{(.*?)\}};', text.replace('\n', ' '))
        fields = dict(re.findall(r'(\w+)=("[^"]*"|\[4\]u8\.\[[^\]]*\]|RETARGET_\w+|[\w\s*]+)', body))
        fields = {k: consts.get(v.strip(), v.strip()) for k, v in fields.items()}
        out[name] = fields
    return out, consts


def check_constants(nets):
    lib, consts = coax_params()
    retarget = RETARGET.read_text()
    unq = lambda v: v.strip('"')
    for n in nets:
        f = lib[n['name']]
        magic = bytes(int(x, 16) for x in one(r'\[4\]u8\.\[([^\]]*)\]', f['magic']).split(','))
        assert magic == n['magic'], n['name']
        g = n['genesis']
        want = dict(port=n['port'], pubkey_prefix=n['pubkey'], script_prefix=n['script'], secret_prefix=n['secret'],
                    xpub=n['xpub'], xprv=n['xprv'], halving=n['halving'], bip34_height=n['BIP34Height'],
                    bip65_height=n['BIP65Height'], bip66_height=n['BIP66Height'], csv_height=n['CSVHeight'],
                    segwit_height=n['SegwitHeight'], min_bip9_warning=n['MinBIP9WarningHeight'],
                    assume_valid_height=n['assume_height'], time=g['time'], nonce=g['nonce'], bits=g['bits'],
                    version=g['version'])
        for k, v in want.items():
            assert int(f[k], 0) == v, (n['name'], k, f[k], v)
        assert eval(f['reward'].replace('CHAIN_COIN', str(COIN)), {}) == g['reward']
        strings = dict(hrp=n['hrp'], bip34_hash=n['BIP34Hash'], bip16_exception=n['bip16'], taproot_exception=n['taproot'],
                       min_chain_work=n['work'], assume_valid=n['assume'], hash=g['hash'], merkle_root=g['root'],
                       message=g['message'].decode(), script=g['script'].hex())
        for k, v in strings.items():
            assert unq(f.get(k, '""')) == v, (n['name'], k)
        assert unq(f.get('challenge', '""')) == n['challenge'].hex()
        assert (f.get('signet', 'false') == 'true') == (n['name'] == 'signet')
        # The retargeting fields: chapter 48's constants, and signet's here.
        r = f['retarget']
        src = retarget if r in retarget else LIB.read_text()
        body = one(rf'{r} :: RetargetParams\.\{{(.*?)\}};', src)
        limit = one(r'pow_limit=(U256\.\{limb=\[8\]u32\.\[[^\]]*\]\}|\w+)', body)
        if limit.startswith('U256'):
            limbs = [int(x, 16) for x in one(r'\[8\]u32\.\[([^\]]*)\]', limit).split(',')]
        else:
            limbs = [int(x, 16) for x in one(rf'{limit} :: U256\.\{{limb=\[8\]u32\.\[([^\]]*)\]\}};', retarget + LIB.read_text() + (REPO / 'coax/lib/bitcoin/pow.coax').read_text()).split(',')]
        assert sum(v << (32 * i) for i, v in enumerate(limbs)) == n['powLimit'], n['name']
        assert eval(one(r'spacing=([^,]+),', body), {}) == n['spacing']
        assert eval(one(r'timespan=([^,}]+)', body).replace('TWO_WEEKS', '1209600'), {}) == n['timespan']
        for flag, key in [('allow_min_difficulty', 'min_difficulty'), ('bip94', 'bip94'), ('no_retargeting', 'no_retargeting')]:
            assert (f'{flag}=true' in body) == n[key], (n['name'], flag)
    # The BIP-30 blocks and the limit, from validation.cpp.
    v = pinned('bitcoin-core/validation.cpp').decode()
    repeat = re.findall(r'nHeight==(\d+) && block_index\.GetBlockHash\(\) == uint256\{"([0-9a-f]{64})"\}', v)
    unspendable = re.findall(r'block_height==(\d+) && block_hash == uint256\{"([0-9a-f]{64})"\}', v)
    text = LIB.read_text()
    assert [int(h) for h, _ in repeat] == [int(x) for x in one(r'CHAIN_BIP30_REPEAT_HEIGHTS :: \[2\] int\.\[([^\]]*)\]', text).split(',')]
    assert [int(h) for h, _ in unspendable] == [int(x) for x in one(r'CHAIN_BIP30_UNSPENDABLE_HEIGHTS :: \[2\] int\.\[([^\]]*)\]', text).split(',')]
    flat = text.replace('\n', ' ')
    assert re.findall(r'"([0-9a-f]{64})"', one(r'CHAIN_BIP30_REPEAT_HASHES :: \[2\] string\.\[([^\]]*)\]', flat)) == [x for _, x in repeat]
    assert re.findall(r'"([0-9a-f]{64})"', one(r'CHAIN_BIP30_UNSPENDABLE_HASHES :: \[2\] string\.\[([^\]]*)\]', flat)) == [x for _, x in unspendable]
    limit = int(one(r'BIP34_IMPLIES_BIP30_LIMIT = (\d+);', v))
    assert int(one(r'CHAIN_BIP34_IMPLIES_BIP30_LIMIT :: (\d+);', text)) == limit
    # The buried deployments are the five Core lists; Taproot is not one.
    params = pinned('bitcoin-core/params.h').decode()
    buried = re.findall(r'DEPLOYMENT_(\w+)', params[params.index('enum BuriedDeployment'):params.index('constexpr bool ValidDeployment(BuriedDeployment')])
    assert buried == ['HEIGHTINCB', 'CLTV', 'DERSIG', 'CSV', 'SEGWIT'], buried
    # The signet blocks in the vectors file are the pinned bytes.
    vectors = VECTORS.read_text()
    assert one(r'SIGNET_BLOCK_1 :: "([0-9a-f]+)";', vectors) == pinned(f'signet/{BLOCK_1}.raw').hex()
    assert one(r'SIGNET_BLOCK_19990 :: "([0-9a-f]+)";', vectors) == pinned(f'signet/{BLOCK_19990}.raw').hex()
    assert SIGNET_FLAGS_TEXT in pinned('bitcoin-core/signet.cpp').decode()
    return limit, repeat, unspendable


SIGNET_FLAGS_TEXT = 'BLOCK_SCRIPT_VERIFY_FLAGS = SCRIPT_VERIFY_P2SH | SCRIPT_VERIFY_WITNESS | SCRIPT_VERIFY_DERSIG | SCRIPT_VERIFY_NULLDUMMY;'


def transcript():
    nets = networks()
    limit, repeat, unspendable = check_constants(nets)
    out = []
    p = out.append
    yes = lambda b: 'yes' if b else 'no'
    for n in nets:
        g = n['genesis']
        b = genesis_block(g)
        h = sha256d(b[:80])[::-1].hex()
        root = b[36:68][::-1].hex()
        assert root == g['root'] and h == g['hash'], n['name']
        assert int(h, 16) <= target(g['bits']) <= n['powLimit'] and compact(n['powLimit']) == g['bits']
        p(f"{n['name']}: magic {n['magic'].hex()}, port {n['port']}")
        p(f"  Base58 versions P2PKH {n['pubkey']}, P2SH {n['script']}, WIF {n['secret']}; BIP-32 {n['xpub']:08x} and {n['xprv']:08x}; bech32 \"{n['hrp']}\"")
        p(f"  genesis {h}: {len(b)} bytes, time {g['time']}, nonce {g['nonce']}, nBits {g['bits']:08x}: ok")
        p(f"    Merkle root {root}")
        p(f"  powLimit {n['powLimit']:064x}")
        p(f"  {n['spacing']} s a block, {n['timespan']} s a period ({n['timespan'] // n['spacing']} blocks); minimum-difficulty blocks "
          f"{yes(n['min_difficulty'])}, BIP-94 {yes(n['bip94'])}, retargeting {yes(not n['no_retargeting'])}")
        p(f"  the subsidy halves every {n['halving']} blocks")
        p(f"  heights: BIP-34 {n['BIP34Height']}, BIP-65 {n['BIP65Height']}, BIP-66 {n['BIP66Height']}, CSV {n['CSVHeight']}, segwit {n['SegwitHeight']}")
        if n['BIP34Hash']: p(f"    BIP-34 block {n['BIP34Hash']}")
        if n['bip16']: p(f"    verified with no flags: {n['bip16']}")
        if n['taproot']: p(f"    verified with P2SH and WITNESS only: {n['taproot']}")
        if not n['work']:
            p('  no minimum chain work, no assumed-valid block')
        else:
            p(f"  minimum chain work 0x{n['work']}")
            p(f"  assumed valid: block {n['assume_height']} ({n['assume']})")
    main, t4, signet = nets[0], nets[2], nets[3]
    b = genesis_block(main['genesis'])
    p(f'the mainnet genesis coinbase, {len(b) - 81} bytes:')
    p(f'  {b[81:].hex()}')
    p(f"  scriptSig: push 4 ffff001d (486604799), push 1 04, push 69 \"{main['genesis']['message'].decode()}\"")
    b = genesis_block(t4['genesis'])
    assert len(t4['genesis']['message']) == 76
    p(f"testnet4's message is 76 bytes, so its push is OP_PUSHDATA1 76: {b[81 + 42 + 7:81 + 42 + 9].hex()}; its output pays a key of 33 zero bytes")
    c = signet['challenge']
    p(f'signet\'s challenge, {len(c)} bytes: OP_1 <{c[2:35].hex()}>')
    p(f'  <{c[36:69].hex()}> OP_2 OP_CHECKMULTISIG')
    p(f"  magic: the first 4 bytes of SHA-256d({len(c):02x} || challenge) = {signet['magic'].hex()}")
    gh = signet['genesis']['hash']
    for height, name in [(1, BLOCK_1), (19990, BLOCK_19990)]:
        blk = pinned(f'signet/{name}.raw')
        assert sha256d(blk[:80])[::-1].hex() == name
        txs = read_block(blk)
        o = commitment_index(txs[0])
        spk = txs[0]['outs'][o]['script']
        cleared, sol = fetch_and_clear(spk)
        p(f"block {height} ({name}): {len(blk)} bytes, {len(txs)} {'transaction' if len(txs) == 1 else 'transactions'}")
        p(f'  commitment in output {o}, {len(spk)} bytes; the solution is {len(sol)} bytes after ecc7daa2')
        p(f'  scriptSig {sol[0]} bytes: OP_0 and a {sol[2]} byte signature; {sol[-1]} witness items')
        p(f'  cleared, the commitment is {len(cleared)} bytes, ending {cleared[-5:].hex()}')
        r, e = signet_check(blk, c, gh)
        p(f'  default signet: {r}' + (f' ({e})' if e else ''))
    p('block 1 changed:')
    base = bytearray(pinned(f'signet/{BLOCK_1}.raw'))
    op_true = bytes([0x51])
    true_magic = sha256d(T.compact(1) + op_true)[:4]
    def variant(label, edits, challenge=c):
        blk = bytearray(base)
        for at, v in edits: blk[at] = v
        r, e = signet_check(bytes(blk), challenge, gh)
        p(f'  {label}: {r}' + (f' ({e})' if e else ''))
    variant("one bit of the signature's r", [(230, base[230] ^ 1)])
    variant('one bit of the time', [(68, base[68] ^ 1)])
    variant('a scriptSig length one too long', [(216, 0x4a)])
    variant('ecc7daa3, so no solution', [(215, base[215] ^ 1)])
    p(f'  a signet whose challenge is OP_TRUE has magic {true_magic.hex()}')
    variant('ecc7daa3 on that signet', [(215, base[215] ^ 1)], op_true)
    variant('ab21a9ed, so no commitment', [(174, base[174] ^ 1)])
    variant('unchanged, on the OP_TRUE signet', [], op_true)
    magics = ' '.join(f"{n['magic'].hex()} {n['name']};" for n in nets)
    assert true_magic not in [n['magic'] for n in nets]
    p(f'network for magic: {magics} {true_magic.hex()} none')
    # 91,842 repeats 91,812's coinbase and 91,880 repeats 91,722's.
    for (h, _), (u, _) in zip(repeat, reversed(unspendable)):
        p(f'BIP-30: block {h} repeats the coinbase of block {u}')
    p(f'BIP-30 is checked again from height {limit}')
    return '\n'.join(out) + '\n'


def main():
    want = transcript()
    assert FIXTURE.read_text() == want, 'the fixture differs from the Python transcript'
    print(f'chain-params fixture: {want.count(chr(10))} lines agree with Python')
    if '--coax' in sys.argv:
        coax = sys.argv[sys.argv.index('--coax') + 1]
        for mode in ([], ['--release']):
            run = subprocess.run([coax, 'run', EXAMPLE] + mode, cwd=REPO, capture_output=True, text=True)
            assert run.returncode == 0 and run.stdout == want, f'native {mode or "debug"} output differs'
            print(f'native {"release" if mode else "debug"}: agrees')


if __name__ == '__main__':
    main()
