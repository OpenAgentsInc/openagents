#!/usr/bin/env python3
# Firm reason: 2 (independent JSON/parent-graph oracle for the Coax reader; never stores transcript text)
"""Compare the registered Coder MCP reader with an independent Claude graph.

Only aggregate observations reach stdout. Private input and tool replies stay
in memory. --inspect reports structural input counts before implementation.
"""
import argparse
import collections
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile
import time
from datetime import datetime, timezone


def require(condition, label):
    if not condition:
        raise SystemExit("oracle failed: " + label)


def segment(row):
    m = row.get("compactMetadata", row.get("compact_metadata", {}))
    return m.get("preservedSegment", m.get("preserved_segment")) if isinstance(m, dict) else None


def parent(row):
    return row.get("parentUuid") or row.get("logicalParentUuid")


def foreign_text(block):
    return isinstance(block, str) and not re.match(r"^\s*(?:<[a-z][\w.:-]*(?:\s|/?>)|\[Request interrupted by user)", block)


def blocks(row):
    value = row.get("message", {}).get("content", [])
    return [{"type": "text", "text": value}] if isinstance(value, str) else value if isinstance(value, list) else []


def visible(row):
    if row.get("type") not in ("user", "assistant") or any(row.get(k) for k in ("isMeta", "isCompactSummary", "isVirtual", "isVisibleInTranscriptOnly")):
        return False
    return any(isinstance(b, dict) and (b.get("type") in ("tool_use", "tool_result", "image") or
               b.get("type") in ("text", "input_text", "output_text") and foreign_text(b.get("text")) and b.get("text", "").strip()) for b in blocks(row))


def reference(rows, include_context=False):
    """Independent edge/ancestor oracle; returns the recoverable visible IDs."""
    absolute = [i for i, r in enumerate(rows) if r.get("subtype") == "compact_boundary" and segment(r) is None]
    # Preserve UUID/parent edges of attachments and future record types too.
    # Their payloads are never eligible conversation turns.
    graph = {r["uuid"]: dict(r) for r in rows[absolute[-1] if absolute else 0:]
             if r.get("uuid") and not r.get("isSidechain")}
    boundaries = [key for key, r in graph.items() if r.get("subtype") == "compact_boundary"]
    if boundaries and segment(graph[boundaries[-1]]) is not None:
        s = segment(graph[boundaries[-1]])
        head, tail, anchor = (s.get(a, s.get(b)) for a, b in (("headUuid", "head_uuid"), ("tailUuid", "tail_uuid"), ("anchorUuid", "anchor_uuid")))
        kept = set()
        current = tail
        while current in graph and current not in kept:
            kept.add(current)
            if current == head:
                break
            current = parent(graph[current])
        require(current == head and anchor in graph, "live preserved segment is complete")
        graph[head]["parentUuid"] = anchor
        for key, row in graph.items():
            if key != head and parent(row) == anchor:
                row["parentUuid"] = tail
        keys = list(graph)
        graph = {key: r for i, (key, r) in enumerate(graph.items()) if i >= keys.index(boundaries[-1]) or key in kept}
    removed = set()
    for r in graph.values():
        s = r.get("snipMetadata", r.get("snip_metadata", {}))
        if isinstance(s, dict):
            removed.update(s.get("removedUuids", s.get("removed_uuids", [])))
    for row in graph.values():
        p, seen = parent(row), set()
        while p in removed and p in graph and p not in seen:
            seen.add(p)
            p = parent(graph[p])
        row["parentUuid"] = p
    graph = {k: r for k, r in graph.items() if k not in removed}
    positions = {k: i for i, k in enumerate(graph)}
    parents = {parent(r) for r in graph.values()}
    leaves = []
    for key in graph.keys() - parents:
        visited = set()
        while key in graph and key not in visited:
            visited.add(key)
            if graph[key]["type"] in ("user", "assistant"):
                leaves.append(key)
                break
            key = parent(graph[key])
    require(bool(leaves), "live graph has a conversation leaf")
    newest = lambda key: (graph[key].get("timestamp", ""), positions[key])
    leaf = max(leaves, key=newest)
    chain, seen = [], set()
    while leaf in graph and leaf not in seen:
        chain.append(leaf)
        seen.add(leaf)
        leaf = parent(graph[leaf])
    chain.reverse()
    full = []
    for key in chain:
        full.append(key)
        r = graph[key]
        message_id = r.get("message", {}).get("id")
        if r["type"] != "assistant" or not message_id:
            continue
        group = {k for k, v in graph.items() if v["type"] == "assistant" and v.get("message", {}).get("id") == message_id}
        siblings = sorted(group - seen, key=newest)
        results = sorted((k for k, v in graph.items() if k not in seen and parent(v) in group and v["type"] == "user" and any(b.get("type") == "tool_result" for b in blocks(v) if isinstance(b, dict))), key=newest)
        full.extend(siblings + results)
        seen.update(siblings + results)
    return [graph[key] for key in full if visible(graph[key]) or include_context and graph[key].get("isCompactSummary") and graph[key].get("type") == "user" and not graph[key].get("isMeta") and not graph[key].get("isVirtual")]


def context_reference(rows):
    """Independent UTF-8 chunk oracle over selected historical summaries."""
    expected = []
    for row in reference(rows, include_context=True):
        if not row.get("isCompactSummary"):
            continue
        value = row.get("message", {}).get("content", [])
        text = value if isinstance(value, str) else "\n".join(
            b["text"] for b in value if isinstance(b, dict) and
            b.get("type") in ("text", "input_text", "output_text") and isinstance(b.get("text"), str))
        chunks, chunk, size = [], "", 0
        for character in text:
            width = len(character.encode("utf-8"))
            if size + width > 2048:
                chunks.append(chunk)
                chunk, size = "", 0
            chunk += character
            size += width
        if chunk:
            chunks.append(chunk)
        expected.extend((row["uuid"], i, len(chunks), s) for i, s in enumerate(chunks))
    return expected


def check_context(client, session_id, rows, stop):
    expected = context_reference(rows)
    chunks, cursor, pages = [], 0, 0
    while True:
        page = client.read({"action": "context", "ref": session_id, "max_turns": 2, "before": cursor})
        require(page.get("view") == "compaction_context", "explicit context view")
        require(page["total_turns"] == len(expected), "context chunk count")
        require((page["last_user_request"], page["last_assistant_action"]) == stop, "summaries cannot replace handoff instructions")
        require(page["end"] - page["start"] == len(page["turns"]), "context page bounds")
        chunks[0:0] = page["turns"]
        pages += 1
        next_cursor = page["next_before"]
        if next_cursor is None:
            break
        require(next_cursor > 0 and (cursor == 0 or next_cursor < cursor), "context cursor makes progress")
        cursor = next_cursor
        require(pages < 4097, "bounded context pages")
    for chunk in chunks:
        require(chunk["inert"] is True and chunk["role"] == "context" and chunk["kind"] == "compaction_summary", "generated context is explicitly inert")
        require(not chunk["tool_calls"] and not chunk["tool_results"], "context cannot dispatch historical tools")
    require([(c["uuid"], c["part"], c["parts"], c["text"]) for c in chunks] == expected, "full selected compaction text matches independent oracle byte for byte")
    return {"context_chunks": len(chunks), "context_pages": pages,
            "context_bytes": sum(len(c["text"].encode()) for c in chunks)}


def verify_codex_rollout(rows, session_id, original_rows):
    meta = next(r["payload"] for r in rows if r.get("type") == "session_meta")
    require(meta.get("originator") == "openagents-coder", "live chat was driven by Coder")
    require(any(d.get("name") == "coder_session_read" for d in meta.get("dynamic_tools", [])), "reader persisted in real Codex tool catalog")
    replies, shell_calls = [], 0
    for row in rows:
        if row.get("type") != "response_item":
            continue
        item = row.get("payload", {})
        kind = item.get("type")
        if kind in ("function_call", "custom_tool_call"):
            name = item.get("name", "")
            code = item.get("input", "")
            shell_calls += int(name.endswith("exec_command") or name in ("shell", "shell_command") or
                               bool(re.search(r"tools\.(?:exec_command|shell|shell_command)\s*\(", code)))
        if kind not in ("function_call_output", "custom_tool_call_output"):
            continue
        # Real code-mode tool output is a list of text blocks. Only decode
        # tool replies, never reasoning or strings quoted in foreign history.
        value = item.get("output", [])
        texts = [value] if isinstance(value, str) else [b.get("text", "") for b in value if isinstance(b, dict)]
        for text in texts:
            try:
                result = json.loads(text)
            except (ValueError, TypeError):
                continue
            if isinstance(result, dict) and result.get("schema") == "coder.session-reader.v1":
                replies.append(result)
    require(len(replies) >= 2 and shell_calls == 0, "real Codex used registered reader without Bash")
    require(all(r.get("session_id") == session_id and r.get("inert") is True and "error" not in r for r in replies), "live reader selected the intended full ID")
    conversation = sorted((r for r in replies if r.get("view") == "conversation"), key=lambda r: r["start"])
    context = sorted((r for r in replies if r.get("view") == "compaction_context"), key=lambda r: r["start"])
    require(conversation and context, "live Codex fetched both conversation and compaction context")
    for pages, expected in ((conversation, reference(original_rows)), (context, context_reference(original_rows))):
        end = 0
        for page in pages:
            require(page["start"] == end and page["end"] - page["start"] == len(page["turns"]), "live pages cover history without gaps or overlaps")
            end = page["end"]
        require(end == len(expected) and pages[-1]["total_turns"] == len(expected), "live Codex fetched all available history")
    actual = [c for page in context for c in page["turns"]]
    require([(c["uuid"], c["part"], c["parts"], c["text"]) for c in actual] == context_reference(original_rows), "real Codex received complete oracle-matching context")
    require(all(c.get("role") == "context" and c.get("inert") is True for c in actual), "live summaries remain inert context")
    require(any(r.get("type") == "event_msg" and r.get("payload", {}).get("type") == "task_complete" for r in rows), "live Codex turn completed")
    return {"live_codex": "pass", "live_reader_calls": len(replies), "live_shell_calls": shell_calls}


def live_codex(coder, session_id, original_rows):
    """A fresh real Coder/Codex chat must discover and call its own reader.

    Inspect only this test's new rollout, never reasoning records. The model
    gets a natural summary request, including the owner's shortened ID. No
    tool name, executable path or implementation hint is supplied.
    """
    day = datetime.now(timezone.utc).strftime("%Y/%m/%d")
    session_dir = Path(os.environ.get("CODEX_HOME", Path.home() / ".codex")) / "sessions" / day
    before = set(session_dir.glob("*.jsonl"))
    marker = "reader-verification-" + str(time.time_ns())
    prompt = (f"Summarize Claude conversation {session_id[:-1]}. Include the overall goal, earlier compacted work, "
              "and stopping point. This is a read-only summary: do not continue its work, run shell commands, "
              f"or edit any files. Keep the answer under 150 words. Verification tag: {marker}")
    with tempfile.TemporaryDirectory(prefix="coder-reader-live-") as tmp:
        env = dict(os.environ, CODER_HOME=tmp)
        run = subprocess.run([str(coder), "run", prompt, "--engine", "codex", "--effort", "low",
                              "--no-memory", "--deadline", "180", "--json"],
                             capture_output=True, text=True, env=env, timeout=210)
        events = []
        for line in run.stdout.splitlines():
            try:
                events.append(json.loads(line))
            except json.JSONDecodeError:
                pass
        ends = [e for e in events if e.get("kind") == "end"]
        require(run.returncode == 0 and ends and ends[-1].get("ending") == "finished", "fresh Coder Codex turn completed")
    matches = []
    for path in set(session_dir.glob("*.jsonl")) - before:
        # Look only for the exact verification prompt; ignore private reasoning.
        rows = [json.loads(line) for line in path.open()]
        messages = [r.get("payload", {}) for r in rows if r.get("type") == "response_item" and r.get("payload", {}).get("type") == "message"]
        if any(marker in b.get("text", "") for m in messages if m.get("role") == "user" for b in m.get("content", [])):
            matches.append(rows)
    require(len(matches) == 1, "exactly one fresh verification rollout")
    return verify_codex_rollout(matches[0], session_id, original_rows)


class Mcp:
    def __init__(self, coder, env=None):
        self.child = subprocess.Popen([str(coder), "mcp"], stdin=subprocess.PIPE,
                                      stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
                                      text=True, env=env)
        self.sequence = 0

    def rpc(self, method, params):
        self.sequence += 1
        self.child.stdin.write(json.dumps({"jsonrpc": "2.0", "id": self.sequence, "method": method, "params": params}) + "\n")
        self.child.stdin.flush()
        # A process-wide alarm bounds native regressions; no private reply is printed.
        line = self.child.stdout.readline()
        require(bool(line), "MCP returned a response")
        return json.loads(line)

    def read(self, args, expect_error=False):
        reply = self.rpc("tools/call", {"name": "coder_session_read", "arguments": args})
        require("result" in reply, "registered tool dispatch")
        result = reply["result"]
        require(result.get("isError", False) == expect_error, "tool exit status")
        value = json.loads(result["content"][0]["text"])
        require(value.get("inert") is True, "top-level inert marker")
        return value

    def close(self):
        self.child.stdin.close()
        try:
            self.child.wait(timeout=10)
        except subprocess.TimeoutExpired:
            self.child.kill()
            self.child.wait(timeout=5)


class Cli:
    """Exercise the exact native reader under ASan/UBSan without child fallback."""
    def __init__(self, coder, env=None):
        self.coder, self.env = coder, env

    def read(self, args, expect_error=False):
        reply = subprocess.run([str(self.coder), "session-read", "--request", json.dumps(args)],
                               capture_output=True, text=True, env=self.env, timeout=30)
        require(reply.returncode == (2 if expect_error else 0), "native CLI exit status")
        require("ERROR: AddressSanitizer" not in reply.stderr and "runtime error:" not in reply.stderr, "native sanitizer diagnostics")
        result = json.loads(reply.stdout)
        require(result.get("inert") is True, "CLI inert marker")
        return result

    def close(self):
        pass


def synthetic(coder, transport):
    with tempfile.TemporaryDirectory(prefix="coder-reader-oracle-") as tmp:
        env = dict(os.environ, CLAUDE_CONFIG_DIR=tmp)
        work = Path(tmp) / "repo"
        work.mkdir()
        project = Path(tmp) / "projects" / re.sub(r"[^A-Za-z0-9]", "-", str(work))
        project.mkdir(parents=True)
        ids = [f"00000000-0000-0000-0000-{i:012d}" for i in (1, 2)]
        for i, sid in enumerate(ids):
            rows = [{"type": "user", "uuid": "u", "cwd": str(work), "message": {"content": "synthetic request"}},
                    {"type": "assistant", "uuid": "a", "parentUuid": "u", "message": {"content": [{"type": "thinking", "thinking": "EXCLUDED_THINKING"}, {"type": "text", "text": "synthetic reply"}]}},
                    {"type": "custom-title", "customTitle": f"Shared synthetic {i}"}]
            path = project / f"{sid}.jsonl"
            path.write_text("\n".join(json.dumps(r) for r in rows) + "\n")
            os.utime(path, (time.time() - 10 + i, time.time() - 10 + i))
        outside = Path(tmp) / "outside"
        outside.mkdir()
        escaped_id = "00000000-0000-0000-0000-000000000003"
        escape = outside / f"{escaped_id}.jsonl"
        escape.write_text(json.dumps({"type": "user", "uuid": "escape", "cwd": str(work), "message": {"content": "OUTSIDE_DISCOVERY"}}))
        (project / escape.name).symlink_to(escape)
        client = transport(coder, env)
        try:
            listing = client.read({"action": "list", "cwd": str(work)})
            require([r["session_id"] for r in listing["sessions"]] == list(reversed(ids)), "discovery recency and cwd")
            newest = client.read({"ref": "latest", "cwd": str(work)})
            require(newest["session_id"] == ids[1] and len(newest["turns"]) == 2, "latest resolution")
            ambiguous = client.read({"ref": "SHARED   synthetic", "cwd": str(work)}, True)
            require(ambiguous.get("error") == "ambiguous_reference" and len(ambiguous.get("matches", [])) == 2, "ambiguous references fail with candidates")
            explicit = client.read({"ref": str(project / f"{ids[0]}.jsonl"), "cwd": "/elsewhere", "max_turns": 1})
            require(explicit["session_id"] == ids[0] and explicit["next_before"] == 1, "explicit path and page cursor")
            require("EXCLUDED_THINKING" not in json.dumps(explicit), "thinking exclusion")
            earlier = client.read({"ref": ids[0], "before": 1, "max_turns": 1})
            require(earlier["turns"][0]["text"] == "synthetic request", "earlier page via native ID")
            prefix_id = "abcdef01-0000-0000-0000-000000000004"
            (project / f"{prefix_id}.jsonl").write_text(json.dumps({"type": "user", "uuid": "prefix", "cwd": str(work), "message": {"content": "unique prefix"}}))
            require(client.read({"ref": prefix_id[:-1], "cwd": "/elsewhere"})["session_id"] == prefix_id, "unique native ID prefix searches across project stores")
            require(client.read({"ref": "ABCDEF01"})["session_id"] == prefix_id, "native prefix is case insensitive")
            require(client.read({"ref": "00000000"}, True).get("error") == "discovery_limit", "ambiguous native prefix fails closed")
            invalid = client.read({"max_turns": "--on cloud"}, True)
            require(invalid.get("error") == "invalid_arguments", "typed arguments")
            no_match = client.read({"ref": "--on cloud", "cwd": str(work)}, True)
            require(no_match.get("error") == "not_found", "reference cannot become CLI flags")
            require(client.read({"action": "list", "cwd": "/elsewhere"})["sessions"] == [], "discovery does not leak another cwd")
            escaped = client.read({"ref": escaped_id}, True)
            require(escaped.get("error") == "not_found", "discovery rejects file symlink escapes")
            oversized = Path(tmp) / "oversized.jsonl"
            with oversized.open("wb") as stream:
                stream.truncate(32 * 1024 * 1024 + 1)
            require(client.read({"ref": str(oversized)}, True).get("error") == "file_limit", "file budget checked before loading")
            many = Path(tmp) / "many.jsonl"
            with many.open("w") as stream:
                for i in range(8193):
                    stream.write(json.dumps({"type": "user", "uuid": str(i), "message": {"content": "bounded"}}) + "\n")
            require(client.read({"ref": str(many)}, True).get("error") == "record_limit", "metadata budget refuses a partial branch")
            duplicate = client.read({"ref": str(project / f"{ids[0]}.jsonl"), "before": 3}, True)
            require(duplicate.get("error") == "invalid_before", "out-of-range paging is explicit")
            for path in project.glob("*.jsonl"):
                if not path.is_symlink():
                    os.utime(path, (time.time() - 120, time.time() - 120))
            require(client.read({"action": "list", "cwd": str(work), "within_min": 1})["sessions"] == [], "age filter")
        finally:
            client.close()
    return 17


def records(path):
    result = []
    malformed = 0
    with path.open(encoding="utf-8", errors="replace") as stream:
        for line in stream:
            try:
                row = json.loads(line)
            except json.JSONDecodeError:
                malformed += bool(line.strip())
                continue
            if isinstance(row, dict):
                result.append(row)
            else:
                malformed += 1
    return result, malformed


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("session_id")
    p.add_argument("--inspect", action="store_true")
    p.add_argument("--coder", default="coder/tui/coder.sh")
    p.add_argument("--direct", action="store_true", help="test the native CLI, including sanitizer builds")
    p.add_argument("--codex", action="store_true", help="also test a fresh actual Coder Codex chat on the installed login")
    p.add_argument("--codex-rollout", type=Path, help="verify a completed live check without repeating model inference")
    a = p.parse_args()
    root = Path(os.environ.get("CLAUDE_CONFIG_DIR", Path.home() / ".claude"))
    paths = list((root / "projects").glob(f"*/{a.session_id}.jsonl"))
    if len(paths) != 1:
        raise SystemExit("oracle: expected exactly one matching local transcript")
    source = paths[0]
    rows, bad = records(source)
    kinds = collections.Counter(str(r.get("type")) for r in rows)
    summary = {"bytes": source.stat().st_size, "records": len(rows),
               "malformed": bad, "user": kinds["user"], "assistant": kinds["assistant"],
               "compact_boundaries": sum(r.get("subtype") == "compact_boundary" for r in rows),
               "snips": sum(bool(r.get("snipMetadata") or r.get("snip_metadata")) for r in rows),
               "sidechains": sum(bool(r.get("isSidechain")) for r in rows),
               "max_line_bytes": max(len(json.dumps(r, ensure_ascii=False).encode()) for r in rows)}
    if a.inspect:
        print(json.dumps(summary, sort_keys=True))
        return
    if a.codex_rollout:
        digest = hashlib.sha256(source.read_bytes()).digest()
        rollout = [json.loads(line) for line in a.codex_rollout.open()]
        summary.update(verify_codex_rollout(rollout, a.session_id, rows))
        require(hashlib.sha256(source.read_bytes()).digest() == digest, "rollout verification is read only")
        print(json.dumps(summary, sort_keys=True))
        return
    coder = Path(a.coder).resolve()
    require(coder.exists(), "Coder executable exists")
    # A real-time deadline also covers a stuck tools/call read.
    import signal
    signal.signal(signal.SIGALRM, lambda *_: (_ for _ in ()).throw(SystemExit("oracle deadline exceeded")))
    signal.alarm(180)
    transport = Cli if a.direct else Mcp
    synthetic_checks = synthetic(coder, transport)
    before_digest = hashlib.sha256(source.read_bytes()).digest()
    expected = reference(rows)
    client = transport(coder)
    all_turns, pages = [], 0
    try:
        if not a.direct:
            tools = client.rpc("tools/list", {})["result"]["tools"]
            tool = next((t for t in tools if t.get("name") == "coder_session_read"), None)
            require(tool is not None and tool["inputSchema"]["properties"]["action"]["enum"] == ["list", "show", "context"], "MCP catalog schema")
        cursor, stop = 0, None
        while True:
            page = client.read({"ref": a.session_id, "max_turns": 7, "before": cursor})
            require(page["session_id"] == a.session_id, "requested native ID")
            require(page["total_turns"] == len(expected), "oracle active-turn count")
            require(page["end"] == (len(expected) if cursor == 0 else cursor), "page endpoint")
            require(page["end"] - page["start"] == len(page["turns"]), "page bounds")
            all_turns[0:0] = page["turns"]
            pages += 1
            if stop is None:
                stop = (page["last_user_request"], page["last_assistant_action"])
            else:
                require(stop == (page["last_user_request"], page["last_assistant_action"]), "handoff stays stable across pages")
            next_cursor = page["next_before"]
            if next_cursor is None:
                break
            require(next_cursor > 0 and (cursor == 0 or next_cursor < cursor), "cursor makes progress")
            cursor = next_cursor
            require(pages < 128, "bounded page count")
        require([t.get("uuid") for t in all_turns] == [r["uuid"] for r in expected], "every active message and parallel result in oracle order")
        calls, results = 0, 0
        for actual, original in zip(all_turns, expected):
            require(actual["inert"] is True and actual["role"] == original["type"], "turn role and inert marker")
            bc = [b for b in blocks(original) if isinstance(b, dict) and b.get("type") == "tool_use"]
            br = [b for b in blocks(original) if isinstance(b, dict) and b.get("type") == "tool_result"]
            require([b["id"] for b in actual["tool_calls"]] == [b["id"] for b in bc], "tool-call identity")
            require([b["tool_use_id"] for b in actual["tool_results"]] == [b["tool_use_id"] for b in br], "tool-result identity")
            require(all(b["inert"] is True for b in actual["tool_calls"] + actual["tool_results"]), "tool previews are inert")
            calls += len(bc)
            results += len(br)
        summary.update(check_context(client, a.session_id, rows, stop))
    finally:
        client.close()
        signal.alarm(0)
    require(hashlib.sha256(source.read_bytes()).digest() == before_digest, "source transcript unchanged")
    if a.codex:
        summary.update(live_codex(coder, a.session_id, rows))
        require(hashlib.sha256(source.read_bytes()).digest() == before_digest, "live Codex left source transcript unchanged")
    summary.update({"synthetic_checks": synthetic_checks, "active_turns": len(expected),
                    "pages": pages, "tool_calls": calls, "tool_results": results,
                    "registered_mcp": not a.direct, "source_unchanged": True, "oracle": "pass"})
    print(json.dumps(summary, sort_keys=True))


if __name__ == "__main__":
    main()
