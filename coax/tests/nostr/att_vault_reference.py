#!/usr/bin/env python3
"""An independent NIP-ATT endpoint and NIP-VAULT object for chapter 83.

Written from NIP-ATT, NIP-VAULT, FIPS-197, NIST SP 800-38D, RFC 5869,
RFC 8785 (integers and ASCII strings only), NIP-01, NIP-44 v2 and BIP-340
with hashlib, hmac, base64 and integers; it shares no code with Coax
(BIP-340 and NIP-44 come from nip59_reference.py, also independent).
Tooling only, never shipped. Test keys, test secrets and a test token only:
the token is not signed by Google.

  python3 coax/tests/nostr/att_vault_reference.py

It prints the release, head and endpoint events (and the refusal variants)
signed with aux 0, the slot and object the example opens, and the values
nostr/examples/checks/attested-vault.coax compares.
"""
import base64
import hashlib
import hmac
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from nip59_reference import conversation_key, k, nip44_decrypt, nip44_encrypt, signed, xonly  # noqa: E402

# ---- AES-256 forward cipher, FIPS-197 5.1, with a table built once
def xtime(a):
    return ((a << 1) & 255) ^ (0x1B if a & 0x80 else 0)


def gmul(a, b):
    r = 0
    for _ in range(8):
        if b & 1:
            r ^= a
        a, b = xtime(a), b >> 1
    return r


def sbox_of(x):
    inv = 0 if x == 0 else next(y for y in range(1, 256) if gmul(x, y) == 1)
    rot = lambda b, n: ((b << n) | (b >> (8 - n))) & 255
    return inv ^ rot(inv, 1) ^ rot(inv, 2) ^ rot(inv, 3) ^ rot(inv, 4) ^ 0x63


SBOX = [sbox_of(x) for x in range(256)]


def expand_key(key):
    w = [list(key[4 * i:4 * i + 4]) for i in range(8)]
    rcon = 1
    for i in range(8, 60):
        t = list(w[i - 1])
        if i % 8 == 0:
            t = [SBOX[b] for b in t[1:] + t[:1]]
            t[0] ^= rcon
            rcon = xtime(rcon)
        elif i % 8 == 4:
            t = [SBOX[b] for b in t]
        w.append([a ^ b for a, b in zip(w[i - 8], t)])
    return [sum(w[4 * r:4 * r + 4], []) for r in range(15)]


def encrypt_block(rk, block):
    s = [a ^ b for a, b in zip(block, rk[0])]
    for rnd in range(1, 15):
        s = [SBOX[b] for b in s]
        s = [s[r + 4 * ((c + r) % 4)] for c in range(4) for r in range(4)]
        if rnd != 14:
            m = []
            for c in range(4):
                col = s[4 * c:4 * c + 4]
                m += [gmul(col[0], 2) ^ gmul(col[1], 3) ^ col[2] ^ col[3],
                      col[0] ^ gmul(col[1], 2) ^ gmul(col[2], 3) ^ col[3],
                      col[0] ^ col[1] ^ gmul(col[2], 2) ^ gmul(col[3], 3),
                      gmul(col[0], 3) ^ col[1] ^ col[2] ^ gmul(col[3], 2)]
            s = m
        s = [a ^ b for a, b in zip(s, rk[rnd])]
    return bytes(s)


# ---- GCM, SP 800-38D, 96-bit nonce
def ghash_mul(x, y):
    z, v = 0, y
    for i in range(128):
        if (x >> (127 - i)) & 1:
            z ^= v
        v = (v >> 1) ^ (0xE1 << 120) if v & 1 else v >> 1
    return z


def ghash(h, aad, ct):
    def blocks(d):
        return [d[i:i + 16].ljust(16, b"\0") for i in range(0, len(d), 16)]
    y = 0
    for b in blocks(aad) + blocks(ct) + [(8 * len(aad)).to_bytes(8, "big") + (8 * len(ct)).to_bytes(8, "big")]:
        y = ghash_mul(y ^ int.from_bytes(b, "big"), h)
    return y


def gcm_seal(key, nonce, aad, plain):
    rk = expand_key(key)
    h = int.from_bytes(encrypt_block(rk, bytes(16)), "big")
    out = b""
    for i in range(0, len(plain), 16):
        pad = encrypt_block(rk, nonce + (i // 16 + 2).to_bytes(4, "big"))
        out += bytes(a ^ b for a, b in zip(plain[i:i + 16], pad))
    tag = ghash(h, aad, out) ^ int.from_bytes(encrypt_block(rk, nonce + b"\0\0\0\1"), "big")
    return out + tag.to_bytes(16, "big")


def gcm_open(key, nonce, aad, sealed):
    rk = expand_key(key)
    h = int.from_bytes(encrypt_block(rk, bytes(16)), "big")
    ct, tag = sealed[:-16], sealed[-16:]
    want = ghash(h, aad, ct) ^ int.from_bytes(encrypt_block(rk, nonce + b"\0\0\0\1"), "big")
    assert want.to_bytes(16, "big") == tag, "gcm tag"
    out = b""
    for i in range(0, len(ct), 16):
        pad = encrypt_block(rk, nonce + (i // 16 + 2).to_bytes(4, "big"))
        out += bytes(a ^ b for a, b in zip(ct[i:i + 16], pad))
    return out


def self_check():
    key = bytes.fromhex("feffe9928665731c6d6a8f9467308308feffe9928665731c6d6a8f9467308308")
    nonce = bytes.fromhex("cafebabefacedbaddecaf888")
    p = bytes.fromhex("d9313225f88406e5a55909c5aff5269a86a7a9531534f7da2e4c303d8a318a721c3c0c95956809532fcf0e2449a6b525b16aedf5aa0de657ba637b39")
    a = bytes.fromhex("feedfacedeadbeeffeedfacedeadbeefabaddad2")
    assert gcm_seal(key, nonce, a, p)[-16:].hex() == "76fc6ece0f4e1768cddf8853bb2d551b"
    assert encrypt_block(expand_key(bytes(range(32))), bytes.fromhex("00112233445566778899aabbccddeeff")).hex() == "8ea2b7ca516745bfeafc49904b496089"


def hkdf(ikm, salt, info, n=32):
    prk = hmac.new(salt, ikm, hashlib.sha256).digest()
    out, t, i = b"", b"", 1
    while len(out) < n:
        t = hmac.new(prk, t + info + bytes([i]), hashlib.sha256).digest()
        out, i = out + t, i + 1
    return out[:n]


def jcs(v):
    # RFC 8785 for these documents: integers, ASCII strings, nested objects.
    return json.dumps(v, separators=(",", ":"), sort_keys=True, ensure_ascii=False)


def b64(b):
    return base64.b64encode(b).decode()


def b64url(b):
    return base64.urlsafe_b64encode(b).decode().rstrip("=")


def sha(b):
    return hashlib.sha256(b).digest()


T0 = 1792000000
K1, K2, K3, K4 = k(1), k(2), k(3), k(4)   # publisher, person, endpoint, operator
P1, P2, P3, P4 = (xonly(s).hex() for s in (K1, K2, K3, K4))
IMAGE = "sha256:" + sha(b"vault-workload image, chapter 83").hex()
OTHER_IMAGE = "sha256:" + sha(b"another image").hex()


def release(published_at, image):
    body = {"v": "openagents.att-release.v1", "requires": [], "workload": "vault", "publisher": P1,
            "image": {"reference": "us-docker.pkg.dev/openagents-vault/vault/vault-workload", "digest": image},
            "platforms": [{"kind": "gcp-confidential-space", "hwmodel": "GCP_INTEL_TDX", "support": "STABLE"}],
            "measurements": [], "gpu": None, "models": [], "components": [],
            "source": {"repo": "https://github.com/OpenAgentsInc/openagents", "commit": "0" * 40,
                       "recipe": "deploy/att/vault.recipe", "recipe_digest": "sha256:" + sha(b"recipe").hex()},
            "rebuilds": [], "transparency": [], "changes": "First release of the vault workload.",
            "published_at": published_at}
    c = jcs(body)
    tags = [["t", "oa:att-release:v1"], ["x", sha(c.encode()).hex()], ["w", "vault"]]
    return c, json.loads(signed(K1, bytes(32), published_at, 3202, tags, c))


def head(release_id, effective_at, generation):
    body = {"v": "openagents.att-head.v1", "requires": [], "workload": "vault", "generation": generation,
            "notice_seconds": 604800,
            "admitted": [{"release": release_id, "effective_at": effective_at, "retire_at": None}],
            "emergency": None}
    c = jcs(body)
    tags = [["d", "vault"], ["t", "oa:att-head:v1"], ["e", release_id]]
    return c, json.loads(signed(K1, bytes(32), T0 - 1000, 30202, tags, c))


def binding(endpoint_hex, release_id):
    return sha(b"openagents.att.v1\0" + bytes.fromhex(endpoint_hex) + bytes.fromhex(release_id)).hex()


def token(nonce, image, dbgstat="disabled-since-boot", support=("LATEST", "STABLE", "USABLE")):
    header = {"alg": "RS256", "kid": "chapter-83-test", "typ": "JWT"}
    claims = {"aud": "openagents.att.v1", "dbgstat": dbgstat, "eat_nonce": [nonce], "exp": T0 + 3600,
              "hwmodel": "GCP_INTEL_TDX", "iat": T0 - 60, "iss": "https://confidentialcomputing.googleapis.com",
              "submods": {"confidential_space": {"support_attributes": list(support)},
                          "container": {"image_digest": image}},
              "swname": "CONFIDENTIAL_SPACE"}
    return b64url(jcs(header).encode()) + "." + b64url(jcs(claims).encode()) + "." + b64url(b"not signed by Google: test token")


INSTANCE = sha(b"instance 1").hex()


def endpoint(signer, release_id, tok, bind):
    body = {"v": "openagents.att-endpoint.v1", "requires": [], "release": release_id, "endpoint": xonly(signer).hex(),
            "hpke": None, "binding": bind,
            "evidence": [{"kind": "gcp-confidential-space-token", "format": "pki", "token": tok}],
            "operator": P4, "issued_at": T0 - 60, "valid_until": T0 + 3540}
    c = jcs(body)
    tags = [["d", INSTANCE], ["t", "oa:att-endpoint:v1"], ["a", "30202:" + P1 + ":vault"], ["e", release_id],
            ["expiration", str(T0 + 3540)]]
    return c, json.loads(signed(signer, bytes(32), T0 - 60, 30203, tags, c))


def att():
    rc, rel = release(T0 - 700000, IMAGE)
    rc2, rel2 = release(T0 - 100000, OTHER_IMAGE)
    rid = rel["id"]
    hc, hd = head(rid, T0 - 700000 + 604800, 7)
    bind = binding(P3, rid)
    ec, ep = endpoint(K3, rid, token(bind, IMAGE), bind)
    out = {"release": rel, "release_x": sha(rc.encode()).hex(), "release_len": len(rc),
           "release2": rel2, "head": hd, "head_len": len(hc), "binding": bind, "endpoint": ep, "endpoint_len": len(ec),
           "token": token(bind, IMAGE)}
    # refusal variants, each signed by its own signer
    bind2 = binding(P3, rel2["id"])
    out["ep_unlisted"] = endpoint(K3, rel2["id"], token(bind2, OTHER_IMAGE), bind2)[1]
    out["ep_wrong_image"] = endpoint(K3, rid, token(bind, OTHER_IMAGE), bind)[1]
    out["ep_debug"] = endpoint(K3, rid, token(bind, IMAGE, "enabled", ("LATEST", "USABLE")), bind)[1]
    bind4 = binding(P4, rid)   # the operator's key claims the instance; the token names key 3's binding
    out["ep_other_key"] = endpoint(K4, rid, token(bind, IMAGE), bind4)[1]
    return out


# ---- NIP-VAULT, tier user, one nostr slot and one device slot
VAULT = sha(b"chapter 83 vault").hex()
OBJECT = sha(b"chapter 83 object").hex()
VMK = sha(b"chapter 83 VMK, test only")
NOSTR_SECRET = sha(b"chapter 83 nostr slot secret, test only")
DEVICE_SECRET = sha(b"chapter 83 device slot secret, test only")
DEK = sha(b"chapter 83 DEK, test only")
PLAIN = (b"Statement for the chapter 83 vault: opening balance 21000 sat, "
         b"closing balance 34000 sat. Test data only.\n") * 10


def slot(slot_id, method, params, secret, label, nonce):
    body = {"v": "openagents.vault-slot.v1", "requires": [], "vault": VAULT, "slot": slot_id, "method": method,
            "params": params, "label": label, "created_at": T0 - 5000}
    aad = jcs(body).encode()
    key = hkdf(secret, bytes.fromhex(slot_id), b"openagents.vault.v1/slot\0" + method.encode())
    body["nonce"] = b64(nonce)
    body["vmk"] = b64(gcm_seal(key, nonce, aad, VMK))
    return jcs(body), aad, key


def vault_object(object_id, plain, chunk_bytes, kwrap, key_id, prefix, wnonce, created_at):
    pieces = [plain[i:i + chunk_bytes] for i in range(0, len(plain), chunk_bytes)] or [b""]
    core = {"object": object_id, "vault": VAULT, "tier": "user",
            "content": {"alg": "aes-256-gcm-chunked-v1", "chunk_bytes": chunk_bytes, "nonce_prefix": b64(prefix),
                        "chunks": len(pieces)}, "created_at": created_at}
    core_digest = sha(jcs(core).encode())
    wrap = {"mode": "user", "key": key_id, "nonce": b64(wnonce),
            "dek": b64(gcm_seal(kwrap, wnonce, b"openagents.vault.v1/wrap-user\0" + core_digest, DEK))}
    header = jcs({"v": "openagents.vault-object.v1", "requires": [], "core": core, "wraps": [wrap]}).encode()
    body = b""
    for i, p in enumerate(pieces):
        last = 1 if i == len(pieces) - 1 else 0
        nonce = prefix + i.to_bytes(4, "big") + bytes([last])
        body += gcm_seal(DEK, nonce, b"openagents.vault.v1/chunk\0" + core_digest, p)
    return core, core_digest, wrap, header, b"OAVAULT1" + len(header).to_bytes(4, "big") + header + body


def vault():
    ck = conversation_key(K2, bytes.fromhex(P2))
    sealed = nip44_encrypt(ck, sha(b"slot nonce"), NOSTR_SECRET.hex().encode())
    assert nip44_decrypt(ck, sealed) == NOSTR_SECRET.hex().encode()
    s1, aad1, key1 = slot(sha(b"slot 1").hex(), "nostr", {"pubkey": P2, "sealed": sealed}, NOSTR_SECRET,
                          "Key 2 (test)", sha(b"n1")[:12])
    s2, aad2, key2 = slot(sha(b"slot 2").hex(), "device", {"platform": "linux", "key_ref": "openagents-vault-test"},
                          DEVICE_SECRET, "This laptop (test)", sha(b"n2")[:12])
    kwrap = hkdf(VMK, b"", b"openagents.vault.v1/user-wrap")
    key_id = sha(b"openagents.vault.v1/key-id\0" + kwrap)[:16].hex()
    prefix = sha(b"prefix")[:7]
    chunk_bytes = 1024
    pieces = [PLAIN[i:i + chunk_bytes] for i in range(0, len(PLAIN), chunk_bytes)]
    core = {"object": OBJECT, "vault": VAULT, "tier": "user",
            "content": {"alg": "aes-256-gcm-chunked-v1", "chunk_bytes": chunk_bytes, "nonce_prefix": b64(prefix),
                        "chunks": len(pieces)}, "created_at": T0 - 4000}
    core_digest = sha(jcs(core).encode())
    wnonce = sha(b"wrap nonce")[:12]
    wrap = {"mode": "user", "key": key_id, "nonce": b64(wnonce),
            "dek": b64(gcm_seal(kwrap, wnonce, b"openagents.vault.v1/wrap-user\0" + core_digest, DEK))}
    header = jcs({"v": "openagents.vault-object.v1", "requires": [], "core": core, "wraps": [wrap]}).encode()
    body = b""
    for i, p in enumerate(pieces):
        last = 1 if i == len(pieces) - 1 else 0
        nonce = prefix + i.to_bytes(4, "big") + bytes([last])
        body += gcm_seal(DEK, nonce, b"openagents.vault.v1/chunk\0" + core_digest, p)
    blob = b"OAVAULT1" + len(header).to_bytes(4, "big") + header + body
    # open it back by hand: slot -> VMK -> K_user_wrap -> DEK -> chunks
    s = json.loads(s1)
    vmk = gcm_open(key1, base64.b64decode(s["nonce"]), aad1, base64.b64decode(s["vmk"]))
    assert vmk == VMK
    dek = gcm_open(kwrap, wnonce, b"openagents.vault.v1/wrap-user\0" + core_digest, base64.b64decode(wrap["dek"]))
    assert dek == DEK
    return {"slot_nostr": s1, "slot_device": s2, "aad_nostr": aad1.decode(), "aad_nostr_len": len(aad1),
            "slot_key_nostr": key1.hex(), "slot_key_device": key2.hex(), "sealed": sealed,
            "kwrap": kwrap.hex(), "key_id": key_id, "core": jcs(core), "core_digest": core_digest.hex(),
            "header_len": len(header), "blob_len": len(blob), "chunks": len(pieces),
            "plain_len": len(PLAIN), "plain_sha": sha(PLAIN).hex(), "blob": b64(blob),
            "device_secret": DEVICE_SECRET.hex(), "vmk": VMK.hex(), "dek": DEK.hex(),
            "ck": ck.hex(), "nostr_secret": NOSTR_SECRET.hex(), "small": small()}


def small():
    kwrap = hkdf(VMK, b"", b"openagents.vault.v1/user-wrap")
    key_id = sha(b"openagents.vault.v1/key-id\0" + kwrap)[:16].hex()
    core, cd, wrap, header, blob = vault_object(sha(b"small object").hex(), b"Test data only: 21000 sat.", 1024,
                                                kwrap, key_id, sha(b"small prefix")[:7], sha(b"small wrap")[:12], T0 - 3000)
    return {"blob": b64(blob), "core_digest": cd.hex(), "header_len": len(header), "blob_len": len(blob)}


if __name__ == "__main__":
    self_check()
    print(json.dumps({"att": att(), "vault": vault()}, indent=1))
