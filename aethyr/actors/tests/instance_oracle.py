#!/usr/bin/env python3
# Firm reason: 2 (independent actor lifecycle, entity and quota state-machine oracle).
"""Check complete paid-step observations without importing the Coax implementation."""
from pathlib import Path
import hashlib
import sys


def empty():
    return dict(phase=0, key=(-1, 0, 0, 0, 0), entity=-1, entity_id=0, handle=0,
                control=0, tick=0, active=False, serial=0, draws=0,
                successes=0, failures=0, interrupts=0, cooling=False,
                completed=0, available=0, started=0)


def expected():
    slots = [empty() for _ in range(3)]
    saved = [(-1, 0, 0, 0, 0) for _ in range(4)]
    # Entity reference model: an append-only identity log, with reusable
    # storage places. It does not call the production handle/store code.
    objects, free = [], []
    next_id = serial = refused = peak = 0
    closed = False

    def matches(key):
        place = key[0]
        return 0 <= place < 3 and slots[place]["phase"] != 0 and slots[place]["key"] == key

    def current(key):
        if not matches(key) or closed:
            return False
        slot = slots[key[0]]
        obj = objects[slot["entity"]] if slot["entity"] >= 0 else None
        return slot["phase"] == 3 and obj is not None and obj["live"] and obj["id"] == slot["entity_id"]

    def despawn(slot):
        if slot["entity"] >= 0:
            obj = objects[slot["entity"]]
            if obj["id"] == slot["entity_id"] and (obj["live"] or obj["born"]):
                obj["doomed"] = True

    def entity_digest():
        h = 2166136261
        words = [len(objects), len(free), sum(o["live"] for o in objects),
                 0, 0, next_id, 0, 0]
        for obj in objects:
            words.extend([obj["generation"], obj["id"], int(obj["live"])])
        words.extend(free)
        for word in words:
            h = ((h ^ (word & 0xFFFFFFFF)) * 16777619) & 0xFFFFFFFF
        return h

    for tick in range(1, 65):
        request_status, request_slot, request_generation = -1, -1, 0
        stopped, transfer_status, transfer_owner = -1, -1, 0
        if tick % 3 == 1:
            identity = 1000 + (tick // 3) % 4
            if any(s["phase"] and s["key"][1] == identity for s in slots):
                request_status = 4
            elif all(s["phase"] for s in slots):
                request_status = 5
            elif closed:
                request_status = 7
            else:
                request_status = 0
                request_slot = next(i for i, s in enumerate(slots) if not s["phase"])
                serial += 1
                request_generation = serial
                slot = empty()
                slot.update(phase=1, key=(request_slot, identity, serial, 1, 7), tick=tick - 1)
                slots[request_slot] = slot
                saved[identity - 1000] = slot["key"]
                peak = max(peak, sum(bool(s["phase"]) for s in slots))
            refused += request_status != 0
        if tick % 7 == 0:
            key = saved[(tick // 7) % 4]
            stopped = int(matches(key) and slots[key[0]]["phase"] != 4)
            if stopped:
                slots[key[0]]["phase"] = 4
                despawn(slots[key[0]])
        if tick % 11 == 0:
            at = (tick // 11) % 4
            key = saved[at]
            transfer_status = 3
            if current(key):
                transfer_status = 0
                slot = slots[key[0]]
                if slot["active"]:
                    slot.update(active=False, cooling=True, completed=tick - 1, available=tick + 1)
                    slot["interrupts"] += 1
                slot["control"] = 1 - slot["control"]
                slot["key"] = key[:3] + (key[3] + 1, key[4])
                saved[at] = slot["key"]
                transfer_owner = slot["key"][3]
        if tick == 49:
            closed = True

        events = []
        for i, slot in enumerate(slots):
            phase = slot["phase"]
            if not phase:
                continue
            event = None
            if phase == 4:
                obj = objects[slot["entity"]] if slot["entity"] >= 0 else None
                if obj is not None and obj["id"] == slot["entity_id"] and (obj["live"] or obj["born"]):
                    despawn(slot)
                else:
                    event = [5, *slot["key"]]
                    slots[i] = empty()
            elif closed:
                slot["phase"] = 4
                despawn(slot)
                event = [4, *slot["key"]]
            elif phase == 1:
                if free:
                    place = free.pop()
                else:
                    place = len(objects)
                    objects.append(dict(generation=0))
                next_id += 1
                objects[place].update(id=next_id, born=True, live=False, doomed=False)
                slot.update(phase=2, entity=place, entity_id=next_id,
                            handle=((objects[place]["generation"] + 1) << 32) | place)
                event = [2, *slot["key"]]
            elif phase == 2 and objects[slot["entity"]]["live"]:
                slot.update(phase=3, tick=tick - 1)
                event = [3, *slot["key"]]
            if event:
                events.append(event)

        probes = 0
        for slot in slots:
            if not current(slot["key"]):
                continue
            probes += tick % 5 == 0
            if slot["active"] and tick >= slot["started"] + 3:
                slot.update(active=False, cooling=True, completed=tick, available=tick + 2)
                slot["successes"] += 1
            if not slot["active"] and tick >= slot["available"]:
                slot.update(active=True, started=tick)
                slot["serial"] += 1
                slot["draws"] += 1
            slot["tick"] = tick

        for place, obj in enumerate(objects):
            if obj["doomed"]:
                obj.update(id=0, live=False, born=False, doomed=False)
                obj["generation"] += 1
                free.append(place)
            elif obj["born"]:
                obj.update(born=False, live=True)

        count = sum(bool(s["phase"]) for s in slots)
        row = [tick, tick, count, serial, refused, peak,
               count * 8, count * 128, count * 8, peak * 8, peak * 128, peak * 8,
               request_status, request_slot, request_generation, stopped,
               transfer_status, transfer_owner, 0, len(events),
               sum(o["live"] for o in objects), next_id, entity_digest(), probes]
        row += sum(events + [[0, -1, 0, 0, 0, 0]] * (3 - len(events)), [])
        for slot in slots:
            row += [slot["phase"], *slot["key"][1:], slot["handle"], slot["entity_id"], slot["control"],
                    slot["tick"], 0 if slot["active"] else -1, slot["serial"], slot["draws"],
                    slot["successes"], slot["failures"], slot["interrupts"], int(slot["cooling"]),
                    slot["completed"], 0]
        # Two four-byte index sources, each retaining 168 bytes of RCM1
        # ResourcePlan metadata, four encoded bytes and four GPU bytes.
        row += [int(closed), 0 if closed else count + 1, int(tick < 51),
                352 if tick < 51 else 0, 344 if tick < 51 else 0,
                2 if tick < 51 else 0, 3 if tick < 49 else 4 if tick < 51 else 0,
                0 if tick < 51 else 2]
        assert len(row) == 104
        yield row


def verify(path):
    raw = Path(path).read_bytes()
    rows = list(expected())
    seen, profiles = set(), set()
    for line in raw.splitlines():
        fps, index, *actual = map(int, line.split())
        assert fps in (0, 30, 60, 144) and 0 <= index < 64
        assert (fps, index) not in seen
        seen.add((fps, index))
        profiles.add(fps)
        assert len(actual) == 104
        for word, (got, want) in enumerate(zip(actual, rows[index])):
            assert got == want, (path, fps, index + 1, word, got, want)
    assert profiles and len(seen) == len(profiles) * 64
    print(f"PASS: {path}: {len(seen)} exact lifecycle/selection rows, {sorted(profiles)}; sha256={hashlib.sha256(raw).hexdigest()}")


if __name__ == "__main__":
    if len(sys.argv) < 2:
        raise SystemExit("usage: instance_oracle.py OBSERVATION.txt...")
    for filename in sys.argv[1:]:
        verify(filename)
