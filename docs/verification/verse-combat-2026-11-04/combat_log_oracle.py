# Firm reason: 2 (independent oracle: compare Coax kill identities with the client's CSV combat log).
"""Verification only; never loaded by the controller or installed runtime."""
import csv
import json
from collections import Counter
from pathlib import Path

root = Path(__file__).resolve().parent
observed = []
for line in (root / "client-combat-log.txt").read_text().splitlines():
    _, separator, payload = line.partition("  ")
    if not separator:
        raise AssertionError("missing client timestamp separator")
    event = next(csv.reader([payload]))
    if event[0] == "PARTY_KILL" and event[2] == "Foob-DeviateDelight-US":
        guid = event[5].split("-")
        observed.append((int(guid[-2]), int(guid[-1], 16) & 0xFFFFFFFF))

recorded = []
for name in ("04-final-sanitized.jsonl", "05-installed-finish.jsonl"):
    for line in (root / name).read_text().splitlines():
        row = json.loads(line)
        if row["type"] == "verified-kill":
            assert row["gained_xp"] > 0
            recorded.append((row["npc"], row["spawn"]))

assert len(observed) == len(recorded) == 4
assert Counter(observed) == Counter(recorded), (observed, recorded)
print("ok: all four final/native XP kills match the client's player PARTY_KILL NPC and spawn GUIDs")
