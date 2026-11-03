#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: Python's hashlib, chapter 32's and 29's Python references and the pinned secp256k1lab decide every line outside Coax)
"""The cross-check of Bitcoin chapter 121, one seed for Nostr and Bitcoin.

It checks the hash of every pinned document it reads, checks that the
published values the chapter quotes are in them (NIP-06's two keys and
npubs, BIP-84's and BIP-86's addresses and account xpub, and BIP-39's
vector 0 seed with the passphrase TREZOR), and then writes the transcript
bitcoin/examples/one-seed.coax must print, with code that shares nothing
with Coax:

- BIP-39: the checksum from english.txt and hashlib's sha256, the seed
  from hashlib.pbkdf2_hmac;
- BIP-32 and the addresses: chapter 32's bip32.py and chapter 29's
  address.py references;
- the descriptor checksum: chapter 30's descriptor.py reference;
- the npub: a bech32 encoder written here from BIP-173;
- BIP-340: the secp256k1lab copy pinned with BIP-445 (chapter 102).

  python3 -I coax/tests/bitcoin/oneseed.py                       # compare with the fixture
  python3 -I coax/tests/bitcoin/oneseed.py --coax coax/bin/coax  # also run the example
  python3 -I coax/tests/bitcoin/oneseed.py --write               # write the fixture
"""
import hashlib, json, subprocess, sys
sys.dont_write_bytecode = True   # nothing written into the pinned upstream folder
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
sys.path.insert(0, str(REPO / "coax/tests/nostr"))
sys.path.insert(0, str(HERE))
UP = REPO / "coax/tests/vectors/upstream"
PINS = {
    UP / "bip39/vectors.json": "fa3b937b7cff9c9b8ecd3aa011faeb8d6dd67993174b72326e83f4de8fdb30f8",
    UP / "bip39/english.txt": "2f5eed53a4727b4bf8880d8f3f199efc90e58503646d9ff8eff3a2ed3b24dbda",
    UP / "bip84/bip-0084.mediawiki": "1900feec6cafca65b8c09906ca0658d2d742b4c9b44cb15678996985b6bfe627",
    UP / "bip86/bip-0086.mediawiki": "d8d01dee331da07c2562615bc1f064c1868ec3fce61184973da76d1196c7f5b0",
    REPO / "nostr/nips/official/06.md": "a7d86f43c3723b7a67765e81910a2478007d4655024e2899a163a09e2c834b3a",
    UP / "bip445/python/secp256k1lab/src/secp256k1lab/bip340.py": "428ee8110175b32b67d22a88efe07a44cede2419d0f547949164f2a8f9d76b96",
}
for path, digest in PINS.items():
    assert hashlib.sha256(path.read_bytes()).hexdigest() == digest, path
sys.path.insert(0, str(UP / "bip445/python/secp256k1lab/src"))
import bip32 as B32  # noqa: E402
import address as AD  # noqa: E402
import descriptor as DS  # noqa: E402
from secp256k1lab.bip340 import schnorr_sign, schnorr_verify  # noqa: E402

FIXTURE = REPO / "bitcoin/fixtures/one-seed.out"
EXAMPLE = "bitcoin/examples/one-seed.coax"
H = 2**31
MAINNET, TESTNET = 1, 2
WORDS = (UP / "bip39/english.txt").read_text().split()
NIP06_WORDS = "leader monkey parrot ring guide accident before fence cannon height naive bean"
NIP06_NPUB = "npub1zutzeysacnf9rru6zqwmxd54mud0k44tst6l70ja5mhv8jjumytsd2x7nu"
ABANDON = " ".join(["abandon"] * 11 + ["about"])


def published():
    nip06 = (REPO / "nostr/nips/official/06.md").read_text()
    for text in (NIP06_WORDS, NIP06_NPUB, "7f7ff03d123792d6ac594bfa67bf6d0c0ab55b6b1fdb6249303fe861f1ccba9a",
                 "c15d739894c81a2fcfd3a2df85a0d2c0dbc47a280d092799f144d73d7ae78add",
                 "npub16sdj9zv4f8sl85e45vgq9n7nsgt5qphpvmf7vk8r5hhvmdjxx4es8rq74h"):
        assert text in nip06, text
    b84 = (UP / "bip84/bip-0084.mediawiki").read_text()
    for text in ("bc1qcr8te4kr609gcawutmrza0j4xv80jy8z306fyu", "bc1qnjg0jd8228aq7egyzacy8cys3knf9xvrerkf9g",
                 "bc1q8c6fshw2dlwun7ekn9qwf37cu2rn755upcp6el", ABANDON):
        assert text in b84, text
    b86 = (UP / "bip86/bip-0086.mediawiki").read_text()
    for text in ("bc1p5cyxnuxmeuwuvkwfem96lqzszd02n6xdcjrs20cac6yqjjwudpxqkedrcr",
                 "bc1p4qhjn9zdvkux4e44uhx8tc55attvtyu358kutcqkudyccelu0was9fqzwh",
                 "bc1p3qkhfews2uk44qtvauqyr2ttdsw7svhkl9nkm9s9c3x4ax5h60wqwruhk7",
                 "xpub6BgBgsespWvERF3LHQu6CnqdvfEvtMcQjYrcRzx53QJjSxarj2afYWcLteoGVky7D3UKDP9QyrLprQ3VCECoY49yfdDEHGCtMMj92pReUsQ"):
        assert text in b86, text
    v0 = json.loads((UP / "bip39/vectors.json").read_text())["english"][0]
    assert v0[1] == ABANDON
    return v0[2]


# ---- BIP-39
def words_ok(phrase):
    words = phrase.split()
    if len(words) not in (12, 15, 18, 21, 24) or any(w not in WORDS for w in words): return False
    bits = "".join(format(WORDS.index(w), "011b") for w in words)
    cs = len(bits) // 33
    entropy = int(bits[:-cs], 2).to_bytes(len(bits[:-cs]) // 8, "big")
    return format(hashlib.sha256(entropy).digest()[0], "08b")[:cs] == bits[-cs:]


def seed_of(phrase, passphrase):
    return hashlib.pbkdf2_hmac("sha512", " ".join(phrase.split()).encode(), ("mnemonic" + passphrase).encode(), 2048)


# ---- BIP-32
def derive(seed, path, testnet=False):
    r, node = B32.master(seed, testnet)
    assert r == 78
    for i in path:
        r, node = B32.ckd_node(node, i)
        assert r == 78
    return node


def public(node):
    r, pub = B32.neuter_node(node)
    assert r == 78
    return pub


# ---- the npub (BIP-173 bech32)
CHARSET = "qpzry9x8gf2tvdw0s3jn54khce6mua7l"


def bech32(hrp, data):
    def polymod(values):
        gen = [0x3b6a57b2, 0x26508e6d, 0x1ea119fa, 0x3d4233dd, 0x2a1462b3]
        chk = 1
        for v in values:
            top = chk >> 25
            chk = (chk & 0x1ffffff) << 5 ^ v
            for i in range(5): chk ^= gen[i] if (top >> i) & 1 else 0
        return chk
    acc, bits, five = 0, 0, []
    for b in data:
        acc = acc << 8 | b; bits += 8
        while bits >= 5: bits -= 5; five.append(acc >> bits & 31)
    if bits: five.append(acc << (5 - bits) & 31)
    expand = [ord(c) >> 5 for c in hrp] + [0] + [ord(c) & 31 for c in hrp]
    pm = polymod(expand + five + [0] * 6) ^ 1
    return hrp + "1" + "".join(CHARSET[d] for d in five + [pm >> 5 * (5 - i) & 31 for i in range(6)])


# ---- the card
def nostr_key(seed, account):
    return derive(seed, [44 + H, 1237 + H, account + H, 0, 0])[46:]


def descriptor(seed, network, purpose, account):
    testnet = network != MAINNET
    coin = 0 if network == MAINNET else 1
    fp = B32.hash160(public(derive(seed, [], testnet))[45:])[:4].hex()
    xpub = B32.to_text(public(derive(seed, [purpose + H, coin + H, account + H], testnet)))
    body = f"{'wpkh' if purpose == 84 else 'tr'}([{fp}/{purpose}h/{coin}h/{account}h]{xpub}/<0;1>/*)"
    return body + "#" + DS.checksum(body.encode())


def card(phrase, passphrase, network, account):
    if not words_ok(phrase): return None
    seed = seed_of(phrase, passphrase)
    npub = bech32("npub", public(derive(seed, [44 + H, 1237 + H, account + H, 0, 0]))[46:])
    return (f"nostr m/44h/1237h/{account}h/0/0 {npub}\n" + descriptor(seed, network, 84, account) + "\n"
            + descriptor(seed, network, 86, account) + "\n")


def restore(phrase, passphrase, network, account, text):
    mine = card(phrase, passphrase, network, account)
    return "words refused" if mine is None else "the same card" if mine == text else "mismatch"


def address(seed, network, purpose, branch, index):
    coin = 0 if network == MAINNET else 1
    pub = public(derive(seed, [purpose + H, coin + H, H, branch, index], network != MAINNET))[45:]
    r, script = AD.build("p2wpkh" if purpose == 84 else "p2tr", pub if purpose == 84 else pub[1:])
    assert r > 0
    n, text = AD.encode(network, script)
    assert n > 0
    return text.decode()


def addresses(out, phrase, passphrase, network):
    seed = seed_of(phrase, passphrase)
    for purpose, name in ((84, "wpkh"), (86, "tr")):
        for branch, index in ((0, 0), (0, 1), (1, 0)):
            out.append(f"{name} {branch}/{index}: {address(seed, network, purpose, branch, index)}")


def tagged(tag, msg):
    t = hashlib.sha256(tag.encode()).digest()
    return hashlib.sha256(t + t + msg).digest()


def output_key(x):
    parity, q = AD.tweak(x)
    assert parity >= 0
    return q


def yes(b): return "true" if b else "false"


def transcript(trezor_seed):
    out = []
    out.append("The card of NIP-06's first phrase, account 0")
    c = card(NIP06_WORDS, "", MAINNET, 0)
    out += c.splitlines()
    out.append(f"npub is NIP-06's: {yes(c.splitlines()[0].endswith(NIP06_NPUB))}")
    out.append("Addresses from the card alone")
    addresses(out, NIP06_WORDS, "", MAINNET)
    out.append("Restore")
    out.append("same words, no passphrase: " + restore(NIP06_WORDS, "", MAINNET, 0, c))
    out.append("same words, passphrase x: " + restore(NIP06_WORDS, "x", MAINNET, 0, c))
    out.append("same words, account 1: " + restore(NIP06_WORDS, "", MAINNET, 1, c))
    out.append("last word beauty: " + restore(NIP06_WORDS.replace(" bean", " beauty"), "", MAINNET, 0, c))
    out.append("The card of BIP-84's and BIP-86's phrase")
    a = card(ABANDON, "", MAINNET, 0)
    out += a.splitlines()
    addresses(out, ABANDON, "", MAINNET)
    out.append("The passphrase TREZOR")
    seed = seed_of(ABANDON, "TREZOR")
    assert seed.hex() == trezor_seed
    out.append("seed: " + seed.hex())
    out += card(ABANDON, "TREZOR", MAINNET, 0).splitlines()
    out.append("The same phrase on testnet")
    t = card(ABANDON, "", TESTNET, 0)
    out += t.splitlines()
    out.append(f"npub line unchanged: {yes(t.splitlines()[0] == a.splitlines()[0])}")
    out.append("Separate keys")
    seed = seed_of(ABANDON, "")
    nostr = nostr_key(seed, 0)
    wallet = derive(seed, [86 + H, H, H, 0, 0])[46:]
    np = public(derive(seed, [44 + H, 1237 + H, H, 0, 0]))[46:]
    wp = public(derive(seed, [86 + H, H, H, 0, 0]))[46:]
    wq, nq = output_key(wp), output_key(np)
    out.append("nostr key m/44'/1237'/0'/0/0: " + np.hex())
    out.append("wallet key m/86'/0'/0'/0/0: " + wp.hex())
    out.append("the wallet key's BIP-86 output key: " + wq.hex())
    out.append("the nostr key's BIP-86 output key: " + nq.hex())
    out.append("A raw 32-byte signature request to the Nostr key")
    h = tagged("TapSighash", b"a spend the agent never approved")
    out.append("message: " + h.hex())
    sig = schnorr_sign(h, nostr, bytes(32))
    out.append("valid under the nostr key, as in a <key> OP_CHECKSIG leaf: " + yes(schnorr_verify(h, np, sig)))
    out.append("valid under the nostr key's output key: " + yes(schnorr_verify(h, nq, sig)))
    out.append("valid under the wallet key: " + yes(schnorr_verify(h, wp, sig)))
    out.append("valid under the wallet key's output key: " + yes(schnorr_verify(h, wq, sig)))
    assert wallet != nostr
    return "\n".join(out) + "\n"


def main():
    text = transcript(published())
    print("pinned documents and their published values: ok")
    if "--write" in sys.argv:
        FIXTURE.write_text(text)
        print(f"wrote {FIXTURE.relative_to(REPO)}")
    assert FIXTURE.read_text() == text, "the fixture differs from Python's transcript"
    print(f"one-seed fixture: {text.count(chr(10))} lines agree with Python")
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
