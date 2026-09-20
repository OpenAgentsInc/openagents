#!/usr/bin/env python3
"""Builds `support-v2-unseen.json`, the 98 items no Lev adapter trained on.

`support-v2-three-way` partitioned the same 196 items that the two-way
`support-v2` had already split, and every adapter under `docs/lev/` was
trained from that two-way `calibration` split before the three-way file
locked 20 of its 98 records (openagents#9399). The three-way locked
partition is therefore half training data for every adapted door, and a
confirmation read off it is half memory.

This builder keeps the 98 items the two-way `evaluation` split held, which
no adapter saw, and keeps the partition `support-v2-three-way` gave each of
them: 40 calibration, 39 development, and 19 locked. Nothing is redrawn, so
a development read of this suite is a development read of the three-way
suite restricted to clean items, and the two stay comparable. The locked
partition is smaller than the three-way suite's and it is clean, which is
the trade the issue names.

The three-way suite keeps its digest and its name; this is a new artifact
under a new name, and the three-way manifest names it as the `successor`
in its `exposure` record.

    python3 build_support_v2_unseen.py > support-v2-unseen.json
"""

import hashlib
import json
import pathlib
import sys

HERE = pathlib.Path(__file__).resolve().parent
TWO_WAY = HERE / "support-v2.json"
THREE_WAY = HERE / "support-v2-three-way.json"


def build():
    two_way = json.loads(TWO_WAY.read_text())
    three_way = json.loads(THREE_WAY.read_text())
    trained = {item["id"] for item in two_way["items"] if item["split"] == "calibration"}
    if len(trained) != 98:
        raise SystemExit(f"support-v2 has {len(trained)} calibration items, not 98")

    items = [item for item in three_way["items"] if item["id"] not in trained]
    if len(items) != 98:
        raise SystemExit(f"{len(items)} items survive the training split, not 98")

    suite = {
        "schema": three_way["schema"],
        "name": "support-v2-unseen",
        "created": "2026-09-20",
        "description": (
            "The 98 items of support-v2 that no Lev adapter trained on, under "
            "the partitions support-v2-three-way gave them: 40 calibration, "
            "39 development, 19 locked. support-v2-three-way locked 20 items "
            "that training/lev-adapter had already trained every adapter on "
            "from the two-way calibration split, so its locked partition "
            "cannot confirm an adapted door. This one can. Labels and "
            "partitions are unchanged from support-v2-three-way; only the "
            "trained-on items are gone."
        ),
        "tier": three_way["tier"],
        "gate": three_way["gate"],
        "questions": three_way["questions"],
        "items": items,
    }
    blob = json.dumps(items, sort_keys=True, separators=(",", ":")).encode()
    suite["digest"] = hashlib.sha256(blob).hexdigest()
    return suite


if __name__ == "__main__":
    json.dump(build(), sys.stdout, indent=1)
    sys.stdout.write("\n")
