#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: recomputes our answer outside Coax, so it must not share our implementation)
"""Chapter 159's independent reference, written from the chapter's specification.

It shares no code with psionic/lib/pylon/selfcheck.coax. It reads a shipped
reference set, parses engine answers with the json module, converts each
decimal to billionths with Decimal, and checks every case. Given a set and a
file of answers (one JSON answer per line, in case order), it prints the
verdict; with --tests it checks the chapter's pinned cases.
"""
import json
import sys
from decimal import Decimal

NANO = 10 ** 9


def nanos(text):
    if not text or text[0] not in "01" or "e" in text.lower() or "-" in text:
        return None
    if len(text) > 1 and (text[1] != "." or len(text) < 3 or len(text) > 11):
        return None
    v = Decimal(text) * NANO
    if v != v.to_integral_value() or v > NANO:
        return None
    return int(v)


def read_set(text):
    lines = text.split("\n")
    if lines and lines[-1] == "":
        lines.pop()
    assert lines[0] == "psionic.selfcheck.v1"
    identity = lines[1].removeprefix("identity ")
    tolerance = int(lines[2].removeprefix("tolerance "))
    cases = []
    for i in range(3, len(lines), 3):
        question = lines[i].removeprefix("case ")
        request = lines[i + 1].removeprefix("request ")
        words = lines[i + 2].split(" ")
        if words[1] == "noul":
            cases.append(dict(question=question, request=request, kind="noul", values={None: int(words[2])}))
        else:
            values = {w.split("=")[0]: int(w.split("=")[1]) for w in words[3:]}
            cases.append(dict(question=question, request=request, kind="choice", choice=words[2], values=values))
    return identity, tolerance, cases


def compare(case, raw, identity, tolerance):
    answer = json.loads(raw, parse_float=str, parse_int=str)
    named = answer.get("service", {}).get("identity")
    if named is not None and named != identity:
        return "shape", 0
    a = answer.get("answers", {}).get(case["question"])
    if a is None:
        return "missing", 0
    if a.get("type") != case["kind"]:
        return "kind", 0
    if case["kind"] == "noul":
        got = {None: nanos(a["noul"])}
    else:
        if a["choice"] != case["choice"]:
            return "choice", 0
        if len(a["probabilities"]) != len(case["values"]):
            return "options", 0
        got = {k: nanos(v) for k, v in a["probabilities"].items()}
    worst = 0
    for k, want in case["values"].items():
        if k not in got:
            return "missing", 0
        e = abs(got[k] - want)
        if e > tolerance:
            return "value", e
        worst = max(worst, e)
    return "pass", worst


def check(set_text, answers, identity):
    want, tolerance, cases = read_set(set_text)
    if want != identity:
        return "other_model", None
    worst = 0
    for i, case in enumerate(cases):
        if i >= len(answers):
            return "failed", (i, "missing", 0)
        code, e = compare(case, answers[i], identity, tolerance)
        if code != "pass":
            return "failed", (i, code, e)
        worst = max(worst, e)
    return "passed", worst


def gate(last, declared, engine):
    if not engine:
        return "unnamed"
    if declared and declared != engine:
        return "other"
    if not last:
        return "first"
    if last == engine:
        return "same"
    return "declared" if declared else "undeclared"


def tests():
    assert nanos("0.757202773") == 757202773 and nanos("0.98758345") == 987583450 and nanos("1") == NANO
    assert nanos("0.0000000001") is None and nanos("1.000000001") is None and nanos("1e-3") is None
    text = open("psionic/fixtures/pylon/elephant-1.selfcheck").read()
    identity, tolerance, cases = read_set(text)
    print("set", identity[:24], tolerance, len(cases), [c["question"] for c in cases])
    for g in [("", "", "a"), ("a", "", "a"), ("b", "a", "a"), ("b", "", "a"), ("a", "b", "a"), ("a", "", "")]:
        print("gate", g, gate(*g))


def main():
    if "--tests" in sys.argv:
        tests()
        return
    set_path, answers_path, identity = sys.argv[1:4]
    answers = [l for l in open(answers_path).read().split("\n") if l.strip()]
    print(check(open(set_path).read(), answers, identity))


if __name__ == "__main__":
    main()
