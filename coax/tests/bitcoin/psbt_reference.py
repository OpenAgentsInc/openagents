#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: a Python transliteration of Bitcoin Core's PSBT reader and roles, outside Coax)
"""Bitcoin Core's PSBT reader and the six PSBT roles in Python, for Bitcoin
chapter 94.

The reader transliterates PartiallySignedTransaction::Unserialize,
PSBTInput::Unserialize and PSBTOutput::Unserialize (psbt.h) and
DecodeRawPSBT (psbt.cpp) at the pinned revision ddea9196: BIP-174 v0,
BIP-370 v2, BIP-371's Taproot fields and BIP-373's MuSig2 fields, with
each of Core's failures as a code and Core's message. The roles follow
BIP-174's Creator, Updater, Signer (its simple signer algorithm and the
checks it lists), Combiner, Input Finalizer and Transaction Extractor,
with BIP-370's lock-time determination and BIP-371's key-path signer.
It shares no code with coax/lib/bitcoin/psbt.coax.
"""
import hashlib, hmac

import der_reference as D
import script_reference as S
import tx_reference as T

# ---- the refusal codes, numbered as psbt.coax numbers them, and Core's messages
REASONS = [
    'ok',
    'invalid base64',
    'Invalid PSBT magic bytes',
    'end of data',
    'non-canonical ReadCompactSize()',
    'ReadCompactSize(): size too large',
    'Unknown transaction optional data',
    'Superfluous witness record',
    'Duplicate Key, global key already provided',
    'Duplicate Key, input key already provided',
    'Duplicate Key, output key already provided',
    'Size of key was not the expected size',
    'Size of value was not the stated size',
    'Unsigned tx does not have empty scriptSigs and scriptWitnesses.',
    'Invalid pubkey',
    'Invalid length for HD key path',
    'Size of key was not the expected size for the type BIP32 keypath',
    'Size of key was not the expected size for the type partial signature pubkey',
    'Signature is not a valid encoding',
    'Unsupported version number',
    'Separator is missing at the end of the global map',
    'Separator is missing at the end of an input map',
    'Separator is missing at the end of an output map',
    'No unsigned transaction was provided',
    'PSBT_GLOBAL_TX_VERSION is not allowed in PSBTv0',
    'PSBT_GLOBAL_FALLBACK_LOCKTIME is not allowed in PSBTv0',
    'PSBT_GLOBAL_INPUT_COUNT is not allowed in PSBTv0',
    'PSBT_GLOBAL_OUTPUT_COUNT is not allowed in PSBTv0',
    'PSBT_GLOBAL_TX_MODIFIABLE is not allowed in PSBTv0',
    'There is no PSBT version 1',
    'PSBT_GLOBAL_TX_VERSION is required in PSBTv2',
    'PSBT_GLOBAL_INPUT_COUNT is required in PSBTv2',
    'PSBT_GLOBAL_OUTPUT_COUNT is required in PSBTv2',
    'PSBT_GLOBAL_UNSIGNED_TX is not allowed in PSBTv2',
    'Non-witness UTXO does not match outpoint hash',
    'Input specifies output index that does not exist',
    'Inputs provided does not match the number of inputs in transaction.',
    'Outputs provided does not match the number of outputs in transaction.',
    'extra data after PSBT',
    'Previous txid is not allowed in PSBTv0',
    "Previous output's index is not allowed in PSBTv0",
    'Sequence is not allowed in PSBTv0',
    'Required time based locktime is not allowed in PSBTv0',
    'Required time based locktime is invalid (less than 500000000)',
    'Required height based locktime is not allowed in PSBTv0',
    'Required height based locktime is invalid (greater than or equal to 500000000)',
    'Required height based locktime is invalid (0)',
    'Input Taproot key path signature is shorter than 64 bytes',
    'Input Taproot key path signature is longer than 65 bytes',
    'Input Taproot script path signature is shorter than 64 bytes',
    'Input Taproot script path signature is longer than 65 bytes',
    'Input Taproot leaf script key is not at least 34 bytes',
    "Input Taproot leaf script key's control block size is not valid",
    'Input Taproot leaf script must be at least 1 byte',
    'Input Taproot BIP32 keypath has an invalid length',
    'Input musig2 aggregate pubkey is invalid',
    'Output musig2 aggregate pubkey is invalid',
    'Input musig2 participant pubkey is invalid',
    'Output musig2 participant pubkey is invalid',
    'Input musig2 participants pubkeys value size is not a multiple of 33',
    'Output musig2 participants pubkeys value size is not a multiple of 33',
    'musig2 aggregate pubkey is invalid',
    'musig2 participant pubkey is invalid',
    'Input musig2 pubnonce key is not expected size of 67 or 99 bytes',
    'Input musig2 pubnonce value is not 66 bytes',
    'Input musig2 partial sig key is not expected size of 67 or 99 bytes',
    'Previous TXID is required in PSBTv2',
    "Previous output's index is required in PSBTv2",
    'Output amount is not allowed in PSBTv0',
    'Output script is not allowed in PSBTv0',
    'Output Taproot tree must not be empty',
    'Output Taproot tree has as leaf greater than Taproot maximum depth',
    'Output Taproot tree has a leaf with an invalid leaf version',
    'Output Taproot tree is malformed',
    'Output Taproot BIP32 keypath has an invalid length',
    'Output amount is required in PSBTv2',
    'Output script is required in PSBTv2',
    'capacity',
]
C = {}
for _k, _name in enumerate(['OK', 'BASE64', 'MAGIC', 'TRUNCATED', 'NONCANONICAL', 'SIZE_TOO_LARGE', 'TX_FLAG', 'TX_NO_WITNESS',
                            'DUP_GLOBAL', 'DUP_INPUT', 'DUP_OUTPUT', 'KEY_SIZE', 'VALUE_SIZE', 'TX_NOT_EMPTY', 'PUBKEY',
                            'KEYPATH_LENGTH', 'BIP32_KEY_SIZE', 'PARTIAL_KEY_SIZE', 'SIG_ENCODING', 'VERSION',
                            'SEP_GLOBAL', 'SEP_INPUT', 'SEP_OUTPUT', 'NO_TX', 'V0_TX_VERSION', 'V0_FALLBACK',
                            'V0_INPUT_COUNT', 'V0_OUTPUT_COUNT', 'V0_MODIFIABLE', 'V1', 'V2_TX_VERSION',
                            'V2_INPUT_COUNT', 'V2_OUTPUT_COUNT', 'V2_TX', 'UTXO_HASH', 'UTXO_INDEX', 'INPUT_COUNT',
                            'OUTPUT_COUNT', 'EXTRA', 'V0_PREV_TXID', 'V0_PREV_INDEX', 'V0_SEQUENCE', 'V0_TIME',
                            'TIME_LOW', 'V0_HEIGHT', 'HEIGHT_HIGH', 'HEIGHT_ZERO', 'TAP_KEY_SHORT', 'TAP_KEY_LONG',
                            'TAP_SCRIPT_SHORT', 'TAP_SCRIPT_LONG', 'LEAF_KEY_SHORT', 'LEAF_CONTROL', 'LEAF_EMPTY',
                            'IN_TAP_PATH', 'IN_MUSIG_AGG', 'OUT_MUSIG_AGG', 'IN_MUSIG_PART', 'OUT_MUSIG_PART',
                            'IN_MUSIG_MULTIPLE', 'OUT_MUSIG_MULTIPLE', 'MUSIG_AGG', 'MUSIG_PART', 'NONCE_KEY',
                            'NONCE_VALUE', 'PSIG_KEY', 'V2_PREV_TXID', 'V2_PREV_INDEX', 'V0_AMOUNT', 'V0_SCRIPT',
                            'TREE_EMPTY', 'TREE_DEPTH', 'TREE_LEAF_VERSION', 'TREE_MALFORMED', 'OUT_TAP_PATH',
                            'V2_AMOUNT', 'V2_SCRIPT', 'CAPACITY']):
    C[_name] = _k
assert len(C) == len(REASONS)

GLOBAL, INPUT, OUTPUT = 0, 1, 2
MAP_NAMES = ['global', 'input', 'output']
# ExpectedKeySize's names and sizes, by map and key type.
KEY_SIZES = {
    (GLOBAL, 0x00): ('Global Unsigned TX', 1), (GLOBAL, 0x01): ('Global XPUB', 79),
    (GLOBAL, 0x02): ('Global Transaction Version', 1), (GLOBAL, 0x03): ('Global Fallback Locktime', 1),
    (GLOBAL, 0x04): ('Global Input Count', 1), (GLOBAL, 0x05): ('Global Output Count', 1),
    (GLOBAL, 0x06): ('Global TX Modifiable Flags', 1), (GLOBAL, 0xfb): ('Global PSBT Version', 1),
    (INPUT, 0x00): ('Input Non-witness UTXO', 1), (INPUT, 0x01): ('Input Witness UTXO', 1),
    (INPUT, 0x03): ('Input Sighash Type', 1), (INPUT, 0x04): ('Input redeemScript', 1),
    (INPUT, 0x05): ('Input witnessScript', 1), (INPUT, 0x07): ('Input Final scriptSig', 1),
    (INPUT, 0x08): ('Input Final scriptWitness', 1), (INPUT, 0x0a): ('Input RIPEMD160 Preimage', 21),
    (INPUT, 0x0b): ('Input SHA256 Preimage', 33), (INPUT, 0x0c): ('Input Hash160 Preimage', 21),
    (INPUT, 0x0d): ('Input Hash256 Preimage', 33), (INPUT, 0x0e): ('Input Previous TXID', 1),
    (INPUT, 0x0f): ("Input Previous Output's Index", 1), (INPUT, 0x10): ('Input Sequence', 1),
    (INPUT, 0x11): ('Input Required Time Based Locktime', 1), (INPUT, 0x12): ('Input Required Height Based Locktime', 1),
    (INPUT, 0x13): ('Input Taproot Key Path Signature', 1), (INPUT, 0x14): ('Input Taproot Script Path Signature', 65),
    (INPUT, 0x16): ('Input Taproot BIP32 Keypath', 33), (INPUT, 0x17): ('Input Taproot Internal Key', 1),
    (INPUT, 0x18): ('Input Taproot Merkle Root', 1), (INPUT, 0x1a): ('Input MuSig2 Participants Pubkeys', 34),
    (OUTPUT, 0x00): ('Output redeemScript', 1), (OUTPUT, 0x01): ('Output witnessScript', 1),
    (OUTPUT, 0x03): ('Output Amount', 1), (OUTPUT, 0x04): ('Output Script', 1),
    (OUTPUT, 0x05): ('Output Taproot Internal Key', 1), (OUTPUT, 0x06): ('Output Taproot Tree Key', 1),
    (OUTPUT, 0x07): ('Output Taproot BIP32 Keypath', 33), (OUTPUT, 0x08): ('Output MuSig2 Participants Pubkeys', 34),
}
MAX_SIZE = 0x02000000


class Refused(Exception):
    def __init__(self, code, at, where=None):
        super().__init__(REASONS[code])
        self.code, self.at, self.where = code, at, where   # where: (map, key type) for KEY_SIZE

    def message(self):
        if self.code == C['KEY_SIZE']:
            name, size = KEY_SIZES[self.where]
            return f'Size of key was not {size} for the type {name}'
        return REASONS[self.code]


TX_CODES = {T.TRUNCATED: 'TRUNCATED', T.NONCANONICAL: 'NONCANONICAL', T.SIZE_TOO_LARGE: 'SIZE_TOO_LARGE',
            T.UNKNOWN_FLAG: 'TX_FLAG', T.NO_WITNESS: 'TX_NO_WITNESS'}


class Stream:
    """Core's SpanReader over the PSBT: every read past the end is "end of data"."""

    def __init__(self, b, at=0, end=None):
        self.b, self.at, self.end = b, at, len(b) if end is None else end

    def empty(self):
        return self.at >= self.end

    def take(self, n):
        if n > self.end - self.at: raise Refused(C['TRUNCATED'], self.at)
        out = self.b[self.at:self.at + n]
        self.at += n
        return out

    def compact(self):
        r = T.Reader(self.b[:self.end], None)
        r.at = self.at
        try:
            v = r.compact()
        except T.Refused as e:
            raise Refused(C[TX_CODES[e.code]], e.at)
        self.at = r.at
        return v

    def vector(self):
        return self.take(self.compact())

    def tx(self, witness):
        """UnserializeTransaction from here, leaving what follows."""
        rest = self.b[self.at:self.end]
        # tx_reference refuses trailing bytes; find the transaction's end by
        # trying each length as Core's stream reader stops on its own.
        r = T.Reader(rest, (1 << 62, 1 << 62, 1 << 62))
        try:
            t = read_tx(r, witness)
        except T.Refused as e:
            raise Refused(C[TX_CODES[e.code]], self.at + e.at)
        self.at += r.at
        return t


def read_tx(r, witness):
    """tx_reference.unserialize without the trailing-bytes check."""
    version = r.take(4)
    flags = 0
    ins = T.read_inputs(r, 1 << 62)
    outs = []
    if len(ins) == 0 and witness:
        flags = r.take(1)[0]
        if flags != 0:
            ins = T.read_inputs(r, 1 << 62)
            outs = T.read_outputs(r, 1 << 62)
    else:
        outs = T.read_outputs(r, 1 << 62)
    segwit = False
    if flags & 1 and witness:
        flags ^= 1
        start = r.at
        for i in ins:
            for _ in range(r.compact()):
                i['witness'].append(r.bytes())
        if not any(i['witness'] for i in ins): raise T.Refused(T.NO_WITNESS, start)
        segwit = True
    if flags: raise T.Refused(T.UNKNOWN_FLAG, 5)
    locktime = r.take(4)
    return dict(version=version, ins=ins, outs=outs, locktime=locktime, segwit=segwit)


def pubkey_valid(key):
    """CPubKey(begin, end).IsFullyValid(): the length its first byte names,
    then secp256k1_ec_pubkey_parse, hybrid keys included."""
    return S.parse_pubkey(bytes(key)) is not None


def sized(s, n, at):
    """UnserializeFromVector of an n-byte object: the stated size, then n
    bytes read whatever it says, then the sizes compared."""
    stated = s.compact()
    v = s.take(n)
    if stated != n: raise Refused(C['VALUE_SIZE'], at)
    return v


def from_vector(s, read, at):
    """UnserializeFromVector of an object read by read(s)."""
    stated = s.compact()
    before = s.at
    v = read(s)
    if s.at - before != stated: raise Refused(C['VALUE_SIZE'], at)
    return v


def key_origin(s, length, at):
    if length % 4 or length == 0: raise Refused(C['KEYPATH_LENGTH'], at)
    return s.take(length)


def expect(key, where, at):
    if len(key) != KEY_SIZES[where][1]: raise Refused(C['KEY_SIZE'], at, where)


def tap_tree_complete(leaves):
    """TaprootBuilder::Add for each (depth, version), then IsComplete."""
    branch, valid = [], True
    for depth, _ in leaves:
        if not valid: break
        if depth + 1 < len(branch):
            valid = False
            break
        while valid and len(branch) > depth and branch[depth]:
            branch.pop()
            if depth == 0: valid = False
            depth -= 1
        if valid:
            while len(branch) <= depth: branch.append(False)
            branch[depth] = True
    return valid and (len(branch) == 0 or (len(branch) == 1 and branch[0]))


def read_map(s, kind, version, fields, index):
    """One map; appends (map, index, type, key, value, key position, value position)."""
    seen = set()
    found = set()
    while not s.empty():
        at = s.at
        key = s.vector()
        if not key:
            break
        if key in seen: raise Refused(C[['DUP_GLOBAL', 'DUP_INPUT', 'DUP_OUTPUT'][kind]], at)
        seen.add(key)
        sk = Stream(key)
        t = sk.compact()
        vat = s.at
        v = None
        g, i, o = kind == GLOBAL, kind == INPUT, kind == OUTPUT
        if g and t == 0x00:
            expect(key, (GLOBAL, t), at)
            tx = from_vector(s, lambda x: x.tx(False), at)
            if any(x['script'] for x in tx['ins']): raise Refused(C['TX_NOT_EMPTY'], at)
            found.add(('tx', tuple()))
            fields.append(dict(map=kind, index=index, type=t, key=key, at=at, value=None, tx=tx, vat=vat))
            fields[-1]['value'] = s.b[vat:s.at]
            continue
        elif g and t in (0x02, 0x03):
            expect(key, (GLOBAL, t), at); sized(s, 4, at)
        elif g and t in (0x04, 0x05):
            expect(key, (GLOBAL, t), at); from_vector(s, lambda x: x.compact(), at)
        elif g and t == 0x06:
            expect(key, (GLOBAL, t), at); sized(s, 1, at)
        elif g and t == 0x01:
            expect(key, (GLOBAL, t), at)
            depth, fp, child, pub = key[5], key[6:10], key[10:14], key[46:79]
            if not pubkey_valid(pub) or (depth == 0 and (child != bytes(4) or fp != bytes(4))):
                raise Refused(C['PUBKEY'], at)
            key_origin(s, s.compact(), at)
        elif g and t == 0xfb:
            expect(key, (GLOBAL, t), at)
            if int.from_bytes(sized(s, 4, at), 'little') > 2: raise Refused(C['VERSION'], at)
        elif i and t == 0x00:
            expect(key, (INPUT, t), at); from_vector(s, lambda x: x.tx(True), at)
        elif i and t == 0x01:
            expect(key, (INPUT, t), at); from_vector(s, lambda x: (x.take(8), x.vector()), at)
        elif i and t == 0x02:
            if len(key) not in (34, 66): raise Refused(C['PARTIAL_KEY_SIZE'], at)
            if not pubkey_valid(key[1:]): raise Refused(C['PUBKEY'], at)
            sig = s.vector()
            if not sig or not D.valid_encoding(sig) or not D.defined_hashtype(sig): raise Refused(C['SIG_ENCODING'], at)
        elif i and t == 0x03:
            expect(key, (INPUT, t), at); sized(s, 4, at)
        elif (i and t in (0x04, 0x05, 0x07)) or (o and t in (0x00, 0x01)):
            expect(key, (kind, t), at); s.vector()
        elif (i and t == 0x06) or (o and t == 0x02):
            if len(key) not in (34, 66): raise Refused(C['BIP32_KEY_SIZE'], at)
            if not pubkey_valid(key[1:]): raise Refused(C['PUBKEY'], at)
            key_origin(s, s.compact(), at)
        elif i and t == 0x08:
            expect(key, (INPUT, t), at)
            from_vector(s, lambda x: [x.vector() for _ in range(x.compact())], at)
        elif i and t in (0x0a, 0x0b, 0x0c, 0x0d):
            expect(key, (INPUT, t), at); s.vector()
        elif i and t in (0x0e, 0x0f, 0x10, 0x11, 0x12):
            expect(key, (INPUT, t), at)
            if version < 2:
                raise Refused(C[{0x0e: 'V0_PREV_TXID', 0x0f: 'V0_PREV_INDEX', 0x10: 'V0_SEQUENCE',
                                 0x11: 'V0_TIME', 0x12: 'V0_HEIGHT'}[t]], at)
            v = int.from_bytes(sized(s, 32 if t == 0x0e else 4, at), 'little')
            if t == 0x11 and v < 500000000: raise Refused(C['TIME_LOW'], at)
            if t == 0x12 and v >= 500000000: raise Refused(C['HEIGHT_HIGH'], at)
            if t == 0x12 and v == 0: raise Refused(C['HEIGHT_ZERO'], at)
            found.add(t)
        elif i and t == 0x13:
            expect(key, (INPUT, t), at)
            sig = s.vector()
            if len(sig) < 64: raise Refused(C['TAP_KEY_SHORT'], at)
            if len(sig) > 65: raise Refused(C['TAP_KEY_LONG'], at)
        elif i and t == 0x14:
            expect(key, (INPUT, t), at)
            sig = s.vector()
            if len(sig) < 64: raise Refused(C['TAP_SCRIPT_SHORT'], at)
            if len(sig) > 65: raise Refused(C['TAP_SCRIPT_LONG'], at)
        elif i and t == 0x15:
            if len(key) < 34: raise Refused(C['LEAF_KEY_SHORT'], at)
            if (len(key) - 2) % 32: raise Refused(C['LEAF_CONTROL'], at)
            if not s.vector(): raise Refused(C['LEAF_EMPTY'], at)
        elif (i and t == 0x16) or (o and t == 0x07):
            expect(key, (kind, t), at)
            stated = s.compact()
            before = s.at
            for _ in range(s.compact()): s.take(32)
            if s.at - before > stated: raise Refused(C['IN_TAP_PATH' if i else 'OUT_TAP_PATH'], at)
            key_origin(s, stated - (s.at - before), at)
        elif (i and t in (0x17, 0x18)) or (o and t == 0x05):
            expect(key, (kind, t), at); sized(s, 32, at)
        elif (i and t == 0x1a) or (o and t == 0x08):
            expect(key, (kind, t), at)
            if not pubkey_valid(key[1:34]) or len(key[1:34]) != 33 or key[1] not in (2, 3):
                raise Refused(C['IN_MUSIG_AGG' if i else 'OUT_MUSIG_AGG'], at)
            val = s.vector()
            for k in range(0, len(val) - len(val) % 33, 33):
                p = val[k:k + 33]
                if p[0] not in (2, 3) or not pubkey_valid(p):
                    raise Refused(C['IN_MUSIG_PART' if i else 'OUT_MUSIG_PART'], at)
            if len(val) % 33: raise Refused(C['IN_MUSIG_MULTIPLE' if i else 'OUT_MUSIG_MULTIPLE'], at)
        elif i and t in (0x1b, 0x1c):
            if len(key) not in (67, 99): raise Refused(C['NONCE_KEY' if t == 0x1b else 'PSIG_KEY'], at)
            part, agg = key[1:34], key[34:67]
            if agg[0] not in (2, 3) or not pubkey_valid(agg): raise Refused(C['MUSIG_AGG'], at)
            if part[0] not in (2, 3) or not pubkey_valid(part): raise Refused(C['MUSIG_PART'], at)
            if t == 0x1b:
                if len(s.vector()) != 66: raise Refused(C['NONCE_VALUE'], at)
            else:
                sized(s, 32, at)
        elif o and t in (0x03, 0x04):
            expect(key, (OUTPUT, t), at)
            if version < 2: raise Refused(C['V0_AMOUNT' if t == 0x03 else 'V0_SCRIPT'], at)
            if t == 0x03: sized(s, 8, at)
            else: s.vector()
            found.add(t)
        elif o and t == 0x06:
            expect(key, (OUTPUT, t), at)
            tree = Stream(s.vector())
            if tree.empty(): raise Refused(C['TREE_EMPTY'], at)
            leaves = []
            while not tree.empty():
                depth, ver = tree.take(1)[0], tree.take(1)[0]
                tree.vector()
                if depth > 128: raise Refused(C['TREE_DEPTH'], at)
                if ver & 1: raise Refused(C['TREE_LEAF_VERSION'], at)
                leaves.append((depth, ver))
            if not tap_tree_complete(leaves): raise Refused(C['TREE_MALFORMED'], at)
        elif t == 0xfc:
            sk.vector(); sk.compact(); s.vector()
        else:
            s.vector()
        if g and t in (0x02, 0x03, 0x04, 0x05, 0x06, 0xfb):
            found.add(t)
        fields.append(dict(map=kind, index=index, type=t, key=key, at=at, value=s.b[vat:s.at]))
    else:
        raise Refused(C[['SEP_GLOBAL', 'SEP_INPUT', 'SEP_OUTPUT'][kind]], s.at)
    if kind == INPUT and version >= 2:
        if 0x0e not in found: raise Refused(C['V2_PREV_TXID'], s.at)
        if 0x0f not in found: raise Refused(C['V2_PREV_INDEX'], s.at)
    if kind == OUTPUT and version >= 2:
        if 0x03 not in found: raise Refused(C['V2_AMOUNT'], s.at)
        if 0x04 not in found: raise Refused(C['V2_SCRIPT'], s.at)
    return found


def value_bytes(f):
    """A field's value without its length prefix."""
    s = Stream(f['value'])
    n = s.compact()
    return f['value'][s.at:s.at + n]


def decode(b):
    """DecodeRawPSBT: a dict, or raises Refused."""
    s = Stream(b)
    if s.take(5) != b'psbt\xff' if len(b) >= 5 else True:
        if len(b) < 5: raise Refused(C['TRUNCATED'], 0)
        raise Refused(C['MAGIC'], 0)
    fields = []
    found = read_map(s, GLOBAL, 0, fields, 0)
    g = {f['type']: f for f in fields if len(f['key']) == 1}
    version = int.from_bytes(value_bytes(g[0xfb]), 'little') if 0xfb in found else 0
    tx = g[0x00]['tx'] if 0x00 in g and 'tx' in g[0x00] else None
    if version == 0:
        if tx is None: raise Refused(C['NO_TX'], s.at)
        for t, name in ((0x02, 'V0_TX_VERSION'), (0x03, 'V0_FALLBACK'), (0x04, 'V0_INPUT_COUNT'),
                        (0x05, 'V0_OUTPUT_COUNT'), (0x06, 'V0_MODIFIABLE')):
            if t in found: raise Refused(C[name], s.at)
    if version == 1: raise Refused(C['V1'], s.at)
    if version == 2:
        for t, name in ((0x02, 'V2_TX_VERSION'), (0x04, 'V2_INPUT_COUNT'), (0x05, 'V2_OUTPUT_COUNT')):
            if t not in found: raise Refused(C[name], s.at)
        if tx is not None: raise Refused(C['V2_TX'], s.at)
    if version < 2:
        nin, nout = len(tx['ins']), len(tx['outs'])
    else:
        nin = Stream(value_bytes(g[0x04])).compact()
        nout = Stream(value_bytes(g[0x05])).compact()
    i = 0
    while not s.empty() and i < nin:
        before = len(fields)
        read_map(s, INPUT, version, fields, i)
        mine = {f['type']: value_bytes(f) for f in fields[before:] if len(f['key']) == 1}
        if 0x00 in mine:
            prev = Stream(mine[0x00]).tx(True)
            if version < 2:
                h, n = tx['ins'][i]['prevout'][:32], int.from_bytes(tx['ins'][i]['prevout'][32:], 'little')
            else:
                h, n = mine[0x0e], int.from_bytes(mine[0x0f], 'little')
            if T.txid(prev) != h: raise Refused(C['UTXO_HASH'], s.at)
            if n >= len(prev['outs']): raise Refused(C['UTXO_INDEX'], s.at)
        i += 1
    if i != nin: raise Refused(C['INPUT_COUNT'], s.at)
    i = 0
    while not s.empty() and i < nout:
        read_map(s, OUTPUT, version, fields, i)
        i += 1
    if i != nout: raise Refused(C['OUTPUT_COUNT'], s.at)
    if not s.empty(): raise Refused(C['EXTRA'], s.at)
    return dict(version=version, nin=nin, nout=nout, fields=fields, tx=tx)


B64 = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/'


def unbase64(text):
    """Core's DecodeBase64: a multiple of 4 characters, at most two '=' at
    the end, and no bits left over; None otherwise."""
    if len(text) % 4: return None
    if text.endswith('='): text = text[:-1]
    if text.endswith('='): text = text[:-1]
    acc = bits = 0
    out = bytearray()
    for c in text:
        if c not in B64: return None
        acc = (acc << 6) | B64.index(c); bits += 6
        if bits >= 8:
            bits -= 8; out.append((acc >> bits) & 255)
    if bits >= 6 or acc & ((1 << bits) - 1): return None
    return bytes(out)


def refusal_text(text):
    """DecodeBase64PSBT: None when the text decodes, else the Refused."""
    b = unbase64(text)
    if b is None: return Refused(C['BASE64'], 0)
    return refusal(b)


def refusal(b):
    """None when b decodes, else the Refused."""
    try:
        decode(b)
        return None
    except Refused as e:
        return e


# ---- the maps as lists of (key, value), and Core's order for writing them
def maps(b):
    """The decoded PSBT as its key-value pairs: version, the global map, one
    map per input and one per output, each a list of (key, value)."""
    p = decode(b)
    m = dict(version=p['version'], g=[], ins=[[] for _ in range(p['nin'])], outs=[[] for _ in range(p['nout'])])
    for f in p['fields']:
        pair = (f['key'], value_bytes(f))
        if f['map'] == GLOBAL: m['g'].append(pair)
        elif f['map'] == INPUT: m['ins'][f['index']].append(pair)
        else: m['outs'][f['index']].append(pair)
    return m


# The order Core's Serialize writes each map's types in; any other type
# comes last. Within a type, keys sort as bytes, except partial signatures,
# which Core keys by the public key's hash.
ORDER = {
    GLOBAL: [0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0xfb, 0xfc],
    INPUT: [0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x0a, 0x0b, 0x0c, 0x0d, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18,
            0x1a, 0x1b, 0x1c, 0x07, 0x08, 0x0e, 0x0f, 0x10, 0x11, 0x12, 0xfc],
    OUTPUT: [0x00, 0x01, 0x02, 0x03, 0x04, 0xfc, 0x05, 0x06, 0x07, 0x08],
}
# The input types a finalized input drops: everything a signer needed.
SIGNING = {0x02, 0x03, 0x04, 0x05, 0x06, 0x0a, 0x0b, 0x0c, 0x0d, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x1a, 0x1b, 0x1c}


def hash160(b):
    return S.ripemd160(hashlib.sha256(b).digest())


def key_type(key):
    return Stream(key).compact()


def sort_key(kind, key):
    t = key_type(key)
    order = ORDER[kind]
    rank = order.index(t) if t in order else len(order)
    if kind == INPUT and t == 0x02: return (rank, hash160(key[1:]))
    return (rank, key)


def write_map(kind, pairs):
    final = kind == INPUT and any(key_type(k) in (0x07, 0x08) for k, _ in pairs)
    out = bytearray()
    for k, v in sorted(pairs, key=lambda kv: sort_key(kind, kv[0])):
        if final and key_type(k) in SIGNING: continue
        out += T.compact(len(k)) + k + T.compact(len(v)) + v
    return bytes(out + b'\x00')


def write(m):
    out = bytearray(b'psbt\xff') + write_map(GLOBAL, m['g'])
    for pairs in m['ins']: out += write_map(INPUT, pairs)
    for pairs in m['outs']: out += write_map(OUTPUT, pairs)
    return bytes(out)


def get(pairs, key):
    for k, v in pairs:
        if k == key: return v
    return None


def put(pairs, key, value):
    """The Updater: set key to value, replacing any value it had."""
    for n, (k, _) in enumerate(pairs):
        if k == key:
            pairs[n] = (key, value)
            return
    pairs.append((key, value))


# ---- Creator
def create(unsigned):
    """A v0 PSBT for an unsigned transaction (empty scriptSigs, no witness)."""
    tx = T.unserialize(unsigned, False)
    assert all(not i['script'] for i in tx['ins'])
    return dict(version=0, g=[(b'\x00', unsigned)], ins=[[] for _ in tx['ins']], outs=[[] for _ in tx['outs']])


# ---- BIP-370's lock time and the unsigned transaction of either version
def locktime(m):
    """BIP-370's Determining Lock Time: the lock time, or None when no lock
    time satisfies every input."""
    heights, times, any_height, any_time, seen = [], [], True, True, False
    for pairs in m['ins']:
        h, t = get(pairs, b'\x12'), get(pairs, b'\x11')
        if h is None and t is None: continue
        seen = True
        if h is not None: heights.append(int.from_bytes(h, 'little'))
        else: any_height = False
        if t is not None: times.append(int.from_bytes(t, 'little'))
        else: any_time = False
    if not seen:
        f = get(m['g'], b'\x03')
        return int.from_bytes(f, 'little') if f is not None else 0
    if any_height: return max(heights)
    if any_time: return max(times)
    return None


def unsigned_tx(m):
    """The unsigned transaction: v0's global one, or v2's built from its fields."""
    if m['version'] < 2: return get(m['g'], b'\x00')
    lt = locktime(m)
    if lt is None: return None
    out = bytearray(get(m['g'], b'\x02')) + T.compact(len(m['ins']))
    for pairs in m['ins']:
        seq = get(pairs, b'\x10')
        out += get(pairs, b'\x0e') + get(pairs, b'\x0f') + b'\x00' + (seq if seq is not None else b'\xff\xff\xff\xff')
    out += T.compact(len(m['outs']))
    for pairs in m['outs']:
        s = get(pairs, b'\x04')
        out += get(pairs, b'\x03') + T.compact(len(s)) + s
    return bytes(out + lt.to_bytes(4, 'little'))


# ---- keys and signatures
def compress(pt):
    return bytes([2 | (pt[1] & 1)]) + pt[0].to_bytes(32, 'big')


def pubkey(secret):
    return compress(D.mul(secret, D.G))


def rfc6979(secret, digest, extra=b''):
    """RFC 6979's nonce; extra is libsecp256k1's 32 bytes of extra data,
    which follow the key and the hash in both seeding steps."""
    x, h = secret.to_bytes(32, 'big'), (int.from_bytes(digest, 'big') % D.N).to_bytes(32, 'big')
    v, k = b'\1' * 32, b'\0' * 32
    mac = lambda key, data: hmac.new(key, data, hashlib.sha256).digest()
    k = mac(k, v + b'\0' + x + h + extra); v = mac(k, v)
    k = mac(k, v + b'\1' + x + h + extra); v = mac(k, v)
    while True:
        v = mac(k, v)
        c = int.from_bytes(v, 'big')
        if 1 <= c < D.N: return c
        k = mac(k, v + b'\0'); v = mac(k, v)


def der_int(x):
    b = x.to_bytes(32, 'big').lstrip(b'\0')
    if b[0] & 0x80: b = b'\0' + b
    return b'\x02' + bytes([len(b)]) + b


def ecdsa(secret, digest, grind=True):
    """Low-S ECDSA with RFC 6979's nonce, DER-encoded. With grind, it is
    ground to a low R as Bitcoin Core's CKey::Sign does: while r's top bit
    is set, sign again with the counter 1, 2, ... as extra data (32 bytes,
    little-endian)."""
    counter = 0
    while True:
        k = rfc6979(secret, digest, counter.to_bytes(32, 'little') if counter else b'')
        r = D.mul(k, D.G)[0] % D.N
        if r < 2 ** 255 or not grind: break
        counter += 1
    s = pow(k, -1, D.N) * (int.from_bytes(digest, 'big') + r * secret) % D.N
    if s > D.N // 2: s = D.N - s
    body = der_int(r) + der_int(s)
    return b'\x30' + bytes([len(body)]) + body


def tagged(tag, b):
    t = hashlib.sha256(tag.encode()).digest()
    return hashlib.sha256(t + t + b).digest()


def lift_x(x):
    if x >= D.P: return None
    y = pow((x ** 3 + 7) % D.P, (D.P + 1) // 4, D.P)
    if (y * y - x ** 3 - 7) % D.P: return None
    return (x, y if y % 2 == 0 else D.P - y)


def schnorr(secret, msg, aux):
    """BIP-340 signing."""
    pt = D.mul(secret, D.G)
    d = secret if pt[1] % 2 == 0 else D.N - secret
    t = (d ^ int.from_bytes(tagged('BIP0340/aux', aux), 'big')).to_bytes(32, 'big')
    px = pt[0].to_bytes(32, 'big')
    k0 = int.from_bytes(tagged('BIP0340/nonce', t + px + msg), 'big') % D.N
    R = D.mul(k0, D.G)
    k = k0 if R[1] % 2 == 0 else D.N - k0
    e = int.from_bytes(tagged('BIP0340/challenge', R[0].to_bytes(32, 'big') + px + msg), 'big') % D.N
    return R[0].to_bytes(32, 'big') + ((k + e * d) % D.N).to_bytes(32, 'big')


def schnorr_verify(pub, msg, sig):
    P = lift_x(int.from_bytes(pub, 'big'))
    r, s = int.from_bytes(sig[:32], 'big'), int.from_bytes(sig[32:64], 'big')
    if P is None or r >= D.P or s >= D.N: return False
    e = int.from_bytes(tagged('BIP0340/challenge', sig[:32] + pub + msg), 'big') % D.N
    R = D.add(D.mul(s, D.G), D.mul(D.N - e, P))
    return R is not None and R[1] % 2 == 0 and R[0] == r


def taproot_tweak(secret, merkle):
    """The tweaked secret key and the x-only output key of a key-path spend."""
    pt = D.mul(secret, D.G)
    d = secret if pt[1] % 2 == 0 else D.N - secret
    t = int.from_bytes(tagged('TapTweak', pt[0].to_bytes(32, 'big') + merkle), 'big')
    assert t < D.N
    q = (d + t) % D.N
    return q, D.mul(q, D.G)[0].to_bytes(32, 'big')


def sighash_taproot(tx, n_in, spent, hashtype):
    """BIP-341's key-path signature hash; spent is (amount, scriptPubKey) per input."""
    sha = lambda b: hashlib.sha256(b).digest()
    ser = lambda o: o['value'] + T.compact(len(o['script'])) + o['script']
    anyone, out_type = hashtype & 0x80, hashtype & 3
    m = bytes([0, hashtype]) + tx['version'] + tx['locktime']
    if not anyone:
        m += sha(b''.join(i['prevout'] for i in tx['ins']))
        m += sha(b''.join(a.to_bytes(8, 'little') for a, _ in spent))
        m += sha(b''.join(T.compact(len(s)) + s for _, s in spent))
        m += sha(b''.join(i['sequence'] for i in tx['ins']))
    if out_type not in (2, 3): m += sha(b''.join(ser(o) for o in tx['outs']))
    m += b'\x00'
    if anyone:
        a, s = spent[n_in]
        m += tx['ins'][n_in]['prevout'] + a.to_bytes(8, 'little') + T.compact(len(s)) + s + tx['ins'][n_in]['sequence']
    else:
        m += n_in.to_bytes(4, 'little')
    if out_type == 3: m += sha(ser(tx['outs'][n_in]))
    return tagged('TapSighash', m)


# ---- Signer
SIGN_REASONS = ['ok', 'no UTXO', 'a witness UTXO for a non-witness input', 'redeemScript does not match the scriptPubKey',
                'witnessScript does not match the scriptPubKey or redeemScript', 'unacceptable sighash type',
                'key not in the script', 'already finalized', 'no lock time satisfies every input']
SC = {n: k for k, n in enumerate(['OK', 'NO_UTXO', 'WITNESS_UTXO', 'REDEEM', 'WITNESS_SCRIPT', 'SIGHASH', 'NOT_MINE',
                                  'FINAL', 'LOCKTIME'])}


def spent_output(m, tx, n):
    """(amount, scriptPubKey, from a witness UTXO) of the output input n
    spends, or None."""
    pairs = m['ins'][n]
    w = get(pairs, b'\x01')
    if w is not None:
        s = Stream(w)
        amount = int.from_bytes(s.take(8), 'little')
        return amount, s.vector(), True
    nw = get(pairs, b'\x00')
    if nw is not None:
        prev = Stream(nw).tx(True)
        o = prev['outs'][int.from_bytes(tx['ins'][n]['prevout'][32:], 'little')]
        return int.from_bytes(o['value'], 'little'), o['script'], False
    return None


def is_p2wpkh(s): return len(s) == 22 and s[0] == 0 and s[1] == 20
def is_p2wsh(s): return len(s) == 34 and s[0] == 0 and s[1] == 32
def is_p2tr(s): return len(s) == 34 and s[0] == 0x51 and s[1] == 32
def is_p2pkh(s): return len(s) == 25 and s[:3] == b'\x76\xa9\x14' and s[23:] == b'\x88\xac'
def p2pkh(h): return b'\x76\xa9\x14' + h + b'\x88\xac'


def sign(m, n, secret, aux=bytes(32), grind=True):
    """BIP-174's simple signer for input n with one key: the checks it lists,
    then a signature added as a partial signature (ECDSA) or as the Taproot
    key-path signature. A partial signature the input already has for the
    key is kept, as Bitcoin Core keeps it. grind selects Core's low-R
    signatures; BIP-174's worked example signs without it. Returns an SC
    code."""
    pairs = m['ins'][n]
    if get(pairs, b'\x07') is not None or get(pairs, b'\x08') is not None: return SC['FINAL']
    raw = unsigned_tx(m)
    if raw is None: return SC['LOCKTIME']
    tx = T.unserialize(raw, False)
    sp = spent_output(m, tx, n)
    if sp is None: return SC['NO_UTXO']
    amount, spk, from_witness = sp
    pub = pubkey(secret)
    ht = get(pairs, b'\x03')
    if is_p2tr(spk):
        hashtype = int.from_bytes(ht, 'little') if ht is not None else 0
        if hashtype not in (0, 1, 2, 3, 0x81, 0x82, 0x83): return SC['SIGHASH']
        spent = []
        for k in range(len(m['ins'])):
            o = spent_output(m, tx, k)
            if o is None: return SC['NO_UTXO']
            spent.append(o[:2])
        merkle = get(pairs, b'\x18') or b''
        q, out = taproot_tweak(secret, merkle)
        if out != spk[2:]: return SC['NOT_MINE']
        sig = schnorr(q, sighash_taproot(tx, n, spent, hashtype), aux)
        put(pairs, b'\x13', sig + (bytes([hashtype]) if hashtype else b''))
        return SC['OK']
    hashtype = int.from_bytes(ht, 'little') if ht is not None else 1
    if hashtype not in (1, 2, 3, 0x81, 0x82, 0x83): return SC['SIGHASH']
    script, witness = spk, False
    redeem = get(pairs, b'\x04')
    if redeem is not None:
        if not S.is_p2sh(spk) or hash160(redeem) != spk[2:22]: return SC['REDEEM']
        script = redeem
    ws = get(pairs, b'\x05')
    if is_p2wpkh(script):
        script, witness = p2pkh(script[2:22]), True
    elif is_p2wsh(script):
        if ws is None or hashlib.sha256(ws).digest() != script[2:]: return SC['WITNESS_SCRIPT']
        script, witness = ws, True
    elif ws is not None:
        return SC['WITNESS_SCRIPT']
    if not witness and from_witness: return SC['WITNESS_UTXO']
    mine = script[3:23] == hash160(pub) if is_p2pkh(script) else (bytes([33]) + pub) in script
    if not mine: return SC['NOT_MINE']
    if get(pairs, b'\x02' + pub) is not None: return SC['OK']
    digest = S.sighash_v0(script, tx, n, hashtype, amount) if witness else S.sighash_legacy(script, tx, n, hashtype)
    put(pairs, b'\x02' + pub, ecdsa(secret, digest, grind) + bytes([hashtype]))
    return SC['OK']


# ---- Combiner
def combine(a, b):
    """The union of two PSBTs of one transaction; for a key both have, a's value."""
    if unsigned_tx(a) != unsigned_tx(b): return None
    out = dict(version=a['version'], g=list(a['g']), ins=[list(p) for p in a['ins']], outs=[list(p) for p in a['outs']])
    for mine, theirs in [(out['g'], b['g'])] + list(zip(out['ins'], b['ins'])) + list(zip(out['outs'], b['outs'])):
        for k, v in theirs:
            if get(mine, k) is None: mine.append((k, v))
    return out


# ---- Input Finalizer
def push(b):
    if len(b) == 0: return b'\x00'
    if len(b) < 0x4c: return bytes([len(b)]) + b
    if len(b) <= 0xff: return b'\x4c' + bytes([len(b)]) + b
    return b'\x4d' + len(b).to_bytes(2, 'little') + b


def multisig_keys(script):
    """(m, keys) of a k-of-n CHECKMULTISIG script, or None."""
    if len(script) < 3 or script[-1] != 0xae or not (0x51 <= script[0] <= 0x60): return None
    m, keys, at = script[0] - 0x50, [], 1
    while at < len(script) - 2:
        n = script[at]
        if n not in (33, 65) or at + 1 + n > len(script) - 2: return None
        keys.append(script[at + 1:at + 1 + n]); at += 1 + n
    if at != len(script) - 2 or script[at] != 0x50 + len(keys) or m > len(keys): return None
    return m, keys


def satisfy(script, pairs):
    """The stack that satisfies script from the partial signatures, or None."""
    sigs = {k[1:]: v for k, v in pairs if key_type(k) == 0x02}
    if is_p2pkh(script):
        for k, v in sigs.items():
            if hash160(k) == script[3:23]: return [v, k]
        return None
    ms = multisig_keys(script)
    if ms is None: return None
    need, keys = ms
    chosen = [sigs[k] for k in keys if k in sigs][:need]
    return [b''] + chosen if len(chosen) == need else None


def finalize_input(m, n):
    """Writes the final scriptSig and witness of input n; True when it is final."""
    pairs = m['ins'][n]
    if get(pairs, b'\x07') is not None or get(pairs, b'\x08') is not None: return True
    raw = unsigned_tx(m)
    if raw is None: return False
    sp = spent_output(m, T.unserialize(raw, False), n)
    if sp is None: return False
    spk = sp[1]
    if is_p2tr(spk):
        sig = get(pairs, b'\x13')
        if sig is None: return False
        put(pairs, b'\x08', T.compact(1) + T.compact(len(sig)) + sig)
        return True
    script_sig, witness, script = b'', None, spk
    redeem = get(pairs, b'\x04')
    if S.is_p2sh(spk):
        if redeem is None: return False
        script = redeem
    if is_p2wpkh(script):
        witness = satisfy(p2pkh(script[2:22]), pairs)
        if witness is None: return False
    elif is_p2wsh(script):
        ws = get(pairs, b'\x05')
        st = satisfy(ws, pairs) if ws is not None else None
        if st is None: return False
        witness = st + [ws]
    else:
        st = satisfy(script, pairs)
        if st is None: return False
        script_sig = b''.join(push(x) for x in st)
    if S.is_p2sh(spk): script_sig += push(redeem)
    if script_sig: put(pairs, b'\x07', script_sig)
    if witness is not None:
        put(pairs, b'\x08', T.compact(len(witness)) + b''.join(T.compact(len(x)) + x for x in witness))
    return True


def finalize(m):
    return all([finalize_input(m, n) for n in range(len(m['ins']))])


# ---- Transaction Extractor
def extract(m):
    """The network transaction of a finalized PSBT, or None."""
    raw = unsigned_tx(m)
    if raw is None: return None
    tx = T.unserialize(raw, False)
    for n, pairs in enumerate(m['ins']):
        ss, w = get(pairs, b'\x07'), get(pairs, b'\x08')
        if ss is None and w is None: return None
        tx['ins'][n]['script'] = ss or b''
        if w is not None:
            s = Stream(w)
            tx['ins'][n]['witness'] = [s.vector() for _ in range(s.compact())]
    return T.serialize(tx, True)
