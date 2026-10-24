#!/usr/bin/env python3
"""Check the design corpus and bounded synthetic contracts, not engine behavior.

Standard library only. The schema evaluator intentionally implements only the
keywords used in this repository; use a full Draft 2020-12 validator for production.
"""
from collections import deque
from copy import deepcopy
from datetime import datetime, timedelta
from hashlib import sha256
from itertools import permutations
import json
import os
from pathlib import Path
import re
from urllib.parse import unquote, urlsplit

# The openagents repository root. The engine specification lives in
# docs/aethyr/engine; the code it describes is in apps/aethyr and crates/aethyr.
ROOT = Path(__file__).resolve().parents[3]
SPEC = ROOT / "docs/aethyr/engine"
SCANNED = [SPEC, ROOT / "apps/aethyr", ROOT / "crates/aethyr", ROOT / "scripts/deploy/aethyr"]


def repo_files(suffix):
    excluded = {".git", "node_modules", "dist", "target", "__pycache__", ".private-assets", "private-assets"}
    for top in SCANNED:
        for directory, names, files in os.walk(top):
            names[:] = sorted(name for name in names if name not in excluded)
            for name in sorted(files):
                if name.endswith(suffix):
                    yield Path(directory) / name


def require(condition, message):
    if not condition:
        raise AssertionError(message)


def unique_object(pairs):
    obj = {}
    for key, value in pairs:
        require(key not in obj, f"Duplicate JSON property: {key}")
        obj[key] = value
    return obj


def load(path):
    return json.loads(path.read_text(), object_pairs_hook=unique_object,
                      parse_constant=lambda x: require(False, f"Nonfinite JSON: {x}"))


def canonical_fixture(value):
    # This is deliberately not a general implementation of RFC 8785.
    def check(v):
        require(not isinstance(v, float), "JCS fixture subset excludes floats")
        if isinstance(v, str):
            require(v.isascii(), "JCS fixture subset requires ASCII strings")
        if isinstance(v, dict):
            for k, item in v.items():
                check(k)
                check(item)
        if isinstance(v, list):
            for item in v:
                check(item)
    check(value)
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()


def digest(value):
    return "sha256:" + sha256(canonical_fixture(value)).hexdigest()


def schema_check(value, rule, document, path="$"):
    if "$ref" in rule:
        ref = rule["$ref"]
        require(ref.startswith("#/"), f"Unsupported external schema ref: {ref}")
        resolved = document
        for key in ref[2:].split("/"):
            resolved = resolved[key.replace("~1", "/").replace("~0", "~")]
        schema_check(value, resolved, document, path)
    if "oneOf" in rule:
        matches = 0
        for option in rule["oneOf"]:
            try:
                schema_check(value, option, document, path)
                matches += 1
            except AssertionError:
                pass
        require(matches == 1, f"{path}: expected exactly one schema alternative, got {matches}")
    if "const" in rule:
        require(value == rule["const"] and type(value) == type(rule["const"]), f"{path}: invalid constant")
    if "enum" in rule:
        require(any(value == x and type(value) == type(x) for x in rule["enum"]), f"{path}: invalid enum")
    kinds = {"object": lambda x: isinstance(x, dict), "array": lambda x: isinstance(x, list),
             "string": lambda x: isinstance(x, str),
             "integer": lambda x: type(x) is int or (type(x) is float and x.is_integer()),
             "number": lambda x: type(x) in (int, float), "boolean": lambda x: type(x) is bool,
             "null": lambda x: x is None}
    if "type" in rule:
        require(kinds[rule["type"]](value), f"{path}: expected {rule['type']}")
    if isinstance(value, dict):
        for key in rule.get("required", []):
            require(key in value, f"{path}: missing {key}")
        props = rule.get("properties", {})
        if rule.get("additionalProperties") is False:
            require(set(value) <= set(props), f"{path}: unexpected field {set(value)-set(props)}")
        for key in set(value) & set(props):
            schema_check(value[key], props[key], document, f"{path}.{key}")
    if isinstance(value, list):
        require(len(value) >= rule.get("minItems", 0), f"{path}: too few items")
        require(len(value) <= rule.get("maxItems", 10**20), f"{path}: too many items")
        if rule.get("uniqueItems"):
            require(len({json.dumps(x, sort_keys=True) for x in value}) == len(value), f"{path}: duplicate item")
        if "items" in rule:
            for i, item in enumerate(value):
                schema_check(item, rule["items"], document, f"{path}[{i}]")
    if isinstance(value, str):
        require(len(value) >= rule.get("minLength", 0), f"{path}: string too short")
        require(len(value) <= rule.get("maxLength", 10**20), f"{path}: string too long")
        if "pattern" in rule:
            require(re.search(rule["pattern"], value) is not None, f"{path}: pattern mismatch")
    if type(value) in (int, float):
        require(value >= rule.get("minimum", -float("inf")), f"{path}: below minimum")
        require(value <= rule.get("maximum", float("inf")), f"{path}: above maximum")


def validate_schema_keywords(node):
    allowed = {"$schema", "$id", "$defs", "$ref", "title", "description", "type", "const", "enum",
               "required", "properties", "additionalProperties", "items", "minItems", "maxItems",
               "minimum", "maximum", "minLength", "maxLength", "pattern", "uniqueItems", "oneOf"}
    require(set(node) <= allowed, f"Unsupported schema keyword: {set(node)-allowed}")
    for container in ("properties", "$defs"):
        for child in node.get(container, {}).values():
            validate_schema_keywords(child)
    if "items" in node:
        validate_schema_keywords(node["items"])
    for child in node.get("oneOf", []):
        validate_schema_keywords(child)


def check_links():
    markdown = list(repo_files(".md"))
    anchors = {}
    for path in markdown + [p for p in (ROOT / "docs").rglob("*.md") if p.resolve() not in {m.resolve() for m in markdown}]:
        counts, ids = {}, set()
        for line in path.read_text().splitlines():
            if not re.match(r"^#{1,6} ", line):
                continue
            heading = re.sub(r"^#{1,6} +", "", line).strip().lower()
            slug = re.sub(r"[^\w\- ]", "", heading).replace(" ", "-")
            count = counts.get(slug, 0)
            counts[slug] = count + 1
            ids.add(slug if count == 0 else f"{slug}-{count}")
        anchors[path.resolve()] = ids
    checked = 0
    for path in markdown:
        for raw in re.findall(r"\[[^\]]*\]\(([^\s)]+)\)", path.read_text()):
            url = urlsplit(raw.strip("<>"))
            if url.scheme or url.netloc:
                continue
            target = (path.parent / unquote(url.path)).resolve() if url.path else path.resolve()
            require(target.is_relative_to(ROOT), f"{path.relative_to(ROOT)}: out-of-repo link {raw}")
            require(target.exists(), f"{path.relative_to(ROOT)}: missing link {raw}")
            if url.fragment and target.suffix == ".md":
                require(unquote(url.fragment) in anchors[target], f"{path.relative_to(ROOT)}: missing anchor {raw}")
            checked += 1
    return len(markdown), checked


def check_examples():
    schemas = {p.name.split(".")[0]: load(p) for p in (SPEC / "contracts").glob("*.schema.json")}
    for rule in schemas.values():
        validate_schema_keywords(rule)
    docs = {name: load(SPEC / "examples" / f"{name}.json") for name in schemas}
    for name, doc in docs.items():
        schema_check(doc, schemas[name], schemas[name])
    bads = []
    command = docs["command"]
    bad = deepcopy(command); bad["extra"] = "unregistered"; bads.append(("command", bad))
    bad = deepcopy(command); bad["operations"][0]["kind"] = "eval"; bads.append(("command", bad))
    bad = deepcopy(command); bad["operations"] *= 9; bads.append(("command", bad))
    bad = deepcopy(command); bad["baseRevision"] = True; bads.append(("command", bad))
    bad = deepcopy(docs["world"]); bad["entities"][0]["components"][0]["data"]["walls"][0]["startMm"][0] = .5; bads.append(("world", bad))
    bad = deepcopy(docs["channel"]); bad["sequence"] = 0; bads.append(("channel", bad))
    bad = deepcopy(docs["channel"]); del bad["trustVersion"]; bads.append(("channel", bad))
    bad = deepcopy(docs["channel"]); bad["expiresAt"] = "tomorrow"; bads.append(("channel", bad))
    bad = deepcopy(docs["release"]); del bad["engine"]["buildDigest"]; bads.append(("release", bad))
    bad = deepcopy(docs["release"]); bad["startupDigests"] = []; bads.append(("release", bad))
    bad = deepcopy(docs["release"]); bad["offlinePolicy"] = "ignore-revocations"; bads.append(("release", bad))
    for name, bad in bads:
        try:
            schema_check(bad, schemas[name], schemas[name])
        except AssertionError:
            continue
        raise AssertionError(f"Negative {name} fixture was accepted")
    world, release, channel = docs["world"], docs["release"], docs["channel"]
    require(release["worldDigest"] == digest(world), "World digest mismatch")
    require(channel["releaseId"] == digest(release), "Release digest mismatch")
    require(docs["signature-statement"]["subjectDigest"] == digest(channel), "Channel digest mismatch")
    require(docs["signature-statement"]["keyId"] == channel["keyId"], "Channel signer mismatch")
    require(docs["signature-statement"]["projectId"] == channel["projectId"] == release["projectId"] == world["projectId"], "Project identity mismatch")
    issued = datetime.fromisoformat(channel["issuedAt"].replace("Z", "+00:00"))
    expires = datetime.fromisoformat(channel["expiresAt"].replace("Z", "+00:00"))
    require(timedelta(0) < expires-issued <= timedelta(hours=24), "Invalid channel validity interval")
    artifact_ids = {item["digest"] for item in release["artifacts"]}
    require(set(release["startupDigests"]) <= artifact_ids, "Startup blob outside release closure")
    for role, expected in (("engine", release["engine"]["buildDigest"]),
                           ("behavior-compiler", release["engine"]["behaviorCompilerDigest"]),
                           ("world-compiler", release["compilerDigest"]),
                           ("rights", release["rightsDigest"]), ("world", release["worldDigest"])):
        require(any(x["role"] == role and x["digest"] == expected for x in release["artifacts"]), f"Missing pinned {role} artifact")
    bodies = {digest(load(p)): canonical_fixture(load(p)) for p in (SPEC / "examples").glob("*.json")}
    for item in release["artifacts"]:
        require(item["digest"] in bodies, "Fixture artifact unavailable")
        require(len(bodies[item["digest"]]) == item["byteLength"], "Fixture length mismatch")
    building = world["entities"][0]["components"][0]["data"]
    walls = {w["id"]: w for w in building["walls"]}
    room_ids = {r["id"] for r in building["rooms"]}
    require(len(walls) == len(building["walls"]), "Duplicate wall IDs")
    for wall in walls.values():
        require(set(wall["roomIds"]) <= room_ids, "Unknown adjacent room")
    for room in building["rooms"]:
        segments = []
        for edge in room["boundary"]:
            wall = walls[edge["wallId"]]
            require(room["id"] in wall["roomIds"], "Room boundary adjacency mismatch")
            ends = (wall["startMm"], wall["endMm"])
            segments.append(ends if edge["direction"] == 1 else ends[::-1])
        require(all(a[1] == b[0] for a, b in zip(segments, segments[1:] + segments[:1])), "Unclosed room boundary")
    for opening in building["openings"]:
        wall = walls[opening["wallId"]]
        length2 = sum((a-b)**2 for a, b in zip(wall["startMm"], wall["endMm"]))
        require((opening["offsetMm"] + opening["widthMm"])**2 <= length2, "Opening exceeds edge")
        require(opening["sillMm"] + opening["heightMm"] <= wall["heightMm"], "Opening exceeds height")
    return len(schemas), len(bads)


def generation_model(broken=False):
    # State: desired generation, four active output identities, job1/job2 state.
    initial = (0, (0, 0, 0, 0), ("absent", "absent"))
    seen, queue = {initial}, deque([(initial, [])])
    while queue:
        (desired, active, jobs), trace = queue.popleft()
        actions = []
        if desired < 2:
            newer = list(jobs); newer[desired] = "pending"
            actions.append(("edit" + str(desired+1), (desired+1, active, tuple(newer))))
        for i, job in enumerate(jobs):
            if job == "pending":
                for result in ("ready", "failed"):
                    updated = list(jobs); updated[i] = result
                    actions.append((result + str(i+1), (desired, active, tuple(updated))))
            if job == "ready" and (broken or i+1 == desired):
                updated = list(jobs); updated[i] = "active"
                actions.append(("activate" + str(i+1), (desired, (i+1,)*4, tuple(updated))))
        for action, nxt in actions:
            if action.startswith("activate") and nxt[1][0] != desired:
                return len(seen), trace + [action]
            require(len(set(nxt[1])) == 1, "Partial generation activation")
            if nxt not in seen:
                seen.add(nxt); queue.append((nxt, trace + [action]))
    return len(seen), None


def channel_model(broken=False):
    admitted = {"release-a", "release-b"}
    proposals = [(None, 1, "release-a"), (None, 1, "release-b"), (None, 1, "missing")]
    schedules = 0
    for order in permutations(proposals):
        head, sequence = None, 0
        for expected, proposed_sequence, release in order:
            accepted = release in admitted and (broken or (expected == head and proposed_sequence == sequence+1))
            if accepted:
                if expected != head or proposed_sequence != sequence+1:
                    return schedules, list(order)
                head, sequence = release, proposed_sequence
        # A fresh valid follow-up is possible after the race; stale proposals fail.
        require(sequence == 1, "Race must commit one complete release")
        next_sequence, previous = sequence+1, head
        require(next_sequence > sequence and previous in admitted, "Nonmonotonic follow-up")
        schedules += 1
    return schedules, None


def main():
    for path in repo_files(".json"):
        if ".git" not in path.parts:
            load(path)
    count, links = check_links()
    requirements = load(SPEC / "requirements.json")["requirements"]
    ids = [r["id"] for r in requirements]
    require(len(ids) == len(set(ids)), "Duplicate requirement ID")
    for item in requirements:
        require((SPEC / item["owner"].removeprefix("docs/")).exists(), f"Missing requirement owner {item['id']}")
        require(item["status"] == "specified", "Initial requirements cannot claim implementation")
        require(re.fullmatch(r"M[0-7]", item["milestone"]) is not None, "Unknown milestone")
    schemas, negatives = check_examples()
    generation_states, counterexample = generation_model()
    require(counterexample is None, f"Activation counterexample: {counterexample}")
    require(generation_model(broken=True)[1] is not None, "Activation mutation did not expose a counterexample")
    schedules, counterexample = channel_model()
    require(counterexample is None, f"Channel counterexample: {counterexample}")
    require(channel_model(broken=True)[1] is not None, "Channel mutation did not expose a counterexample")
    print(f"PASS: {count} Markdown files, {links} local links, {len(ids)} requirement groups")
    print(f"PASS: {schemas} schemas/examples, {negatives} invalid variants, synthetic identities/topology")
    print(f"PASS: bounded models: {generation_states} activation states, {schedules} channel schedules; broken-rule controls detected")
    print("Scope: documentation, fixtures, and bounded abstract models only; no engine qualification.")


if __name__ == "__main__":
    main()
