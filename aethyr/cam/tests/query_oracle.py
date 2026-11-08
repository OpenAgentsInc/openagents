# Firm reason: 2 (independent state-machine and analytic camera-clearance oracle).
"""Check every trace field; require bit-identical native/Wasm cadence records."""
import math
import pathlib
import struct
import sys

FLOATS = {14, 15, 16, 17, 20, 21, 22, 23}
CADENCES = {0, 30, 60, 144}


def unpack(word):
    return struct.unpack("<f", struct.pack("<I", word))[0]


def model():
    # Remember absolute acquisition and visibility stamps. A failed snapshot
    # revokes the selection immediately; occlusion has a five-step lease.
    held, generation, acquired, visible = -1, 0, 0, 0
    rows = []
    for tick in range(1, 65):
        category = (1 if tick <= 8 else 2 if tick <= 16 else 4 if tick <= 24
                    else 8 if tick <= 32 else 16 if tick <= 40 else 3)
        stale = tick == 57
        occluded = category in {2, 3} and tick < 49
        current_generation = 1 if tick < 33 else 2
        if stale:
            held, generation, acquired, visible = -1, 0, 0, 0
            reason = 6
        elif not occluded:
            reason = 2 if held == 2 and generation == current_generation else (3 if held >= 0 else 1)
            if reason != 2:
                acquired = tick
            held, generation, visible = 2, current_generation, tick
        elif held >= 0:
            reason = 5
            if tick >= visible + 5:
                held = -1
        else:
            reason = 0

        x, y, z = (10.0, 6.0, 10.0) if tick >= 49 else (0.0, 1.6, 0.0)
        body_hit = category in {1, 3} and tick < 49
        # The entire swept segment meets a broad wall's front face. Its
        # signed z distance is 2 - radius - skin. The corner is qualified
        # independently by the exact quadratic in the Coax test fixture.
        length = math.sqrt(0.25**2 + 0.125**2 + 4**2)
        fraction = (2 - 0.2 - 0.001) / 4 if body_hit else 1.0
        eye = [x + 0.25 * fraction, y + 0.125 * fraction, z - 4 * fraction]
        aim = [eye[0], eye[1], eye[2] + 5]
        room = length * fraction
        centred = 2 - 0.25 - 0.001 if body_hit else 4.0
        if stale:
            eye, aim, room, centred = [x, y, z], [x, y, z + 1], 0.0, 0.0
        rows.append([
            tick, category, held, generation, acquired, visible, reason, -1,
            int(occluded and not stale), 0, int(stale),
            int(not stale), int(body_hit and not stale), int(stale), room,
            *eye, 77 if body_hit and not stale else 0,
            3 if body_hit and not stale else 0, *aim, centred,
            int(stale), int(not stale), int(body_hit and not stale), 1,
        ])
    return rows


def check(record, expected):
    if len(record) != 28:
        raise AssertionError(f"expected 28 words, got {len(record)}")
    for column, (actual, want) in enumerate(zip(record, expected)):
        if column in FLOATS:
            value = unpack(actual)
            if not math.isfinite(value) or abs(value - want) > 0.00001:
                raise AssertionError((expected[0], column, value, want))
        elif actual != want:
            raise AssertionError((expected[0], column, actual, want))


def main(paths):
    expected = model()
    reference = {}
    total = 0
    observed_cadences = set()
    for name in paths:
        records = {}
        for line in pathlib.Path(name).read_text().splitlines():
            words = list(map(int, line.split()))
            fps, row, *record = words
            if fps not in CADENCES or not 0 <= row < 64 or (fps, row) in records:
                raise AssertionError((name, "invalid/duplicate row", fps, row))
            check(record, expected[row])
            records[fps, row] = record
            if row in reference and record != reference[row]:
                raise AssertionError((name, "cadence/platform bit mismatch", fps, row))
            reference[row] = record
        profiles = {fps for fps, _ in records}
        observed_cadences.update(profiles)
        if not profiles or any(sum(f == fps for f, _ in records) != 64 for fps in profiles):
            raise AssertionError((name, "incomplete trace"))
        total += len(records)
        print(f"PASS {name}: {len(records)} complete independent rows")
    if not paths:
        raise SystemExit("usage: query_oracle.py TRACE...")
    if observed_cadences != CADENCES:
        raise AssertionError(("missing required cadences", CADENCES - observed_cadences))
    # Negative controls: every reported lane must reject a material wrong
    # result. Float tolerances do not excuse a position or clearance error.
    for column in range(28):
        bad = list(reference[0])
        if column in FLOATS:
            bad[column] = struct.unpack("<I", struct.pack("<f", unpack(bad[column]) + 0.01))[0]
        else:
            bad[column] += 1
        try:
            check(bad, expected[0])
        except AssertionError:
            continue
        raise AssertionError(("oracle missed corrupted column", column))
    print(f"PASS {total} rows; cross-cadence/platform words identical; 28 negative controls rejected")


if __name__ == "__main__":
    main(sys.argv[1:])
