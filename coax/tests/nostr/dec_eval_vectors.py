#!/usr/bin/env python3
"""Decision and evaluation vectors for chapter 81.

Written from NIP-DEC, NIP-EVAL and the shared contracts with only json and
hashlib. Tooling only, never shipped. Test keys only: the x-only public
keys of the secp256k1 scalars 1, 2, 4 and 7.

  python3 coax/tests/nostr/dec_eval_vectors.py > coax/tests/nostr/dec_eval_vectors.coax

It prints the Coax constants nostr/examples/decisions-evals.coax and its
checks read:
- the decision body and answer, and the Digests (sha256: of the JCS form)
  of the body's state and questions, the decision-call record's two digests;
- four evaluation reports (by key 1 about the suite the reader holds, by
  key 2 about a suite it does not hold, by key 7, and one whose coverage does
  not sum), the suite and partition bytes the reader holds, and each
  artifact's Digest of its exact bytes;
- three admissions by key 4 citing reports by those digests;
- the decision bodies in data/decisions/, kept from the retired Rust
  crate's fixtures (crates/nostr/fixtures/decisions at 0b401561a0^): seven
  it admitted and seven it refused.
"""
import hashlib
import json
import pathlib

HERE = pathlib.Path(__file__).resolve().parent
K1 = "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798"
K2 = "c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5"
K4 = "e493dbf1c10d80f3581e4904930b1404cc6c13900ee0758474fa94abe8c4cd13"
K7 = "5cbdf0646e5db4eaa398f365f2ea7a0e3d419b7e0330e39ce92bddedcac4f9bc"
T0 = 1790000000


def compact(v):
    return json.dumps(v, separators=(",", ":"), ensure_ascii=False)


def jcs(v):
    # JCS for these values: ASCII strings and objects, no numbers.
    return json.dumps(v, separators=(",", ":"), sort_keys=True, ensure_ascii=False)


def digest(text):
    return "sha256:" + hashlib.sha256(text.encode()).hexdigest()


def ref(text, media="application/json", schema=None):
    r = {"digest": digest(text), "size": len(text.encode()), "media_type": media}
    if schema:
        r["schema"] = schema
    return r


out = []


def const(name, text):
    out.append("%s :: %s;" % (name, json.dumps(text, ensure_ascii=False)))


body = {
    "model": "jev-1.13.0",
    "state": "cargo test ran 3 tests: 3 passed, 0 failed.",
    "questions": {
        "passed": {"type": "noul", "instructions": "Did every test pass?"},
        "next": {"type": "choice", "instructions": "What should happen next?",
                 "criteria": {"ship": "Merge the change.", "fix": "Change the code first."}},
        "risk": {"type": "score", "instructions": "How risky is merging now?",
                 "criteria": ["No risk.", "Some risk.", "High risk."]},
    },
}
answer = {
    "model": "jev-1.13.0",
    "answers": {
        "passed": {"type": "noul", "noul": 0.97},
        "next": {"type": "choice", "choice": "ship", "confidence": 0.8,
                 "probabilities": {"ship": 0.8, "fix": 0.2}},
        "risk": {"type": "score", "score": 0.5, "confidence": 0.6,
                 "legend": {"0": "No risk.", "1": "Some risk.", "2": "High risk."},
                 "probabilities": {"0": 0.6, "1": 0.3, "2": 0.1}},
    },
    "usage": None,
}
const("DEV_BODY", compact(body))
const("DEV_ANSWER", compact(answer))
const("DEV_STATE_DIGEST", digest(jcs(body["state"])))
const("DEV_QUESTIONS_DIGEST", digest(jcs(body["questions"])))

CASES = ["c%d" % i for i in range(1, 11)]
suite = compact({"v": "openagents.eval-suite.v1", "requires": [], "id": K1 + ":tests/merge-gate",
                 "purpose": "decision", "cases": CASES})
other_suite = compact({"v": "openagents.eval-suite.v1", "requires": [], "id": K2 + ":tests/merge-gate",
                       "purpose": "decision", "cases": ["d1", "d2"]})
partition = compact({"development": [], "held_out": CASES, "excluded": []})
const("DEV_SUITE", suite)
const("DEV_PARTITION", partition)
const("DEV_SUITE_DIGEST", digest(suite))
const("DEV_OTHER_SUITE_DIGEST", digest(other_suite))
const("DEV_PARTITION_DIGEST", digest(partition))

NOTE = ref("no limitations recorded", "text/plain")
RUNS = ref("[]")


def arm(completed, failed):
    return {"planned": 10, "attempted": 10, "completed": completed, "refused": 0, "failed": failed,
            "cancelled": 0, "unknown": 0, "excluded": 0}


def report(evaluator, the_suite, subject_arm, correct, verdict):
    m = lambda a, v: {"arm": a, "metric": "correct", "value": v, "denominator": 10, "unknown_count": 0,
                      "uncertainty": None, "evidence": []}
    return compact({
        "v": "openagents.eval-report.v1", "requires": [],
        "suite": ref(the_suite, schema="openagents.eval-suite.v1"),
        "partition": ref(partition),
        "subject": {"definition": {"id": K1 + ":merge/judge"}, "lock": None, "configuration": None},
        "baseline": {"definition": {"id": K1 + ":merge/always-ship"}, "lock": None, "configuration": None},
        "evaluator": evaluator, "started_at": T0, "ended_at": T0 + 3600, "runs": RUNS,
        "coverage": {"subject": subject_arm, "baseline": arm(10, 0)},
        "measurements": [m("subject", correct), m("baseline", 0.6), m("comparison", round(correct - 0.6, 6))],
        "verdict": verdict, "limitations": NOTE})


r1 = report(K1, suite, arm(10, 0), 0.9, "pass")
r2 = report(K2, other_suite, arm(10, 0), 1.0, "pass")
r3 = report(K7, suite, arm(10, 0), 0.5, "fail")
broken = arm(9, 0)                      # 9 + 0 + 0 + 0 + 0 is not the 10 attempted
r4 = report(K1, suite, broken, 0.9, "pass")
for name, r in (("R1", r1), ("R2", r2), ("R3", r3), ("R4", r4)):
    const("DEV_" + name, r)
    const("DEV_" + name + "_DIGEST", digest(r))


def admission(reports, validation):
    a = {"v": "openagents.eval-admission.v1", "requires": [],
         "subject": {"id": K1 + ":merge/judge", "artifact": ref("judge definition")},
         "reports": [ref(r) for r in reports],
         "policy": {"id": K4 + ":ops/merge-policy", "artifact": ref("policy")},
         "scope": ref("merge decisions in this repository"), "decision": "admit", "issuer": K4,
         "expires_at": T0 + 30 * 86400}
    if validation:
        a["validation"] = [ref(r1)]
    return compact(a)


const("DEV_ADMIT_ALL", admission([r1, r2, r3], True))
const("DEV_ADMIT_BARE", admission([r1], False))
const("DEV_ADMIT_R1", admission([r1], True))

for kind in ("valid", "invalid"):
    files = sorted((HERE / "data" / "decisions" / kind).glob("*.json"))
    out.append("DEV_%s :: [%d]string.[" % (kind.upper(), len(files)))
    for i, f in enumerate(files):
        out.append("  %s%s" % (json.dumps(f.read_text(), ensure_ascii=False), "," if i + 1 < len(files) else ""))
    out.append("];")
    out.append("DEV_%s_NAMES :: [%d]string.[%s];" % (kind.upper(), len(files), ",".join(json.dumps(f.stem) for f in files)))

print("// Generated by coax/tests/nostr/dec_eval_vectors.py; do not edit.")
print("\n".join(out))
