#!/usr/bin/env python3
# Firm reason: 2 (independent archetype, schema and RCM1 identity oracle).
"""Compare complete supplied native archetypes with independent literal contracts."""
import hashlib
from pathlib import Path
import struct
import sys


def words_id(data):
    return list(struct.unpack(">8I", hashlib.sha256(data).digest()))


def float_word(value):
    return struct.unpack("<q", struct.pack("<d", value))[0]


def schema_id():
    words = [1179866161, 12992002, 1, 5529600]
    for ident, name, kind, scope, owner, high in [
        (101, "actor.mode", 1, 2, 7, 2), (102, "world.gate", 0, 0, 0, 1)
    ]:
        words += [ident, kind, scope, owner, 1, 1, 1, 0, high, 0,
                  float_word(1.0), 0, len(name), 15]
        words += list(name.encode("ascii"))
        words += [kind] + [0] * 14
    return words_id(struct.pack("<" + "q" * len(words), *words))


def manifest_id():
    # Published RCM1; this fixture's LP64 and Wasm ResourcePlan is 168 bytes.
    model = b"RCM1/positions4-1/indices-1/query-box-1/query-capsule-1/htx-1/apw-1/cell-body-1"
    content = [words_id(bytes([i, 0, 0, 0])) for i in (1, 2, 3)]
    out = bytearray(1368)
    struct.pack_into("<4sIII8I", out, 0, b"RCM1", 1, 3, 440, *words_id(model))
    costs = [4, 0, 4, 168, 0, 0, 4, 0, 0, 0, 0, 0, 1]
    for i in range(3):
        at = 48 + 440 * i
        struct.pack_into("<IBBBBI8I13qI", out, at, 1, 2, 0, 0, 0, 0,
                         *content[i], *costs, int(i != 0))
        if i:
            struct.pack_into("<I8I", out, at + 152, 0, *content[0])
    return words_id(out)


def expected(sample):
    mode = sample % 16
    statuses = {8: 5, 9: 8, 10: 12, 11: 10, 12: 6, 13: 7, 14: 11, 15: 8}
    if mode in statuses:
        return [sample, statuses[mode]] + [0] * 9
    row = [sample, 0, 1, 30, 3, 3, 1, 6, 2, 2, 3, 10, 1, 20, 2, 30, 3]
    values = [sample % 3, 1, 0, 0.1 + (sample % 40) / 10, sample % 33,
              sample % 21, 20, 30, sample % 3 - 1, 10, 80, 128, 64, 16,
              8, 128, 8, 16, 1.6, 1]
    for slot, value in enumerate(values):
        scalar = slot in (*range(3, 11), 18)
        row += ([2, 0, float_word(float(value))] if scalar else [1, int(value), 0]) + [0] * 12
    reads, writes = [128, 9, 2, 4, 8, 48], [15, 2, 4, 8, 48, 64]
    for i in range(6):
        row += [10 + i, 1, i, i, 0 if i == 0 else 1 << (i - 1), reads[i], writes[i]]
    for i in range(6):
        row += [int(i != 0), max(0, i - 1)] + [0] * 5
    for i in range(6):
        row += [10 + i, i, reads[i], writes[i]]
    row += [0, 101, 1, 2, 7, 1, 102, 0, 0, 0, 1, 2]
    row += [1, 1, 1] + [0] * 61
    row += schema_id() + manifest_id()
    return row


def main():
    raw = Path(sys.argv[1]).read_bytes()
    seen, accepted = set(), 0
    for line in raw.splitlines():
        actual = list(map(int, line.split()))
        sample = actual[0]
        assert 0 <= sample < 64 and sample not in seen
        seen.add(sample)
        want = expected(sample)
        assert len(actual) == len(want), (sample, "length", len(actual), len(want))
        for slot, (a, b) in enumerate(zip(actual, want)):
            assert a == b, (sample, slot, a, b)
        accepted += actual[2]
    assert seen == set(range(64))
    print(f"PASS: 64 exact archetypes/refusals ({accepted} complete); full schema/RCM1 pins; sha256={hashlib.sha256(raw).hexdigest()}")


if __name__ == "__main__":
    main()
