#!/usr/bin/env python3
"""Chapter 178's independent reference, written from the chapter's specification.

It shares no code with psionic/lib/suite.coax: digests come from Python's
hashlib over the canonical encoding, and the suite is read from chapter 12's
held-out file rather than from the example's copy of its text. `--header`
writes the C expectations the native harness compares; `--tests` prints the
digests the Coax checks use.
"""
import hashlib
import json
import os
import struct
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
KINDS = {"choice": 0, "noul": 1, "score": 2}
LOCKED = 1
MASK = 1 << LOCKED
GATE = dict(kind=0, clear=40, min_tasks=10, replicates=1000, seed=7, cost_limit=1.0, seconds_limit=30.0)
REWORDED = (7, "Is the shop open at 8 pm?")
REPLACED = (7, "The shop opens at 9:00 and closes at 17:00 on weekdays.")
ELEPHANT, FIRST = 1, 2


def u64(x):
    return struct.pack("<Q", x & 0xFFFFFFFFFFFFFFFF)


def text(s):
    b = s.encode("utf-8")
    return u64(len(b)) + b


def bits(x):
    return u64(struct.unpack("<Q", struct.pack("<d", x))[0])


def sha(b):
    return hashlib.sha256(b).digest()


def items_digest(items, mask):
    out = text("psionic.suite.items.v1") + u64(mask) + u64(len(items))
    for it in items:
        out += text(it["id"]) + bytes([it["kind"]]) + u64(it["task"]) + u64(it["partition"]) + text(it["state"])
        out += u64(len(it["options"]))
        for o in it["options"]:
            out += bytes([o.get("name") is not None]) + text(o.get("name") or "")
            out += bytes([o.get("description") is not None]) + text(o.get("description") or "")
        out += u64(it["gold"]) + u64(it["abstain"] + 1)
    return sha(out)


def questions_digest(questions):
    return sha(text("psionic.suite.questions.v1") + u64(len(questions)) + b"".join(text(q) for q in questions))


def gate_digest(g):
    return sha(text("psionic.suite.gate.v1") + bytes([g["kind"]]) + u64(g["clear"]) + u64(g["min_tasks"])
               + u64(g["replicates"]) + u64(g["seed"]) + bits(g["cost_limit"]) + bits(g["seconds_limit"]))


def values_digest(values):
    return sha(text("psionic.suite.values.v1") + u64(len(values)) + b"".join(bits(v) for v in values))


def receipt_digest(r):
    s = r["suite"]
    return sha(text("psionic.suite.receipt.v1") + u64(r["sequence"]) + r["previous"] + s["items"] + s["questions"]
               + s["gate"] + u64(s["mask"]) + u64(r["partition"]) + u64(r["system"]) + u64(r["read"])
               + bits(r["score"]) + r["values"])


def suite(items, questions, gate, mask):
    return dict(items=items_digest(items, mask), questions=questions_digest(questions), gate=gate_digest(gate), mask=mask)


def load():
    data = json.load(open(os.path.join(HERE, "..", "fixtures", "quality", "heldout-v1.json")))
    items, questions = [], []
    last = {}
    for i, it in enumerate(data["items"]):
        last[it["kind"]] = i
    for i, it in enumerate(data["items"]):
        options = it.get("options", [])
        abstain = it.get("abstain")
        items.append(dict(id=it["id"], kind=KINDS[it["kind"]], task=i, partition=LOCKED if last[it["kind"]] == i else 0,
                          state=it["state"], options=options, gold=it["gold"], abstain=-1 if abstain is None else abstain))
        questions.append(it["question"])
    return items, questions


def experiment():
    items, questions = load()
    base = suite(items, questions, GATE, MASK)
    reworded_q = list(questions)
    reworded_q[REWORDED[0]] = REWORDED[1]
    reworded = suite(items, reworded_q, GATE, MASK)
    replaced_items = [dict(it) for it in items]
    replaced_items[REPLACED[0]]["state"] = REPLACED[1]
    replaced = suite(replaced_items, questions, GATE, MASK)
    other_gate = suite(items, questions, dict(GATE, replicates=2000), MASK)
    store = []

    def append(s, partition, system):
        picked = [it for it in items if it["partition"] == partition]
        values = [1.0 if system == ELEPHANT else float(it["gold"] == 0) for it in picked]
        r = dict(sequence=len(store), previous=store[-1]["digest"] if store else bytes(32), suite=s, partition=partition,
                 system=system, read=len(values), score=sum(values) / len(values), values=values_digest(values))
        r["digest"] = receipt_digest(r)
        store.append(r)
    append(base, 0, ELEPHANT)
    append(base, LOCKED, ELEPHANT)
    append(base, 0, FIRST)
    append(reworded, 0, FIRST)
    append(replaced, 0, FIRST)
    append(other_gate, 0, FIRST)
    return dict(base=base, reworded=reworded, replaced=replaced, other_gate=other_gate, store=store)


def tests():
    """Digests of the small suites in the Coax checks."""
    items = [dict(id="a", kind=1, task=0, partition=0, state="s", options=[], gold=1, abstain=-1),
             dict(id="bc", kind=0, task=3, partition=2, state="",
                  options=[dict(name="x"), dict(name="y", description="d"), dict(description="")], gold=2, abstain=1)]
    out = dict(items=items_digest(items, 4), questions=questions_digest(["q?", ""]),
               gate=gate_digest(dict(kind=1, clear=40, min_tasks=10, replicates=200, seed=5, cost_limit=1.5, seconds_limit=0.25)),
               values=values_digest([1.0, 0.0, 0.5]))
    small = dict(items=out["items"], questions=out["questions"], gate=out["gate"], mask=4)
    out["receipt"] = receipt_digest(dict(sequence=1, previous=bytes(range(32)), suite=small, partition=2, system=7, read=3,
                                         score=0.5, values=out["values"]))
    for k, v in out.items():
        print(k, v.hex())


def hexes(d):
    return "{" + ",".join(str(b) for b in d) + "}"


def cstr(x):
    return "0" if x is None else json.dumps(x)


def header(r):
    lines = ["/* Generated by psionic/tests/suite_oracle.py --header. Do not edit. */", "#include <stdint.h>"]
    items, questions = load()
    # The suite itself, from chapter 12's file: NULL marks an absent name or description.
    lines.append(f"static const char *const su_oracle_ids[12] = {{{','.join(cstr(i['id']) for i in items)}}};")
    lines.append(f"static const char *const su_oracle_states[12] = {{{','.join(cstr(i['state']) for i in items)}}};")
    lines.append(f"static const char *const su_oracle_questions[12] = {{{','.join(cstr(q) for q in questions)}}};")
    for field in ("kind", "task", "partition", "gold", "abstain"):
        lines.append(f"static const int64_t su_oracle_{field}[12] = {{{','.join(str(i[field]) for i in items)}}};")
    lines.append(f"static const int64_t su_oracle_options[12] = {{{','.join(str(len(i['options'])) for i in items)}}};")
    for field in ("name", "description"):
        rows = ["{" + ",".join(cstr(o.get(field)) for o in i["options"]) + "}" if i["options"] else "{0}" for i in items]
        lines.append(f"static const char *const su_oracle_option_{field}[12][8] = {{{','.join(rows)}}};")
    lines.append(f"static const char *const su_oracle_reworded_text = {cstr(REWORDED[1])};")
    lines.append(f"static const char *const su_oracle_replaced_text = {cstr(REPLACED[1])};")
    for name in ("base", "reworded", "replaced", "other_gate"):
        s = r[name]
        lines.append(f"static const uint8_t su_oracle_{name}[3][32] = {{{hexes(s['items'])},{hexes(s['questions'])},{hexes(s['gate'])}}};")
    lines.append(f"static const uint8_t su_oracle_receipts[{len(r['store'])}][32] = {{{','.join(hexes(x['digest']) for x in r['store'])}}};")
    lines.append(f"static const double su_oracle_scores[{len(r['store'])}] = {{{','.join(repr(x['score']) for x in r['store'])}}};")
    print("\n".join(lines))


if __name__ == "__main__":
    if "--tests" in sys.argv:
        tests()
    else:
        result = experiment()
        if "--header" in sys.argv:
            header(result)
        else:
            for name in ("base", "reworded", "replaced", "other_gate"):
                print(name, {k: (v.hex()[:16] if isinstance(v, bytes) else v) for k, v in result[name].items()})
            for x in result["store"]:
                print(x["sequence"], x["partition"], x["system"], x["read"], x["score"], x["digest"].hex()[:16])
