#!/usr/bin/env python3
"""Independently summarize bound delegation-study receipts without running tools."""
from __future__ import annotations

import argparse
import hashlib
import json
import math
from pathlib import Path
import random
import re
import statistics
import tarfile
import candidate

MANIFEST_SCHEMA = "openagents.delegation.report-manifest.v1"
REPORT_SCHEMA = "openagents.delegation.report.v1"
RATE_KEYS = ("input", "output", "cache_write_5m", "cache_write_1h", "cache_read")
NATIVE_BINDINGS = ("source_commit", "source_archive_sha256", "cli_sha256",
                   "cli_hash_after", "cli_version", "model", "effort", "prompt_sha256")
PRIMARY_MODELS = {"Opus 5.5": "claude-opus-5-5", "Sonnet 5.5": "claude-sonnet-5-5"}
PREPARATION_BINDINGS = ("source_commit", "policy", "issue_sha256", "index_sha256",
                        "script_sha256", "candidate_sha256")
ARTIFACT_LIMITS = {"json": 32 * 1024 * 1024, "text": 32 * 1024 * 1024,
                   "bytes": 32 * 1024 * 1024, "binary": 512 * 1024 * 1024,
                   "source_archive": 2 * 1024 * 1024 * 1024}


def number(value):
    return type(value) in (int, float) and math.isfinite(value) and value >= 0


def token(value):
    return type(value) is int and 0 <= value <= 100_000_000


def sha(data):
    return hashlib.sha256(data).hexdigest()


def digest(value):
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None


def artifact(root, ref, kind="json"):
    if kind not in ARTIFACT_LIMITS:
        raise ValueError("unknown artifact reader role")
    if not isinstance(ref, dict) or not isinstance(ref.get("path"), str):
        raise ValueError("missing artifact reference")
    relative = Path(ref["path"])
    if relative.is_absolute() or ".." in relative.parts:
        raise ValueError("artifact path leaves manifest directory")
    path = (root / relative).resolve()
    if root.resolve() not in path.parents or not path.is_file():
        raise ValueError("artifact missing or outside manifest directory")
    limit = ARTIFACT_LIMITS[kind]
    if path.stat().st_size > limit:
        raise ValueError("artifact exceeds reader bound")
    hasher = hashlib.sha256()
    chunks = []
    streamed = kind in ("binary", "source_archive")
    total = 0
    with path.open("rb") as source:
        while chunk := source.read(1024 * 1024):
            total += len(chunk)
            if total > limit:
                raise ValueError("artifact exceeds reader bound")
            hasher.update(chunk)
            if not streamed:
                chunks.append(chunk)
    if hasher.hexdigest() != ref.get("sha256"):
        raise ValueError("artifact digest mismatch")
    if streamed:
        return ref["sha256"]
    raw = b"".join(chunks)
    return json.loads(raw) if kind == "json" else raw if kind == "bytes" else raw.decode("utf-8")


def sealed_bindings(manifest, protocol):
    """Require an earlier registration instead of trusting run-supplied expectations."""
    frozen = protocol.get("registration", {}).get("report_bindings")
    errors = []
    if not isinstance(frozen, dict):
        return {}, ["sealed reporting bindings missing"]
    for key in ("arms", "prices", "tasks"):
        if not frozen.get(key) or manifest.get(key) != frozen[key]:
            errors.append(key + " differs from sealed registration")
    if (not isinstance(frozen.get("cli"), dict) or not digest(frozen["cli"].get("sha256"))
            or not isinstance(frozen["cli"].get("version"), str) or not frozen["cli"]["version"]):
        errors.append("sealed CLI identity missing")
    if not isinstance(frozen.get("schedule"), list) or not frozen["schedule"]:
        errors.append("sealed original schedule missing")
    else:
        originals = {r["run_id"]: r for r in frozen["schedule"]}
        if len(originals) != len(frozen["schedule"]):
            errors.append("sealed original schedule has duplicate identities")
        entries = {r["run_id"]: r for r in manifest["runs"]}
        if any(entries.get(r["run_id"], {}).get("category") != "replacement"
               and originals.get(r["run_id"]) != r for r in manifest["effective_schedule"]):
            errors.append("effective schedule differs from sealed original schedule")
        positions = ("task_id", "arm", "repetition", "block")
        if (len(frozen["schedule"]) != len(manifest["effective_schedule"])
                or any(any(a.get(k) != b.get(k) for k in positions)
                       for a, b in zip(frozen["schedule"], manifest["effective_schedule"]))):
            errors.append("effective schedule order differs from registration")
    return frozen, errors


def verify_input_bindings(root, refs, data, cell, arm, frozen):
    task = next(t for t in frozen["tasks"] if t["task_id"] == cell["task_id"])
    native = data["native"]
    expected = {"source_commit": task["source_commit"], "source_archive_sha256": task["source_archive_sha256"],
                "cli_sha256": frozen["cli"]["sha256"], "cli_hash_after": frozen["cli"]["sha256"],
                "cli_version": frozen["cli"]["version"], "model": arm["primary_model"], "effort": arm["effort"]}
    if any(native.get(k) != v for k, v in expected.items()):
        raise ValueError("native identity differs from sealed registration")
    argv = native.get("argv")
    if (not isinstance(argv, list) or not argv or not isinstance(arm.get("argv_tail"), list)
            or not arm["argv_tail"] or argv[1:] != arm["argv_tail"]):
        raise ValueError("native prompt or tool argv differs from registration")
    base = artifact(root, task["base_prompt"], "bytes")
    if arm["preparation"]:
        expected_prep = task["preparation"]
        prep = data["preparation"]
        if any(k not in expected_prep or prep.get(k) != expected_prep[k] for k in PREPARATION_BINDINGS):
            raise ValueError("preparation differs from sealed input or policy")
        count = prep.get("coverage", {}).get("candidate_units")
        if (not token(count) or count != expected_prep.get("candidate_units")
                or not digest(prep.get("candidate_sha256"))):
            raise ValueError("candidate pool identity or count missing")
        pool = artifact(root, refs["candidate_pool"])
        if (not isinstance(pool, list) or len(pool) != count
                or refs["candidate_pool"]["sha256"] != prep["candidate_sha256"]):
            raise ValueError("candidate pool artifact differs")
        pack = artifact(root, refs["briefing"], "bytes")
        if (len(pack) > 16 * 1024 or prep.get("briefing_sha256") != sha(pack)
                or prep.get("briefing_bytes") != len(pack)):
            raise ValueError("delivered source pack differs from preparation")
        system = artifact(root, refs["system_prompt"], "bytes")
        if not digest(arm.get("system_prompt_sha256")) or sha(system) != arm["system_prompt_sha256"]:
            raise ValueError("lean system prompt differs")
        expected_prompt = base + b"\n\n" + pack
    else:
        if "preparation" in data or arm.get("system_prompt_sha256") is not None:
            raise ValueError("native control has an experimental preparation")
        expected_prompt = base
    prompt = artifact(root, refs["prompt"], "bytes")
    delivered = ("Benchmark run ID: " + native["run_id"] + "\n\n").encode() + prompt
    if (prompt != expected_prompt or native.get("prompt_sha256") != sha(prompt)
            or native.get("delivered_prompt_sha256") != sha(delivered)):
        raise ValueError("executor prompt bytes differ from frozen composition")


def verify_candidate(root, refs, task):
    names = {"candidate_manifest": "candidate-manifest.json", "candidate_payload": "candidate.tar.gz",
             "candidate_changes": "changes.json"}
    paths = []
    for key, name in names.items():
        artifact(root, refs[key], "binary" if key == "candidate_payload" else "json")
        path = (root / refs[key]["path"]).resolve()
        if path.name != name:
            raise ValueError("candidate artifact name differs from contract")
        paths.append(path.parent)
    if len(set(paths)) != 1:
        raise ValueError("candidate artifacts must share a directory")
    identity = refs["candidate_manifest"]["sha256"]
    value = candidate.validate(paths[0], identity, task["source_commit"], task["source_archive_sha256"])
    if value["payload_sha256"] != refs["candidate_payload"]["sha256"]:
        raise ValueError("candidate payload identity differs")
    return identity


def price_bounds(usage, rates):
    """Unknown cache lifetime is an interval, never a reported point cost."""
    if not isinstance(usage, dict) or not isinstance(rates, dict):
        raise ValueError("usage or rates missing")
    if set(rates) != set(RATE_KEYS) or not all(number(v) for v in rates.values()):
        raise ValueError("invalid rates")
    keys = ("input_tokens", "output_tokens", "cache_creation_input_tokens",
            "cache_read_input_tokens", "cache_write_5m_tokens", "cache_write_1h_tokens")
    if any(k not in usage for k in keys[:2]):
        raise ValueError("input or output usage missing")
    counts = {k: usage.get(k, 0) for k in keys}
    if not all(token(v) for v in counts.values()):
        raise ValueError("invalid token count")
    unknown = counts[keys[2]] - counts[keys[4]] - counts[keys[5]]
    if unknown < 0:
        raise ValueError("inconsistent cache token count")
    known = math.fsum((counts[keys[0]] * rates["input"],
                       counts[keys[1]] * rates["output"],
                       counts[keys[3]] * rates["cache_read"],
                       counts[keys[4]] * rates["cache_write_5m"],
                       counts[keys[5]] * rates["cache_write_1h"]))
    low = (known + unknown * min(rates["cache_write_5m"], rates["cache_write_1h"])) / 1e6
    high = (known + unknown * max(rates["cache_write_5m"], rates["cache_write_1h"])) / 1e6
    return low, high, unknown


def provider_cost(text, run_id, allowed, prices):
    starts, finishes, errors, refused = {}, {}, [], 0
    if text and not text.endswith("\n"):
        errors.append("provider ledger has an incomplete final line")
    for line in text.splitlines():
        try:
            item = json.loads(line)
            if item.get("run_id") != run_id:
                raise ValueError("provider run identity mismatch")
            phase = item.get("phase")
            if phase == "refused":
                refused += 1
                continue
            if phase not in ("admitted", "finished") or not isinstance(item.get("call_id"), str):
                raise ValueError("invalid provider phase or identity")
            target = starts if phase == "admitted" else finishes
            if item["call_id"] in target:
                raise ValueError("duplicate provider phase")
            target[item["call_id"]] = item
        except (ValueError, TypeError, AttributeError):
            errors.append("invalid or duplicate provider receipt")
    if finishes.keys() - starts.keys():
        errors.append("provider completion without admission")
    lower, upper, unknown_ttl, unknown_calls, served = [], [], 0, [], set()
    usage_totals = {}
    for call_id, start in starts.items():
        model = start.get("model")
        finish = finishes.get(call_id)
        try:
            if model not in allowed or model not in prices:
                raise ValueError("unregistered provider model")
            if not isinstance(finish, dict):
                raise ValueError("unfinished provider request")
            if start.get("path", "").split("?", 1)[0] == "/v1/messages/count_tokens":
                if finish.get("usage_status") != "token_count_only" or finish.get("cost_usd") != 0:
                    raise ValueError("unconfirmed token-count outcome")
                lower.append(0.0)
                upper.append(0.0)
                continue
            if start.get("path", "").split("?", 1)[0] != "/v1/messages":
                raise ValueError("unexpected inference endpoint")
            if (finish.get("status") != "complete" or finish.get("http_status") != 200
                    or finish.get("usage_status") != "reported" or finish.get("served_model") != model):
                raise ValueError("unconfirmed inference outcome or served identity")
            lo, hi, uncertain = price_bounds(finish.get("usage"), prices[model])
            reported = finish.get("cost_usd")
            if not number(reported) or not lo - 1e-9 <= reported <= hi + 1e-9:
                raise ValueError("provider cost disagrees with frozen rates")
            lower.append(lo)
            upper.append(hi)
            unknown_ttl += uncertain
            served.add(model)
            for key, value in finish["usage"].items():
                if token(value):
                    usage_totals[key] = usage_totals.get(key, 0) + value
        except (ValueError, TypeError):
            unknown_calls.append(call_id)
    complete = not errors and not unknown_calls
    return {"lower_usd": math.fsum(lower), "upper_usd": math.fsum(upper) if complete else None,
            "unknown_cache_ttl_tokens": unknown_ttl, "unknown_calls": len(unknown_calls),
            "admitted_calls": len(starts), "refused_calls": refused,
            "call_ids": sorted(starts), "served_models": sorted(served),
            "usage": usage_totals, "errors": errors,
            "first_admitted_unix": min((v["started_unix"] for v in starts.values() if number(v.get("started_unix"))), default=None)}


def preparation_cost(prep, protocol, expected_semantic):
    call = prep.get("system_one")
    if not expected_semantic:
        if call is not None or prep.get("mode") != "deterministic":
            raise ValueError("unexpected System One call in deterministic arm")
        return 0.0, 0.0, False
    if prep.get("mode") != "jev":
        raise ValueError("System One assignment changed")
    if call is None:
        if prep.get("coverage", {}).get("candidate_units") == 0:
            return 0.0, 0.0, False
        raise ValueError("missing System One receipt")
    if call.get("attempts") not in (0, 1) or type(call.get("attempts")) is not int:
        raise ValueError("System One call count differs from policy")
    if call.get("attempts") == 0 and call.get("cost_status") == "no_call":
        return 0.0, 0.0, False
    model = protocol["system_one"]["requested_model"]
    if call.get("requested_model") != model:
        raise ValueError("System One requested identity changed")
    usage = call.get("usage")
    count = usage.get("input_tokens") if isinstance(usage, dict) else None
    if call.get("served_model") != model or not token(count):
        return 0.0, None, False
    rate = protocol["system_one"]["price"]["usd_per_million_input_tokens"]
    if not number(rate):
        raise ValueError("invalid System One rate")
    value = count * rate / 1e6
    if not number(call.get("cost_usd")) or abs(call["cost_usd"] - value) > 1e-9:
        raise ValueError("System One cost disagrees with frozen rate")
    return value, value, call.get("selection") == "system_one" and call.get("outcome") == "answered"


def read_run(root, entry, cell, manifest, protocol):
    run_id = entry["run_id"]
    errors, observations, data = [], [], {}
    refs = entry.get("artifacts", {})
    for name, kind in (("native", "json"), ("provider_calls", "text"), ("preparation", "json"),
                       ("candidate", "binary"), ("checks", "json"), ("endpoint", "json"), ("review", "json")):
        if name not in refs:
            continue
        try:
            value = artifact(root, refs[name], kind)
            if kind == "json" and not isinstance(value, dict):
                raise ValueError("receipt must be an object")
            data[name] = value
        except (ValueError, OSError, UnicodeError, TypeError):
            errors.append(name + " artifact unavailable or invalid")
    arm = manifest["arms"].get(cell["arm"] if cell else entry.get("arm"), entry.get("arm_config", {}))
    native = data.get("native", {})
    allowed = arm.get("allowed_models", entry.get("allowed_models", []))
    if "provider_calls" in data:
        costs = provider_cost(data["provider_calls"], run_id, allowed, manifest["prices"])
        errors.extend(costs.pop("errors"))
    else:
        costs = {"lower_usd": 0.0, "upper_usd": None, "unknown_cache_ttl_tokens": 0,
                 "unknown_calls": 0, "admitted_calls": 0, "refused_calls": 0,
                 "call_ids": [], "served_models": [], "usage": {}}
        # A receipt explicitly limited to preparation has no native inference.
        if entry.get("category") == "preparation_probe" and "preparation" in data:
            costs["upper_usd"] = 0.0
        elif cell:
            errors.append("provider ledger missing")
    jev_lower, jev_upper, delivered = 0.0, 0.0, False
    requires_prep = bool(arm.get("preparation"))
    if "preparation" in data:
        prep = data["preparation"]
        try:
            jev_lower, jev_upper, delivered = preparation_cost(
                prep, protocol, bool(arm.get("system_one", entry.get("system_one", False))))
            for key, value in entry.get("bindings", {}).get("preparation", {}).items():
                if value is None or prep.get(key) != value:
                    raise ValueError("preparation binding mismatch")
            if cell and prep.get("source_commit") != manifest["task_sources"].get(cell["task_id"]):
                raise ValueError("preparation source commit mismatch")
        except (ValueError, KeyError, TypeError, AttributeError):
            jev_upper = None
            errors.append("preparation identity, assignment, or accounting invalid")
    elif requires_prep:
        jev_upper = None
        errors.append("required preparation missing")
    low = costs["lower_usd"] + jev_lower
    high = costs["upper_usd"] + jev_upper if costs["upper_usd"] is not None and jev_upper is not None else None
    accepted, reviewed, wall = None, None, None
    if cell:
        try:
            verify_input_bindings(root, refs, data, cell, arm, manifest["frozen_bindings"])
            expected = entry.get("bindings", {}).get("native", {})
            if (any(k not in expected for k in NATIVE_BINDINGS)
                    or any(value is None or native.get(key) != value for key, value in expected.items())
                    or native.get("cli_sha256") != native.get("cli_hash_after")):
                raise ValueError("native binding missing or changed")
            if (native.get("run_id") != run_id or native.get("model") != arm.get("primary_model")
                    or native.get("effort") != arm.get("effort")
                    or native.get("source_commit") != manifest["task_sources"].get(cell["task_id"])):
                raise ValueError("native source, model, effort, or run identity changed")
            if not set(native.get("served_models") or []).issubset(set(allowed)):
                raise ValueError("native reports an unregistered auxiliary model")
            task = next(t for t in manifest["frozen_bindings"]["tasks"] if t["task_id"] == cell["task_id"])
            candidate_identity = verify_candidate(root, refs, task)
            if native.get("candidate_manifest_sha256") != candidate_identity:
                raise ValueError("native final candidate identity differs")
            endpoint, checks, review = (data.get(k, {}) for k in ("endpoint", "checks", "review"))
            for value, schema in ((endpoint, "endpoint"), (checks, "final-checks")):
                if (value.get("schema") != "openagents.delegation." + schema + ".v1"
                        or value.get("run_id") != run_id
                        or value.get("candidate_manifest_sha256") != candidate_identity):
                    raise ValueError("final candidate receipt binding mismatch")
            start, end = endpoint.get("start_monotonic_ns"), endpoint.get("end_monotonic_ns")
            if (type(start) is not int or type(end) is not int or start < 0 or end < start
                    or not isinstance(endpoint.get("clock_id"), str) or not endpoint["clock_id"]
                    or endpoint.get("execution_closed") is not True):
                raise ValueError("endpoint clock or closed execution missing")
            wall = (end - start) / 1e9
            values = [checks.get(k, {}).get("passed") for k in ("scope", "format", "ordinary", "independent")]
            if (checks.get("completed") is not True or checks.get('execution_closed') is not True
                    or any(type(x) is not bool for x in values)):
                raise ValueError("final checks incomplete")
            # A budget-ended native session can produce an accepted final patch.
            accepted = all(values)
            if (review.get("schema") != "openagents.delegation.review.v1" or review.get("run_id") != run_id
                    or review.get("candidate_manifest_sha256") != candidate_identity
                    or review.get("completed") is not True or review.get("labels_hidden_at_judgment") is not True
                    or type(review.get("material_defect")) is not bool):
                raise ValueError("completed blinded review missing")
            reviewed = not review["material_defect"]
        except (ValueError, TypeError, AttributeError, KeyError, StopIteration, OSError, tarfile.TarError, EOFError):
            errors.append("final source, execution, check, or blinded-review binding invalid")
    if number(native.get("cost_usd")) and costs["upper_usd"] is not None:
        observations.append({"native_cumulative_usd": native["cost_usd"],
                             "provider_lower_usd": costs["lower_usd"], "provider_upper_usd": costs["upper_usd"],
                             "native_inside_provider_bounds": costs["lower_usd"] - 1e-9 <= native["cost_usd"] <= costs["upper_usd"] + 1e-9,
                             "treatment": "cross_check_only_never_added"})
        if native["cost_usd"] > costs["upper_usd"] + 1e-9:
            # A direct shell call can explain broker > native, never the reverse.
            errors.append("native cumulative cost exceeds broker upper bound")
            high = None
    semantic = bool(arm.get("system_one", entry.get("system_one", False)))
    prep = data.get("preparation", {})
    delivery = ("delivered" if delivered else "empty_pool" if prep.get("coverage", {}).get("candidate_units") == 0
                else "fallback" if (prep.get("system_one") or {}).get("selection") == "deterministic_fallback"
                else "not_delivered") if semantic else "not_assigned"
    return {"run_id": run_id, "category": entry.get("category", "unspecified"),
            "cell": cell, "cost_lower_usd": low, "cost_upper_usd": high,
            "cost_usd": low if high is not None and abs(high - low) < 1e-12 else None,
            "provider": costs, "system_one_assigned": semantic, "system_one_delivered": delivered,
            "system_one_delivery": delivery, "system_one_cost_lower_usd": jev_lower,
            "system_one_cost_upper_usd": jev_upper, "accepted": accepted, "review_passed": reviewed,
            "wall_s": wall, "cli_completed": native.get("model_completed"),
            "cli_exit_code": native.get("exit_code"), "cli_timed_out": native.get("timed_out", False),
            "observations": observations, "errors": errors,
            "candidate_pool_sha256": prep.get("candidate_sha256"), "briefing_sha256": prep.get("briefing_sha256")}


def ratio(a, b):
    return a / b if a is not None and b is not None and b > 0 else None


def mean(values):
    return math.fsum(values) / len(values) if values and all(v is not None for v in values) else None


def quantile(values, p):
    if not values:
        return None
    ordered = sorted(values)
    position = (len(ordered) - 1) * p
    lo = math.floor(position)
    hi = math.ceil(position)
    return ordered[lo] + (ordered[hi] - ordered[lo]) * (position - lo)


def geometric(values):
    if not values or any(v is None or v <= 0 for v in values):
        return None
    return math.exp(math.fsum(math.log(v) for v in values) / len(values))


def summarize(rows):
    low = math.fsum(r["cost_lower_usd"] for r in rows)
    high = math.fsum(r["cost_upper_usd"] for r in rows) if all(r["cost_upper_usd"] is not None for r in rows) else None
    walls = [r["wall_s"] for r in rows]
    passed = sum(r["accepted"] is True for r in rows)
    point = low if high is not None and abs(high - low) < 1e-12 else None
    return {"assigned": len(rows), "accepted": passed, "failed": sum(r["accepted"] is False for r in rows),
            "system_one_assigned": sum(r.get("system_one_assigned", False) for r in rows),
            "system_one_delivered": sum(r.get("system_one_delivered", False) for r in rows),
            "system_one_fallback": sum(r.get("system_one_delivery") == "fallback" for r in rows),
            "system_one_empty_pool": sum(r.get("system_one_delivery") == "empty_pool" for r in rows),
            "system_one_not_delivered": sum(r.get("system_one_delivery") == "not_delivered" for r in rows),
            "acceptance_unknown": sum(r["accepted"] is None for r in rows),
            "review_failures": sum(r["review_passed"] is False for r in rows),
            "cost_lower_usd": low, "cost_upper_usd": high, "cost_usd": point,
            "mean_cost_lower_usd": low / len(rows) if rows else None,
            "mean_cost_upper_usd": high / len(rows) if rows and high is not None else None,
            "mean_wall_s": mean(walls), "summed_wall_s": math.fsum(walls) if all(v is not None for v in walls) else None,
            "median_wall_s": statistics.median(walls) if walls and all(v is not None for v in walls) else None,
            "cost_per_accepted_lower_usd": low / passed if passed else None,
            "cost_per_accepted_upper_usd": high / passed if passed and high is not None else None}


def bootstrap(pairs, draws, seed):
    rng = random.Random(seed)
    values = {"cost_ratio_lower": [], "cost_ratio_upper": [], "time_ratio": []}
    specifications = (("cost_ratio_lower", "cost_lower_usd", "cost_upper_usd"),
                      ("cost_ratio_upper", "cost_upper_usd", "cost_lower_usd"),
                      ("time_ratio", "wall_s", "wall_s"))
    eligible = {key: all(a[first] is not None and b[second] is not None and b[second] > 0
                         for group in pairs for a, b in group)
                for key, first, second in specifications}
    for _ in range(draws):
        treated, controls = [], []
        for _ in pairs:
            repetitions = pairs[rng.randrange(len(pairs))]
            for _ in repetitions:
                a, b = repetitions[rng.randrange(len(repetitions))]
                treated.append(a)
                controls.append(b)
        for key, a_field, b_field in specifications:
            if not eligible[key]:
                continue
            value = ratio(mean([r[a_field] for r in treated]), mean([r[b_field] for r in controls]))
            if value is not None:
                values[key].append(value)
    return {"draws": draws, "seed": seed, "interpretation": "descriptive_four_task_clusters_no_familywise_claim",
            "intervals": {k: {"p025": quantile(v, .025), "p975": quantile(v, .975), "defined_draws": len(v)}
                          for k, v in values.items()}}


def comparison(treatment, control, by_cell, tasks, repetitions, rule, draws, seed, global_errors):
    per_task, pairs, paired = [], [], []
    for task in tasks:
        values = [(by_cell[(task, treatment, rep)], by_cell[(task, control, rep)]) for rep in repetitions]
        pairs.append(values)
        a, b = summarize([x[0] for x in values]), summarize([x[1] for x in values])
        per_task.append({"task_id": task,
                         "cost_ratio_lower": ratio(a["cost_lower_usd"], b["cost_upper_usd"]),
                         "cost_ratio_upper": ratio(a["cost_upper_usd"], b["cost_lower_usd"]),
                         "time_ratio": ratio(a["mean_wall_s"], b["mean_wall_s"]),
                         "treatment": a, "control": b})
        for rep, (first, second) in zip(repetitions, values):
            paired.append({"task_id": task, "repetition": rep,
                           "cost_ratio_lower": ratio(first["cost_lower_usd"], second["cost_upper_usd"]),
                           "cost_ratio_upper": ratio(first["cost_upper_usd"], second["cost_lower_usd"]),
                           "time_ratio": ratio(first["wall_s"], second["wall_s"]),
                           "time_difference_s": first["wall_s"] - second["wall_s"] if first["wall_s"] is not None and second["wall_s"] is not None else None})
    ar = [p[0] for group in pairs for p in group]
    br = [p[1] for group in pairs for p in group]
    a, b = summarize(ar), summarize(br)
    lo, hi = ratio(a["cost_lower_usd"], b["cost_upper_usd"]), ratio(a["cost_upper_usd"], b["cost_lower_usd"])
    wall = ratio(a["mean_wall_s"], b["mean_wall_s"])
    cost_wins = sum(t["cost_ratio_upper"] is not None and t["cost_ratio_upper"] < 1 for t in per_task)
    possible_cost_wins = sum(t["cost_ratio_lower"] is None or t["cost_ratio_lower"] < 1 for t in per_task)
    time_wins = sum(t["time_ratio"] is not None and t["time_ratio"] < 1 for t in per_task)
    reasons, states = [], []
    delivery = {"assigned": a["system_one_assigned"], "delivered": a["system_one_delivered"],
                "fallback": a["system_one_fallback"], "empty_pool": a["system_one_empty_pool"],
                "changed_packs": sum(first.get("system_one_delivered", False)
                                     and first.get("briefing_sha256") != second.get("briefing_sha256")
                                     for first, second in [p for group in pairs for p in group])}
    if treatment + "/" + control in ("C/B", "F/E") and (not delivery["delivered"] or not delivery["changed_packs"]):
        states.append("not_evaluable")
        reasons.append("no delivered System One change to executor input; intention-to-treat outcomes retained")
    if global_errors or any(r["errors"] for r in ar + br):
        states.append("not_evaluable"); reasons.append("binding or manifest error")
    elif any(r["accepted"] is None or r["review_passed"] is None for r in ar + br):
        states.append("not_evaluable"); reasons.append("acceptance or blinded review incomplete")
    elif any(r["accepted"] is False or r["review_passed"] is False for r in ar + br):
        states.append("fail"); reasons.append("quality gate not met")
    if not rule.get("descriptive"):
        threshold = rule["cost_ratio_max"]
        states.append("pass" if hi is not None and hi <= threshold else "fail" if lo is not None and lo > threshold else "not_evaluable")
        states.append("pass" if wall is not None and wall <= rule["time_ratio_max"] else "fail" if wall is not None else "not_evaluable")
        states.append("pass" if cost_wins >= rule["cost_task_mean_wins_min"] else "fail" if possible_cost_wins < rule["cost_task_mean_wins_min"] else "not_evaluable")
        states.append("pass" if time_wins >= rule["time_task_mean_wins_min"] else "fail" if all(t["time_ratio"] is not None for t in per_task) else "not_evaluable")
    gate = "not_evaluable" if "not_evaluable" in states else "fail" if "fail" in states else "descriptive_only" if rule.get("descriptive") else "pass"
    return {"treatment": treatment, "control": control, "cost_ratio_lower": lo, "cost_ratio_upper": hi,
            "cost_ratio": lo if lo is not None and hi is not None and abs(lo - hi) < 1e-12 else None,
            "time_ratio": wall, "task_macro_geometric_cost_ratio_lower": geometric([t["cost_ratio_lower"] for t in per_task]),
            "task_macro_geometric_cost_ratio_upper": geometric([t["cost_ratio_upper"] for t in per_task]),
            "task_macro_geometric_time_ratio": geometric([t["time_ratio"] for t in per_task]),
            "confirmed_cost_task_wins": cost_wins, "possible_cost_task_wins": possible_cost_wins,
            "time_task_wins": time_wins, "gate": gate, "gate_reasons": reasons, "treatment_delivery": delivery,
            "per_task": per_task, "paired_repetitions": paired,
            "bootstrap": bootstrap(pairs, draws, seed)}


def replacement_errors(root, manifest, protocol, entries, rows, schedule):
    blocks = {}
    for run_id, cell in schedule.items():
        if entries.get(run_id, {}).get("category") == "replacement":
            blocks.setdefault(cell["block"], []).append((run_id, cell))
    if not blocks:
        return []
    errors, receipts = [], {}
    if len(blocks) > protocol.get("amendments", {}).get("max_post_scoring_infrastructure_replacement_blocks", 1):
        errors.append("too many replacement blocks")
    for ref in manifest.get("replacement_registrations", []):
        try:
            receipt = artifact(root, ref)
            if receipt["schema"] != "openagents.delegation.replacement.v1" or receipt["block_id"] in receipts:
                raise ValueError("replacement registration identity invalid")
            receipts[receipt["block_id"]] = receipt
        except (ValueError, KeyError, TypeError, OSError):
            errors.append("replacement registration unavailable or invalid")
    for block, values in blocks.items():
        receipt = receipts.get(block, {})
        try:
            if (len(values) != protocol["design"]["core_arms"]
                    or {c["arm"] for _, c in values} != set(manifest["arms"])
                    or len({(c["task_id"], c["repetition"]) for _, c in values}) != 1):
                raise ValueError("replacement is not a complete matched block")
            if (receipt.get("previous_protocol_sha256") != manifest["protocol"]["sha256"]
                    or receipt.get("registered_before_launch") is not True
                    or receipt.get("reason_category") != "confirmed_infrastructure_defect"
                    or not number(receipt.get("registered_unix"))):
                raise ValueError("replacement is not prospectively registered")
            mappings = receipt.get("replacements", [])
            if len(mappings) != len(values) or {m["replacement_run_id"] for m in mappings} != {r for r, _ in values}:
                raise ValueError("replacement mapping differs from schedule")
            originals = set()
            for mapping in mappings:
                old, new = mapping["original_run_id"], mapping["replacement_run_id"]
                if old in originals or old not in entries or old in schedule or entries[old].get("arm") != schedule[new]["arm"]:
                    raise ValueError("original replacement block is not retained uniquely")
                originals.add(old)
                admitted = rows[new]["provider"].get("first_admitted_unix")
                if rows[new]["provider"]["admitted_calls"] and (admitted is None or admitted <= receipt["registered_unix"]):
                    raise ValueError("replacement inference precedes registration or lacks timestamp")
        except (ValueError, KeyError, TypeError):
            errors.append("replacement block lacks valid prospective whole-block lineage")
    return errors


def build(manifest_path, draws=None):
    root = manifest_path.resolve().parent
    raw = manifest_path.read_bytes()
    manifest = json.loads(raw)
    if manifest.get("schema") != MANIFEST_SCHEMA:
        raise ValueError("unexpected report manifest schema")
    protocol = artifact(root, manifest["protocol"])
    errors = []
    if not protocol.get("registration", {}).get("sealed") or protocol.get("scored_execution_allowed") is not True:
        errors.append("protocol is not sealed for scored execution")
    manifest["frozen_bindings"], binding_errors = sealed_bindings(manifest, protocol)
    errors.extend(binding_errors)
    tasks = [t["task_id"] for t in manifest["tasks"]]
    manifest["task_sources"] = {t["task_id"]: t["source_commit"] for t in manifest["tasks"]}
    arms = sorted(manifest["arms"])
    for planned in protocol["arms"]:
        arm = manifest["arms"].get(planned["id"], {})
        if (arm.get("effort") != planned["requested_effort"]
                or arm.get("primary_model") != PRIMARY_MODELS.get(planned.get("model_family"))
                or manifest["prices"].get(arm.get("primary_model")) != protocol.get("executor_prices", {}).get(arm.get("primary_model"))
                or arm.get("primary_model") not in protocol.get("executor_prices", {})
                or bool(arm.get("preparation")) != (planned["source_pack"] != "none")
                or bool(arm.get("system_one")) != (planned["system_one"] == "jev_rerank")
                or arm.get("primary_model") not in arm.get("allowed_models", [])):
            errors.append("arm configuration differs from protocol")
    reps = list(range(1, protocol["design"]["repetitions"] + 1))
    expected = {(t, a, r) for t in tasks for a in arms for r in reps}
    if (len(set(tasks)) != len(tasks) or len(tasks) != protocol["design"]["tasks"]
            or len(arms) != protocol["design"]["core_arms"]):
        errors.append("task or arm registration count mismatch")
    schedule, cells = {}, {}
    for cell in manifest["effective_schedule"]:
        key = (cell["task_id"], cell["arm"], cell["repetition"])
        if key in cells or cell["run_id"] in schedule or key not in expected:
            errors.append("duplicate or unexpected effective schedule entry")
            continue
        schedule[cell["run_id"]] = cell
        cells[key] = cell["run_id"]
    if set(cells) != expected:
        errors.append("effective schedule is incomplete")
    entries = {}
    for entry in manifest["runs"]:
        if entry["run_id"] in entries:
            errors.append("duplicate accounting run identity")
            continue
        entries[entry["run_id"]] = entry
    rows = {}
    call_owners = {}
    for run_id, entry in entries.items():
        row = read_run(root, entry, schedule.get(run_id), manifest, protocol)
        rows[run_id] = row
        for call_id in row["provider"]["call_ids"]:
            if call_id in call_owners:
                errors.append("provider call identity appears in multiple accounting runs")
                row["errors"].append("duplicate cross-run provider call")
                row["cost_upper_usd"] = None
                row["cost_usd"] = None
            call_owners[call_id] = run_id
    errors.extend(replacement_errors(root, manifest, protocol, entries, rows, schedule))
    by_cell = {}
    for key in sorted(expected):
        run_id = cells.get(key)
        if run_id not in rows:
            row = {"run_id": run_id, "cell": schedule.get(run_id), "cost_lower_usd": 0.0, "cost_upper_usd": None,
                   "cost_usd": None, "wall_s": None, "accepted": None, "review_passed": None,
                   "errors": ["assigned run missing"], "candidate_pool_sha256": None}
        else:
            row = rows[run_id]
        by_cell[key] = row
    for task in tasks:
        pools = {r["candidate_pool_sha256"] for (t, _, _), r in by_cell.items() if t == task and r.get("candidate_pool_sha256")}
        if len(pools) > 1:
            errors.append("candidate pool differs across prepared arms for task " + task)
    bootstrap_config = protocol["analysis"]["bootstrap"]
    count = bootstrap_config["draws"] if draws is None else draws
    comparisons = {}
    for name, original in protocol["comparisons"].items():
        rule = protocol["comparisons"][original["same_rule_as"]] if "same_rule_as" in original else original
        first, second = name.split("/")
        comparisons[name] = comparison(first, second, by_cell, tasks, reps, rule, count, bootstrap_config["seed"], errors)
    all_rows = list(rows.values())
    effective_rows = list(by_cell.values())
    total = summarize(all_rows)
    if any(run_id not in rows for run_id in schedule) or set(cells) != expected:
        total["cost_usd"] = total["cost_upper_usd"] = None
    if any("duplicate" in e for e in errors):
        # Ambiguous ledger identities cannot support a unique-spend total.
        total["cost_usd"] = total["cost_upper_usd"] = None
        total["cost_lower_usd"] = None
    required = [comparisons[c]["gate"] for c in protocol["combined_gate_requires"]]
    combined = "not_evaluable" if "not_evaluable" in required else "fail" if "fail" in required else "pass"
    combined_reasons = []
    if total["cost_upper_usd"] is None:
        combined = "not_evaluable"
        combined_reasons.append("complete program accounting is unbounded or ambiguous")
    result = {"schema": REPORT_SCHEMA, "manifest_sha256": sha(raw), "protocol_sha256": manifest["protocol"]["sha256"],
            "errors": errors, "complete_effective_data": not errors and all(not r["errors"] and r["cost_upper_usd"] is not None and r["accepted"] is not None and r["review_passed"] is not None for r in effective_rows),
            "program_accounting_bounded": total["cost_upper_usd"] is not None,
            "unique_accounting_runs": len(all_rows), "effective_assigned_runs": len(expected),
            "program_totals": total, "effective_totals": summarize(effective_rows),
            "arms": {a: summarize([r for (t, arm, rep), r in by_cell.items() if arm == a]) for a in arms},
            "comparisons": comparisons, "combined_gate": combined, "combined_gate_reasons": combined_reasons,
            "effective_rows": effective_rows, "accounting_rows": all_rows,
            "bootstrap_draws": count, "generalization": "four_selected_task_clusters; no broad repository or population claim"}
    result["labels_released"] = all(r["review_passed"] is not None for r in effective_rows)
    if not result["labels_released"]:
        # A partial report must not reveal candidate-to-arm assignments to a
        # reviewer who has not yet sealed the blinded judgment.
        result["arms"] = {}
        result["comparisons"] = {name: {"gate": "not_evaluable", "reason": "labels withheld pending bound blinded review"}
                                 for name in comparisons}
        result["combined_gate"] = "not_evaluable"
        for row in all_rows + effective_rows:
            row["cell"] = None
            if "provider" in row:
                row["provider"]["served_models"] = []
    return result


def markdown(result):
    lines = ["# Delegation study report", "", "Combined registered gate: **" + result["combined_gate"] + "**.", "",
             "Costs include all uniquely recorded runs. Unknown cache lifetimes remain cost bounds. Native cumulative estimates are cross-checks, not additional charges.", "",
             "| Arm | Accepted / assigned | Cost bounds (USD) | Mean endpoint (s) | System One delivered / assigned | Fallback / empty pool |", "| --- | ---: | ---: | ---: | ---: | ---: |"]
    def show(value):
        return "unknown" if value is None else f"{value:.6f}"
    for arm, value in result["arms"].items():
        lines.append(f"| {arm} | {value['accepted']}/{value['assigned']} | {show(value['cost_lower_usd'])}–{show(value['cost_upper_usd'])} | {show(value['mean_wall_s'])} | {value['system_one_delivered']}/{value['system_one_assigned']} | {value['system_one_fallback']}/{value['system_one_empty_pool']} |")
    lines += ["", "| Comparison | Cost-ratio bounds | Time ratio | Gate |", "| --- | ---: | ---: | --- |"]
    for name, value in result["comparisons"].items():
        lines.append(f"| {name} | {show(value.get('cost_ratio_lower'))}–{show(value.get('cost_ratio_upper'))} | {show(value.get('time_ratio'))} | {value['gate']} |")
    lines += ["", "Four task clusters provide a bounded engineering observation. Intervals and paired variability are descriptive; tied acceptance does not establish better correctness.", "",
              "See `metrics.json` for every row, failure, binding problem, paired result, bootstrap interval, and the complete accounting ledger.", ""]
    return "\n".join(lines)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    result = build(args.manifest)
    args.output.mkdir(parents=True, exist_ok=False)
    (args.output / "metrics.json").write_text(json.dumps(result, indent=2, allow_nan=False) + "\n")
    (args.output / "README.md").write_text(markdown(result))
    print(json.dumps({"combined_gate": result["combined_gate"], "unique_accounting_runs": result["unique_accounting_runs"]}))


if __name__ == "__main__":
    main()
