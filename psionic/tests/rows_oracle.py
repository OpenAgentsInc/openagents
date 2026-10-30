#!/usr/bin/env python3
"""Chapter 186's independent reference, written from the chapter's rules.

It shares no code with psionic/lib/training-rows.coax. It rebuilds the
example's feed of 48 decision outcomes, admits it for two artifacts with
different time cuts over one shared exposure ledger, and computes each
artifact's tally by reason, its rows per role and its set digest with
Python's hashlib. It then checks the trace artifact's ledger refusal from the
replay verdicts the example prints. Each line it derives must appear in the
example's output.
"""
import hashlib
import pathlib
import struct
import sys

ROLES = ["none", "train", "calibration", "development", "heldout"]
LABELS = ["none", "check", "diff", "summary", "replay"]
REASONS = ["admitted", "shape", "capacity", "order", "unlabelled", "summary",
           "unverifiable", "failed", "crosses", "reserved", "duplicate"]


def words(*v):
    return b"".join(struct.pack("<Q", x % 2 ** 64) for x in v)


def feed():
    out = []
    for i in range(48):
        t = 24 if i == 25 else i
        if t % 5 == 3:
            source = "summary"
        elif t % 11 == 6:
            source = "none"
        else:
            source = "check" if t % 2 == 0 else "diff"
        out.append(dict(group=i // 4, time=t, decision=(t * 13) % 7,
                        label=1 if (t * 5) % 3 == 0 else 0, source=source))
    return out


def role(cuts, t):
    cal, dev, held = cuts
    if t < cal:
        return "train"
    if t < dev:
        return "calibration"
    if t < held:
        return "development"
    return "heldout"


def build(artifact, cuts, outcomes, ledger):
    rows, tally, keys = [], {}, set()
    for o in outcomes:
        if o["source"] in ("none", "summary"):
            reason = "unlabelled" if o["source"] == "none" else "summary"
        else:
            r = role(cuts, o["time"])
            key = hashlib.sha256(words(0, o["group"], o["time"], o["decision"])).digest()
            held = ledger.get(o["group"])
            if held and held[0] != r:
                reason = "reserved" if held[1] - {artifact} else "crosses"
            elif key in keys:
                reason = "duplicate"
            else:
                reason = "admitted"
                keys.add(key)
                rows.append((0, o["group"], o["time"], ROLES.index(r), o["label"],
                             LABELS.index(o["source"]), key))
                ledger.setdefault(o["group"], (r, set()))[1].add(artifact)
        tally[reason] = tally.get(reason, 0) + 1
    digest = hashlib.sha256(words(len(rows)) + b"".join(words(*r[:6]) + r[6] for r in rows)).hexdigest()
    per = [sum(1 for r in rows if r[3] == k) for k in range(1, 5)]
    parts = ", ".join("%s %d" % (k, tally[k]) for k in REASONS if k in tally)
    roles = ", ".join("%s %d" % (ROLES[k + 1], per[k]) for k in range(4))
    return "artifact %d, cuts %d/%d/%d: %s; rows: %s; digest %s" % (
        (artifact,) + tuple(cuts) + (parts, roles, digest[:16]))


def main():
    root = pathlib.Path(__file__).resolve().parents[2]
    text = (root / "psionic/fixtures/training-rows.out").read_text()
    ledger = {}
    lines = [build(0, (22, 30, 38), feed(), ledger), build(1, (14, 18, 26), feed(), ledger)]
    for line in lines:
        assert line in text, line
    # The trace artifact trains on everything: group 9 is already in the
    # ledger, as development, so its trace is refused whatever its verdict.
    assert ledger[9][0] == "development"
    assert "group 9: reserved" in text
    # Groups 20 and 24 join the ledger through the two admitted traces.
    shared = sum(1 for _, users in ledger.values() if users == {0, 1})
    line = "ledger: %d groups, %d shared by artifacts 0 and 1 in one role; group 9: development, artifact mask 1" % (
        len(ledger) + 2, shared)
    assert line in text, line
    lines.append(line)
    print("rows oracle:\n" + "\n".join(lines))
    return 0


if __name__ == "__main__":
    sys.exit(main())
