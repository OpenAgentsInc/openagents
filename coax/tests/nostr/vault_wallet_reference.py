#!/usr/bin/env python3
"""Firm reason: 2 (an independent oracle). The wallet seed backup's test vectors (#13217).

Written from NIP-VAULT ("Wallet seed backup") with hashlib (SHA-256, HMAC, and OpenSSL's scrypt),
base64, integers and att_vault_reference.py's own AES-256-GCM and HKDF; it shares no code with
Coax. Tooling only, never shipped. TEST SECRETS ONLY: every key, id and nonce is the hash of a
fixed phrase, the seed is BIP-39's all-zero vector and the recovery code is its 0x7f vector.

  python3 coax/tests/nostr/vault_wallet_reference.py            # prints the vectors (JSON)
  python3 coax/tests/nostr/vault_wallet_reference.py --check    # the committed files match
  python3 coax/tests/nostr/vault_wallet_reference.py --write    # rewrites them

It writes proto/nostr/fixtures/wallet-backup-v1.vectors.json (the committed vectors any client can check
itself against) and coax/tests/nostr/vault_wallet_fixtures.coax (the same values for the checks of
coax/lib/nostr/vault-wallet.coax, which must make and open them byte for byte).
"""
import base64
import hashlib
import json
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
from att_vault_reference import b64, gcm_open, gcm_seal, hkdf, jcs, self_check, sha  # noqa: E402

ROOT = os.path.normpath(os.path.join(HERE, "..", "..", ".."))
VECTORS = os.path.join(ROOT, "proto", "nostr", "fixtures", "wallet-backup-v1.vectors.json")
FIXTURES = os.path.join(HERE, "vault_wallet_fixtures.coax")

ACCOUNT = "acct_aaaaaaaaaaaaaaaaaaaaaaaaaa"
T0 = 1794000000
PRF_INPUT = sha(b"openagents.wallet-backup.v1/prf")
SEED = bytes(16)                                   # "abandon ... about"
CODE_ENTROPY = bytes([0x7F]) * 32
CODE = ("legal winner thank year wave sausage worth useful legal winner thank year wave sausage worth useful "
        "legal winner thank year wave sausage worth title")
LOG_N = 17


def t(name):
    return sha(b"wallet backup vector, test only: " + name.encode())


def scrypt(words, salt, log_n):
    return hashlib.scrypt(words.encode(), salt=salt, n=1 << log_n, r=8, p=1, maxmem=1 << 30, dklen=32)


def slot(vault, slot_id, method, params, secret, label, nonce, vmk, created_at):
    body = {"v": "openagents.vault-slot.v1", "requires": [], "vault": vault, "slot": slot_id, "method": method,
            "params": params, "label": label, "created_at": created_at}
    aad = jcs(body).encode()
    key = hkdf(secret, bytes.fromhex(slot_id), b"openagents.vault.v1/slot\0" + method.encode())
    body["nonce"] = b64(nonce)
    body["vmk"] = b64(gcm_seal(key, nonce, aad, vmk))
    return jcs(body)


def slot_open(text, secret):
    s = json.loads(text)
    body = {k: v for k, v in s.items() if k not in ("nonce", "vmk")}
    key = hkdf(secret, bytes.fromhex(s["slot"]), b"openagents.vault.v1/slot\0" + s["method"].encode())
    return gcm_open(key, base64.b64decode(s["nonce"]), jcs(body).encode(), base64.b64decode(s["vmk"]))


def record(vault, epoch, slot_ids, seed, created_at):
    return jcs({"v": "openagents.wallet-backup.v1", "requires": [], "account": ACCOUNT, "vault": vault, "epoch": epoch,
                "created_at": created_at, "entropy": seed.hex(), "slots": slot_ids}).encode()


def vault_object(vault, object_id, plain, vmk, dek, prefix, wnonce, created_at):
    assert len(plain) <= 1024
    core = {"object": object_id, "vault": vault, "tier": "user",
            "content": {"alg": "aes-256-gcm-chunked-v1", "chunk_bytes": 1024, "nonce_prefix": b64(prefix), "chunks": 1},
            "created_at": created_at}
    core_digest = sha(jcs(core).encode())
    kwrap = hkdf(vmk, b"", b"openagents.vault.v1/user-wrap")
    key_id = sha(b"openagents.vault.v1/key-id\0" + kwrap)[:16].hex()
    wrap = {"mode": "user", "key": key_id, "nonce": b64(wnonce),
            "dek": b64(gcm_seal(kwrap, wnonce, b"openagents.vault.v1/wrap-user\0" + core_digest, dek))}
    header = jcs({"v": "openagents.vault-object.v1", "requires": [], "core": core, "wraps": [wrap]}).encode()
    body = gcm_seal(dek, prefix + (0).to_bytes(4, "big") + b"\x01", b"openagents.vault.v1/chunk\0" + core_digest, plain)
    return b"OAVAULT1" + len(header).to_bytes(4, "big") + header + body


def object_open(blob, vmk):
    n = int.from_bytes(blob[8:12], "big")
    header = json.loads(blob[12:12 + n])
    core_digest = sha(jcs(header["core"]).encode())
    kwrap = hkdf(vmk, b"", b"openagents.vault.v1/user-wrap")
    w = header["wraps"][0]
    dek = gcm_open(kwrap, base64.b64decode(w["nonce"]), b"openagents.vault.v1/wrap-user\0" + core_digest, base64.b64decode(w["dek"]))
    prefix = base64.b64decode(header["core"]["content"]["nonce_prefix"])
    return gcm_open(dek, prefix + (0).to_bytes(4, "big") + b"\x01", b"openagents.vault.v1/chunk\0" + core_digest, blob[12 + n:])


def vectors():
    # What the app's first backup draws, in the order vault-wallet.coax reads its random bytes:
    # vault, VMK, DEK, object, the passkey slot, the recovery slot, their nonces, the wrap's nonce, the prefix.
    fresh = (t("vault") + t("vmk") + t("dek") + t("object") + t("slot passkey") + t("slot recovery")
             + t("nonce passkey")[:12] + t("nonce recovery")[:12] + t("nonce wrap")[:12] + t("prefix")[:7])
    vault, vmk, dek, obj = fresh[0:32].hex(), fresh[32:64], fresh[64:96], fresh[96:128].hex()
    s_prf, s_rec = fresh[128:160].hex(), fresh[160:192].hex()
    n_prf, n_rec, n_wrap, prefix = fresh[192:204], fresh[204:216], fresh[216:228], fresh[228:235]
    prf_secret = t("passkey PRF output")
    credential = t("credential id")[:20]
    salt = t("recovery salt")[:16]
    small = scrypt("pleaseletmein", salt, 4)
    rec_secret = scrypt(CODE, salt, LOG_N)
    slot_prf = slot(vault, s_prf, "passkey-prf", {"rp_id": "openagents.com", "credential_id": b64(credential), "prf_salt": b64(PRF_INPUT)},
                    prf_secret, "iPhone (test)", n_prf, vmk, T0)
    slot_rec = slot(vault, s_rec, "recovery", {"log_n": LOG_N, "salt": b64(salt)}, rec_secret, "Recovery code", n_rec, vmk, T0)
    rec1 = record(vault, 1, [s_prf, s_rec], SEED, T0)
    object1 = vault_object(vault, obj, rec1, vmk, dek, prefix, n_wrap, T0)
    assert slot_open(slot_prf, prf_secret) == vmk and slot_open(slot_rec, rec_secret) == vmk
    assert object_open(object1, vmk) == rec1
    # Epoch 2: a Nostr-key slot is added. The VMK stays; the record is sealed again under a new DEK.
    sys.path.insert(0, HERE)
    from nip59_reference import conversation_key, k, nip44_encrypt, xonly
    nsec = k(2)
    npub = xonly(nsec).hex()
    ck = conversation_key(nsec, bytes.fromhex(npub))
    nostr_secret = t("nostr slot secret")
    sealed = nip44_encrypt(ck, t("nip44 nonce"), nostr_secret.hex().encode())
    s_nostr = t("slot nostr").hex()
    slot_nostr = slot(vault, s_nostr, "nostr", {"pubkey": npub, "sealed": sealed}, nostr_secret, "Nostr key (test)", t("nonce nostr")[:12], vmk, T0 + 60)
    fresh2 = t("dek 2") + t("object 2") + t("nonce wrap 2")[:12] + t("prefix 2")[:7]
    rec2 = record(vault, 2, [s_prf, s_rec, s_nostr], SEED, T0 + 60)
    object2 = vault_object(vault, fresh2[32:64].hex(), rec2, vmk, fresh2[0:32], fresh2[76:83], fresh2[64:76], T0 + 60)
    assert object_open(object2, vmk) == rec2
    # A 24-word seed, sealed in another account's vault with the same secrets (what a swapped vault looks like).
    return {
        "v": "openagents.wallet-backup.v1",
        "note": "TEST SECRETS ONLY. Written by coax/tests/nostr/vault_wallet_reference.py, an independent implementation; "
                "coax/lib/nostr/vault-wallet.coax makes and opens every value byte for byte. Binary values are standard base64; "
                "keys, ids and digests are lowercase hex.",
        "account": ACCOUNT, "created_at": T0, "prf_input": PRF_INPUT.hex(), "seed_entropy": SEED.hex(),
        "seed_words": "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
        "fresh": fresh.hex(), "vault": vault, "vmk": vmk.hex(), "dek": dek.hex(),
        "passkey": {"rp_id": "openagents.com", "credential_id": b64(credential), "prf_output": prf_secret.hex(), "label": "iPhone (test)", "slot": slot_prf},
        "recovery": {"words": CODE, "entropy": CODE_ENTROPY.hex(), "salt": b64(salt), "log_n": LOG_N, "secret": rec_secret.hex(),
                     "label": "Recovery code", "slot": slot_rec},
        "scrypt_small": {"password": "pleaseletmein", "salt": b64(salt), "log_n": 4, "secret": small.hex()},
        "epoch_1": {"record": rec1.decode(), "object": b64(object1), "slots": [slot_prf, slot_rec]},
        "nostr": {"secret_key": nsec.hex(), "pubkey": npub, "conversation_key": ck.hex(), "nip44_nonce": t("nip44 nonce").hex(),
                  "secret": nostr_secret.hex(), "nonce": b64(t("nonce nostr")[:12]), "label": "Nostr key (test)", "slot": slot_nostr},
        "epoch_2": {"fresh": fresh2.hex(), "created_at": T0 + 60, "record": rec2.decode(), "object": b64(object2), "slots": [slot_prf, slot_rec, slot_nostr]},
    }


def lit(s):
    return '"' + s.replace("\\", "\\\\").replace('"', '\\"') + '"'


def fixtures(v):
    rows = [
        "// Fixtures for coax/lib/nostr/vault-wallet.coax's checks, written by",
        "// coax/tests/nostr/vault_wallet_reference.py (an independent implementation; the same values",
        "// are in proto/nostr/fixtures/wallet-backup-v1.vectors.json). TEST SECRETS ONLY.",
        "",
    ]
    def row(name, value):
        rows.append(f"WBT_{name} :: {value if isinstance(value, int) else lit(value)};")
    row("ACCOUNT", v["account"]); row("T0", v["created_at"]); row("PRF_INPUT", v["prf_input"]); row("SEED", v["seed_entropy"])
    row("FRESH", v["fresh"]); row("VAULT", v["vault"]); row("VMK", v["vmk"])
    row("CREDENTIAL", v["passkey"]["credential_id"]); row("PRF_SECRET", v["passkey"]["prf_output"]); row("SLOT_PRF", v["passkey"]["slot"])
    row("CODE", v["recovery"]["words"]); row("CODE_SALT", v["recovery"]["salt"]); row("CODE_SECRET", v["recovery"]["secret"])
    row("SLOT_RECOVERY", v["recovery"]["slot"])
    row("SMALL_SECRET", v["scrypt_small"]["secret"])
    row("RECORD_1", v["epoch_1"]["record"]); row("OBJECT_1", v["epoch_1"]["object"])
    row("CK", v["nostr"]["conversation_key"]); row("NPUB", v["nostr"]["pubkey"]); row("NIP44_NONCE", v["nostr"]["nip44_nonce"])
    row("NOSTR_SECRET", v["nostr"]["secret"]); row("NOSTR_NONCE", v["nostr"]["nonce"]); row("SLOT_NOSTR", v["nostr"]["slot"])
    row("FRESH_2", v["epoch_2"]["fresh"]); row("RECORD_2", v["epoch_2"]["record"]); row("OBJECT_2", v["epoch_2"]["object"])
    return "\n".join(rows) + "\n"


if __name__ == "__main__":
    self_check()
    v = vectors()
    text = json.dumps(v, indent=1) + "\n"
    if "--write" in sys.argv:
        open(VECTORS, "w").write(text)
        open(FIXTURES, "w").write(fixtures(v))
    elif "--check" in sys.argv:
        ok = open(VECTORS).read() == text and open(FIXTURES).read() == fixtures(v)
        print("wallet backup vectors: " + ("match" if ok else "DIFFER from the reference"))
        sys.exit(0 if ok else 1)
    else:
        sys.stdout.write(text)
