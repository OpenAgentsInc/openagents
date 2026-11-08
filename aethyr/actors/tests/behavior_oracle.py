#!/usr/bin/env python3
# Firm reason: 2 (independent behavior state machine, weighted choice and identity oracle).
"""Literal profile contracts and an independently implemented small behavior model."""
import hashlib
import bisect
from pathlib import Path
import struct
import sys


def digest(words):
    return hashlib.sha256(b"".join(struct.pack("<Q", x & ((1 << 64) - 1)) for x in words)).digest()


def lanes(raw):
    return list(struct.unpack(">8I", raw))


def value(kind=1, integer=0):
    return [kind, integer] + [0] * 13


def schema():
    words = [1179866161, 12992002, 1, 8]
    one = struct.unpack("<Q", struct.pack("<d", 1.0))[0]
    for identity, name, kind, scope, owner, high in [
        (101, "actor.mode", 1, 2, 7, 2), (102, "world.gate", 0, 0, 0, 1)
    ]:
        words += [identity, kind, scope, owner, 1, 1, 1, 0, high, 0, one, 0, len(name), 15]
        words += list(name.encode("ascii")) + value(kind)
    return lanes(digest(words))


def guard(kind=2, field=0, slot=-1, signal=1, expected=True, literal=None):
    return [kind, field, slot, signal, int(expected)] + (value() if literal is None else literal)


def base_identity():
    words = [0x31424741, 1, 12992004, 1, 30, 3, 8, 8, 1, 2, 2, 2, 2]
    words += schema() + [0, 101, 1, 2, 7, 1, 102, 0, 0, 0, 100, 200]
    words += [10, 0, 1, 3, 1, 1] + guard() + [5, 4, -1, 2, 1]
    words += [20, 1, 1, 0, 0, 1] + [0, 0, -1, 1, 1]
    words += [0, 100, 0, 1, 0, 0, 4, 200, 1, 1, 0, 0]
    return digest(words)


def random_below(seed, stream, counter, size):
    def mix(x):
        for shift, multiplier in ((16, 0x7FEB352D), (15, 0x846CA68B)):
            x = ((x ^ (x >> shift)) * multiplier) & 0xFFFFFFFF
        return x ^ (x >> 16)
    parts = [seed & 0xFFFFFFFF ^ 0x9E3779B9, (seed >> 32) & 0xFFFFFFFF,
             stream & 0xFFFFFFFF, (stream >> 32) & 0xFFFFFFFF,
             counter & 0xFFFFFFFF, (counter >> 32) & 0xFFFFFFFF]
    h = 0
    for part in parts:
        h = mix(h ^ part)
    return (h * size) >> 32


def weighted_choices(count=24, suppress=True):
    ids, weights = [10, 20, 30], [1, 2, 3]
    result = []
    for draw in range(count):
        options = [(i, weight) for i, weight in zip(ids, weights) if not suppress or not result or i != result[-1]]
        ticket = random_below(2026, 7, draw, sum(weight for _, weight in options))
        for ident, weight in options:
            if ticket < weight:
                result.append(ident)
                break
            ticket -= weight
    return result


def trial_identity():
    words = [0x31424741, 1, 12992005, 1, 30, 3, 8, 8, 1, 2, 2, 4, 2]
    words += schema() + [0, 101, 1, 2, 7, 1, 102, 0, 0, 0, 100, 200]
    words += [10, 0, 1, 5, 1, 2] + guard() + [5, 4, -1, 3, 0, 0, 0, -1, 2, 1]
    words += [20, 1, 2, 2, 2, 1] + guard(3, signal=0) + guard(1, 101, 0, 0, literal=value(integer=2)) + [1, 0, -1, 6, 1]
    words += [30, 2, 1, 0, 0, 1] + [0, 0, -1, 1, 1]
    words += [40, 2, 3, 0, 1, 1] + guard(0, 102, 1, 0, literal=value(0, 1)) + [5, 7, -1, 2, 1]
    words += [0, 100, 0, 2, 0, 0, 4, 200, 1, 1, 0, 1] + guard(0, 102, 1, 0, literal=value(0, 0))
    return lanes(digest(words))


def trial_rows():
    # Oracle uses absolute availability deadlines and cumulative-weight bisection.
    # Production uses completed-tick subtraction and a subtracting ticket walk.
    actions = [[(5, 4, 3, False), (0, 0, 2, True)], [(1, 0, 6, True)],
               [(0, 0, 1, True)], [(5, 7, 2, True)]]
    priority, weights, cooldown = [0, 1, 2, 2], [1, 2, 1, 3], [5, 2, 0, 0]
    active, step, started, argument, anchor_slot = None, 0, 0, 0, -1
    serial = draws = successes = failures = interrupts = 0
    last = -1
    cooling, completed, available_at = [0] * 4, [0] * 4, [0] * 4
    day = phase = mode = gate = sequence = 0
    rows = []
    for tick in range(1, 257):
        phase += 1
        day += phase // 8
        phase %= 8
        edited = tick % 13 == 0 or tick % 17 == 0 or tick in (73, 129)
        sequence += int(edited)
        if tick % 13 == 0:
            mode = (tick // 13) % 3
        if tick % 17 == 0:
            gate = 1 - gate
        if tick == 73:
            day, phase = 15, 7
        if tick == 129:
            day, phase = 2, 2
        known = tick % 48 < 32
        visible = known and tick % 24 < 12
        signals = int(known) + 2 * int(visible) + 4 * int(known and tick % 16 >= 8)
        signals += 16 if tick % 8 == 0 else 8
        signals += sum(bit for divisor, bit in ((37, 32), (41, 64), (43, 128)) if tick % divisor == 0)
        schedule_row = int(phase >= 4)
        scheduled = not gate if schedule_row else day % 2 == 0
        anchor = (200 if schedule_row else 100) if scheduled else 0
        conditions = [visible, scheduled and mode < 2, True, bool(gate)]
        events = []
        feedback = 0
        feedback_serial = 0
        pending_start = False

        def candidates():
            return [i for i in range(4) if conditions[i] and tick >= available_at[i]
                    and (i != 1 or scheduled)]

        def retire(outcome, event_serial, cool=True):
            nonlocal active, step, started, argument, anchor_slot, successes, failures, interrupts
            events.append([event_serial, (active + 1) * 10, step, outcome, 1])
            successes += int(outcome == 2)
            failures += int(outcome == 3)
            interrupts += int(outcome == 4)
            if cool:
                cooling[active] = 1
                completed[active] = tick
                available_at[active] = tick + cooldown[active]
            active, step, started, argument, anchor_slot = None, 0, 0, 0, -1

        if active is not None:
            verb, _, duration, _ = actions[active][step]
            if verb == 1 and tick >= started + 2:
                feedback = 3 if tick % 7 == 0 else 2
                feedback_serial = serial
            outcome = feedback
            if not outcome and tick >= started + duration:
                outcome = 2 if verb in (0, 5) else 3
            if outcome:
                if outcome == 2 and step + 1 < len(actions[active]):
                    events.append([serial, (active + 1) * 10, step, 2, 0])
                    step += 1
                    pending_start = True
                else:
                    retire(outcome, serial)
        if active is not None:
            verb, _, _, interruptible = actions[active][step]
            possible = candidates()
            higher = bool(possible) and priority[possible[0]] < priority[active]
            unavailable = verb == 1 and not scheduled
            shifted = verb == 1 and not pending_start and anchor != argument
            if unavailable or shifted or (interruptible and (not conditions[active] or higher)):
                retire(3 if pending_start and unavailable else 4, 0 if pending_start else serial, not shifted)
                pending_start = False
        new_draw = 0
        if active is None:
            possible = candidates()
            if possible:
                top = priority[possible[0]]
                options = [i for i in possible if priority[i] == top]
                if len(options) > 1:
                    options = [i for i in options if i != last]
                cumulative = []
                for i in options:
                    cumulative.append(weights[i] + (cumulative[-1] if cumulative else 0))
                ticket = random_below(2026, 7, draws, cumulative[-1])
                active = options[bisect.bisect_right(cumulative, ticket)]
                last, step, pending_start = active, 0, True
                draws += 1
                new_draw = 1
        if active is not None and pending_start:
            serial += 1
            started = tick
            verb, arg, _, _ = actions[active][step]
            argument = anchor if verb == 1 else arg
            anchor_slot = schedule_row if verb == 1 else -1
        assert active is not None and len(events) <= 2  # authored unconditional fallback
        verb, _, _, interruptible = actions[active][step]
        attributes = int(verb == 1) + 2 * int(verb == 5) + 4 * int(interruptible)
        row = [tick, tick - 1, day, phase, mode, gate, sequence, signals, schedule_row, anchor,
               0, active, (active + 1) * 10, step, serial, draws, started, argument, anchor_slot, last,
               successes, failures, interrupts, 1, int(pending_start), verb, attributes, 0, 0,
               4 + schedule_row, 4, new_draw, 1, len(events)]
        row += sum(events + [[0] * 5] * (2 - len(events)), [])
        row += cooling + completed
        row += [feedback, feedback_serial, 7 if tick % 11 == 0 else -1, tick] + trial_identity()
        assert len(row) == 64
        rows.append(row)
    return rows


def verify(path):
    raw = Path(path).read_bytes()
    expected = trial_rows()
    seen, profiles = set(), set()
    for line in raw.splitlines():
        fps, index, *row = map(int, line.split())
        assert fps in (30, 60, 144, 0) and 0 <= index < 256
        assert (fps, index) not in seen
        seen.add((fps, index))
        profiles.add(fps)
        assert len(row) == 64
        for word, (actual, want) in enumerate(zip(row, expected[index])):
            assert actual == want, (path, fps, index + 1, word, actual, want)
    assert profiles and len(seen) == 256 * len(profiles)
    print(f"PASS: {path}: {len(seen)} exact paid-step rows, {sorted(profiles)}; sha256={hashlib.sha256(raw).hexdigest()}")


if __name__ == "__main__":
    if len(sys.argv) > 1:
        for filename in sys.argv[1:]:
            verify(filename)
    else:
        print("base identity:", base_identity().hex(), lanes(base_identity()))
        print("weighted:", weighted_choices())
        print("repeat allowed:", weighted_choices(suppress=False))
        print("trial identity:", trial_identity())
