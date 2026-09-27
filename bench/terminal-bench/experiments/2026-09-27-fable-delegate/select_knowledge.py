"""Select knowledge-base entries for a delegate's briefing (issue #9746).

Runs Coder's own knowledge search over the task instruction, then applies
a fixed rule to the ranked output:

- keep hits in rank order whose score is at least ``--min-score``;
- keep at most ``--max-entries`` of them;
- skip a hit whose entry text would take the kept text past
  ``--budget`` characters, and go on to the next one.

Each kept entry is the synced entry file, read whole, with its version and
its digest (the sha256 of the file, checked against the signed event's
``x`` tag). ``--note`` records the paragraph the briefing puts under the
section's heading in place of Coder One's default. The output is the JSON file the ``briefing_knowledge`` adapter
kwarg takes. It records the exact command, the search's full output, the
rule, and every hit's fate.

With ``--for-jev`` (series 6), the rule is only the search's top
``--limit`` hits (12 unless given), with no score floor, entry limit, or
budget: the file holds candidates, and Jev decides in the episode which
ones the briefing carries (``coder-one-delegate-fable-low-kb-jev``).

Usage::

    python3 select_knowledge.py --instruction TASK/instruction.md --out FILE
    python3 select_knowledge.py --for-jev --instruction TASK/instruction.md --out FILE
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import subprocess
import sys
from pathlib import Path

HIT = re.compile(r"^\s*(\d+)\.\s+([0-9.]+)\s+(\S+)\s+\[([^\]]*)\]")
VERSION = re.compile(r"^version:\s*(\d+)\s*$", re.MULTILINE)


def parse_hits(output: str) -> list[dict]:
    """The ranked hits in ``kb search`` output: rank, score, id, and the
    bracketed kind, status, and author."""
    hits = []
    for line in output.splitlines():
        match = HIT.match(line)
        if match:
            kind, status, author = (part.strip() for part in match.group(4).split(","))
            hits.append(
                {
                    "rank": int(match.group(1)),
                    "score": float(match.group(2)),
                    "id": match.group(3),
                    "kind": kind,
                    "status": status,
                    "author": author,
                }
            )
    return hits


def entry_file(remote: Path, entry_id: str) -> tuple[Path, str | None]:
    """The synced entry file and the digest its signed event carries."""
    found = sorted(remote.glob(f"*/{entry_id}.md"))
    if len(found) != 1:
        raise SystemExit(f"expected one synced file for {entry_id}, found {len(found)}")
    event = found[0].with_suffix(".event.json")
    digest = None
    if event.is_file():
        tags = json.loads(event.read_text()).get("tags") or []
        digest = next((tag[1] for tag in tags if tag and tag[0] == "x"), None)
    return found[0], digest


def select(hits: list[dict], read, min_score: float, max_entries: int, budget: int):
    """Apply the rule. ``read(id)`` returns ``(text, sha256, version)``."""
    entries, fates, used = [], [], 0
    for hit in hits:
        if hit["score"] < min_score:
            fates.append({**hit, "fate": f"below the {min_score} score floor"})
            continue
        if len(entries) >= max_entries:
            fates.append({**hit, "fate": f"past the {max_entries}-entry limit"})
            continue
        text, digest, version = read(hit["id"])
        size = len(text)
        if used + size > budget:
            fates.append({**hit, "chars": size, "fate": f"past the {budget}-character budget"})
            continue
        used += size
        entries.append(
            {
                "id": hit["id"],
                "version": version,
                "sha256": digest,
                "score": hit["score"],
                "rank": hit["rank"],
                "status": hit["status"],
                "chars": size,
                "text": text,
            }
        )
        fates.append({**hit, "chars": size, "fate": "selected"})
    return entries, fates


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--instruction", required=True, type=Path)
    parser.add_argument("--out", required=True, type=Path)
    parser.add_argument("--microcoder", default=str(Path("~/.local/bin/microcoder").expanduser()))
    parser.add_argument("--dir", default=str(Path("~/.openagents/knowledge/empty-local").expanduser()),
                        help="the local knowledge directory; empty, so only synced entries count")
    parser.add_argument("--remote", default=str(Path("~/.openagents/knowledge/remote").expanduser()))
    parser.add_argument("--limit", type=int, default=10)
    parser.add_argument("--min-score", type=float, default=0.45)
    parser.add_argument("--max-entries", type=int, default=6)
    parser.add_argument("--budget", type=int, default=16_000)
    parser.add_argument("--note", help="the paragraph under the section's heading, in place of the default")
    parser.add_argument("--for-jev", action="store_true",
                        help="write the top hits as candidates for Jev to choose from in the episode")
    args = parser.parse_args(argv)
    if args.for_jev:
        if "--limit" not in (argv if argv is not None else sys.argv[1:]):
            args.limit = 12
        args.min_score, args.max_entries, args.budget = float("-inf"), args.limit, 10**9

    instruction = args.instruction.read_text()
    command = [
        args.microcoder, "kb", "search", instruction, "--candidates",
        "--dir", args.dir, "--remote", args.remote, "--limit", str(args.limit),
    ]
    done = subprocess.run(command, capture_output=True, text=True, timeout=300)
    if done.returncode != 0:
        sys.stderr.write(done.stdout + done.stderr)
        return done.returncode
    hits = parse_hits(done.stdout)
    remote = Path(args.remote)

    def read(entry_id: str):
        path, signed = entry_file(remote, entry_id)
        raw = path.read_bytes()
        digest = hashlib.sha256(raw).hexdigest()
        if signed is not None and signed != digest:
            raise SystemExit(f"{entry_id}: file sha256 {digest} != signed x tag {signed}")
        text = raw.decode()
        version = VERSION.search(text)
        return text, digest, int(version.group(1)) if version else None

    entries, fates = select(hits, read, args.min_score, args.max_entries, args.budget)
    shown = [part if part != instruction else "<instruction>" for part in command]
    doc = {
        "schema": "openagents.coder_one.briefing_knowledge.v1",
        "instruction": {
            "path": str(args.instruction),
            "sha256": hashlib.sha256(instruction.encode()).hexdigest(),
        },
        "command": shown,
        "search_output": done.stdout,
        "rule": {
            "candidates_for_jev": True,
            "limit": args.limit,
            "order": "search rank, no score floor; Jev chooses in the episode",
        } if args.for_jev else {
            "min_score": args.min_score,
            "max_entries": args.max_entries,
            "budget_chars": args.budget,
            "order": "search rank; a hit that would pass the budget is skipped",
        },
        "hits": fates,
        **({"note": args.note} if args.note else {}),
        "entries": entries,
    }
    args.out.write_text(json.dumps(doc, indent=2, ensure_ascii=False) + "\n")
    for fate in fates:
        print(f"{fate['rank']:>2}. {fate['score']:.3f} {fate['id']}: {fate['fate']}")
    print(f"{len(entries)} entries, {sum(e['chars'] for e in entries)} characters -> {args.out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
