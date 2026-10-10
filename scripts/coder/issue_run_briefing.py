#!/usr/bin/env python3
"""The briefing for `coder issue-run` (#11214), built by #11211's generator.

    issue_run_briefing.py < request.json > briefing.json

The request names the issue (number, title, body), the base commit, and the
files the decision steps chose (path, why, lines). This adapter reuses
`scripts/bench/briefed-ab/briefing.py` (`build` and `render`) and the
briefed agent's system prompt template from `ab.py`, so the terminal run
briefs its agent exactly as the A/B bench's arm B does. The only change is
where the files come from: the run's own decision steps (the context finder
of #11210 plus Jev/Clef), passed in, instead of `briefing.py`'s finder
lookup.

The answer is one JSON object: the briefing, its Markdown, the system
prompt, and the first message.
"""

from __future__ import annotations

import json
import os
import sys
from pathlib import Path

BENCH = Path(__file__).resolve().parents[1] / "bench" / "briefed-ab"


def main() -> None:
    request = json.load(sys.stdin)
    os.environ.setdefault("AB_REPO", request["repo"])
    sys.path.insert(0, str(BENCH))
    import ab  # noqa: E402  (after AB_REPO is set; ab imports briefing and common)
    import briefing  # noqa: E402
    from common import LEVERS  # noqa: E402

    chosen = [
        {"path": f["path"], "score": f.get("score"), "lines": f.get("lines", []),
         "why": f.get("why") or ["the decision steps chose it"]}
        for f in request["files"]
    ]
    briefing.finder_files = lambda task, levers: (chosen, "coder issue-run decision steps")
    levers = dict(LEVERS)
    levers["finder"] = "finder"
    levers["briefing_files"] = max(1, len(chosen))
    for key, value in (request.get("levers") or {}).items():
        levers[key] = value
    task = {
        "issue": request["issue"],
        "title": request["title"],
        "body": request["body"],
        "parent": request["base"],
    }
    built = briefing.build(task, levers)
    md = briefing.render(built)
    template = ab.TEMPLATES[levers["template"]]
    system = template + "\n\n" + md
    prompt = f"Complete issue #{request['issue']} as the briefing describes."
    json.dump({"briefing": built, "markdown": md, "system": system, "prompt": prompt}, sys.stdout)


if __name__ == "__main__":
    main()
