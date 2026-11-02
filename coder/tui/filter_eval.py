#!/usr/bin/env python3
"""Filtering (chapter 17): a labelled event stream through the Router.

100 events on four sources, each labelled by whether it should start work:
a candidate team chat, a candidate CI feed, an explicit_requests_only source
and an ignored one. The Router decides each: the fast
path in code, the candidates by System One in batches. Reported: the fraction
the fast path drops, the slow path's misses (work it dropped) and false starts
(chatter it started), the fast path's errors, and System One requests per
hundred events.

Coder's state goes under a scratch CODER_HOME with a session grant for
System One; the Baseten key comes from the usual place. Workers run on the
echo engine, so nothing but the decisions costs anything.

usage: python3 coder/tui/filter_eval.py [--json]
"""
import json, os, subprocess, sys, tempfile, time

REPO = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
CODER = os.path.join(REPO, "coder", "tui", "coder.sh")

CHAT_WORK = [
    "the build on main is failing in the parser tests, can someone fix it?",
    "can you add a --json flag to coder jobs?",
    "please review the change to router.coax for races",
    "the release script crashes when the tag already exists, investigate",
    "could someone look at why CI is red on main",
    "rename cs_redact to cs_hide everywhere please",
    "we need a unit test for rt_mail_address, can the agent write one",
    "the docs for chapter 14 still say forks can't resume, update them",
    "find out why coder status hangs on a cut log",
    "port the watchdog's backoff to the daemon",
    "bump the coax compiler pin and fix whatever breaks",
    "write a smoke check for the ACP session/load path",
    "the TUI flickers when resizing, please fix",
    "profile coder router --once and tell us where the time goes",
    "delete the dead code in plugins.coax that nothing calls",
]
CHAT_CHATTER = [
    "good morning all", "lol", "thanks, that looks great!", "anyone want lunch at noon?",
    "I'm out tomorrow", "nice work on the release", "who's watching the game tonight", "brb",
    "the parser bug was fixed yesterday, thanks to whoever did it", "standup in 5",
    "did you see the new office plants", "happy friday", "I merged my PR, all good",
    "the coffee machine is broken again", "welcome to the team, Sam!", "+1",
    "I'll be a bit late", "has anyone tried the new pizza place", "agreed", "ok sounds good",
    "the demo went well today", "can't make it to the meeting", "haha yes",
    "reminder: all-hands at 3", "the wifi is slow today", "congrats on shipping!",
    "I'm reading a good book on compilers", "see you all monday", "that was a fun offsite",
    "we hit 1,000 users today", "my laptop battery died", "thanks for the review yesterday",
    "is it cold in the office or just me", "going to grab lunch", "night all",
]
CI_WORK = [
    "build #812 on main failed: parser tests (3 failures)",
    "build #815 on main failed: link error in coder.coax",
    "main: smoke failed at 'watchdog: a worker whose process died ends as lost'",
    "build #820 on main failed: timeout in router tests",
    "nightly on main failed: 2 unit checks in session.coax",
    "build #824 on main failed: coax test exit 1",
]
CI_NOISE = [
    "build #811 on main passed", "build #813 on branch feature/tui failed: lint", "build #814 on main passed",
    "deploy to staging succeeded", "build #816 on main passed", "build #817 on branch wip/x failed: tests",
    "cache warmed for main", "build #818 on main passed", "dependabot opened 2 pull requests",
    "build #819 on branch exp/acp failed: smoke", "build #821 on main passed", "release v1.2.0 published",
    "build #822 on main passed", "build #823 on branch fix/typo passed",
]


def stream():
    ev = []
    for t in CHAT_WORK: ev.append(("chat", "sam", t, False, True))
    for t in CHAT_CHATTER: ev.append(("chat", "sam", t, False, False))
    for t in CI_WORK: ev.append(("ci", "ci-bot", t, False, True))
    for t in CI_NOISE: ev.append(("ci", "ci-bot", t, False, False))
    for i, t in enumerate(CHAT_WORK[:8]): ev.append(("team", "kim", t, True, True))
    for t in CHAT_WORK[8:14]: ev.append(("team", "kim", t, False, False))   # not addressed: explicit-only drops it
    for t in CHAT_WORK[:6]: ev.append(("team", "mallory", t, True, False))  # not on the author list
    for t in CHAT_CHATTER[:5] + CHAT_WORK[:5]: ev.append(("spam", "bot", t, False, False))
    return ev


def main():
    as_json = "--json" in sys.argv
    home = tempfile.mkdtemp(prefix="coder-filter-")
    env = dict(os.environ, CODER_HOME=home, BROWSER="/usr/bin/true")
    cdir = os.path.join(home, ".openagents", "coder")
    os.makedirs(os.path.join(cdir, "router"), exist_ok=True)
    with open(os.path.join(cdir, "router", "sources.json"), "w") as f:
        json.dump({"sources": {
            "chat": {"mode": "candidate", "authors": ["*"], "engine": "echo",
                     "policy": "A team chat. The agent works on requests about this repository; ignore social talk."},
            "ci": {"mode": "candidate", "authors": ["ci-bot"], "engine": "echo",
                   "policy": "CI notifications. Start work only when a build or check on main fails."},
            "team": {"mode": "explicit_requests_only", "authors": ["kim"], "engine": "echo"},
            "spam": {"mode": "ignore", "authors": ["*"]}}}, f)
    def cq(*a):
        return subprocess.run([CODER, *a], cwd=REPO, env=env, capture_output=True, text=True, timeout=300)
    g = cq("consent", "grant", "system-one", "--classes", "session", "--days", "1")
    if g.returncode != 0:
        print("consent grant failed:", g.stderr.strip()); return 2
    ev = stream()
    for i, (src, author, text, to_agent, _) in enumerate(ev):
        a = ["trigger", text, "--source", src, "--author", author, "--thread", "e%03d" % i]
        if to_agent: a.append("--to-agent")
        cq(*a)
    fates, batches, started = {}, [], time.time()
    while len(fates) < len(ev) and time.time() - started < 600:
        for l in cq("router", "--once", "--json").stdout.splitlines():
            if not l.startswith("{"): continue
            j = json.loads(l)
            if j.get("kind") == "batch": batches.append(j)
            th = j.get("thread", "")
            if th.startswith("e") and th not in fates and j.get("kind") == "trigger" and "worker" in j.get("fate", "") or \
               th.startswith("e") and th not in fates and j.get("fate", "").startswith("dropped"):
                fates[th] = j["fate"]
    FAST = ("dropped: source ignored", "dropped: author not allowed", "dropped: not addressed to the agent")
    rows = []
    for i, (src, author, text, to_agent, work) in enumerate(ev):
        f = fates.get("e%03d" % i, "none")
        rows.append({"source": src, "author": author, "text": text, "work": work, "fate": f,
                     "fast": f in FAST or f == "addressed to the agent: new worker",
                     "start": "new worker" in f})
    slow = [r for r in rows if not r["fast"]]
    fast = [r for r in rows if r["fast"]]
    rep = {
        "events": len(rows), "decided": sum(1 for r in rows if r["fate"] != "none"),
        "fast_dropped": sum(1 for r in fast if not r["start"]) / len(rows),
        "fast_errors": sum(1 for r in fast if r["start"] != r["work"]),
        "slow": len(slow),
        "slow_misses": [r["text"] for r in slow if r["work"] and not r["start"]],
        "slow_false_starts": [r["text"] for r in slow if not r["work"] and r["start"]],
        "no_decision": sum(1 for r in slow if r["fate"] == "dropped: no decision"),
        "requests": len(batches), "requests_per_100": 100.0 * len(batches) / len(rows),
        "batch_ms": [b["ms"] for b in batches], "batch_sizes": [b["events"] for b in batches],
    }
    if as_json:
        print(json.dumps({"report": rep, "rows": rows}))
    else:
        print("events %d, decided %d" % (rep["events"], rep["decided"]))
        print("fast path: dropped %.0f%% of all events, %d errors" % (100 * rep["fast_dropped"], rep["fast_errors"]))
        print("slow path: %d candidates, %d misses, %d false starts, %d without a decision" %
              (rep["slow"], len(rep["slow_misses"]), len(rep["slow_false_starts"]), rep["no_decision"]))
        for t in rep["slow_misses"]: print("  missed:", t)
        for t in rep["slow_false_starts"]: print("  false start:", t)
        print("System One: %d requests (%.1f per 100 events), sizes %s, ms %s" %
              (rep["requests"], rep["requests_per_100"], rep["batch_sizes"], rep["batch_ms"]))
    return 0 if rep["decided"] == rep["events"] and rep["fast_errors"] == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
