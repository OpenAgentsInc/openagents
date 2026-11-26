#!/usr/bin/env python3
# Firm reason: 5 (subagent-bench eval fixture/verifier)
from datetime import datetime
from plugins.csv_norm import normalize_csv
from plugins.json_patch import apply_patch
from plugins.log_parser import parse_and_summarize
from plugins.rate_limiter import SlidingWindowCounter, TokenBucketLimiter
from plugins.cron_matcher import CronSpec

# 1. csv_norm
raw = '\ufeff Name , Role , Bio \r\n"Alice","eng","line1\nline2 ""ok"""\r\n , , \r\nBob, pm\r\n'
hdrs, rows = normalize_csv(raw, required_cols=["NAME", "role"])
assert hdrs == ["name", "role", "bio"], hdrs
assert len(rows) == 2, rows
assert rows[0] == {"name": "Alice", "role": "eng", "bio": 'line1\nline2 "ok"'}, rows[0]
assert rows[1] == {"name": "Bob", "role": "pm", "bio": ""}, rows[1]
for bad in ["a,a\n1,2", "a,b\n1,2,3"]:
    try:
        normalize_csv(bad)
        raise AssertionError(f"expected ValueError for {bad!r}")
    except ValueError:
        pass

# 2. json_patch
orig = {"a/b": [1, 2], "m~n": {"x": True}}
res = apply_patch(orig, [
    {"op": "test", "path": "/m~0n/x", "value": True},
    {"op": "add", "path": "/a~1b/-", "value": 3},
    {"op": "add", "path": "/a~1b/1", "value": 99},
    {"op": "copy", "from": "/a~1b", "path": "/copied"},
    {"op": "move", "from": "/m~0n/x", "path": "/flag"},
    {"op": "replace", "path": "/copied/0", "value": 7},
    {"op": "remove", "path": "/a~1b/2"},
])
assert orig == {"a/b": [1, 2], "m~n": {"x": True}}, "mutated input!"
assert res["a/b"] == [1, 99, 3], res
assert res["copied"] == [7, 99, 2, 3], res
assert res["flag"] is True and "x" not in res["m~n"], res
try:
    apply_patch({"x": True}, [{"op": "test", "path": "/x", "value": 1}])
    raise AssertionError("True == 1 should fail strict test")
except ValueError:
    pass
try:
    apply_patch({"a": {"b": 1}}, [{"op": "move", "from": "/a", "path": "/a/b/c"}])
    raise AssertionError("move into child must fail")
except ValueError:
    pass

# 3. log_parser
logs = [
    "# comment",
    "",
    '2026-11-26T10:00:01Z info api status=200 latency_ms=40 path="/v1/a b"',
    '2026-11-26T10:00:02Z ERROR api status=503 latency_ms=120 msg="bad \\"quote\\""',
    '2026-11-26T10:00:03Z WARN auth status=500 latency_ms=50',
    'BROKEN_LINE',
    '2026-11-26T10:00:04Z INFO api msg="unclosed',
]
s = parse_and_summarize(logs)
assert s["total_valid"] == 3, s
assert s["malformed_count"] == 2, s
assert s["by_level"] == {"INFO": 1, "ERROR": 1, "WARN": 1}, s
assert s["by_service"]["api"] == {"count": 2, "errors": 1, "avg_latency_ms": 80.0}, s
assert s["by_service"]["auth"] == {"count": 1, "errors": 1, "avg_latency_ms": 50.0}, s

# 4. rate_limiter
tb = TokenBucketLimiter(capacity=5.0, refill_per_sec=2.0)
assert tb.allow("k", 0.0, 3.0) == (True, 0.0)
ok, wait = tb.allow("k", 0.0, 3.0)
assert not ok and abs(wait - 0.5) < 1e-6, (ok, wait)
assert tb.allow("k", 0.5, 3.0) == (True, 0.0)
sw = SlidingWindowCounter(limit=4, window_s=10.0)
for t in [1.0, 2.0, 3.0, 4.0]:
    assert sw.allow("u", t) is True
assert sw.allow("u", 5.0) is False
# At t=15.0: prev=4 * (1 - 0.5) = 2.0 -> 2 more allowed, 3rd denied
assert sw.allow("u", 15.0) is True
assert sw.allow("u", 15.1) is True
assert sw.allow("u", 15.2) is False

# 5. cron_matcher
c1 = CronSpec.parse("*/15 9-17 * * 1-5")
assert c1.matches(datetime(2026, 11, 26, 9, 0)) is True  # Thursday
assert c1.matches(datetime(2026, 11, 28, 9, 0)) is False # Saturday
assert c1.next_after(datetime(2026, 11, 27, 17, 45, 30)) == datetime(2026, 11, 30, 9, 0)
c2 = CronSpec.parse("0 0 15 * 0")  # 15th OR Sunday
assert c2.matches(datetime(2026, 11, 15, 0, 0)) is True  # Sunday & 15th
assert c2.matches(datetime(2026, 11, 22, 0, 0)) is True  # Sunday only
assert c2.matches(datetime(2026, 12, 15, 0, 0)) is True  # Tuesday 15th only
assert c2.matches(datetime(2026, 12, 16, 0, 0)) is False
c3 = CronSpec.parse("0 0 * * 7")  # 7 == Sunday
assert c3.matches(datetime(2026, 11, 22, 0, 0)) is True
print("SB2_ALL_5_PLUGINS_OK")
