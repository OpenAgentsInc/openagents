#!/usr/bin/env python3
"""An independent NIP-59 gift wrap for nostr/examples/checks/gift-wrap.coax.

Written from NIP-01, NIP-44 v2, NIP-59, BIP-340 and RFC 8439 with only
hashlib, hmac, base64 and integers; it shares no code with Coax. Tooling
only, never shipped. Test keys only.

  python3 coax/tests/nostr/nip59_reference.py

It builds chapter 25's wrap from the same keys, nonces, aux bytes and time
offsets as nostr/examples/gift-wrap.coax and prints the rumor, seal and wrap
JSON, then opens NIP-59's own published example with the recipient's key.
"""
import base64
import hashlib
import hmac
import json
import struct

P = 2**256 - 2**32 - 977
N = 0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEBAAEDCE6AF48A03BBFD25E8CD0364141
G = (0x79BE667EF9DCBBAC55A06295CE870B07029BFCDB2DCE28D959F2815B16F81798,
     0x483ADA7726A3C4655DA4FBFC0E1108A8FD17B448A68554199C47D08FFB10D4B8)


def add(a, b):
    if a is None:
        return b
    if b is None:
        return a
    if a[0] == b[0] and (a[1] + b[1]) % P == 0:
        return None
    if a == b:
        m = 3 * a[0] * a[0] * pow(2 * a[1], P - 2, P) % P
    else:
        m = (b[1] - a[1]) * pow(b[0] - a[0], P - 2, P) % P
    x = (m * m - a[0] - b[0]) % P
    return (x, (m * (a[0] - x) - a[1]) % P)


def mul(k, pt):
    r = None
    while k:
        if k & 1:
            r = add(r, pt)
        pt = add(pt, pt)
        k >>= 1
    return r


def lift(x):
    y = pow((x * x * x + 7) % P, (P + 1) // 4, P)
    assert y * y % P == (x * x * x + 7) % P
    return (x, y if y % 2 == 0 else P - y)


def tagged(tag, data):
    t = hashlib.sha256(tag.encode()).digest()
    return hashlib.sha256(t + t + data).digest()


def xonly(secret):
    return mul(int.from_bytes(secret, "big"), G)[0].to_bytes(32, "big")


def sign(secret, msg, aux):
    d0 = int.from_bytes(secret, "big")
    pt = mul(d0, G)
    d = d0 if pt[1] % 2 == 0 else N - d0
    t = (d ^ int.from_bytes(tagged("BIP0340/aux", aux), "big")).to_bytes(32, "big")
    k0 = int.from_bytes(tagged("BIP0340/nonce", t + pt[0].to_bytes(32, "big") + msg), "big") % N
    r = mul(k0, G)
    k = k0 if r[1] % 2 == 0 else N - k0
    e = int.from_bytes(tagged("BIP0340/challenge", r[0].to_bytes(32, "big") + pt[0].to_bytes(32, "big") + msg), "big") % N
    return r[0].to_bytes(32, "big") + ((k + e * d) % N).to_bytes(32, "big")


def verify(pub, msg, sig):
    pt = lift(int.from_bytes(pub, "big"))
    r = int.from_bytes(sig[:32], "big")
    s = int.from_bytes(sig[32:], "big")
    e = int.from_bytes(tagged("BIP0340/challenge", sig[:32] + pub + msg), "big") % N
    rr = add(mul(s, G), mul(N - e, pt))
    return rr is not None and rr[1] % 2 == 0 and rr[0] == r


def rotl(v, c):
    return ((v << c) & 0xFFFFFFFF) | (v >> (32 - c))


def chacha20(key, nonce, data):
    out = bytearray()
    for block in range((len(data) + 63) // 64):
        s = [0x61707865, 0x3320646E, 0x79622D32, 0x6B206574] + list(struct.unpack("<8I", key)) + [block] + list(struct.unpack("<3I", nonce))
        w = s[:]
        for _ in range(10):
            for a, b, c, d in ((0, 4, 8, 12), (1, 5, 9, 13), (2, 6, 10, 14), (3, 7, 11, 15),
                               (0, 5, 10, 15), (1, 6, 11, 12), (2, 7, 8, 13), (3, 4, 9, 14)):
                w[a] = (w[a] + w[b]) & 0xFFFFFFFF; w[d] = rotl(w[d] ^ w[a], 16)
                w[c] = (w[c] + w[d]) & 0xFFFFFFFF; w[b] = rotl(w[b] ^ w[c], 12)
                w[a] = (w[a] + w[b]) & 0xFFFFFFFF; w[d] = rotl(w[d] ^ w[a], 8)
                w[c] = (w[c] + w[d]) & 0xFFFFFFFF; w[b] = rotl(w[b] ^ w[c], 7)
        ks = struct.pack("<16I", *[(w[i] + s[i]) & 0xFFFFFFFF for i in range(16)])
        chunk = data[block * 64:(block + 1) * 64]
        out += bytes(x ^ y for x, y in zip(chunk, ks))
    return bytes(out)


def conversation_key(secret, pub):
    shared = mul(int.from_bytes(secret, "big"), lift(int.from_bytes(pub, "big")))[0].to_bytes(32, "big")
    return hmac.new(b"nip44-v2", shared, hashlib.sha256).digest()


def expand(prk, info, length):
    out, t, i = b"", b"", 1
    while len(out) < length:
        t = hmac.new(prk, t + info + bytes([i]), hashlib.sha256).digest()
        out += t
        i += 1
    return out[:length]


def padded_len(n):
    if n <= 32:
        return 32
    nxt = 1 << (n - 1).bit_length()
    chunk = 32 if nxt <= 256 else nxt // 8
    return chunk * ((n - 1) // chunk + 1)


def nip44_encrypt(ck, nonce, text):
    keys = expand(ck, nonce, 76)
    padded = struct.pack(">H", len(text)) + text + bytes(padded_len(len(text)) - len(text))
    ct = chacha20(keys[:32], keys[32:44], padded)
    mac = hmac.new(keys[44:], nonce + ct, hashlib.sha256).digest()
    return base64.b64encode(b"\x02" + nonce + ct + mac).decode()


def nip44_decrypt(ck, payload):
    raw = base64.b64decode(payload)
    assert raw[0] == 2
    nonce, ct, mac = raw[1:33], raw[33:-32], raw[-32:]
    keys = expand(ck, nonce, 76)
    assert hmac.compare_digest(mac, hmac.new(keys[44:], nonce + ct, hashlib.sha256).digest())
    padded = chacha20(keys[:32], keys[32:44], ct)
    n = struct.unpack(">H", padded[:2])[0]
    return padded[2:2 + n]


def dumps(v):
    return json.dumps(v, separators=(",", ":"), ensure_ascii=False)


def event_id(pub_hex, at, kind, tags, content):
    return hashlib.sha256(dumps([0, pub_hex, at, kind, tags, content]).encode()).digest()


def signed(secret, aux, at, kind, tags, content):
    pub = xonly(secret).hex()
    eid = event_id(pub, at, kind, tags, content)
    sig = sign(secret, eid, aux)
    return dumps({"id": eid.hex(), "pubkey": pub, "created_at": at, "kind": kind,
                  "tags": tags, "content": content, "sig": sig.hex()})


def k(n):
    return n.to_bytes(32, "big")


def chapter_wrap():
    alice = k(3)
    bob = bytes.fromhex("b7e151628aed2a6abf7158809cf4f3c762e7160f38b4da56a784d9045190cfef")
    one_time = k(5)
    seal_nonce, wrap_nonce = bytes([0x11]) * 32, bytes([0x22]) * 32
    seal_aux, wrap_aux = bytes([0x33]) * 32, bytes([0x44]) * 32
    now, seal_back, wrap_back = 1793300000, 3600, 200000
    bob_pub = xonly(bob).hex()
    tags = [["p", bob_pub]]
    content = "Are you going to the party tonight?"
    apub = xonly(alice).hex()
    rid = event_id(apub, now, 14, tags, content)
    rumor = dumps({"id": rid.hex(), "pubkey": apub, "created_at": now, "kind": 14, "tags": tags, "content": content})
    seal_at = now - seal_back % 172800
    wrap_at = now - wrap_back % 172800
    seal = signed(alice, seal_aux, seal_at, 13, [], nip44_encrypt(conversation_key(alice, bytes.fromhex(bob_pub)), seal_nonce, rumor.encode()))
    wrap = signed(one_time, wrap_aux, wrap_at, 1059, [["p", bob_pub]],
                  nip44_encrypt(conversation_key(one_time, bytes.fromhex(bob_pub)), wrap_nonce, seal.encode()))
    print("alice", apub)
    print("bob", bob_pub)
    print("one-time", xonly(one_time).hex())
    print("rumor id", rid.hex())
    print("seal_at", seal_at, "wrap_at", wrap_at)
    print("rumor", len(rumor), rumor)
    print("seal", len(seal), seal)
    print("wrap", len(wrap), wrap)
    assert unwrap(json.loads(wrap), bob) == (seal, rumor)
    print("the wrap opens for bob to the same seal and rumor")
    print("seal ck", conversation_key(alice, bytes.fromhex(bob_pub)).hex())
    print("wrap ck", conversation_key(one_time, bytes.fromhex(bob_pub)).hex())


def unwrap(wrap, recipient):
    """Open a gift wrap (a dict) as recipient, checking every layer."""
    assert ["p", xonly(recipient).hex()] in wrap["tags"]
    wid = event_id(wrap["pubkey"], wrap["created_at"], wrap["kind"], wrap["tags"], wrap["content"])
    assert wrap["kind"] in (1059, 21059) and wid.hex() == wrap["id"]
    assert verify(bytes.fromhex(wrap["pubkey"]), wid, bytes.fromhex(wrap["sig"]))
    seal_text = nip44_decrypt(conversation_key(recipient, bytes.fromhex(wrap["pubkey"])), wrap["content"]).decode()
    seal = json.loads(seal_text)
    sid = event_id(seal["pubkey"], seal["created_at"], seal["kind"], seal["tags"], seal["content"])
    assert seal["kind"] == 13 and seal["tags"] == [] and sid.hex() == seal["id"]
    assert verify(bytes.fromhex(seal["pubkey"]), sid, bytes.fromhex(seal["sig"]))
    rumor_text = nip44_decrypt(conversation_key(recipient, bytes.fromhex(seal["pubkey"])), seal["content"]).decode()
    rumor = json.loads(rumor_text)
    rid = event_id(rumor["pubkey"], rumor["created_at"], rumor["kind"], rumor["tags"], rumor["content"])
    assert "sig" not in rumor and rid.hex() == rumor["id"] and rumor["pubkey"] == seal["pubkey"]
    return seal_text, rumor_text


def nip59_example(path):
    """Open the wrap printed in NIP-59's example with the recipient's key."""
    text = open(path).read()
    block = text.split("### 3. Wrap the seal", 1)[1].split("```json", 1)[1].split("```", 1)[0]
    try:
        json.loads(block)
        print("nip-59 wrap parses as published")
    except json.JSONDecodeError as e:
        print("nip-59 wrap does not parse as published:", e)
    wrap = json.loads(block.replace("]],\n}", "]]\n}"))
    recipient = bytes.fromhex("e108399bd8424357a710b606ae0c13166d853d327e47a6e5e038197346bdbf45")
    seal_text, rumor_text = unwrap(wrap, recipient)
    print("nip-59 seal", seal_text)
    print("nip-59 rumor", rumor_text)


if __name__ == "__main__":
    import pathlib
    chapter_wrap()
    nip59_example(pathlib.Path(__file__).resolve().parents[3] / "nostr/nips/official/59.md")
