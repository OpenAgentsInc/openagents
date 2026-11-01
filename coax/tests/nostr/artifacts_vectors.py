#!/usr/bin/env python3
"""Artifact and reminder events for chapter 90 (NIP-AR, NIP-ER).

Written from NIP-01, NIP-44 and Block's NIP-AR and NIP-ER with only hashlib
and integers (signing and NIP-44 come from nip59_reference.py, which shares
no code with Coax). Tooling only, never shipped. TEST keys only: Alice is the
secp256k1 scalar 1, Bob 2.

  python3 coax/tests/nostr/artifacts_vectors.py > coax/tests/nostr/artifacts_vectors.coax

Besides the signed events it prints two answers computed here
independently: the artifact's current revision after the relay applies the
submissions in order under NIP-AR's prev rule, and which reminders are due
at two fixed clocks under NIP-ER's head and not_before rules.
"""
import hashlib
import json
import pathlib
import re
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from nip59_reference import conversation_key, dumps, k, nip44_decrypt, nip44_encrypt, signed, xonly  # noqa: E402

ALICE, BOB = k(1), k(2)
alice, bob = xonly(ALICE).hex(), xonly(BOB).hex()

# ---- NIP-AR: one task through create, two competing updates, move, delete,
# restore, and a second create of the same d.
T0 = 1790000000
D = "04737c81-e5e8-4412-bb47-f446813cfeba"
H1 = "9b353519-f4fe-4757-aef4-bec6cc0ae54c"
H2 = "17cfe3ca-b3d0-4a76-a2a4-4eaf0d3a4109"


def art(secret, aux, at, h, op, prev, title, content, extra=()):
    tags = [["ar", "1"], ["d", D], ["h", h], ["type", "buzz.task"]]
    if title is not None:
        tags.append(["title", title])
    tags.append(["op", op])
    if prev:
        tags.append(["prev", prev])
    tags += list(extra)
    return json.loads(signed(secret, bytes([aux]) * 32, at, 45010, tags, content))


create = art(ALICE, 0x61, T0, H1, "create", None, "Fix the payment timeout",
             "{\"version\":1,\"status\":\"open\"}", [["assignee", bob]])
upd_a = art(ALICE, 0x62, T0 + 60, H1, "update", create["id"], "Fix the payment timeout",
            "{\"version\":1,\"status\":\"in_progress\"}", [["assignee", bob]])
upd_b = art(BOB, 0x63, T0 + 61, H1, "update", create["id"], "Fix the payment timeout",
            "{\"version\":1,\"status\":\"blocked\"}", [["assignee", bob]])
move = art(ALICE, 0x64, T0 + 120, H2, "move", upd_a["id"], "Fix the payment timeout",
           "{\"version\":1,\"status\":\"in_progress\"}", [["assignee", bob]])
delete = art(ALICE, 0x65, T0 + 180, H2, "delete", move["id"], None, "")
restore = art(ALICE, 0x66, T0 + 240, H2, "restore", delete["id"], "Fix the payment timeout",
              "{\"version\":1,\"status\":\"open\"}", [["assignee", bob]])
again = art(BOB, 0x67, T0 + 300, H1, "create", None, "Another task", "{}")
submissions = [("CREATE", create), ("UPDATE_ALICE", upd_a), ("UPDATE_BOB", upd_b), ("MOVE", move),
               ("DELETE", delete), ("RESTORE", restore), ("UPDATE_ALICE_AGAIN", upd_a), ("CREATE_AGAIN", again)]


def tag(e, name):
    for t in e["tags"]:
        if t[0] == name:
            return t[1]
    return None


# The prev rule, applied in order: an accepted id again is success without
# change; create needs an unused d; every other op names the current id.
current, accepted, outcome = None, set(), []
for name, e in submissions:
    op = tag(e, "op")
    if e["id"] in accepted:
        outcome.append("again")
    elif op == "create":
        outcome.append("taken" if current else "ok")
        if not current:
            current = e["id"]
            accepted.add(e["id"])
    elif tag(e, "prev") != current:
        outcome.append("conflict")
    else:
        current = e["id"]
        accepted.add(e["id"])
        outcome.append("ok")

# ---- NIP-ER: three reminders, Alice's, encrypted to herself.
NOW = 1770000000
LATER = 1770086400
ck = conversation_key(ALICE, xonly(ALICE))


def reminder(aux, at, d, nb, plain, expiration=None, nb_text=None):
    tags = [["d", d]]
    if nb is not None or nb_text is not None:
        tags.append(["not_before", nb_text if nb_text is not None else str(nb)])
    tags.append(["alt", "Encrypted reminder"])
    if expiration is not None:
        tags.append(["expiration", str(expiration)])
    content = nip44_encrypt(ck, bytes([aux]) * 32, dumps(plain).encode())
    assert json.loads(nip44_decrypt(ck, content)) == plain
    return json.loads(signed(ALICE, bytes([aux]) * 32, at, 30300, tags, content))


DA, DB, DC = "a3f8c2e1b4d79600e5d2f1a8c3b6094d", "5c1e0f3a9b7d42e6a1c8f0d3b2e49a7c", "e7b2d4c19a3f5e8061c7b9a2d4f3e1b0"
target = {"a": "30023:" + alice + ":proposal",
          "id": "7b4f3c2a1e9d8c7061524334aabbccddeeff00112233445566778899aabbccdd",
          "relays": ["wss://relay.example"], "preview": "Can you review this before Friday?"}
reminders = [
    ("A", reminder(0x71, 1769990000, DA, NOW, {"status": "pending", "note": "Submit travel receipt"})),
    ("B1", reminder(0x72, 1769990100, DB, 1769999990, {"target": target, "status": "pending", "note": "Follow up before planning"})),
    ("B2", reminder(0x73, 1769999995, DB, LATER, {"target": target, "status": "pending", "note": "Follow up before planning"})),
    ("C1", reminder(0x74, 1769990200, DC, 1769999000, {"status": "pending", "note": "Call the bank"})),
    ("C2", reminder(0x75, 1769999100, DC, None, {"status": "cancelled", "note": "Call the bank"}, 1777542730)),
]
bad_nb = reminder(0x76, 1769990300, "0f1e2d3c4b5a69788796a5b4c3d2e1f0", None,
                  {"status": "pending", "note": "x"}, nb_text="01770000000")


def due(clock):
    heads = {}
    for name, e in reminders:
        d = tag(e, "d")
        h = heads.get(d)
        if h is None or (e["created_at"], [-ord(c) for c in e["id"]]) > (h[1]["created_at"], [-ord(c) for c in h[1]["id"]]):
            heads[d] = (name, e)
    out = []
    for name, e in heads.values():
        status = json.loads(nip44_decrypt(ck, e["content"]))["status"]
        nb = tag(e, "not_before")
        if status == "pending" and nb is not None and int(nb) <= clock:
            out.append(name)
    return ",".join(sorted(out))


def coax(name, s):
    print("%s :: \"%s\";" % (name, s.replace("\\", "\\\\").replace("\"", "\\\"").replace("\n", "\\n")))


print("// Generated by coax/tests/nostr/artifacts_vectors.py: artifact and reminder")
print("// events signed by an independent BIP-340 and NIP-44, and the answers")
print("// computed in Python. TEST keys: Alice is scalar 1, Bob 2.")
coax("ARV_ALICE", alice)
coax("ARV_BOB", bob)
for name, e in submissions[:6] + submissions[7:]:
    coax("ARV_" + name, dumps(e))
    coax("ARV_%s_ID" % name, e["id"])
coax("ARV_OUTCOMES", ",".join(outcome))
coax("ARV_CURRENT", current)
for name, e in reminders:
    coax("ERV_" + name, dumps(e))
    coax("ERV_%s_ID" % name, e["id"])
coax("ERV_BAD_NB", dumps(bad_nb))
coax("ERV_DUE_NOW", due(NOW))
coax("ERV_DUE_LATER", due(LATER))

# ---- NIP-IA: the five published test vectors, copied from the pinned text
# as full events, with their published ids checked here with hashlib.
nip = (pathlib.Path(__file__).resolve().parents[3] / "nostr/nips/block/NIP-IA.md").read_text()
pat = re.compile(r"kind         = (\d+)\npubkey       = (\w+)\ncreated_at   = (\d+)\ncontent      = (\".*?\")\n"
                 r"tags         = (.*?)\nid_preimage  = (.*?)\nid           = (\w+)\nsig          = (\w+)")
for n, m in enumerate(pat.finditer(nip), 1):
    kind, pub, at, content, tags, pre, eid, sig = m.groups()
    ev = {"id": eid, "pubkey": pub, "created_at": int(at), "kind": int(kind),
          "tags": json.loads(tags), "content": json.loads(content), "sig": sig}
    mine = hashlib.sha256(dumps([0, pub, int(at), int(kind), ev["tags"], ev["content"]]).encode()).hexdigest()
    assert mine == eid == hashlib.sha256(pre.encode()).hexdigest()
    coax("IAV_%d" % n, dumps(ev))
    coax("IAV_%d_ID" % n, eid)
    coax("IAV_%d_TAGS" % n, tags)

# ---- NIP-OA auth signatures for the NIP-IA owner path's refusals: the owner
# (key 1) signs, over sha256("nostr:agent-auth:" + agent + ":" + conditions),
# a credential whose time window closed, one whose conditions don't read,
# and one for a different agent (key 3).
from nip59_reference import sign  # noqa: E402


def auth_sig(agent_hex, conditions, aux):
    pre = ("nostr:agent-auth:%s:%s" % (agent_hex, conditions)).encode()
    return sign(ALICE, hashlib.sha256(pre).digest(), bytes([aux]) * 32).hex()


coax("IAV_SIG_EXPIRED", auth_sig(bob, "created_at<1713956000", 0x81))
coax("IAV_SIG_BADCOND", auth_sig(bob, "kind=1&&", 0x82))
coax("IAV_SIG_OTHER_AGENT", auth_sig(xonly(k(3)).hex(), "kind=1&created_at<1713957000", 0x83))
