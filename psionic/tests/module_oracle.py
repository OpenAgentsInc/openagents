#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: recomputes our answer outside Coax, so it must not share our implementation)
"""Chapter 176's independent reference, written from the chapter's specification.

It shares no code with psionic/lib/weight-module.coax. From the two modules'
meanings alone (chapter 168's arithmetic step, which also exports a; then
max(x, y), x == 0 and y) it computes what the linked module must return on
the example's 105 inputs and the example's checksum over them. From the two
modules' printed sizes and the linking rule it derives the linked block's
layers, width and weight count, and it decides the 10000-by-10000 case from
chapter 166's input range. It checks each against the example's output.
"""
import pathlib
import re
import sys

VALUE_MAX = 2 ** 26


def step(op, a, b):
    return {0: a + b, 1: a - b, 2: a * b}[op]


def linked(op, a, b):
    r = step(op, a, b)
    return max(r, a), int(r == 0), a


def main():
    root = pathlib.Path(__file__).resolve().parents[2]
    text = (root / "psionic/fixtures/link-modules.out").read_text()
    sizes = dict((m[0], (int(m[1]), int(m[2]), int(m[3])))
                 for m in re.findall(r"^(\w[\w ]*?): (\d+) layers, width (\d+), (\d+) weights", text, re.M))
    la, wa, na = sizes["step module"]
    lb, wb, nb = sizes["max module"]
    # The glue layer has one unit per wire (2); every layer of width W and u units has 3*W*u weights.
    width = wa + wb - 1
    units = na // (3 * wa) + 2 + nb // (3 * wb)
    want = (la + 1 + lb, width, 3 * width * units)
    assert sizes["linked"] == want, (sizes["linked"], want)
    assert ("seam after layer %d" % (la + 1)) in text
    count = total = 0
    for op in range(3):
        for a in range(-3, 4):
            for b in range(-2, 3):
                x, z, y = linked(op, a, b)
                count += 1
                total += count * (x * 9 + z * 3 + y)
    line = "%d inputs: the linked block, the chain and the composed graph agree; checksum %d" % (count, total)
    assert line in text, line
    # 10000 * 10000 is past the range of every module's inputs and of the evaluator.
    assert step(2, 10000, 10000) > VALUE_MAX
    assert "multiply 10000 by 10000: linked range at layer %d; chain range; graph range at node 9" % (la + 1) in text
    print("module oracle: sizes %s, %s" % (want, line))
    return 0


if __name__ == "__main__":
    sys.exit(main())
