#!/usr/bin/env python3
"""Acceptance criteria from an issue's own text, as typed items (#11310).

The briefing used to list the issue's bullet points as "Required:" lines
and `verify` marked every one "the touched crates' tests ran and passed".
That counts only Rust tests and cannot tell a test from a capture, a
receipt, or a step only the owner can take.

This module asks one structured question of a model (Vertex AI Gemini,
Google first for anything paid; workspace rule: a typed extraction, not
keyword matching): what must be true for this issue to be done, in the
issue's own words, each item typed by what would show it, and mapped to
the briefing's own check ids (an enum built from the checks, so the model
can only name checks that exist). `verify` then reports each item with
its checks' results; items no check here can show (a capture, a receipt,
a deploy, an owner step, a measurement) say so instead of passing.

    acceptance.py ISSUE_JSON CHECKS_JSON     # prints {"items": [...], "source": ...}

Answers are cached by the digest of the issue text and the check ids. When
no Google credential is here, or the call fails, the items are the issue's
own bullet points with every required check, labelled as that fallback:
the briefing still renders, and the record says the typed extraction did
not run.
"""

from __future__ import annotations

import hashlib
import json
import os
import re
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
MODEL = os.environ.get("OA_ACCEPTANCE_MODEL", "gemini-2.5-flash")
KINDS = ("behavior", "test", "docs", "script", "check", "capture", "receipt", "deploy", "owner", "measurement")
# What a check on this computer cannot show.
UNCHECKABLE = ("capture", "receipt", "deploy", "owner", "measurement")
MAX_ITEMS = 8

PROMPT = """You read a GitHub issue for a software repository and list its acceptance criteria: what must be true of the change for the issue to be done.

Rules:
- Use the issue's own words. When it has a "Done when", "Acceptance", or "Fix" section, take the items from there; otherwise take the changes it asks for. Do not invent requirements. At most {max_items} items, each one short sentence.
- kind: behavior (code does something new or different), test (a test must exist or pass), docs (a document says something or its links work), script (a script or tool behaves a way), check (a named command must pass), capture (a screenshot, recording, or visual check), receipt (a live run, payment, or production proof), deploy (a release or deploy), owner (a step only the owner can take), measurement (a benchmark or report with numbers).
- check_ids: which of the checks below would show the item on a developer's computer. None for capture, receipt, deploy, owner, and measurement items, and none when no listed check covers it.

Checks available:
{checks}

Issue #{number}: {title}

{body}
"""


def schema(check_ids: list[str]) -> dict:
    ids = {"type": "ARRAY", "items": {"type": "STRING"}}
    if check_ids:
        ids["items"]["enum"] = check_ids
    return {
        "type": "OBJECT",
        "properties": {"items": {"type": "ARRAY", "items": {
            "type": "OBJECT",
            "properties": {"text": {"type": "STRING"}, "kind": {"type": "STRING", "enum": list(KINDS)},
                           "check_ids": ids},
            "required": ["text", "kind", "check_ids"]}}},
        "required": ["items"],
    }


def bullets(body: str) -> list[str]:
    """The issue's own bullet points (the fallback, and what the plan
    listed before)."""
    return [re.sub(r"^\s*(?:[-*]|\d+\.)\s*", "", line).strip()[:220]
            for line in (body or "").splitlines() if re.match(r"^\s*(?:[-*]|\d+\.)\s+\S", line)][:MAX_ITEMS]


def fallback(issue: dict, checks: list[dict], why: str) -> dict:
    required = [c["id"] for c in checks if c.get("required", True) and c.get("available", True)
                and not c["id"].startswith("fmt:")]
    items = [{"text": b, "kind": "behavior", "check_ids": required} for b in bullets(issue.get("body") or "")]
    if not items:
        items = [{"text": issue.get("title") or "the issue", "kind": "behavior", "check_ids": required}]
    return {"items": items, "source": f"issue bullets ({why})", "cost_usd": 0.0}


def cache_dir() -> Path:
    d = Path(os.environ.get("OA_ACCEPTANCE_CACHE", Path.home() / ".openagents" / "work" / "acceptance"))
    d.mkdir(parents=True, exist_ok=True)
    return d


def extract(issue: dict, checks: list[dict]) -> dict:
    """`{items: [{text, kind, check_ids}], source, cost_usd}`."""
    ids = [c["id"] for c in checks]
    key = hashlib.sha256(json.dumps([issue.get("title"), issue.get("body"), ids, MODEL]).encode()).hexdigest()[:24]
    hit = cache_dir() / f"{key}.json"
    if hit.exists():
        try:
            return json.loads(hit.read_text())
        except ValueError:
            pass
    if os.environ.get("OA_ACCEPTANCE") == "off":
        return fallback(issue, checks, "typed extraction off")
    try:
        sys.path.insert(0, str(ROOT / "scripts" / "filefind"))
        import cards  # noqa: E402  (Vertex AI generateContent with a response schema)
        gemini = cards.Gemini(MODEL)
        listing = "\n".join(f"- {c['id']}: {c.get('what', '')}" + ("" if c.get("available", True)
                                                                   else " (not runnable here)") for c in checks) \
            or "(none)"
        answer = gemini(PROMPT.format(max_items=MAX_ITEMS, checks=listing, number=issue.get("issue"),
                                      title=issue.get("title"), body=(issue.get("body") or "")[:12000]),
                        schema(ids), max_tokens=1500)
    except Exception as error:  # noqa: BLE001 - no credential or a failed call: say so, fall back
        return fallback(issue, checks, f"typed extraction unavailable: {type(error).__name__}")
    items = []
    for item in (answer or {}).get("items", [])[:MAX_ITEMS]:
        kind = item.get("kind") if item.get("kind") in KINDS else "behavior"
        cids = [] if kind in UNCHECKABLE else [i for i in item.get("check_ids") or [] if i in ids]
        text = " ".join(str(item.get("text") or "").split())[:240]
        if text:
            items.append({"text": text, "kind": kind, "check_ids": cids})
    if not items:
        return fallback(issue, checks, "the typed extraction returned no item")
    out = {"items": items, "source": f"typed extraction ({MODEL} on Vertex AI)", "cost_usd": round(gemini.dollars(), 6)}
    hit.write_text(json.dumps(out))
    return out


def main() -> None:
    issue = json.loads(Path(sys.argv[1]).read_text())
    checks = json.loads(Path(sys.argv[2]).read_text())
    print(json.dumps(extract(issue, checks), indent=2))


if __name__ == "__main__":
    main()
