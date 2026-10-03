#!/usr/bin/env python3
"""Create an offline schedule and validate registration; never dispatch execution."""
from __future__ import annotations

import argparse
import json
from pathlib import Path
import random
import re
import uuid

from report import artifact, digest, number, token, price_bounds, PRIMARY_MODELS, PREPARATION_BINDINGS
from seed_manifest import cargo_features, feature_check_command

SCHEMA = "openagents.delegation.registration.v1"
GLOBAL_ROLES = ("preparer", "syntax_binary", "native_cli", "broker", "sandbox", "native_runner",
                "reporter", "candidate_validator", "dispatch_coordinator", "acceptance_coordinator", "trial_config", "harness_manifest")
TASK_ROLES = ("source_archive", "issue", "index", "base_prompt", "instructions", "checker",
              "verifier", "calibration", "target_seed_manifest")
HARNESS_MODULES = {'run_remote.py','run_inner.py','bridge.py','broker.py','candidate.py','capture_limits.py',
                   'seed_manifest.py','check_candidate.py','trial.py','schedule.py','prepare.py','report.py'}
HARNESS_ROLES = {'native_runner':'run_remote.py','broker':'broker.py','candidate_validator':'candidate.py',
                 'acceptance_coordinator':'check_candidate.py','dispatch_coordinator':'trial.py',
                 'preparer':'prepare.py','reporter':'report.py'}


def validate_harness(root, registration):
    """Bind the executable scripts and their complete current local import set."""
    manifest = artifact(root,registration['artifacts']['harness_manifest'])
    files = manifest['files']
    if manifest.get('schema') != 'openagents.delegation.harness-files.v1' or set(files) != HARNESS_MODULES:
        raise ValueError('The harness module set differs')
    parents = set()
    for name, ref in files.items():
        artifact(root,ref,'binary')
        path = (root/ref['path']).resolve()
        if path.name != name:
            raise ValueError('A harness module has the wrong filename')
        parents.add(path.parent)
    if len(parents) != 1:
        raise ValueError('Local harness imports must share their frozen directory')
    for role, name in HARNESS_ROLES.items():
        if registration['artifacts'][role] != files[name]:
            raise ValueError('An executable differs from its harness module')
    return files


def make_schedule(study_id, task_ids, seed):
    """Return eight complete randomized blocks with stable unique attempt IDs."""
    namespace = uuid.UUID(study_id)
    if (len(task_ids) != 4 or len(set(task_ids)) != 4
            or not all(isinstance(t, str) and t for t in task_ids) or type(seed) is not int):
        raise ValueError("Supply four unique task IDs and an integer seed")
    rng = random.Random(seed)
    blocks = [(t, r) for t in task_ids for r in (1, 2)]
    rng.shuffle(blocks)
    rows = []
    for task, repetition in blocks:
        arms = list("ABCDEF")
        rng.shuffle(arms)
        block = str(uuid.uuid5(namespace, json.dumps([task, repetition])))
        for arm in arms:
            rows.append({"run_id": str(uuid.uuid5(namespace, json.dumps([task, repetition, arm]))),
                         "task_id": task, "arm": arm, "repetition": repetition, "block": block})
    return rows


def admission(ledger, preparation_reserve_usd):
    """Reserve one full block; unknown or duplicated liabilities stop admission."""
    if not number(preparation_reserve_usd):
        raise ValueError("Preparation reserve must be finite and nonnegative")
    seen, total, errors = set(), 0.0, []
    for row in ledger:
        identity, upper = row.get("run_id"), row.get("cost_upper_usd")
        if not isinstance(identity, str) or not identity or identity in seen:
            errors.append("missing or duplicate accounting identity")
        seen.add(identity)
        if not number(upper):
            errors.append("unbounded accounting liability")
        else:
            total += upper
        pending = row.get("unresolved_reservation_usd", 0)
        if not number(pending):
            errors.append("invalid unresolved reservation")
        else:
            total += pending
    reserve = 48.0 + preparation_reserve_usd
    return {"admissible": not errors and total + reserve <= 120.0,
            "accounted_and_reserved_upper_usd": total if not errors else None,
            "next_whole_block_reserve_usd": reserve, "ceiling_usd": 120.0, "errors": errors}


def validate(path):
    root = path.resolve().parent
    registration = json.loads(path.read_text())
    errors = []
    if registration.get("schema") != SCHEMA or registration.get("sealed") is not True:
        errors.append("registration is not explicitly sealed")
    try:
        protocol = artifact(root, registration["protocol"])
        if protocol["registration"]["sealed"] is not True or protocol.get("scored_execution_allowed") is not True:
            errors.append("protocol does not authorize scored execution")
        bindings = protocol["registration"]["report_bindings"]
        if not isinstance(bindings, dict):
            raise ValueError("Missing report bindings")
        expected = make_schedule(registration["study_id"], [t["task_id"] for t in bindings["tasks"]], registration["seed"])
        if registration.get("schedule") != expected or bindings.get("schedule") != expected:
            errors.append("schedule differs from fixed randomized complete blocks")
        if protocol["design"] != {**protocol["design"], "tasks": 4, "core_arms": 6, "repetitions": 2, "sessions": 48}:
            errors.append("protocol design differs from the fixed panel")
        loaded = {}
        for role in GLOBAL_ROLES:
            ref = registration.get("artifacts", {}).get(role)
            artifact(root, ref, "binary")
            loaded[role] = ref["sha256"]
        validate_harness(root,registration)
        if bindings["cli"]["sha256"] != loaded["native_cli"] or not bindings["cli"].get("version"):
            errors.append("CLI identity differs from bound executable")
        for planned in protocol["arms"]:
            arm = bindings["arms"][planned["id"]]
            model = PRIMARY_MODELS[planned["model_family"]]
            if (arm["primary_model"] != model or model not in arm["allowed_models"] or arm["effort"] != "medium"
                    or model not in bindings["prices"] or model not in protocol["executor_prices"]
                    or bindings["prices"].get(model) != protocol["executor_prices"].get(model)
                    or not arm.get("argv_tail")):
                errors.append("registered model, price, effort, or tool arguments differ")
            for allowed_model in arm["allowed_models"]:
                price_bounds({"input_tokens": 0, "output_tokens": 0}, bindings["prices"][allowed_model])
        for task in bindings["tasks"]:
            refs = registration["task_artifacts"][task["task_id"]]
            for role in TASK_ROLES:
                artifact(root, refs[role], "source_archive" if role == "source_archive" else "binary")
            prep = task["preparation"]
            if (not isinstance(task.get("source_commit"), str) or not re.fullmatch(r"[0-9a-f]{40}", task["source_commit"])
                    or any(k not in prep for k in PREPARATION_BINDINGS)
                    or not all(digest(prep[k]) for k in PREPARATION_BINDINGS if k.endswith("sha256"))
                    or prep["source_commit"] != task["source_commit"]
                    or prep["policy"] != protocol["source_pool"]["policy"]
                    or type(prep["candidate_units"]) is not int or not 0 <= prep["candidate_units"] <= 32):
                errors.append("task preparation bindings missing or inconsistent")
            for role, wanted in (("source_archive", task["source_archive_sha256"]),
                                 ("issue", prep["issue_sha256"]), ("index", prep["index_sha256"]),
                                 ("base_prompt", task["base_prompt"]["sha256"])):
                if refs[role]["sha256"] != wanted:
                    errors.append("task artifact differs from report binding: " + role)
            features = cargo_features(task.get('cargo_features',[]))
            base_prompt = artifact(root, task["base_prompt"], "bytes")
            if features and feature_check_command(features).encode() not in base_prompt:
                errors.append('task prompt lacks its bound Cargo feature check command')
            if prep["script_sha256"] != loaded["preparer"]:
                errors.append("preparer differs from frozen input policy")
        receipts = registration["preflights"]
        jev = artifact(root, receipts["jev"])
        if (jev.get("transport") != "live" or jev.get("outcome") != "answered"
                or jev.get("selection") != "system_one" or jev.get("attempts") != 1
                or jev.get("requested_model") != "jev-1.13.0" or jev.get("served_model") != "jev-1.13.0"
                or not token(jev.get("usage", {}).get("input_tokens")) or not number(jev.get("cost_usd"))
                or jev.get("preparer_sha256") != loaded["preparer"]):
            errors.append("successful real bound Jev preflight missing")
        elif abs(jev["cost_usd"] - jev["usage"]["input_tokens"] * .042 / 1e6) > 1e-9:
            errors.append("Jev preflight cost differs from the frozen price")
        for name in ("isolation", "native_opus", "native_sonnet"):
            receipt = artifact(root, receipts[name])
            if receipt.get("passed") is not True or receipt.get("infrastructure_sha256") != loaded:
                errors.append("infrastructure preflight missing or for different artifacts: " + name)
            if name != "isolation" and (receipt.get("transport") != "live"
                    or receipt.get("model") != ("claude-opus-5-5" if name == "native_opus" else "claude-sonnet-5-5")):
                errors.append("native capability was not established with the registered model")
        for name in ("paid_dispatch", "final_acceptance"):
            if registration.get("integration", {}).get(name) is not True:
                errors.append(name + " integration remains incomplete")
    except (ValueError, KeyError, TypeError, OSError, AttributeError):
        errors.append("required bound protocol, artifact, or preflight is unavailable")
    try:
        budget = admission(registration.get("accounting", []), registration["next_block_preparation_reserve_usd"])
        errors.extend(budget["errors"])
        if not budget["admissible"]:
            errors.append("full next block cannot be admitted within the accounting ceiling")
    except (ValueError, KeyError, TypeError, AttributeError):
        budget = None
        errors.append("accounting or preparation reservation is missing")
    return {"schema": "openagents.delegation.registration-validation.v1", "ready_for_external_dispatch": not errors,
            "executes_anything": False, "errors": errors, "admission": budget,
            "limitation": "Local hash and receipt validation; no paid dispatch or final acceptance coordinator is implemented by this command"}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    create = commands.add_parser("preview")
    create.add_argument("--study-id", required=True)
    create.add_argument("--task", action="append", required=True)
    create.add_argument("--seed", type=int, default=20261003)
    create.add_argument("--output", type=Path, required=True)
    check = commands.add_parser("validate")
    check.add_argument("--registration", type=Path, required=True)
    args = parser.parse_args()
    if args.command == "preview":
        rows = make_schedule(args.study_id, args.task, args.seed)
        with args.output.open("x") as output:
            json.dump({"draft_only": True, "schedule": rows}, output, indent=2)
            output.write("\n")
    else:
        result = validate(args.registration)
        print(json.dumps(result, indent=2))
        raise SystemExit(0 if result["ready_for_external_dispatch"] else 1)


if __name__ == "__main__":
    main()
