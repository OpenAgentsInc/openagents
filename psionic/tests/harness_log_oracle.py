#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: recomputes our answer outside Coax, so it must not share our implementation)
"""Chapter 201's independent reference for the session log's digests.

It shares no code with psionic/lib/harness/log.coax. The events are written
from chapter 200's hand trace (the numbers its chapter derives by hand), the
step seeds from SplitMix64 as chapter 53 specifies it, and the digests from
Python's hashlib over the encoding chapter 201 states. `--tests` prints the
digests the Coax checks compare.
"""
import hashlib
import struct
import sys

MASK = (1 << 64) - 1
KIND = dict(start=1, system=2, user=3, context=4, request=5, reply=6, output=7, stop=8)


def u64(x):
    return struct.pack("<Q", x & MASK)


def text(s):
    b = s.encode("utf-8")
    return u64(len(b)) + b


def splitmix(seed, counter):
    z = (seed + 0x9E3779B97F4A7C15 * (counter + 1)) & MASK
    z = ((z ^ (z >> 30)) * 0xBF58476D1CE4E5B9) & MASK
    z = ((z ^ (z >> 27)) * 0x94D049BB133111EB) & MASK
    return z ^ (z >> 31)


def content(tokens):
    return hashlib.sha256(text("psionic.harness.log.content.v1") + u64(len(tokens)) + b"".join(u64(t) for t in tokens)).digest()


def event(sequence, previous, kind, tokens, seed, fields):
    fields = list(fields) + [0] * (7 - len(fields))
    body = text("psionic.harness.log.event.v1") + u64(sequence) + previous + bytes([KIND[kind]])
    body += u64(len(tokens)) + content(tokens) + u64(seed) + b"".join(u64(f) for f in fields)
    return hashlib.sha256(body).digest()


def hand_trace():
    """Chapter 200's hand trace as events: the prompt, then the turn."""
    prompt = [320, 321, 63, 322, 323]
    replies = [[325, 51, 55, 43, 52, 56, 326], [325, 56, 53, 42, 50, 326], [49, 55, 48]]
    outputs = [[327, 56, 53, 328], [327, 49, 55, 48, 328]]
    seen, limit, draws = [5, 16, 27], [8, 8, 4], [7, 6, 4]
    stop, positions, value = [5, 5, 1], [11, 21, 30], [85, 170]
    budget = [3, 17, 68, 8, 3, -1]
    out = [("user", prompt, 0, []), ("start", [], 7, budget)]
    for k in range(3):
        out.append(("request", [], splitmix(7, k), [k, seen[k], limit[k]]))
        out.append(("reply", replies[k], 0, [k, draws[k], stop[k], positions[k]]))
        if k < 2:
            out.append(("output", outputs[k], 0, [k, value[k]]))
    out.append(("stop", [], 0, [1, 3, 30, 27, 17, 68, 2]))
    return out


def chain(events):
    previous, digests = bytes(32), []
    for i, (kind, tokens, seed, fields) in enumerate(events):
        previous = event(i, previous, kind, tokens, seed, fields)
        digests.append(previous)
    return digests


def main():
    if sys.argv[1:] != ["--tests"]:
        print("usage: harness_log_oracle.py --tests", file=sys.stderr)
        return 2
    events = hand_trace()
    print("content(prompt)", content(events[0][1]).hex())
    print("content(empty)", content([]).hex())
    # Longer than one 512-byte block of the encoder: tokens 0..99.
    print("content(0..99)", content(list(range(100))).hex())
    for i, d in enumerate(chain(events)):
        print("event", i, events[i][0], d.hex())
    # The same log with a context event injected before the turn: every
    # digest from there on changes.
    injected = events[:1] + [("context", [120], 0, [])] + events[1:]
    print("injected head", chain(injected)[-1].hex())
    return 0


if __name__ == "__main__":
    sys.exit(main())
