"""Deterministic public synthetic corpus of System One requests for Clef parity.

Writes one raw JSON request body per line. Numbers are spelled several ways on
purpose (1E2, 1.50, -0, 1e400, big ints) so the renderer is exercised on
Python's json.loads -> json.dumps round trip, not on our own spelling.
"""

import json
import random
import sys

R = random.Random(20261009)


class Raw:
    def __init__(self, text):
        self.text = text


NUMBERS = [
    "0", "-0", "1", "-1", "42", "100", "1.0", "1.50", "-2.25", "0.1", "0.0001", "0.00001",
    "1e-5", "1E-5", "1e2", "1E2", "1e16", "1e15", "1e22", "1.5e300", "1e400", "-1e400",
    "5e-324", "1.7976931348623157e308", "123456789012345678901234567890",
    "-9223372036854775809", "18446744073709551616", "3.141592653589793", "2.718281828459045",
    "123456789.123456789", "0.30000000000000004", "1e-7", "-0.0", "0.5", "1234.5e-2",
    "9007199254740993", "1.0e+3", "6.02214076e23", "1.1e16", "999999999999999.9",
    "9999999999999998.0", "0.001", "0.000123",
]

WORDS = (
    "checkout login billing refund invoice crash latency timeout outage password reset "
    "upgrade downgrade plan seat team admin user cart payment card declined fraud "
    "dashboard export import webhook api key token rate limit region deploy rollback "
    "build test merge branch release ticket urgent minor major critical feature bug"
).split()

UNICODE = [
    "café", "naïve", "日本語のテキスト", "中文字符", "한국어", "Ελληνικά", "русский текст",
    "עברית", "العربية", "emoji 🚀🔥✅", "family 👨‍👩‍👧‍👦", "flag 🇺🇸", "é combining",
    "Ångström", "ﬁ ligature", "zero​width", "tab\there", "line\nbreak", "cr\rreturn",
    "quote \" and backslash \\", "ctrl \x01\x07\x1f", "del \x7f char", "nbsp space",
    "math ∑∫√∞", "<|im_end|> literal marker", "<think> tag", "   leading spaces",
    "trailing spaces   ", "MiXeD CaSe", "digits 1234567890", "punct !?.,;:-_()[]{}",
    " line sep", "full-width ＡＢＣ１２３", "tilde ~ caret ^ pipe |",
]


def word(n=1):
    return " ".join(R.choice(WORDS) for _ in range(n))


def text(max_words=12):
    parts = []
    for _ in range(R.randint(1, max_words)):
        if R.random() < 0.18:
            parts.append(R.choice(UNICODE))
        else:
            parts.append(R.choice(WORDS))
    return " ".join(parts)


def number():
    return Raw(R.choice(NUMBERS))


def value(depth=0):
    roll = R.random()
    if depth > 3:
        roll = roll * 0.6
    if roll < 0.25:
        return text()
    if roll < 0.45:
        return number()
    if roll < 0.5:
        return R.choice([True, False, None])
    if roll < 0.75:
        return {key(): value(depth + 1) for _ in range(R.randint(0, 5))}
    return [value(depth + 1) for _ in range(R.randint(0, 5))]


def key():
    roll = R.random()
    if roll < 0.6:
        return R.choice(WORDS) + ("_" + R.choice(WORDS) if R.random() < 0.4 else "")
    if roll < 0.8:
        return R.choice(UNICODE)[:12]
    return R.choice(["Zebra", "apple", "Äpfel", "10", "9", "_x", "a b", "", "B", "b", "ß", "z", "Ω"])


def dump(v):
    if isinstance(v, Raw):
        return v.text
    if isinstance(v, dict):
        return "{" + ",".join(json.dumps(k, ensure_ascii=R.random() < 0.3) + ":" + dump(x) for k, x in v.items()) + "}"
    if isinstance(v, list):
        return "[" + ", ".join(dump(x) for x in v) + "]"
    return json.dumps(v, ensure_ascii=R.random() < 0.3)


def state():
    roll = R.random()
    if roll < 0.45:
        return " ".join(text(20) for _ in range(R.randint(1, 6)))
    if roll < 0.9:
        return {key(): value() for _ in range(R.randint(1, 8))}
    return value()


def instructions(qid):
    roll = R.random()
    if roll < 0.55:
        return text(16) + "?"
    if roll < 0.7:
        return {"question": text(10), "context": value(2), "focus": text(4)}
    if roll < 0.8:
        return None  # missing key
    if roll < 0.87:
        return ""
    if roll < 0.92:
        return Raw("null")
    return value(2)


def description():
    roll = R.random()
    if roll < 0.6:
        return text(10)
    if roll < 0.75:
        return {"what": text(6), "not_for": text(4), "examples": [text(3) for _ in range(R.randint(0, 3))]}
    if roll < 0.85:
        return Raw("null")
    return value(2)


def unique_keys(n):
    keys = []
    seen = set()
    while len(keys) < n:
        k = key() if R.random() < 0.7 else f"{R.choice(WORDS)}_{len(keys)}"
        if k in seen:
            continue
        seen.add(k)
        keys.append(k)
    return keys


def question(max_options):
    kind = R.choice(["noul", "noul", "choice", "choice", "score"])
    q = {"type": kind}
    ins = instructions(None)
    if ins is not None:
        q["instructions"] = ins
    if kind == "noul":
        roll = R.random()
        if roll < 0.2:
            q["criteria"] = {"true": text(6), "false": text(6)}
        elif roll < 0.3:
            q["criteria"] = {"true": text(6)}
        elif roll < 0.35:
            q["criteria"] = {"false": {"what": text(4)}}
    elif kind == "choice":
        n = R.randint(2, max_options)
        q["criteria"] = {k: description() for k in unique_keys(n)}
    else:
        n = R.randint(2, min(max_options, 11))
        q["criteria"] = [description() if R.random() < 0.8 else text(3) for _ in range(n)]
    return q


def record(index):
    if index % 25 == 0:
        max_options, max_questions = 60, 13
    elif index % 7 == 0:
        max_options, max_questions = 30, 6
    else:
        max_options, max_questions = 8, 5
    nq = R.randint(1, max_questions)
    qids = []
    while len(qids) < nq:
        qid = R.choice(WORDS) + (f"_{len(qids)}" if R.random() < 0.6 else "")
        if R.random() < 0.08:
            qid = R.choice(["Ünïcode_id", "id with space", "日本", "x"]) + str(len(qids))
        if qid not in qids:
            qids.append(qid)
    body = "{" + '"model":"clef-flash","state":' + dump(state()) + ',"questions":{'
    body += ",".join(json.dumps(qid, ensure_ascii=False) + ":" + dump(question(max_options)) for qid in qids)
    body += "}}"
    json.loads(body)  # must be valid JSON
    return body


def main():
    count = int(sys.argv[1]) if len(sys.argv) > 1 else 200
    for index in range(count):
        print(record(index))


if __name__ == "__main__":
    main()
