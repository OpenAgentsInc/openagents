#!/usr/bin/env python3
"""Replay weka-trace-v1 documents and report prefix reuse.

This tool consumes one or more weka-trace-v1 JSON documents without ever
reading plaintext. It walks each session's events in time order, counts the
64-token block hashes that appear in each turn, and counts how many of those
hashes were already observed earlier in the same session. This is the per-turn
prefix-reuse signal.

Supported schema (robust, accepts any of the listed keys at each level):

  Top-level trace document:
    - "version" (optional): should be "weka-trace-v1".
    - One of "id", "session_id", "thread_id", "trace_id": the root session
      identifier. Falls back to the file stem.
    - One of "events", "requests", "turns": an ordered list of per-turn
      records.

  Each per-turn record supports:
    - Time: one of "t", "timestamp", "offset_ms", "offset".
    - Modality: one of "modality", "type", "role", "name".
    - Context block hashes: one of "hash_ids", "block_hashes", "hashes",
      "blocks". The value is a list of hash identifiers (strings or ints).
    - A subagent record may also carry one of "agent_id", "id", "name" and a
      nested "requests"/"events"/"turns" list. Those nested events are treated
      as a separate thread, e.g. "<root>::sa:<agent_id>".

Per-session and corpus-wide output includes turns, total blocks, repeated
blocks, the prefix-reuse ratio (repeated / total), and the context-growth
blocks (total - repeated, i.e. newly introduced blocks).
"""

import argparse
import datetime as dt
import json
import sys
from pathlib import Path
from typing import Any, Dict, Iterable, List, Optional, Tuple

_VERSION = "weka-trace-v1"
_SESSION_KEYS = ("session_id", "thread_id", "trace_id", "id")
_EVENT_LIST_KEYS = ("events", "requests", "turns")
_HASH_KEYS = ("hash_ids", "block_hashes", "hashes", "blocks")
_TIME_KEYS = ("t", "timestamp", "emitted_at", "offset_ms", "offset")
_MODALITY_KEYS = ("modality", "type", "role", "name", "event_type")
_AGENT_ID_KEYS = ("agent_id", "id", "name")
_NESTED_EVENT_KEYS = ("requests", "events", "turns")


def _get(d: Dict[str, Any], keys: Iterable[str], default: Any = None) -> Any:
    for k in keys:
        if k in d:
            return d[k]
    return default


def _canon(v: Any) -> Optional[Any]:
    if v is None:
        return None
    if isinstance(v, (str, int, float, bool)):
        return (type(v).__name__, v)
    return ("json", json.dumps(v, sort_keys=True, ensure_ascii=False, separators=(",", ":")))


def _sortable_time(v: Any) -> Tuple:
    if isinstance(v, (int, float)):
        return (0, float(v))
    if isinstance(v, str):
        try:
            parsed = dt.datetime.fromisoformat(v.replace("Z", "+00:00"))
            return (0, parsed.timestamp())
        except ValueError:
            return (1, v)
    return (2, "")


def _has_blocks(ev: Dict[str, Any]) -> bool:
    return any(k in ev and isinstance(ev[k], list) for k in _HASH_KEYS)


def _iter_nested(
    events: List[Any],
    thread: str,
    out: List[Tuple[str, Dict[str, Any]]],
) -> None:
    for i, ev in enumerate(events):
        if not isinstance(ev, dict):
            continue
        modality = _get(ev, _MODALITY_KEYS)
        is_subagent = (
            modality == "subagent"
            or "agent_id" in ev
            or "subagent_type" in ev
            or "subagent" in ev
        )
        if any(k in ev and isinstance(ev[k], list) for k in _HASH_KEYS) and not is_subagent:
            # Exported events always carry a blocks list, including empty text
            # payloads; retain those as zero-block turns.
            out.append((thread, ev))
        if is_subagent:
            sub_thread = (
                f"{thread}::sa:{_get(ev, _AGENT_ID_KEYS) or i}"
            )
            nested = _get(ev, _NESTED_EVENT_KEYS)
            if isinstance(nested, list) and nested:
                _iter_nested(nested, sub_thread, out)


def _extract_sessions(doc: Dict[str, Any], fallback_id: str) -> List[Tuple[str, List[Dict[str, Any]]]]:
    if not isinstance(doc, dict):
        raise ValueError("document is not a JSON object")
    version = doc.get("format", doc.get("version"))
    if version is not None and version != _VERSION:
        raise ValueError(f"unsupported format/version: {version!r}")
    root = _get(doc, _SESSION_KEYS) or fallback_id
    events = _get(doc, _EVENT_LIST_KEYS)
    if not isinstance(events, list):
        raise ValueError("missing events/requests/turns list")
    flat: List[Tuple[str, Dict[str, Any]]] = []
    _iter_nested(events, root, flat)
    grouped: Dict[str, List[Dict[str, Any]]] = {}
    for thread, ev in flat:
        eid = _get(ev, _SESSION_KEYS)
        key = eid if isinstance(eid, str) else thread
        grouped.setdefault(key, []).append(ev)
    if not grouped:
        raise ValueError("no turns with block hashes found")
    return list(grouped.items())


def _blocks_for_event(ev: Dict[str, Any]) -> List[Any]:
    return _get(ev, _HASH_KEYS) or []


def _analyze_session(events: List[Dict[str, Any]]) -> Dict[str, Any]:
    indexed = sorted(enumerate(events), key=lambda x: (_sortable_time(_get(x[1], _TIME_KEYS)), x[0]))
    seen: set = set()
    turns: List[Dict[str, Any]] = []
    total, repeated, new = 0, 0, 0
    first_total, last_total = 0, 0
    for orig_idx, ev in indexed:
        raw = _blocks_for_event(ev)
        if not isinstance(raw, list):
            raise ValueError("turn block list is not an array")
        keys = [_canon(b) for b in raw if _canon(b) is not None]
        n_total = len(keys)
        n_repeated = sum(1 for k in keys if k in seen)
        n_new = n_total - n_repeated
        seen.update(keys)
        total += n_total
        repeated += n_repeated
        new += n_new
        if not turns:
            first_total = n_total
        last_total = n_total
        turns.append(
            {
                "index": len(turns) + 1,
                "timestamp": _get(ev, _TIME_KEYS),
                "modality": _get(ev, _MODALITY_KEYS) or "?",
                "total_blocks": n_total,
                "repeated_blocks": n_repeated,
                "new_blocks": n_new,
                "prefix_reuse_ratio": n_repeated / n_total if n_total else 0.0,
            }
        )
    if not turns:
        raise ValueError("session has no turns with block arrays")
    return {
        "turns": turns,
        "summary": {
            "turns": len(turns),
            "total_blocks": total,
            "repeated_blocks": repeated,
            "new_blocks": new,
            "prefix_reuse_ratio": repeated / total,
            "context_growth": new,
            "first_turn_blocks": first_total,
            "last_turn_blocks": last_total,
        },
    }


def _load_documents(path: Path) -> List[Dict[str, Any]]:
    raw = path.read_text(encoding="utf-8")
    if not raw.strip():
        raise ValueError("empty document")
    try:
        payload = json.loads(raw)
    except json.JSONDecodeError as exc:
        docs: List[Dict[str, Any]] = []
        for i, line in enumerate(raw.splitlines(), 1):
            line = line.strip()
            if not line or line.startswith("#"):
                continue
            try:
                docs.append(json.loads(line))
            except json.JSONDecodeError as line_exc:
                raise ValueError(f"line {i}: invalid JSON ({line_exc})") from exc
        if not docs:
            raise ValueError("no JSON objects") from exc
        return docs
    if isinstance(payload, dict):
        return [payload]
    if isinstance(payload, list):
        if not payload:
            raise ValueError("empty JSON array")
        if all(isinstance(x, dict) and _has_blocks(x) for x in payload):
            return [{"events": payload}]
        docs = [x for x in payload if isinstance(x, dict)]
        if not docs:
            raise ValueError("JSON array contains no documents")
        return docs
    raise ValueError(f"unsupported top-level JSON type: {type(payload).__name__}")


def _summarize_corpus(sessions: List[Dict[str, Any]]) -> Dict[str, Any]:
    total = sum(s["summary"]["total_blocks"] for s in sessions)
    repeated = sum(s["summary"]["repeated_blocks"] for s in sessions)
    turns = sum(s["summary"]["turns"] for s in sessions)
    new = total - repeated
    return {
        "turns": turns,
        "total_blocks": total,
        "repeated_blocks": repeated,
        "new_blocks": new,
        "prefix_reuse_ratio": repeated / total if total else 0.0,
        "context_growth": new,
    }


def _fmt_time(v: Any) -> str:
    if v is None:
        return "-"
    return str(v)


def _print_human(results: List[Dict[str, Any]], corpus: Dict[str, Any]) -> None:
    for r in results:
        path = r["path"]
        print(f"\n{path}")
        if "error" in r:
            print(f"  ERROR: {r['error']}")
            continue
        for d in r["documents"]:
            for s in d["sessions"]:
                sid = s["session_id"]
                print(f"  session {sid}: {len(s['turns'])} turns")
                print(f"    {'idx':<4} {'time':<8} {'modality':<10} {'total':<6} {'repeat':<7} {'reuse':<6} {'new':<5}")
                for t in s["turns"]:
                    print(
                        f"    {t['index']:<4} {_fmt_time(t['timestamp']):<8} "
                        f"{t['modality']:<10} {t['total_blocks']:<6} "
                        f"{t['repeated_blocks']:<7} {t['prefix_reuse_ratio']:<6.4f} "
                        f"{t['new_blocks']:<5}"
                    )
                sm = s["summary"]
                print(
                    f"  summary: turns={sm['turns']} total={sm['total_blocks']} "
                    f"repeated={sm['repeated_blocks']} "
                    f"reuse={sm['prefix_reuse_ratio']:.4f} growth={sm['context_growth']}"
                )
    print("\nCorpus summary")
    print(
        f"  turns={corpus['turns']} total={corpus['total_blocks']} "
        f"repeated={corpus['repeated_blocks']} "
        f"reuse={corpus['prefix_reuse_ratio']:.4f} growth={corpus['context_growth']}"
    )


def _build_output(results: List[Dict[str, Any]], corpus: Dict[str, Any]) -> Dict[str, Any]:
    return {"files": results, "corpus": corpus}


def process_paths(paths: List[Path]) -> Tuple[List[Dict[str, Any]], List[str]]:
    results: List[Dict[str, Any]] = []
    errors: List[str] = []
    for p in paths:
        try:
            if not p.exists():
                raise FileNotFoundError(f"file not found: {p}")
            docs = _load_documents(p)
            file_docs: List[Dict[str, Any]] = []
            for i, doc in enumerate(docs):
                sessions = _extract_sessions(doc, f"{p.stem}#{i}")
                analyzed = [
                    {
                        "session_id": sid,
                        "turns": a["turns"],
                        "summary": a["summary"],
                    }
                    for sid, a in ((sid, _analyze_session(ev)) for sid, ev in sessions)
                ]
                file_docs.append({"document_index": i, "sessions": analyzed})
            results.append({"path": str(p), "documents": file_docs})
        except Exception as exc:
            msg = f"{p}: {exc}"
            errors.append(msg)
            results.append({"path": str(p), "error": msg})
    return results, errors


def main(argv: Optional[List[str]] = None) -> int:
    parser = argparse.ArgumentParser(
        description="Replay weka-trace-v1 documents and report prefix reuse."
    )
    parser.add_argument("traces", nargs="+", type=Path, help="weka-trace-v1 JSON files")
    parser.add_argument("--json", action="store_true", help="emit machine-readable JSON output")
    arguments = parser.parse_args(argv)

    results, errors = process_paths(arguments.traces)
    all_sessions = [
        s for r in results for d in r.get("documents", []) for s in d["sessions"]
    ]
    corpus = _summarize_corpus(all_sessions)

    if arguments.json:
        out = _build_output(results, corpus)
        json.dump(out, sys.stdout, indent=None)
        print()
    else:
        _print_human(results, corpus)

    if errors:
        for e in errors:
            print(e, file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
