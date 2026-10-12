#!/usr/bin/env python3
"""File-representation tiers for one batched `/v1/systemone` request.

Batching file relevance means sending a *representation* of each candidate,
not the file: a request holds one `state` of at most 128 KiB inside the
16,384-token budget, so whole files do not fit past a few candidates. This
builds four increasing tiers of per-file detail for one candidate set and
sends each as ONE request — one `noul` per file — then prints the real
`prompt_tokens`, answers and latency the server reported. It is the script
behind the tiers table in `docs/inference/clef-batching.md`.

    AB_REPO=. python3 -I file_tiers.py            # against the Mac Metal lane
    AB_REPO=. python3 -I file_tiers.py --base http://127.0.0.1:18098 --tiers 1,2,3

Every representation comes from code in this repository:

  tier 1  path, finder rank and confidence        (filefind.py query --json)
  tier 2  + the finder's reasons                  (the same `reasons` field)
  tier 3  + the file's outline                    (briefing.py outline())
  tier 4  + code excerpts around matched lines    (briefing.py excerpt())

`AB_REPO` must point at the checkout to read from; the default is the current
directory. The candidate set is the six files a router/first-contact issue
tends to rank, real paths from this tree.
"""
import argparse, json, os, subprocess, sys, time, urllib.request

REPO = os.path.abspath(os.environ.get("AB_REPO", os.getcwd()))
sys.path.insert(0, os.path.join(REPO, "scripts/bench/briefed-ab"))
os.environ.setdefault("AB_REPO", REPO)
import briefing  # noqa: E402  (after AB_REPO is set)

# What the file_questions step ranks for a router/first-contact issue: real
# paths, with the finder fields filefind.py emits.
FILES = [
    {"path": "crates/coder/src/router/judge.rs", "rank": 1, "conf": 0.71,
     "reasons": ["issue names `route`", "defines `route_instructions`", "embedding rank 2 (cos 0.63)"]},
    {"path": "crates/coder/src/first.rs", "rank": 2, "conf": 0.62,
     "reasons": ["crate `coder` named in the issue", "changes with the seeds (co 0.58)"]},
    {"path": "crates/coder/src/cli_route.rs", "rank": 3, "conf": 0.55,
     "reasons": ["issue names `cli_group`", "mentions `CliRoute`"]},
    {"path": "crates/coder/src/select.rs", "rank": 4, "conf": 0.44,
     "reasons": ["defines `route`", "changes with the top-ranked files (co 0.51)"]},
    {"path": "crates/coder/src/codebase.rs", "rank": 5, "conf": 0.39,
     "reasons": ["under `crates/coder/src` named in the issue"]},
    {"path": "crates/jev/src/lib.rs", "rank": 6, "conf": 0.36,
     "reasons": ["crate `jev` named in the issue", "defines `Client`"]},
]
ISSUE = ("Issue #11247: Only what a rule reads\n\nThe router asks every question in its set, but a rule reads only some. "
         "A `cli_group` question is asked even when the route is not a CLI route, which costs a round trip. "
         "Ask only the questions a rule reads.")
QUESTION = "Is file {id} ({path}) relevant to solving the issue?"


def git(*args):
    return subprocess.run(["git", "-C", REPO, *args], capture_output=True, text=True).stdout


REV = git("rev-parse", "HEAD").strip()


def outline_block(path):
    rows = git("show", f"{REV}:{path}").splitlines()
    items = briefing.outline(rows, 60)
    body = [f"--- outline: {len(items)} items of {len(rows)} lines ---"]
    body += [f"{i:>5}  {rows[i - 1]}" for i in items]
    return "\n".join(body)


def excerpt_block(path, hit_lines):
    return briefing.excerpt(REV, path, hit_lines, {"excerpt_lines": 60, "excerpt": "windows"})


def state_tier(tier):
    parts = [ISSUE, "", "CANDIDATE FILES:"]
    for i, f in enumerate(FILES, 1):
        parts.append(f"FILE f{i}: {f['path']}")
        parts.append(f"  finder: rank {f['rank']}, confidence {f['conf']:.2f}")
        if tier >= 2:
            parts.append("  reasons: " + "; ".join(f["reasons"]))
        if tier >= 3:
            parts.append("  outline:")
            parts += ["    " + ln for ln in outline_block(f["path"]).splitlines()]
        if tier >= 4:
            parts.append("  excerpts:")
            parts += ["    " + ln for ln in excerpt_block(f["path"], [10, 30]).splitlines()]
    return "\n".join(parts)


def send(base, state, n, model):
    questions = {f"f{i}": {"type": "noul", "instructions": QUESTION.format(id=f"f{i}", path=FILES[i - 1]["path"])}
                 for i in range(1, n + 1)}
    body = json.dumps({"model": model, "state": state, "questions": questions}).encode()
    req = urllib.request.Request(base + "/v1/systemone", data=body,
                                 headers={"Content-Type": "application/json", "User-Agent": "file-tiers/1"})
    t0 = time.perf_counter()
    with urllib.request.urlopen(req, timeout=300) as x:
        resp = json.loads(x.read())
    ps = resp.get("psionic", {})
    timing = ps.get("timing", {})
    return {"chars": len(state), "prompt_tokens": ps.get("prompt_tokens"),
            "prefill_ms": timing.get("prefill_ms"), "head_ms": timing.get("head_ms"),
            "wall_ms": round((time.perf_counter() - t0) * 1000), "answered": len(resp.get("answers", {})),
            "answers": {k: round(v["noul"], 3) for k, v in resp.get("answers", {}).items()}}


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--base", default="http://127.0.0.1:18098", help="the /v1/systemone server")
    ap.add_argument("--model", default="clef-flash")
    ap.add_argument("--tiers", default="1,2,3,4", help="comma list of tiers to send")
    a = ap.parse_args()
    for tier in [int(t) for t in a.tiers.split(",")]:
        out = send(a.base, state_tier(tier), len(FILES), a.model)
        out["tier"] = tier
        print(json.dumps(out), flush=True)


if __name__ == "__main__":
    main()
