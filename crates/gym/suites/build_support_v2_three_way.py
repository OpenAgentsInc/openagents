#!/usr/bin/env python3
"""Builds `support-v2-three-way.json`, the same 196 items in three partitions.

`build_support_v2.py` splits on `seen % 2`: calibration and evaluation, and
nothing else. Every tuning decision made in this repository in the week to
2026-09-19 was made by reading that one evaluation split — four epochs
against two, the band objective against the choice objective, order-shuffle
augmentation, which families are admitted, and the Brier tolerance. A split
read that many times is a fitting set wearing an evaluation set's name.

This builder imports the item tables from `build_support_v2.py` rather than
restating them, so there is one copy of the labels, and repartitions them:

- `calibration` (about 40%) fits maps.
- `development` (about 40%) chooses between them.
- `locked` (about 20%) is read once, through `gym::suite::LockedLedger`,
  and the read is recorded.

Assignment stays a round robin within each family, on the cycle
`calibration, calibration, development, development, locked`. The item
tables are ordered by difficulty — clear cases first, then near-boundary
cases, then the genuinely arguable ones — so a round robin gives all three
partitions the same difficulty mix, which a contiguous slice would not.

The items are identical to `support-v2` and only the partition field
changes, so this is a new artifact with a new digest and a new name. The
two-way `support-v2.json` stays where it is under its own digest, and every
row and record written against it stays interpretable.

    python3 build_support_v2_three_way.py > support-v2-three-way.json
"""

import hashlib
import json
import sys

from build_support_v2 import build as build_two_way

# One turn of the round robin, applied within each family.
CYCLE = ["calibration", "calibration", "development", "development", "locked"]

# The admission gate from `docs/lev/calibration.md`, carried in the manifest
# and deliberately outside the digest. Tightening a floor must not make
# historical runs read as drifted; each row pins the gate it was judged
# against by that gate's own digest instead.
GATE = {
    "name": "calibration-admission-v1",
    "source": "docs/lev/calibration.md",
    "conditions": {
        "ece_falls_by_at_least": 0.1,
        "nll_rises_by_at_most": 0.0,
        "brier_rises_by_at_most": 0.1,
    },
}


def build():
    items = build_two_way()["items"]
    seen_per_family = {}
    for item in items:
        # The two-way `split` is replaced, not kept alongside: two fields
        # naming the same partition is how they come to disagree.
        item.pop("split", None)
        seen = seen_per_family.get(item["family"], 0)
        item["partition"] = CYCLE[seen % len(CYCLE)]
        seen_per_family[item["family"]] = seen + 1

    suite = {
        "schema": "openagents.gym.suite.v1",
        "name": "support-v2-three-way",
        "created": "2026-09-19",
        "description": (
            "The 196 items of support-v2, repartitioned three ways: "
            "calibration fits a map, development chooses between maps, and "
            "locked is read once and recorded. The items and their labels "
            "are unchanged from support-v2; only the partition field "
            "differs, so this file carries its own digest and its own name."
        ),
        "tier": "scored",
        "gate": GATE,
        "items": items,
    }
    blob = json.dumps(items, sort_keys=True, separators=(",", ":")).encode()
    suite["digest"] = hashlib.sha256(blob).hexdigest()
    return suite


if __name__ == "__main__":
    json.dump(build(), sys.stdout, indent=1)
    sys.stdout.write("\n")
