#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: chapter 94's and 36's Python references, BIP-327's pinned reference.py and the pinned secp256k1lab decide every line outside Coax)
"""The cross-check of Bitcoin chapter 122, agent treasuries.

It writes the transcript bitcoin/examples/treasury.coax must print, with
code that shares nothing with Coax:

- the keys, the P2WSH script, the Taproot leaves, tree, control blocks
  and output keys: written here from BIP-141, BIP-342 and BIP-341;
- the agent's refill rules: written here again from the chapter's table;
- the PSBTs and their partial signatures: chapter 94's psbt_reference.py
  (BIP-174's Creator, Updater, Signer and Extractor); the witnesses are
  assembled here;
- every spend: chapter 36's script_reference.py interpreter, with BIP-68
  and BIP-112's sequence checks;
- the key-path signature: BIP-327's pinned reference.py (chapter 101);
- the script-path signatures: the secp256k1lab copy pinned with BIP-445;
- the addresses: the pinned sipa segwit_addr.py.

  python3 -I coax/tests/bitcoin/treasury.py                       # compare with the fixture
  python3 -I coax/tests/bitcoin/treasury.py --coax coax/bin/coax  # also run the example
  python3 -I coax/tests/bitcoin/treasury.py --write               # write the fixture
"""
import hashlib, subprocess, sys
sys.dont_write_bytecode = True   # nothing written into the pinned upstream folders
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
sys.path.insert(0, str(HERE))
UP = REPO / "coax/tests/vectors/upstream"
SECP = UP / "bip445/python/secp256k1lab/src/secp256k1lab/bip340.py"
assert hashlib.sha256(SECP.read_bytes()).hexdigest() == "428ee8110175b32b67d22a88efe07a44cede2419d0f547949164f2a8f9d76b96"
sys.path.insert(0, str(UP / "bip445/python/secp256k1lab/src"))
import musig as MU  # noqa: E402  (loads BIP-327's reference.py after checking its pin)
import psbt_reference as P  # noqa: E402
import script_reference as S  # noqa: E402
import tx_reference as T  # noqa: E402
from secp256k1lab.bip340 import schnorr_sign, schnorr_verify  # noqa: E402

R = MU.R
SEGWIT = MU.load_segwit()
FIXTURE = REPO / "bitcoin/fixtures/treasury.out"
EXAMPLE = "bitcoin/examples/treasury.coax"
KEYS = [bytes([b]) * 32 for b in (0x11, 0x22, 0x33, 0x44)]
NAMES = ["agent", "owner", "recovery", "hot"]
DELAY, FUNDING = 4320, 100000
REASONS = ["ok", "key", "delay", "capacity", "no UTXO", "foreign input", "payment", "over the ceiling", "fee",
           "no refill", "signer refused"]
FLAGS = 0
for name in ["P2SH", "WITNESS", "DERSIG", "LOW_S", "STRICTENC", "NULLDUMMY", "CLEANSTACK", "NULLFAIL",
             "WITNESS_PUBKEYTYPE", "MINIMALIF", "CHECKLOCKTIMEVERIFY", "CHECKSEQUENCEVERIFY"]:
    FLAGS |= S.F[name]
sha = lambda b: hashlib.sha256(b).digest()
tf = lambda b: "true" if b else "false"


def tagged(tag, b):
    t = sha(tag.encode())
    return sha(t + t + b)


def address(spk):
    return "address: " + SEGWIT.encode("tb", 0 if spk[0] == 0 else spk[0] - 0x50, spk[2:])


def push_num(d):
    """A script number push: OP_1 to OP_16, or minimal little-endian bytes."""
    if 1 <= d <= 16: return bytes([0x50 + d])
    b = d.to_bytes((d.bit_length() + 7) // 8, "little")
    if b[-1] & 0x80: b += b"\x00"
    return bytes([len(b)]) + b


def wsh_script(a, o, r, d):
    k = lambda x: bytes([33]) + x
    return b"\x52" + k(a) + k(o) + k(r) + b"\x53\xae\x73\x64" + k(r) + b"\xad" + push_num(d) + b"\xb2\x68"


def leaf_hash(s):
    return tagged("TapLeaf", b"\xc0" + T.compact(len(s)) + s)


def tweak_point(x, merkle):
    """BIP-341's taproot_tweak_pubkey: (parity, output x)."""
    t = int.from_bytes(tagged("TapTweak", x + merkle), "big")
    Q = R.point_add(R.lift_x(x), R.point_mul(R.G, t))
    return Q[1] & 1, R.xbytes(Q)


def tx_of(prev, sequence, outs):
    return dict(version=(2).to_bytes(4, "little"), locktime=bytes(4),
                ins=[dict(prevout=bytes([prev]) * 32 + bytes(4), script=b"", sequence=sequence.to_bytes(4, "little"), witness=[])],
                outs=[dict(value=v.to_bytes(8, "little"), script=s) for v, s in outs])


def psbt_of(tx, spk, script):
    m = P.create(T.serialize(tx, False))
    P.put(m["ins"][0], b"\x01", FUNDING.to_bytes(8, "little") + T.compact(len(spk)) + spk)
    P.put(m["ins"][0], b"\x05", script)
    return m


def check(tx, in_spk, wsh, hot, policy):
    """The agent's refill rules, in the chapter's order: (code, total, refill, change, fee)."""
    hot_balance, ceiling, fee_cap = policy
    if in_spk != wsh: return (-5, 0, 0, 0, 0)
    total, refill, change = FUNDING, 0, 0
    for o in tx["outs"]:
        v = int.from_bytes(o["value"], "little")
        if o["script"] == hot: refill += v
        elif o["script"] == wsh: change += v
        else: return (-6, total, refill, change, 0)
    fee = total - refill - change
    code = 0
    if refill == 0: code = -9
    elif fee < 0 or fee > fee_cap: code = -8
    elif hot_balance + refill > ceiling: code = -7
    return (code, total, refill, change, fee)


def finalize(m, script, pubs):
    """The treasury's witness from the partial signatures: two of three in
    key order, or the recovery key's alone with three empty items."""
    pairs = m["ins"][0]
    sigs = [P.get(pairs, b"\x02" + pubs[k]) for k in range(3)]
    have = [s for s in sigs if s is not None]
    if len(have) >= 2: items = [b""] + have[:2]
    elif sigs[2] is not None: items = [sigs[2], b"", b"", b""]
    else: return False
    items.append(script)
    P.put(pairs, b"\x08", T.compact(len(items)) + b"".join(T.compact(len(x)) + x for x in items))
    return True


def verify(m, spk, out):
    raw = P.extract(m)
    tx = T.unserialize(raw, True)
    out.append("  txid: " + T.txid(tx)[::-1].hex())
    out.append(f"  witness items: {len(tx['ins'][0]['witness'])}")
    code = S.verify_script(b"", spk, tx["ins"][0]["witness"], FLAGS, S.Checker(tx, 0, FUNDING))
    out.append("  script: " + S.ERRORS[code])
    return code


def sighash_ext(tx, spent, leaf):
    """BIP-341's signature message for SIGHASH_DEFAULT with BIP-342's
    extension (ext_flag 1, key_version 0, no OP_CODESEPARATOR), written
    out here."""
    m = b"\x00\x00" + tx["version"] + tx["locktime"]
    m += sha(b"".join(i["prevout"] for i in tx["ins"]))
    m += sha(b"".join(a.to_bytes(8, "little") for a, _ in spent))
    m += sha(b"".join(T.compact(len(s)) + s for _, s in spent))
    m += sha(b"".join(i["sequence"] for i in tx["ins"]))
    m += sha(b"".join(o["value"] + T.compact(len(o["script"])) + o["script"] for o in tx["outs"]))
    m += b"\x02" + (0).to_bytes(4, "little")
    m += leaf + b"\x00" + b"\xff\xff\xff\xff"
    return tagged("TapSighash", m)


def transcript():
    out = []
    p = out.append
    secrets = [int.from_bytes(k, "big") for k in KEYS]
    pubs = [P.pubkey(s) for s in secrets]
    p("The keys")
    for k in range(4): p(f"{NAMES[k]}: {pubs[k].hex()}")

    p(f"The P2WSH treasury, recovery after {DELAY} blocks")
    ws = wsh_script(pubs[0], pubs[1], pubs[2], DELAY)
    p(f"witness script: {len(ws)} bytes")
    p("script: " + ws.hex())
    wsh = b"\x00\x20" + sha(ws)
    p(address(wsh))

    p("The Taproot treasury")
    keys = R.key_sort([pubs[0], pubs[1]])
    internal = R.get_xonly_pk(R.key_agg(keys))
    p("MuSig2 internal key (agent, owner): " + internal.hex())
    x = [pk[1:] for pk in pubs]
    multi = b"\x20" + x[0] + b"\xac" + b"\x20" + x[1] + b"\xba" + b"\x20" + x[2] + b"\xba" + b"\x52\x9c"
    late = b"\x20" + x[2] + b"\xad" + push_num(DELAY) + b"\xb2"
    p(f"leaf multi_a(2,A,O,R): {multi.hex()}")
    p(f"leaf and_v(v:pk(R),older({DELAY})): {late.hex()}")
    h0, h1 = leaf_hash(multi), leaf_hash(late)
    root = tagged("TapBranch", min(h0, h1) + max(h0, h1))
    p("root: " + root.hex())
    parity, output = tweak_point(internal, root)
    p("output key: " + output.hex())
    trspk = b"\x51\x20" + output
    p(address(trspk))

    p("The hot wallet")
    hot = b"\x51\x20" + tweak_point(x[3], b"")[1]
    p(address(hot))

    p("A refill through PSBT")
    policy = (3000, 25000, 1000)
    p(f"hot balance {policy[0]}, ceiling {policy[1]}, fee cap {policy[2]}")
    tx = tx_of(0xa1, 0xfffffffd, [(20000, hot), (79500, wsh)])
    m = psbt_of(tx, wsh, ws)
    c = check(tx, wsh, wsh, hot, policy)
    p(f"inputs {c[1]}, refill {c[2]}, change {c[3]}, fee {c[4]}: {REASONS[-c[0]]}")
    assert c[0] == 0
    p("the agent signs: " + REASONS[0 if P.sign(m, 0, secrets[0], bytes(32), True) == 0 else 10])
    p("the owner signs: " + P.SIGN_REASONS[P.sign(m, 0, secrets[1], bytes(32), True)])
    p("final: " + tf(finalize(m, ws, pubs)))
    verify(m, wsh, out)

    p("What the agent refuses")
    stranger = b"\x00\x14" + P.hash160(pubs[1])
    cases = [("refill 30,000", 0xa1, wsh, [(30000, hot), (69500, wsh)]),
             ("a payment to another address", 0xa1, wsh, [(20000, stranger), (79500, wsh)]),
             ("a fee of 5,000", 0xa1, wsh, [(20000, hot), (75000, wsh)]),
             ("a coin of the hot wallet", 0xa1, hot, [(20000, hot), (79500, wsh)]),
             ("change only", 0xa1, wsh, [(99500, wsh)])]
    for label, prev, in_spk, outs in cases:
        c = check(tx_of(prev, 0xfffffffd, outs), in_spk, wsh, hot, policy)
        assert c[0] != 0
        p(f"{label}: {REASONS[-c[0]]}")

    p("The recovery path")
    rec = b"\x00\x14" + P.hash160(pubs[2])
    for k in range(2):
        sequence = DELAY - k
        m = psbt_of(tx_of(0xa1, sequence, [(99000, rec)]), wsh, ws)
        p(f"sequence {sequence}: the recovery key signs: {P.SIGN_REASONS[P.sign(m, 0, secrets[2], bytes(32), True)]}")
        p("final: " + tf(finalize(m, ws, pubs)))
        verify(m, wsh, out)
    m = psbt_of(tx_of(0xa1, DELAY, [(99000, rec)]), wsh, ws)
    assert P.sign(m, 0, secrets[0], bytes(32), True) == 0
    p("the agent's signature alone: final " + tf(finalize(m, ws, pubs)))

    p("The Taproot key path, MuSig2")
    tx = tx_of(0xb2, 0xfffffffd, [(20000, hot), (79500, trspk)])
    spent = [(FUNDING, trspk)]
    msg = P.sighash_taproot(tx, 0, spent, 0)
    p("sighash: " + msg.hex())
    tweak = tagged("TapTweak", internal + root)
    who = [0 if keys[i] == pubs[0] else 1 for i in range(2)]
    secs, nonces = [], []
    for i in range(2):
        sec, pub = R.nonce_gen_internal(bytes([0xc0 + i]) * 32, KEYS[who[i]], keys[i], output, msg, b"")
        secs.append(sec); nonces.append(pub)
    agg = R.nonce_agg(nonces)
    ctx = R.SessionContext(agg, keys, [tweak], [True], msg)
    sig = R.partial_sig_agg([R.sign(secs[i], KEYS[who[i]], ctx) for i in range(2)], ctx)
    p("signature: " + sig.hex())
    p("valid under the output key: " + tf(schnorr_verify(msg, output, sig)))

    p("The Taproot script path, owner and recovery")
    leafsig = sighash_ext(tx, spent, h0)
    p("leaf sighash: " + leafsig.hex())
    s1, s2 = schnorr_sign(leafsig, KEYS[1], bytes(32)), schnorr_sign(leafsig, KEYS[2], bytes(32))
    p(f"owner's signature valid: {tf(schnorr_verify(leafsig, x[1], s1))}, recovery's: {tf(schnorr_verify(leafsig, x[2], s2))}")
    cb0 = bytes([0xc0 | parity]) + internal + h1
    p("control block: " + cb0.hex())

    def committed(cb, leaf):
        # BIP-341's script-path check: the control block's path from the
        # leaf gives a root that tweaks the internal key to the output key.
        k = leaf_hash(leaf)
        for j in range(33, len(cb), 32):
            e = cb[j:j + 32]
            k = tagged("TapBranch", min(k, e) + max(k, e))
        par, q = tweak_point(cb[1:33], k)
        return q == output and par == cb[0] & 1
    p("the leaf is in the output: " + tf(committed(cb0, multi)))
    p("witness: recovery's signature, owner's, an empty item for the agent, the leaf, the control block")
    cb1 = bytes([0xc0 | parity]) + internal + h0
    p("the recovery leaf is in the output: " + tf(committed(cb1, late)))
    return "\n".join(out) + "\n"


def main():
    text = transcript()
    if "--write" in sys.argv:
        FIXTURE.write_text(text)
        print(f"wrote {FIXTURE.relative_to(REPO)}")
    assert FIXTURE.read_text() == text, "the fixture differs from Python's transcript"
    print(f"treasury fixture: {text.count(chr(10))} lines agree with Python")
    if "--coax" in sys.argv:
        coax = sys.argv[sys.argv.index("--coax") + 1]
        for mode in ([], ["--release"]):
            run = subprocess.run([coax, "run", EXAMPLE, "--run-steps", "250000000"] + mode, cwd=REPO,
                                 capture_output=True, text=True)
            assert run.returncode == 0, run.stderr[-2000:]
            assert run.stdout == text, "the example's output differs"
            print(f"native {'release' if mode else 'debug'}: agrees")


if __name__ == "__main__":
    main()
