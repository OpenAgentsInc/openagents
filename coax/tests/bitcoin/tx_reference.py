#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: a Python transliteration of Bitcoin Core's reader, outside Coax)
"""Bitcoin Core's transaction reader in Python, for Bitcoin chapter 34.

A transliteration of the pinned Bitcoin Core sources in
coax/tests/vectors/upstream/bitcoin-core/ (revision ddea9196):

- serialize.h: ReadCompactSize with its range check (MAX_SIZE), and the
  "end of data" every short read raises;
- primitives/transaction.h: UnserializeTransaction and
  SerializeTransaction, with and without witnesses;
- core_io.cpp: DecodeTx, which tries both readings, and
  CheckTxScriptsSanity;
- script/script.cpp: CScript::HasValidOps and GetScriptOp.

It shares no code with Coax. It returns the same refusal codes and positions
as coax/lib/bitcoin/tx.coax, which this file defines independently: Core
throws an exception with a message, and the message maps to a code. The
capacity limits of the caller's arrays are not Core's; Coax reports them
with their own code, and this reader takes the same limits.
"""
import hashlib

OK, TRUNCATED, NONCANONICAL, SIZE_TOO_LARGE, UNKNOWN_FLAG, NO_WITNESS, TRAILING, CAPACITY = range(8)
NAMES = ["ok", "end of data", "non-canonical ReadCompactSize()", "ReadCompactSize(): size too large",
         "Unknown transaction optional data", "Superfluous witness record", "trailing bytes", "capacity"]
MAX_SIZE = 0x02000000
MAX_SCRIPT_SIZE = 10000
MAX_SCRIPT_ELEMENT_SIZE = 520
MAX_OPCODE = 0xB9  # OP_NOP10
WITNESS_SCALE_FACTOR = 4


class Refused(Exception):
    def __init__(self, code, at):
        super().__init__(NAMES[code])
        self.code, self.at = code, at


class Reader:
    """A stream over bytes; every read past the end raises "end of data"."""

    def __init__(self, data, caps):
        self.b, self.at, self.caps = data, 0, caps

    def take(self, n):
        if n > len(self.b) - self.at:
            raise Refused(TRUNCATED, self.at)
        out = self.b[self.at:self.at + n]
        self.at += n
        return out

    def compact(self):
        start = self.at
        if self.at >= len(self.b):
            raise Refused(TRUNCATED, start)
        first = self.b[self.at]
        widths = {0xFD: 2, 0xFE: 4, 0xFF: 8}
        if first < 0xFD:
            self.at += 1
            return first
        width = widths[first]
        if width > len(self.b) - self.at - 1:
            raise Refused(TRUNCATED, start)
        value = int.from_bytes(self.b[self.at + 1:self.at + 1 + width], "little")
        if value < {2: 0xFD, 4: 0x10000, 8: 0x100000000}[width]:
            raise Refused(NONCANONICAL, start)
        if value > MAX_SIZE:
            raise Refused(SIZE_TOO_LARGE, start)
        self.at += 1 + width
        return value

    def bytes(self):
        n = self.compact()
        return self.take(n)


def read_inputs(r, cap):
    n = r.compact()
    ins = []
    for k in range(n):
        if k == cap:
            raise Refused(CAPACITY, r.at)
        prevout = r.take(36)
        script = r.bytes()
        sequence = r.take(4)
        ins.append(dict(prevout=prevout, script=script, sequence=sequence, witness=[]))
    return ins


def read_outputs(r, cap):
    n = r.compact()
    outs = []
    for k in range(n):
        if k == cap:
            raise Refused(CAPACITY, r.at)
        value = r.take(8)
        outs.append(dict(value=value, script=r.bytes()))
    return outs


def unserialize(data, allow_witness, caps=(1 << 62, 1 << 62, 1 << 62)):
    """UnserializeTransaction, then the caller's check that nothing is left.
    Returns the transaction as a dict, or raises Refused."""
    r = Reader(data, caps)
    version = r.take(4)
    flags = 0
    ins = read_inputs(r, caps[0])
    outs = []
    if len(ins) == 0 and allow_witness:
        flags = r.take(1)[0]
        if flags != 0:
            ins = read_inputs(r, caps[0])
            outs = read_outputs(r, caps[1])
    else:
        outs = read_outputs(r, caps[1])
    segwit = False
    if flags & 1 and allow_witness:
        flags ^= 1
        start = r.at
        items = 0
        for i in ins:
            n = r.compact()
            for _ in range(n):
                if items == caps[2]:
                    raise Refused(CAPACITY, r.at)
                i["witness"].append(r.bytes())
                items += 1
        if not any(i["witness"] for i in ins):
            raise Refused(NO_WITNESS, start)
        segwit = True
    if flags:
        raise Refused(UNKNOWN_FLAG, 5)
    locktime = r.take(4)
    if r.at != len(data):
        raise Refused(TRAILING, r.at)
    return dict(version=version, ins=ins, outs=outs, locktime=locktime, segwit=segwit)


def compact(n):
    if n < 0xFD: return bytes([n])
    if n <= 0xFFFF: return b"\xfd" + n.to_bytes(2, "little")
    if n <= 0xFFFFFFFF: return b"\xfe" + n.to_bytes(4, "little")
    return b"\xff" + n.to_bytes(8, "little")


def has_witness(tx):
    return any(i["witness"] for i in tx["ins"])


def serialize(tx, allow_witness):
    """SerializeTransaction: the extended form only when some input has a witness."""
    extended = allow_witness and has_witness(tx)
    out = bytearray(tx["version"])
    if extended:
        out += b"\x00\x01"
    out += compact(len(tx["ins"]))
    for i in tx["ins"]:
        out += i["prevout"] + compact(len(i["script"])) + i["script"] + i["sequence"]
    out += compact(len(tx["outs"]))
    for o in tx["outs"]:
        out += o["value"] + compact(len(o["script"])) + o["script"]
    if extended:
        for i in tx["ins"]:
            out += compact(len(i["witness"]))
            for item in i["witness"]:
                out += compact(len(item)) + item
    return bytes(out + tx["locktime"])


def sha256d(b):
    return hashlib.sha256(hashlib.sha256(b).digest()).digest()


def txid(tx):
    """The digest in hash order; displays reverse it."""
    return sha256d(serialize(tx, False))


def wtxid(tx):
    return sha256d(serialize(tx, True))


def weight(tx):
    return len(serialize(tx, False)) * (WITNESS_SCALE_FACTOR - 1) + len(serialize(tx, True))


def has_valid_ops(script):
    """CScript::HasValidOps over GetScriptOp."""
    pc = 0
    while pc < len(script):
        opcode = script[pc]
        pc += 1
        size = 0
        if opcode <= 0x4E:
            if opcode < 0x4C:
                size = opcode
            else:
                width = {0x4C: 1, 0x4D: 2, 0x4E: 4}[opcode]
                if len(script) - pc < width: return False
                size = int.from_bytes(script[pc:pc + width], "little")
                pc += width
            if len(script) - pc < size: return False
            pc += size
        if opcode > MAX_OPCODE or size > MAX_SCRIPT_ELEMENT_SIZE:
            return False
    return True


def is_coinbase(tx):
    return len(tx["ins"]) == 1 and tx["ins"][0]["prevout"] == bytes(32) + b"\xff\xff\xff\xff"


def scripts_sane(tx):
    """CheckTxScriptsSanity."""
    if not is_coinbase(tx):
        for i in tx["ins"]:
            if not has_valid_ops(i["script"]) or len(i["script"]) > MAX_SCRIPT_SIZE: return False
    for o in tx["outs"]:
        if not has_valid_ops(o["script"]) or len(o["script"]) > MAX_SCRIPT_SIZE: return False
    return True


def attempt(data, allow_witness, caps):
    try:
        return unserialize(data, allow_witness, caps), None
    except Refused as e:
        return None, e


def decode(data, caps=(1 << 62, 1 << 62, 1 << 62)):
    """DecodeTx with both readings allowed: (transaction or None, refusal of the
    extended reading or None). Core reports no reason when both fail; Coax
    returns the extended reading's refusal, and so does this."""
    extended, refused = attempt(data, True, caps)
    if extended is not None and scripts_sane(extended):
        return extended, None
    legacy, _ = attempt(data, False, caps)
    if legacy is not None and scripts_sane(legacy):
        return legacy, None
    if extended is not None:
        return extended, None
    if legacy is not None:
        return legacy, None
    return None, refused
