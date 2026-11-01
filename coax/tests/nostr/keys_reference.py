#!/usr/bin/env python3
"""Independent reference for chapter 104 (NIP-KEYS): BIP-32 public
derivation, tpub serialization, BIP-380 checksums, P2WSH sortedmulti
addresses, the treasury's records and two PSBT v0s. Pure Python integers and
hashlib; shares no code with the Coax library. Test keys only.

  python3 coax/tests/nostr/keys_reference.py       # print the values
  python3 coax/tests/nostr/keys_reference.py fx    # the Coax constants in keys_fixtures.coax
"""
import hashlib, hmac, json, sys

P = 2**256 - 2**32 - 977
N = 0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEBAAEDCE6AF48A03BBFD25E8CD0364141
G = (0x79BE667EF9DCBBAC55A06295CE870B07029BFCDB2DCE28D959F2815B16F81798,
     0x483ADA7726A3C4655DA4FBFC0E1108A8FD17B448A68554199C47D08FFB10D4B8)

def add(a, b):
    if a is None: return b
    if b is None: return a
    if a[0] == b[0] and (a[1] + b[1]) % P == 0: return None
    if a == b: l = 3 * a[0] * a[0] * pow(2 * a[1], P - 2, P) % P
    else: l = (b[1] - a[1]) * pow(b[0] - a[0], P - 2, P) % P
    x = (l * l - a[0] - b[0]) % P
    return (x, (l * (a[0] - x) - a[1]) % P)

def mul(k, p=G):
    r = None
    while k:
        if k & 1: r = add(r, p)
        p = add(p, p); k >>= 1
    return r

def ser(p): return bytes([2 + (p[1] & 1)]) + p[0].to_bytes(32, "big")
def unser(b):
    x = int.from_bytes(b[1:], "big"); y = pow((x**3 + 7) % P, (P + 1) // 4, P)
    if y & 1 != b[0] & 1: y = P - y
    return (x, y)

def h160(b): return hashlib.new("ripemd160", hashlib.sha256(b).digest()).digest()
def sha(b): return hashlib.sha256(b).digest()

B58 = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"
def b58check(b):
    b = b + sha(sha(b))[:4]; n = int.from_bytes(b, "big"); s = ""
    while n: n, r = divmod(n, 58); s = B58[r] + s
    return "1" * (len(b) - len(b.lstrip(b"\0"))) + s
def b58decode(s):
    n = 0
    for c in s: n = n * 58 + B58.index(c)
    b = n.to_bytes(82, "big"); assert sha(sha(b[:78]))[:4] == b[78:]; return b[:78]

def master(seed):
    i = hmac.new(b"Bitcoin seed", seed, hashlib.sha512).digest()
    return int.from_bytes(i[:32], "big"), i[32:]
def ckd_priv(k, c, idx):
    data = (b"\0" + k.to_bytes(32, "big")) if idx >= 2**31 else ser(mul(k))
    i = hmac.new(c, data + idx.to_bytes(4, "big"), hashlib.sha512).digest()
    return (int.from_bytes(i[:32], "big") + k) % N, i[32:]
def ckd_pub(K, c, idx):
    i = hmac.new(c, ser(K) + idx.to_bytes(4, "big"), hashlib.sha512).digest()
    return add(mul(int.from_bytes(i[:32], "big")), K), i[32:]

TPUB = bytes.fromhex("043587cf")
def account(seed, path):
    k, c = master(seed); fp = h160(ser(mul(k)))[:4]
    parent = None
    for idx in path:
        parent = k; k, c = ckd_priv(k, c, idx)
    pfp = h160(ser(mul(parent)))[:4]
    x = TPUB + bytes([len(path)]) + pfp + path[-1].to_bytes(4, "big") + c + ser(mul(k))
    return fp.hex(), b58check(x)

CHARSET_IN = "0123456789()[],'/*abcdefgh@:$%{}IJKLMNOPQRSTUVWXYZ&+-.;<=>?!^_|~ijklmnopqrstuvwxyzABCDEFGH`#\"\\ "
CHK = "qpzry9x8gf2tvdw0s3jn54khce6mua7l"
def polymod(c, val):
    c0 = c >> 35; c = ((c & 0x7ffffffff) << 5) ^ val
    for i, g in enumerate([0xf5dee51989, 0xa9fdca3312, 0x1bab10e32d, 0x3706b1677a, 0x644d626ffd]):
        if c0 >> i & 1: c ^= g
    return c
def checksum(s):
    c = 1; cls = 0; cc = 0
    for ch in s:
        pos = CHARSET_IN.find(ch); c = polymod(c, pos & 31); cls = cls * 3 + (pos >> 5); cc += 1
        if cc == 3: c = polymod(c, cls); cls = 0; cc = 0
    if cc > 0: c = polymod(c, cls)
    for _ in range(8): c = polymod(c, 0)
    c ^= 1
    return "".join(CHK[(c >> (5 * (7 - j))) & 31] for j in range(8))

def bech32_polymod(v):
    g = [0x3b6a57b2, 0x26508e6d, 0x1ea119fa, 0x3d4233dd, 0x2a1462b3]; c = 1
    for x in v:
        b = c >> 25; c = (c & 0x1ffffff) << 5 ^ x
        for i in range(5):
            if b >> i & 1: c ^= g[i]
    return c
def segwit_addr(hrp, ver, prog):
    acc = bits = 0; d = [ver]
    for b in prog:
        acc = acc << 8 | b; bits += 8
        while bits >= 5: bits -= 5; d.append(acc >> bits & 31)
    if bits: d.append(acc << (5 - bits) & 31)
    e = [ord(x) >> 5 for x in hrp] + [0] + [ord(x) & 31 for x in hrp]
    pm = bech32_polymod(e + d + [0] * 6) ^ 1
    return hrp + "1" + "".join(CHK[x] for x in d + [pm >> 5 * (5 - i) & 31 for i in range(6)])

def wsh_script(k, keys):
    ws = bytes([0x50 + k]) + b"".join(b"\x21" + x for x in sorted(keys)) + bytes([0x50 + len(keys), 0xae])
    return b"\0\x20" + sha(ws)

def derive(xpub, branch, index):
    b = b58decode(xpub); K = unser(b[45:78]); c = b[13:45]
    K, c = ckd_pub(K, c, branch); K, c = ckd_pub(K, c, index); return ser(K)

def ci(n): return bytes([n]) if n < 0xfd else b"\xfd" + n.to_bytes(2, "little")
def kv(key, val): return ci(len(key)) + key + ci(len(val)) + val

def body(d): return json.dumps(d, separators=(",", ":"), sort_keys=True)
def ref(content, schema):
    return {"digest": "sha256:" + sha(content.encode()).hex(), "size": len(content), "media_type": "application/json", "schema": schema}
def xonly(k): return mul(k)[0].to_bytes(32, "big").hex()

WALLET_ID = "77" * 32
def fixtures():
    H = 2**31
    path = [48 + H, 1 + H, 0 + H, 2 + H]
    names = [("alice", "alice-agent", ["US"]), ("keyco", "keyco-llc", ["CH"]), ("enclave", "enclave-co", ["IS"])]
    lines = []
    def put(name, s): lines.append(f"{name} :: {json.dumps(s)};")
    holder = xonly(1)
    put("KY_HOLDER", holder)
    put("KY_WALLET_ID", WALLET_ID)
    enr, enr_refs = [], []
    for i, (n, op, jur) in enumerate(names):
        agent = xonly(i + 1)
        fp, tpub = account(sha(("chapter 104 " + n).encode()), path)
        ind = body({"v": "openagents.keys-independence.v1", "requires": [], "agent": agent, "operator": op,
                    "jurisdiction": jur, "custody": {"vendor": "software", "host": n + "-host", "region": jur[0].lower()},
                    "affiliations": [], "insurance": None, "observed_at": 1793000000})
        cls = "attested" if n == "enclave" else "software"
        e = body({"v": "openagents.keys-enrollment.v1", "requires": [], "agent": agent, "wallet": WALLET_ID,
                  "xpub": f"[{fp}/48h/1h/0h/2h]{tpub}", "possession": "AkcwRAIgAAAA", "class": cls,
                  "attestation": ({"digest": "sha256:" + "ab" * 32, "size": 100, "media_type": "application/json", "schema": "openagents.att-endpoint.v1"} if cls == "attested" else None),
                  "signing_policy": {"digest": "sha256:" + "cd" * 32, "size": 200, "media_type": "application/json", "schema": "openagents.pol-policy.v1"},
                  "independence": ref(ind, "openagents.keys-independence.v1"), "expires_at": 1795000000})
        put(f"KY_AGENT{i}", agent); put(f"KY_IND{i}", ind); put(f"KY_ENR{i}", e)
        enr.append((fp, tpub)); enr_refs.append(ref(e, "openagents.keys-enrollment.v1"))
    keys = ",".join(f"[{fp}/48h/1h/0h/2h]{t}/<0;1>/*" for fp, t in enr)
    desc = f"wsh(sortedmulti(2,{keys}))"; desc += "#" + checksum(desc)
    w = body({"v": "openagents.keys-wallet.v1", "requires": [], "wallet": WALLET_ID, "holder": holder, "revision": 0,
              "previous": None, "network": "signet", "descriptor": desc,
              "paths": [{"name": "primary", "threshold": 2, "keys": enr_refs, "timelock": None}],
              "enrollments": enr_refs, "recipients": [holder, xonly(2), xonly(3)],
              "starts_at": 1793100000, "expires_at": 1824636000})
    put("KY_WALLET", w)
    lines.append(f"KY_WALLET_DIGEST :: {json.dumps(sha(w.encode()).hex())};")
    k2 = [f"[{fp}/48h/1h/0h/2h]{t}/<0;1>/*" for fp, t in enr]
    d2 = f"wsh(or_d(multi(2,{k2[1]},{k2[2]}),and_v(v:older(4320),multi(1,{k2[0]}))))"
    put("KY_DESC2", d2 + "#" + checksum(d2))
    psbt = make_psbt()
    put("KY_PSBT", psbt.hex())
    req = body({"v": "openagents.keys-sign-request.v1", "requires": [], "wallet": ref(w, "openagents.keys-wallet.v1"),
                "requester": holder,
                "psbt": {"digest": "sha256:" + sha(psbt).hex(), "size": len(psbt), "media_type": "application/octet-stream"},
                "outputs": [{"address": "tb1qqypqxpq9qcrsszg2pvxq6rs0zqg3yyc5r7fxez", "amount_sat": 70000, "change": False},
                            {"address": segwit_addr("tb", 0, SPK[(1, 0)][2:]), "amount_sat": 29000, "change": True}],
                "fee_sat": 1000, "purpose": "payment", "action": None, "treasury": None,
                "nonce": "99" * 32, "deadline": 1793200000})
    put("KY_REQUEST", req)
    evil = make_psbt(b"\0\x14" + bytes(range(21, 41)))
    put("KY_PSBT_EVIL", evil.hex())
    put("KY_EVIL_ADDRESS", segwit_addr("tb", 0, bytes(range(21, 41))))
    r2 = json.loads(req); r2["psbt"]["digest"] = "sha256:" + sha(evil).hex(); r2["nonce"] = "aa" * 32
    put("KY_REQUEST_EVIL", body(r2))
    return lines

SPK = {}
def make_psbt(pay=b"\0\x14" + bytes(range(1, 21))):
    H = 2**31
    path = [48 + H, 1 + H, 0 + H, 2 + H]
    tp = [account(sha(("chapter 104 " + n).encode()), path)[1] for n in ("alice", "keyco", "enclave")]
    for br in (0, 1):
        for i in range(3):
            SPK[(br, i)] = wsh_script(2, [derive(t, br, i) for t in tp])
    tx = (2).to_bytes(4, "little") + ci(2)
    for j in range(2):
        tx += bytes([0x11 * (j + 1)]) * 32 + j.to_bytes(4, "little") + b"\0" + b"\xfd\xff\xff\xff"
    outs = [(70000, pay), (29000, SPK[(1, 0)])]
    tx += ci(2) + b"".join(v.to_bytes(8, "little") + ci(len(s)) + s for v, s in outs) + (0).to_bytes(4, "little")
    psbt = b"psbt\xff" + kv(b"\0", tx) + b"\0"
    for v, s in ((60000, SPK[(0, 0)]), (40000, SPK[(0, 1)])):
        psbt += kv(b"\x01", v.to_bytes(8, "little") + ci(len(s)) + s) + b"\0"
    return psbt + b"\0\0"

if __name__ == "__main__" and len(sys.argv) > 1:
    print("\n".join(fixtures()))

if __name__ == "__main__" and len(sys.argv) == 1:
    H = 2**31
    path = [48 + H, 1 + H, 0 + H, 2 + H]
    agents = []
    for name in ("alice", "keyco", "enclave"):
        fp, tpub = account(sha(("chapter 104 " + name).encode()), path)
        agents.append((name, fp, tpub))
        print(name, fp, tpub)
    keys = ",".join(f"[{fp}/48h/1h/0h/2h]{t}/<0;1>/*" for _, fp, t in agents)
    desc = f"wsh(sortedmulti(2,{keys}))"
    print("descriptor", desc + "#" + checksum(desc))
    print("raw(deadbeef)", checksum("raw(deadbeef)"))
    spk = {}
    for br in (0, 1):
        for i in range(3):
            s = wsh_script(2, [derive(t, br, i) for _, _, t in agents])
            spk[(br, i)] = s
            print(br, i, segwit_addr("tb", 0, s[2:]), s.hex())
    # BIP-32 vector 1: m/0H public -> m/0H/1
    v = "xpub68Gmy5EdvgibQVfPdqkBBCHxA5htiqg55crXYuXoQRKfDBFA1WEjWgP6LHhwBZeNK1VTsfTFUHCdrfp1bgwQ9xv5ski8PX9rL2dZXvgGDnw"
    b = b58decode(v); K, c = ckd_pub(unser(b[45:78]), b[13:45], 1)
    print("bip32 m/0H/1", ser(K).hex())
    # A PSBT v0: two inputs at receive 0 (60000) and receive 1 (40000), pay 70000
    # to a P2WPKH, 29000 change to change 0, fee 1000.
    pay = b"\0\x14" + bytes(range(1, 21))
    tx = (2).to_bytes(4, "little") + ci(2)
    for j in range(2):
        tx += bytes([0x11 * (j + 1)]) * 32 + j.to_bytes(4, "little") + b"\0" + b"\xfd\xff\xff\xff"
    outs = [(70000, pay), (29000, spk[(1, 0)])]
    tx += ci(2) + b"".join(v.to_bytes(8, "little") + ci(len(s)) + s for v, s in outs) + (0).to_bytes(4, "little")
    psbt = b"psbt\xff" + kv(b"\0", tx) + b"\0"
    for v, s in ((60000, spk[(0, 0)]), (40000, spk[(0, 1)])):
        psbt += kv(b"\x01", v.to_bytes(8, "little") + ci(len(s)) + s) + b"\0"
    psbt += b"\0\0"
    print("pay address", segwit_addr("tb", 0, pay[2:]))
    print("psbt", psbt.hex())
    print("psbt digest", sha(psbt).hex())
