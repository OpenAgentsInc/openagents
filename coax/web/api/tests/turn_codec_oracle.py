# Firm reason: 2 (independent stdlib JSON/Unicode oracle against the actual Coax-generated native readers).
"""Run with the UBSan shared image emitted from turn_codec_oracle.coax.

This is a reference of the documented durable-text profile, not a shipped
decoder. It uses Python's JSON and Unicode implementation, not Coax helpers.
"""
import copy
import ctypes
import json
import re
import sys

LIMIT = 2147483645
CONVERSATION = "cnv_0123456789abcdef0123456789abcdef"
TURN = "trn_0123456789abcdef0123456789abcdef"
PROJECT = "prj_0123456789abcdef0123456789abcdef"


def string(value, maximum, minimum=0):
    if not isinstance(value, str) or "\0" in value:
        return False
    try:
        return minimum <= len(value.encode("utf-8")) <= maximum
    except UnicodeError:
        return False


def integer(value, minimum=0):
    return type(value) is int and minimum <= value <= LIMIT


UUID_RE = re.compile(r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}")


def identity(value, prefix, digits=32):
    if not isinstance(value, str):
        return False
    if digits == 32 and UUID_RE.fullmatch(value) is not None:
        return True
    return re.fullmatch(prefix + "[0-9a-f]{%d}" % digits, value) is not None


def reference(kind, obj):
    if not isinstance(obj, dict):
        return False
    if kind == 1:
        if set(obj) != set("schema operation_id conversation_id expected_revision project_id expected_project_revision legacy_thread_id legacy_thread_revision content".split()):
            return False
        if obj["schema"] != "openagents.chat.submit.v2alpha1" or not string(obj["operation_id"], 80, 1):
            return False
        c, rev = obj["conversation_id"], obj["expected_revision"]
        if not integer(rev) or not ((c == "" and rev == 0) or (identity(c, "cnv_") and rev >= 1)):
            return False
        p, pr = obj["project_id"], obj["expected_project_revision"]
        if not ((p is None and pr is None) or (identity(p, "prj_") and integer(pr, 1))):
            return False
        old, oldrev = obj["legacy_thread_id"], obj["legacy_thread_revision"]
        if not ((old is None and oldrev is None) or (c == "" and identity(old, "m_", 20) and integer(oldrev, 1))):
            return False
        blocks = obj["content"]
        if not isinstance(blocks, list) or not 1 <= len(blocks) <= 16:
            return False
        if not all(isinstance(b, dict) and set(b) == {"type", "text"} and b["type"] == "text" and string(b["text"], 16384, 1) for b in blocks):
            return False
        return sum(len(b["text"].encode("utf-8")) for b in blocks) <= 16384
    if kind == 3:
        return set(obj) == {"schema", "operation_id", "turn_id"} and obj["schema"] == "openagents.chat.stop.v2alpha1" and string(obj["operation_id"], 80, 1) and identity(obj["turn_id"], "trn_")
    if kind == 4:
        if set(obj) != set("schema conversation_id turn_id conversation_revision state stop_requested event_cursor".split()):
            return False
        if obj["schema"] != "openagents.chat.control.v2alpha1" or not identity(obj["conversation_id"], "cnv_") or not identity(obj["turn_id"], "trn_") or not integer(obj["conversation_revision"], 1) or not integer(obj["event_cursor"]) or type(obj["stop_requested"]) is not bool:
            return False
        state = obj["state"]
        if state == "queued":
            return obj["event_cursor"] == 0 and obj["stop_requested"] is False
        return state in ("running", "completed", "failed", "cancelled", "unknown") and obj["event_cursor"] >= 1 and (state != "cancelled" or obj["stop_requested"])
    if kind == 5:
        if set(obj) != set("schema operation operation_id worker_id turn_id dispatch_id".split()):
            return False
        if obj["schema"] != "openagents.chat.runner.v2alpha1" or not string(obj["operation_id"], 80, 1) or not string(obj["worker_id"], 80, 1):
            return False
        if obj["operation"] == "next":
            return obj["turn_id"] is None and obj["dispatch_id"] is None
        return obj["operation"] in ("run", "lost") and identity(obj["turn_id"], "trn_") and identity(obj["dispatch_id"], "run_")
    if kind == 6:
        if set(obj) != set("schema has_work turn_id conversation_id dispatch_id".split()) or obj["schema"] != "openagents.chat.job.v2alpha1" or type(obj["has_work"]) is not bool:
            return False
        if not obj["has_work"]:
            return all(obj[key] is None for key in ("turn_id", "conversation_id", "dispatch_id"))
        return identity(obj["turn_id"], "trn_") and identity(obj["conversation_id"], "cnv_") and identity(obj["dispatch_id"], "run_")
    if set(obj) != set("schema conversation_id turn_id sequence type state stop_requested block_index delta usage stop_reason error".split()):
        return False
    if obj["schema"] != "openagents.chat.event.v2alpha1" or not identity(obj["conversation_id"], "cnv_") or not identity(obj["turn_id"], "trn_") or not integer(obj["sequence"], 1) or type(obj["stop_requested"]) is not bool:
        return False
    event = obj["type"]
    states = {"started": "running", "text_delta": "running", "usage": "running", "stop_requested": "running", "completed": "completed", "failed": "failed", "unknown": "unknown", "cancelled": "cancelled"}
    if not isinstance(event, str) or event not in states or obj["state"] != states[event]:
        return False
    if event == "text_delta":
        if type(obj["block_index"]) is not int or obj["block_index"] != 0 or not string(obj["delta"], 16384, 1):
            return False
    elif obj["block_index"] is not None or obj["delta"] is not None:
        return False
    if event in ("stop_requested", "cancelled") and not obj["stop_requested"]:
        return False
    if event == "started" and obj["stop_requested"]:
        return False
    usage = obj["usage"]
    if usage is None:
        if event == "usage":
            return False
    else:
        if event in ("started", "text_delta", "stop_requested") or not isinstance(usage, dict) or set(usage) != set("input_tokens output_tokens cache_read_tokens cache_write_tokens reasoning_tokens".split()):
            return False
        if not all(v is None or integer(v) for v in usage.values()):
            return False
    reason = obj["stop_reason"]
    reasons = {"completed": {"end_turn", "max_tokens", "tool_use", "stop_sequence", "refusal"}, "failed": {"error"}, "unknown": {"unknown"}, "cancelled": {"cancelled"}}
    if event in reasons:
        if not isinstance(reason, str) or reason not in reasons[event]:
            return False
    elif reason is not None:
        return False
    return string(obj["error"], 512, 1) if event in ("failed", "unknown") else obj["error"] is None


class NativeString(ctypes.Structure):
    _fields_ = [("p", ctypes.c_char_p), ("n", ctypes.c_int64)]


lib = ctypes.CDLL(sys.argv[1])
native = lib.ct_oracle_accept
native.argtypes = [ctypes.c_int64, NativeString]
native.restype = ctypes.c_int64
count = 0


def check(kind, obj):
    global count
    raw = json.dumps(obj, ensure_ascii=False, separators=(",", ":")).encode("utf-8")
    expected = reference(kind, obj) and len(raw) <= {3: 1024, 4: 2048, 5: 4096, 6: 2048}.get(kind, 131072)
    observed = native(kind, NativeString(raw, len(raw))) == 1
    if observed != expected:
        raise AssertionError((kind, "expected", expected, "observed", observed, repr(raw[:600])))
    count += 1


submit = dict(schema="openagents.chat.submit.v2alpha1", operation_id="op", conversation_id="", expected_revision=0, project_id=None, expected_project_revision=None, legacy_thread_id=None, legacy_thread_revision=None, content=[dict(type="text", text="Hello café")])
stop = dict(schema="openagents.chat.stop.v2alpha1", operation_id="stop", turn_id=TURN)
event = dict(schema="openagents.chat.event.v2alpha1", conversation_id=CONVERSATION, turn_id=TURN, sequence=1, type="text_delta", state="running", stop_requested=False, block_index=0, delta="Hello café", usage=None, stop_reason=None, error=None)
control = dict(schema="openagents.chat.control.v2alpha1", conversation_id=CONVERSATION, turn_id=TURN, conversation_revision=1, state="queued", stop_requested=False, event_cursor=0)
runner = dict(schema="openagents.chat.runner.v2alpha1", operation="run", operation_id="claim-1", worker_id="worker-1", turn_id=TURN, dispatch_id="run_0123456789abcdef0123456789abcdef")
job = dict(schema="openagents.chat.job.v2alpha1", has_work=True, turn_id=TURN, conversation_id=CONVERSATION, dispatch_id="run_0123456789abcdef0123456789abcdef")
scalars = [None, False, True, -1, 0, 1, LIMIT, LIMIT + 1, 1.5, "", "x", "x\0y", "café", {}, []]
for kind, base in [(1, submit), (2, event), (3, stop), (4, control), (5, runner), (6, job)]:
    check(kind, base)
    for key in base:
        for value in scalars:
            changed = copy.deepcopy(base)
            changed[key] = value
            check(kind, changed)
        missing = copy.deepcopy(base)
        del missing[key]
        check(kind, missing)
    extra = copy.deepcopy(base)
    extra["extra"] = True
    check(kind, extra)
for field, prefix, kind, base in [("conversation_id", "cnv_", 2, event), ("turn_id", "trn_", 2, event), ("turn_id", "trn_", 3, stop)]:
    for index in range(32):
        for c in "0123456789abcdef:AG/g":
            changed = copy.deepcopy(base)
            digits = list("0" * 32)
            digits[index] = c
            changed[field] = prefix + "".join(digits)
            check(kind, changed)
    for sample in [
        "550e8400-e29b-41d4-a716-446655440000",
        "550E8400-e29b-41d4-a716-446655440000",
        "550e84000e29b-41d4-a716-446655440000",
        "550e8400-e29b041d4-a716-446655440000",
        "550e8400-e29b-41d40a716-446655440000",
        "550e8400-e29b-41d4-a7160446655440000",
        "550e8400-e29b-41d4-a716-44665544000",
        "550e8400-e29b-41d4-a716-4466554400000",
    ]:
        changed = copy.deepcopy(base)
        changed[field] = sample
        check(kind, changed)
for size in [0, 1, 80, 81]:
    for text in ["x" * size, "é" * size]:
        for kind, base in [(1, submit), (3, stop)]:
            changed = copy.deepcopy(base)
            changed["operation_id"] = text
            check(kind, changed)
for size in [0, 1, 16384, 16385]:
    changed = copy.deepcopy(submit)
    changed["content"] = [dict(type="text", text="x" * size)]
    check(1, changed)
for blocks in [0, 1, 16, 17]:
    changed = copy.deepcopy(submit)
    changed["content"] = [dict(type="text", text="x")] * blocks
    check(1, changed)
for name, state, reason, error in [("started", "running", None, None), ("usage", "running", None, None), ("stop_requested", "running", None, None), ("completed", "completed", "end_turn", None), ("failed", "failed", "error", "E"), ("unknown", "unknown", "unknown", "E"), ("cancelled", "cancelled", "cancelled", None)]:
    changed = copy.deepcopy(event)
    changed.update(type=name, state=state, stop_reason=reason, error=error, delta=None, block_index=None, stop_requested=name in ("stop_requested", "cancelled"))
    if name == "usage":
        changed["usage"] = dict(input_tokens=None, output_tokens=0, cache_read_tokens=None, cache_write_tokens=None, reasoning_tokens=None)
    check(2, changed)
    if name == "usage":
        for key in changed["usage"]:
            for value in scalars:
                candidate = copy.deepcopy(changed)
                candidate["usage"][key] = value
                check(2, candidate)
for state in ("queued", "running", "completed", "failed", "cancelled", "unknown", "invalid"):
    for stopped in (False, True):
        for cursor in (0, 1, LIMIT, LIMIT + 1):
            changed = dict(control, state=state, stop_requested=stopped, event_cursor=cursor)
            check(4, changed)
check(5, dict(runner, operation="next", turn_id=None, dispatch_id=None))
check(5, dict(runner, operation="lost"))
check(6, dict(job, has_work=False, turn_id=None, conversation_id=None, dispatch_id=None))
print(f"durable-text independent native oracle: {count} cases, no disagreement")
