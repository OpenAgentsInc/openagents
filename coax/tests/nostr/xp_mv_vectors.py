#!/usr/bin/env python3
"""World and experience vectors for chapter 85.

Written from NIP-MV, NIP-XP and NIP-01 with only json and hashlib. Tooling
only, never shipped. Test keys only: the x-only public keys of the
secp256k1 scalars 1, 2, 4 and 7. Signatures are left as zeros: the
chapter's readers take events whose signatures a relay client already
checked (chapter 5), and these vectors check ids, not signatures.

  python3 coax/tests/nostr/xp_mv_vectors.py > coax/tests/nostr/xp_mv_vectors.coax

For each event it prints <NAME>_KIND, _AT, _PK, _TAGS, _BODY (the content),
_ID (the NIP-01 id: sha256 of [0,pubkey,created_at,kind,tags,content]
serialized compactly) and _JSON (the whole event, for the id check).
"""
import hashlib
import json

K1 = "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798"
K2 = "c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5"
K4 = "e493dbf1c10d80f3581e4904930b1404cc6c13900ee0758474fa94abe8c4cd13"
K7 = "5cbdf0646e5db4eaa398f365f2ea7a0e3d419b7e0330e39ce92bddedcac4f9bc"
T0 = 1790000000
PLAZA = "33300:%s:plaza" % K1
ADDRESS = "tb4.fix-git.beat-fable-low@1"
COORD = "30193:%s:%s" % (K1, ADDRESS)
ENTRY = hashlib.sha256(b"entry").hexdigest()      # stand-ins for the 3190 entry and 3189 evidence
EVIDENCE = hashlib.sha256(b"evidence").hexdigest()
DOC = hashlib.sha256(b"document").hexdigest()


def compact(v):
    return json.dumps(v, separators=(",", ":"), ensure_ascii=False)


def event_id(pk, at, kind, tags, content):
    return hashlib.sha256(compact([0, pk, at, kind, tags, content]).encode()).hexdigest()


def coax(s):
    return '"' + s.replace("\\", "\\\\").replace('"', '\\"') + '"'


out = []


def emit(name, pk, at, kind, tags, body):
    content = body if isinstance(body, str) else compact(body)
    i = event_id(pk, at, kind, tags, content)
    whole = compact({"id": i, "pubkey": pk, "created_at": at, "kind": kind,
                     "tags": tags, "content": content, "sig": "0" * 128})
    out.append("%s_KIND :: %d; %s_AT :: %d;" % (name, kind, name, at))
    out.append("%s_PK :: %s;" % (name, coax(pk)))
    out.append("%s_TAGS :: %s;" % (name, coax(compact(tags))))
    out.append("%s_BODY :: %s;" % (name, coax(content)))
    out.append("%s_ID :: %s;" % (name, coax(i)))
    out.append("%s_JSON :: %s;" % (name, coax(whole)))
    return i


# NIP-MV: the plaza (the NIP's own content), then frames from key 2.
emit("WORLD", K1, T0, 33300, [["d", "plaza"], ["w", PLAZA], ["name", "The Plaza"]],
     '{"v":1,"cell":64,"bounds":{"min":[-264,0,-264],"max":[264,200,264]},"spawn":{"center":[0,0,0],"radius":28}}')


def frame(n, t, p, cell):
    return compact({"v": 1, "s": "a41c", "n": n, "t": t,
                    "e": [{"id": "avatar", "role": "avatar", "p": p, "q": [0, 0.38, 0, 0.92]}]}).replace(".0,", ",")


emit("F1", K2, T0 + 1, 23300, [["w", PLAZA], ["c", "0,-1"]], frame(812, 1790000001000, [3.2, 0, -10.5], "0,-1"))
emit("F2", K2, T0 + 1, 23300, [["w", PLAZA], ["c", "0,-1"]], frame(812, 1790000001000, [3.2, 0, -10.5], "0,-1"))
emit("F3", K2, T0 + 2, 23300, [["w", PLAZA], ["c", "4,-1"]], frame(813, 1790000001100, [300, 0, -10.5], "4,-1"))
emit("F4", K2, T0 + 2, 23300, [["w", PLAZA], ["c", "-1,-1"]], frame(814, 1790000001200, [-0.5, 0, -10.5], "-1,-1"))
emit("STATE", K2, T0 + 30, 33301, [["d", PLAZA + "/avatar"], ["w", PLAZA], ["c", "-1,-1"], ["role", "avatar"]],
     '{"v":1,"id":"avatar","role":"avatar","p":[-0.5,0,-10.5],"q":[0,0.38,0,0.92],"t":1790000030000,"online":false}')
out.append("LAMP_NODE :: %s;" % coax("plaza/lamp-0"))
out.append("LAMP_ID :: %s;" % coax("obj-" + hashlib.sha256(b"plaza/lamp-0").hexdigest()[:20]))

# NIP-XP: one quest, an award, its revocation, the replacement, and a
# revocation signed by the wrong key.
quest = emit("QUEST", K1, T0 + 100, 30193,
             [["d", ADDRESS], ["t", "oa:xp:quest:v1"], ["t", "oa:xp:season:2026-q4"], ["t", "oa:xp:rule:kb-transfer"]],
             {"v": 1, "requires": [], "type": "quest", "id": "tb4.fix-git.beat-fable-low", "version": 1,
              "season": {"id": "2026-q4", "opens_at": T0, "closes_at": 1798000000},
              "title": "Beat Fable 5.1 low's cheapest winning run on fix-git",
              "objective": "Publish an entry that makes a paired Microcoder run pass fix-git for less than Fable 5.1 low's cheapest winning run, on a task the entry wasn't written from.",
              "acceptance": {"rule": "kb-transfer", "task": "fix-git", "min_pass_rate": 1, "max_usd_per_run": 0.21},
              "reference": None, "award": {"author": 6, "runner": 4}, "completions": "first"})


def award(author, runner, at):
    return {"v": 1, "requires": [], "type": "award",
            "quest": {"id": quest, "pubkey": K1, "kind": 30193, "coordinate": COORD},
            "key": COORD, "accepted_at": at,
            "entry": {"id": ENTRY, "pubkey": author, "kind": 3190},
            "entry_version": {"id": "git.reflog-recovery", "version": 2, "digest": DOC},
            "evidence": [{"id": EVIDENCE, "pubkey": runner, "kind": 3189}],
            "awardees": [{"role": "author", "pubkey": author, "xp": 6}, {"role": "runner", "pubkey": runner, "xp": 4}]}


def award_tags(author, runner):
    return [["t", "oa:xp:award:v1"], ["a", COORD], ["e", quest], ["e", ENTRY], ["e", EVIDENCE], ["p", author], ["p", runner]]


a1 = emit("AWARD1", K1, 1790500000, 3193, award_tags(K2, K4), award(K2, K4, 1790500000))
emit("REVOKE1", K1, 1790600000, 3194, [["t", "oa:xp:revocation:v1"], ["e", a1], ["a", COORD]],
     {"v": 1, "requires": [], "type": "revocation", "award": {"id": a1, "pubkey": K1, "kind": 3193},
      "key": COORD, "reason": "The runs were graded against the wrong verifier version."})
emit("AWARD2", K1, 1790700000, 3193, award_tags(K7, K4), award(K7, K4, 1790700000))
emit("FORGED", K2, 1790650000, 3194, [["t", "oa:xp:revocation:v1"], ["e", a1], ["a", COORD]],
     {"v": 1, "requires": [], "type": "revocation", "award": {"id": a1, "pubkey": K1, "kind": 3193},
      "key": COORD, "reason": "Key 2 would like this award gone."})

print("// Generated by coax/tests/nostr/xp_mv_vectors.py: Python json and hashlib")
print("// only. Test keys only. Do not edit by hand.")
print("\n".join(out))
