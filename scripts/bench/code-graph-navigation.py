#!/usr/bin/env python3
"""Count navigation operations in explicitly selected local agent histories.

This research census never runs saved commands or emits their text. It reports
session-level observations, not task success, elapsed effort, or training labels.
"""

import argparse
import ast
import collections
import datetime as dt
import json
from pathlib import Path
import re
import statistics
import subprocess


COMMAND = re.compile(r'''\b(?:cmd|command)["']?\s*:\s*("(?:\\.|[^"\\])*"|'(?:\\.|[^'\\])*')''')
PATH = re.compile(r"(?:crates|bins|docs|scripts|deploy|os|nips|assets)/[A-Za-z0-9_./-]+")
RULES = [
    ("edit", r"\b(?:apply_patch|git\s+(?:commit|cherry-pick|merge)|sed\s+-i)\b"),
    ("check", r"\b(?:cargo|rustc|rustfmt|pytest|unittest|nix\s+build)\b"),
    ("graph", r"\b(?:code-graph|graph_(?:symbol|callers|callees|impact|path|module))\b"),
    ("search", r"\b(?:rg|grep|find|fd)\b"),
    ("read", r"\b(?:cat|sed|head|tail|less|awk|nl)\b"),
    ("list", r"\b(?:ls|tree|pwd)\b"),
    ("history", r"\bgit\s+(?:log|show|blame|diff|status|rev-parse)\b"),
]
NAVIGATION = {"graph", "search", "read", "list", "history"}
DIRECT = {"Read": "read", "Grep": "search", "Glob": "list",
          "Edit": "edit", "Write": "edit", "apply_patch": "edit"}


def timestamp(value):
    if not isinstance(value, str):
        return None
    try:
        parsed = dt.datetime.fromisoformat(value.replace("Z", "+00:00"))
        return parsed.replace(tzinfo=dt.timezone.utc) if parsed.tzinfo is None else parsed
    except ValueError:
        return None


def category(command):
    for name, pattern in RULES:
        if re.search(pattern, command):
            return name
    return "other"


def literal_commands(source):
    """Read literal command fields in orchestration code without evaluating it."""
    commands = []
    for match in COMMAND.finditer(source):
        try:
            value = ast.literal_eval(match.group(1))
        except (ValueError, SyntaxError):
            continue
        if isinstance(value, str):
            commands.append(value)
    return commands


def operations(name, args):
    """Return counted operations and whether a wrapper could not be decoded."""
    short = name.rsplit(".", 1)[-1]
    if short in DIRECT:
        return [(DIRECT[short], json.dumps(args, sort_keys=True))], False
    if short.startswith("graph_"):
        return [("graph", json.dumps(args, sort_keys=True))], False
    if short in ("Bash", "exec_command", "run_shell_command", "shell"):
        if isinstance(args, dict):
            command = args.get("command", args.get("cmd", ""))
            if isinstance(command, list):
                command = " ".join(str(part) for part in command)
            if isinstance(command, str) and command:
                return [(category(command), command)], False
        return [("other", "")], True
    if short in ("exec", "parallel") and isinstance(args, str):
        commands = literal_commands(args)
        patch = [("edit", "")] if "tools.apply_patch(" in args else []
        return patch + [(category(c), c) for c in commands], not commands and not patch
    return [("other", "")], False


def calls(record, provider):
    if provider == "claude":
        message = record.get("message", {})
        content = message.get("content", []) if isinstance(message, dict) else []
        if isinstance(content, list):
            for block in content:
                if isinstance(block, dict) and block.get("type") == "tool_use":
                    yield block.get("id"), block.get("name", ""), block.get("input", {})
    elif record.get("type") == "response_item":
        payload = record.get("payload", {})
        if isinstance(payload, dict) and payload.get("type") in ("function_call", "custom_tool_call"):
            args = payload.get("arguments", payload.get("input", {}))
            if payload.get("type") == "function_call" and isinstance(args, str):
                try:
                    args = json.loads(args)
                except json.JSONDecodeError:
                    pass
            yield payload.get("call_id", payload.get("id")), payload.get("name", ""), args


def census_file(path, provider, repo, before, tracked):
    totals = collections.Counter()
    paths = collections.Counter()
    seen_calls, seen_navigation = set(), set()
    before_edit = 0
    edited = False
    accepted = False
    first_record = last_record = None
    size = path.stat().st_size
    if path.is_symlink() or size > 512 * 1024 * 1024:
        return None, "excluded_source"
    with path.open("rb") as stream:
        while stream.tell() < size:
            line = stream.readline(size - stream.tell())
            try:
                record = json.loads(line)
            except (ValueError, UnicodeDecodeError):
                totals["malformed_records"] += 1
                continue
            if not isinstance(record, dict):
                totals["unsupported_records"] += 1
                continue
            payload = record.get("payload", {})
            cwd = (payload.get("cwd") if record.get("type") in ("session_meta", "turn_context")
                   and isinstance(payload, dict) else record.get("cwd"))
            if cwd:
                here = Path(cwd)
                matches = here == repo or repo in here.parents
                if record.get("type") == "session_meta":
                    if not matches:
                        return None, "other_repository"
                    accepted = True
                else:
                    accepted = matches
            when = timestamp(record.get("timestamp"))
            if when is None:
                totals["undated_records"] += 1
                continue
            if when >= before or not accepted:
                continue
            first_record = when if first_record is None else min(first_record, when)
            last_record = when if last_record is None else max(last_record, when)
            totals["records"] += 1
            for call_id, name, args in calls(record, provider):
                if call_id and call_id in seen_calls:
                    totals["duplicate_calls"] += 1
                    continue
                if call_id:
                    seen_calls.add(call_id)
                totals["tool_calls"] += 1
                ops, undecoded = operations(name, args)
                totals["undecoded_wrappers"] += int(undecoded)
                for kind, text in ops:
                    totals["operations"] += 1
                    totals[kind] += 1
                    if kind == "edit":
                        edited = True
                    if kind in NAVIGATION:
                        before_edit += int(not edited)
                        normalized = " ".join(text.split())
                        if normalized in seen_navigation:
                            totals["repeated_navigation_operations"] += 1
                        seen_navigation.add(normalized)
                        totals["navigation"] += 1
                        # Only paths already tracked in this public repository escape.
                        for relative in set(PATH.findall(text)) & tracked:
                            paths[relative] += 1
    if not totals["tool_calls"]:
        return None, "no_eligible_calls"
    return {"totals": totals, "paths": paths, "before_first_edit": before_edit,
            "has_edit": edited, "subagent": "subagents" in path.parts,
            "first_record": first_record, "last_record": last_record}, None


def summarize(rows):
    totals, paths, support = collections.Counter(), collections.Counter(), collections.Counter()
    for row in rows:
        totals.update(row["totals"])
        paths.update(row["paths"])
        support.update(row["paths"].keys())
    edited = [row["before_first_edit"] for row in rows if row["has_edit"]]
    return {
        "source_files_with_calls": len(rows),
        "first_record": min((row["first_record"] for row in rows), default=None).isoformat() if rows else None,
        "last_record": max((row["last_record"] for row in rows), default=None).isoformat() if rows else None,
        "subagent_source_files": sum(row["subagent"] for row in rows),
        "source_files_with_observed_edit": len(edited),
        "median_navigation_before_first_edit": statistics.median(edited) if edited else None,
        "counts": dict(sorted(totals.items())),
        "top_repository_paths_in_navigation": [
            {"path": path, "operations_mentioning_path": n, "source_files": support[path]}
            for path, n in paths.most_common(20)
        ],
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, required=True)
    parser.add_argument("--claude-project", type=Path, required=True)
    parser.add_argument("--codex-sessions", type=Path, required=True)
    parser.add_argument("--before", required=True, help="Exclusive UTC record cutoff")
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    before = timestamp(args.before)
    if before is None:
        parser.error("--before must be an ISO 8601 timestamp")
    tracked = set(subprocess.check_output(
        ["git", "-C", str(args.repo), "ls-files", "-z"], text=True).split("\0"))
    report = {"schema": "openagents.navigation-census.v1", "before": args.before,
              "repository_commit": subprocess.check_output(
                  ["git", "-C", str(args.repo), "rev-parse", "HEAD"], text=True).strip(),
              "providers": {}}
    for provider, root in (("claude", args.claude_project), ("codex", args.codex_sessions)):
        if not root.is_dir() or root.is_symlink():
            parser.error("History roots must be existing directories, without symlinks")
        rows, excluded = [], collections.Counter()
        sources = sorted(root.rglob("*.jsonl"))
        for source in sources:
            row, reason = census_file(source, provider, args.repo.resolve(), before, tracked)
            if row:
                rows.append(row)
            else:
                excluded[reason] += 1
        report["providers"][provider] = {
            "candidate_source_files": len(sources), "excluded": dict(sorted(excluded.items())),
            **summarize(rows),
            "by_source_kind": {
                kind: summarize([row for row in rows if row["subagent"] == subagent])
                for kind, subagent in (("parent", False), ("subagent", True))
            },
        }
    args.out.write_text(json.dumps(report, indent=2) + "\n")


if __name__ == "__main__":
    main()
