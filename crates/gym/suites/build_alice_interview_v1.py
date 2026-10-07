"""Build the `alice-interview-v1` fixture and suite.

The fixture is a synthetic workshop agent's journal and memory over three
weeks, written in the shapes `crates/coder/src/task/agent.rs` and
`agent_memory.rs` write, with the texts the host writes for each event.
The suite asks memory questions whose answers code checks against it.

Run from the repository root:

    python3 crates/gym/suites/build_alice_interview_v1.py

It rewrites `crates/gym/suites/alice-interview-v1/` and
`crates/gym/suites/alice-interview-v1.json`. The output is deterministic, so
a rerun with no edits leaves every digest alone.
"""

import calendar
import datetime
import hashlib
import json
import os

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, "alice-interview-v1")
NAME = "alice-interview-v1"
JOURNAL_SCHEMA = "openagents.agent-journal-entry.v1"
MEMORY_SCHEMA = "openagents.agent-memory-entry.v1"
FIXTURE_SCHEMA = "openagents.agent-interview-fixture.v1"
WORKSPACE = "/work/openagents"
AGENT = "alice"


def at(day, hour, minute=0):
    """Unix seconds for 2026-09-DAY HOUR:MINUTE UTC."""
    return calendar.timegm(datetime.datetime(2026, 9, day, hour, minute).timetuple())


AS_OF = calendar.timegm(datetime.datetime(2026, 9, 28, 9, 0).timetuple())


class Fixture:
    def __init__(self):
        self.journal = []
        self.memory = []

    def row(self, when, kind, text, status=None, sender=None):
        entry = {"schema": JOURNAL_SCHEMA, "at": when, "kind": kind, "text": text}
        if status is not None:
            entry["status"] = status
        if sender is not None:
            entry["from"] = sender
        self.journal.append(entry)
        return len(self.journal)

    def remember(self, when, kind, author, text, sources, state=None):
        """Adds a memory entry and the journal row the host writes for it."""
        ident = len(self.memory) + 1
        if state is None:
            state = "candidate" if kind == "preference" and author != "owner" else "active"
        self.memory.append(
            {
                "schema": MEMORY_SCHEMA,
                "v": 1,
                "requires": [],
                "id": ident,
                "kind": kind,
                "state": state,
                "author": author,
                "text": text,
                "at": when,
                "sources": sources,
            }
        )
        verb = "proposed" if state == "candidate" else "wrote"
        self.row(when, "memory", f"{verb} {kind} entry {ident}: {text}")
        return ident

    def decide(self, when, ident, accept):
        entry = self.memory[ident - 1]
        entry["state"] = "active" if accept else "rejected"
        word = "accepted" if accept else "rejected"
        return self.row(when, "memory", f"the owner {word} preference {ident}")

    def request(self, when, text, sender="owner"):
        return self.row(when, "request", text, sender=sender)

    def briefed(self, when):
        """The selection receipt: the notes and accepted preferences so far."""
        carried = [
            str(e["id"])
            for e in self.memory
            if e["state"] == "active" and e["kind"] in ("note", "preference")
        ]
        if carried:
            self.row(when, "memory", "the briefing carried memory entries " + ", ".join(carried))

    def terminal(self, when, text, commands, reply, sender="owner", plan=None):
        """A terminal-mode request: commands with exit statuses, then a report.

        A command is (text, bytes, status); status None means it did not
        finish. A run whose last command exits 0 is an outcome in memory.
        """
        first = self.request(when, text, sender)
        self.briefed(when)
        if plan:
            self.row(when + 20, "plan", plan)
        clock = when + 60
        last = None
        for command, size, status in commands:
            if status is None:
                self.row(clock, "ran", f"{command}: did not finish")
            else:
                self.row(clock, "ran", f"{command} ({size} bytes of output)", status=status)
            last = status
            clock += 90
        if last == 0:
            self.row(clock, "report", reply)
            self.remember(clock, "outcome", "host", f"{clock}: {text} (ok exit 0)", [f"journal:{first}"])
        else:
            self.row(clock, "failed", reply)
        return first

    def task(self, when, text, task, goal, title, sender="owner"):
        """A task-mode request that ends at the Merge station."""
        first = self.request(when, text, sender)
        self.row(when + 30, "task", f"made task {task} for goal {goal} in openagents, in her own worktree")
        self.row(when + 3600, "task", f"task {task} waits at the Merge station")
        self.row(
            when + 3600,
            "report",
            f'My change for "{title}" waits for you at the Merge station: Merge, '
            "Request changes, or Reject.",
        )
        return first

    def merged(self, when, task):
        pos = self.row(when, "task", f"task {task} merged by the owner")
        self.remember(
            when, "outcome", "host", f"task {task} merged by the owner at the Merge station", [f"task:{task}"]
        )
        return pos

    def rejected(self, when, task):
        return self.row(when, "task", f"task {task} rejected by the owner")


def build():
    f = Fixture()
    refs = {}
    jobs = {"nightly-check": 0, "watch-issues": 0, "keep-green": 0}
    paused = (at(19, 18), at(21, 9))

    def fire(when, job, max_occurrences=30):
        if paused[0] <= when < paused[1]:
            return f.row(when, "job", f"job {job} refused and skipped: alice is paused")
        jobs[job] += 1
        return f.row(when, "job", f"job {job} fired, occurrence {jobs[job]} of {max_occurrences}")

    def nightly(day):
        when = at(day, 2)
        pos = fire(when, "nightly-check")
        if f.journal[pos - 1]["text"].endswith("paused"):
            return
        gym = 101 if day == 15 else 0
        if gym:
            first = f.request(when + 5, "Run the nightly checks: cargo test -p atif, then cargo test -p gym.", "job:nightly-check")
            f.briefed(when + 5)
            f.row(when + 65, "ran", "cargo test -p atif (5310 bytes of output)", status=0)
            refs["gym-failed"] = f.row(when + 155, "ran", "cargo test -p gym (6604 bytes of output)", status=101)
            refs["gym-failed-report"] = f.row(when + 160, "failed", "A check failed: cargo test -p gym exited 101.")
            refs["gym-failed-request"] = first
        else:
            f.terminal(
                when + 5,
                "Run the nightly checks: cargo test -p atif, then cargo test -p gym.",
                [("cargo test -p atif", 5310, 0), ("cargo test -p gym", 6588, 0)],
                "The nightly checks passed: atif and gym, exit 0.",
                sender="job:nightly-check",
            )

    def watch(day):
        when = at(day, 8)
        pos = fire(when, "watch-issues")
        if f.journal[pos - 1]["text"].endswith("paused"):
            refs.setdefault("watch-skipped", []).append(pos)
            return
        if day == 16:
            first = f.request(when + 5, "Look at open issues labeled agent and pick one you can do.", "job:watch-issues")
            f.row(when + 65, "ran", "gh issue list --label agent --state open (1022 bytes of output)", status=0)
            f.row(when + 70, "plan", "Plan: take issue 10412, document the nightly-check job template.")
            f.row(when + 100, "task", "made task t-110 for goal g-61 in openagents, in her own worktree")
            refs["t-110-waits"] = f.row(when + 3700, "task", "task t-110 waits at the Merge station")
            f.row(
                when + 3700,
                "report",
                'My change for "Document the nightly-check job template" waits for you at the Merge '
                "station: Merge, Request changes, or Reject.",
            )
            refs["t-110-request"] = first
            return
        f.terminal(
            when + 5,
            "Look at open issues labeled agent and pick one you can do.",
            [("gh issue list --label agent --state open", 1022, 0)],
            "No open issue labeled agent fits today.",
            sender="job:watch-issues",
        )

    def keep_green(day):
        when = at(day, 3)
        pos = fire(when, "keep-green")
        if f.journal[pos - 1]["text"].endswith("paused"):
            return
        if day == 25:
            f.request(when + 5, "Keep it green: cargo test -p supervise.", "job:keep-green")
            f.row(when + 65, "ran", "cargo test -p supervise (4410 bytes of output)", status=101)
            f.row(when + 70, "failed", "A check failed on the default branch: cargo test -p supervise exited 101.")
            refs["t-129-request"] = f.task(
                when + 120,
                "A check failed on the default branch: cargo test -p supervise exited 101. Fix it in "
                "your own worktree and bring the change to the Merge station. Never merge.",
                "t-129",
                "g-70",
                "Fix the supervise deadline test",
                sender="job:keep-green",
            )
            return
        f.terminal(
            when + 5,
            "Keep it green: cargo test -p supervise.",
            [("cargo test -p supervise", 4398, 0)],
            "The supervise tests pass, exit 0.",
            sender="job:keep-green",
        )

    # Week one.
    f.row(at(7, 9), "created", f"the owner made alice in {WORKSPACE}")
    f.row(at(7, 9, 1), "keyed", "made alice's key")
    f.row(at(7, 9, 2), "keyed", "the owner attested alice's key")
    refs["release-request"] = f.request(at(7, 9, 10), "remember that the release branch is cut on Thursdays")
    refs["release-note"] = f.remember(at(7, 9, 10), "note", "owner", "the release branch is cut on Thursdays", [])
    f.row(at(7, 9, 10), "report", f"I'll remember that (memory entry {refs['release-note']}).")
    f.terminal(
        at(7, 9, 30),
        "run the atif tests",
        [("cargo test -p atif", 5310, 0)],
        "The atif tests passed: 214 tests, exit 0.",
        plan="Plan: run cargo test -p atif in /work/openagents.",
    )
    commit_request = f.request(at(7, 14), "always write commit messages in the imperative mood, under 72 characters")
    refs["commit-pref"] = f.remember(
        at(7, 14),
        "preference",
        "agent",
        "The owner said: always write commit messages in the imperative mood, under 72 characters",
        [f"journal:{commit_request}"],
    )
    f.row(at(7, 14), "report", "I'll follow that once you accept it at F2.")

    refs["t-101-request"] = f.task(
        at(8, 10),
        "fix the flaky timeout in the coder-pty replay test",
        "t-101",
        "g-55",
        "Fix the flaky timeout in the coder-pty replay test",
    )
    refs["commit-accept"] = f.decide(at(8, 16), refs["commit-pref"], True)
    f.remember(
        at(8, 16, 5),
        "project",
        "agent",
        "coder-pty's replay test waits on a 2 second idle timer; a slow machine needs the 10 second bound",
        [f"journal:{refs['t-101-request']}"],
    )

    refs["t-101-merged"] = f.merged(at(9, 11), "t-101")
    f.terminal(
        at(9, 15),
        "what does the gym ledger record?",
        [('rg -n "LockedRead" crates/gym/src', 2210, 0)],
        "The ledger records each locked read: the suite digest, what it was spent on, why, and when.",
    )
    refs["nightly-added"] = f.row(at(9, 20), "job", "the owner added job nightly-check (Nightly checks), off")
    f.row(at(9, 20, 1), "job", "job nightly-check turned on")

    nightly(10)
    relay_request = f.request(at(10, 10), "remember that the staging relay listens on port 7447")
    refs["relay-request"] = relay_request
    refs["relay-note"] = f.remember(at(10, 10), "note", "owner", "the staging relay listens on port 7447", [])
    f.row(at(10, 10), "report", f"I'll remember that (memory entry {refs['relay-note']}).")

    nightly(11)
    f.request(at(11, 10), "push the docs fix to main")
    f.briefed(at(11, 10))
    f.row(at(11, 10, 1), "plan", "Plan: push the committed docs fix to main.")
    f.row(at(11, 10, 2), "proposed", "git push origin HEAD:main (it publishes to a remote)")
    refs["push-rejected"] = f.row(at(11, 10, 4), "rejected", "git push origin HEAD:main")
    f.row(at(11, 10, 5), "report", "You rejected the push, so nothing left this computer.")
    push_request = f.request(at(11, 10, 20), "never push to main yourself")
    refs["push-pref"] = f.remember(
        at(11, 10, 20), "preference", "agent", "The owner said: never push to main yourself", [f"journal:{push_request}"]
    )
    f.row(at(11, 10, 20), "report", "I'll follow that once you accept it at F2.")

    nightly(12)
    f.decide(at(12, 9), refs["push-pref"], True)
    refs["lagrange-request"] = f.terminal(
        at(12, 11),
        "why does the verse-lagrange station-keeping test take 40 seconds?",
        [("cargo test -p verse-lagrange station_keeping -- --nocapture", 8120, 0)],
        "The station-keeping test integrates 3,000 orbits with RK4 at a 60 second step; most of the 40 "
        "seconds is that loop.",
    )
    f.remember(
        at(12, 11, 10),
        "project",
        "agent",
        "verse-lagrange's station-keeping test integrates 3,000 orbits; it is slow by design",
        [f"journal:{refs['lagrange-request']}"],
    )

    nightly(13)

    # Week two.
    nightly(14)
    f.request(at(14, 9), "add a watch-issues job for issues labeled agent")
    refs["watch-added"] = f.row(at(14, 9, 1), "job", "the owner added job watch-issues (Watch issues), off")
    f.row(at(14, 9, 2), "job", "job watch-issues turned on")
    f.row(at(14, 9, 2), "report", "The watch-issues job runs each morning at 08:00.")
    refs["t-105-request"] = f.task(
        at(14, 13),
        "update the everglade-web README with the wasm check command",
        "t-105",
        "g-58",
        "Name the wasm check in the everglade-web README",
    )
    refs["wasm-project"] = f.remember(
        at(14, 14, 5),
        "project",
        "agent",
        "everglade-web builds only for wasm32-unknown-unknown; check it with cargo check -p everglade-web "
        "--target wasm32-unknown-unknown",
        [f"journal:{refs['t-105-request']}"],
    )

    nightly(15)
    watch(15)
    f.request(at(15, 10), "look at why the gym tests failed last night")
    f.briefed(at(15, 10))
    f.row(at(15, 10, 1), "ran", "cargo test -p gym store:: (6600 bytes of output)", status=101)
    f.row(at(15, 10, 3), "ran", "git log -3 --oneline crates/gym (310 bytes of output)", status=0)
    f.row(
        at(15, 10, 5),
        "report",
        "The store test fails because a new row schema was added without updating the allowlist test.",
    )
    refs["t-108-request"] = f.task(
        at(15, 11), "fix the gym allowlist test", "t-108", "g-59", "Name the new row schema in the allowlist test"
    )
    refs["t-108-merged"] = f.merged(at(15, 17), "t-108")

    nightly(16)
    watch(16)
    refs["t-105-merged"] = f.merged(at(16, 9, 30), "t-105")
    f.request(at(16, 10), "remember that Jev thresholds need a measurement document before code trusts them")
    refs["jev-note"] = f.remember(
        at(16, 10), "note", "owner", "Jev thresholds need a measurement document before code trusts them", []
    )
    f.row(at(16, 10), "report", f"I'll remember that (memory entry {refs['jev-note']}).")

    nightly(17)
    watch(17)
    refs["t-118-request"] = f.task(
        at(17, 11), "bump tokio to 1.48 across the workspace", "t-118", "g-63", "Bump tokio to 1.48"
    )
    refs["t-118-rejected"] = f.rejected(at(17, 16), "t-118")
    fmt_request = f.request(at(17, 16, 30), "from now on run cargo fmt before every commit")
    refs["fmt-pref"] = f.remember(
        at(17, 16, 30),
        "preference",
        "agent",
        "The owner said: from now on run cargo fmt before every commit",
        [f"journal:{fmt_request}"],
    )
    f.row(at(17, 16, 30), "report", "I'll follow that once you accept it at F2.")

    nightly(18)
    watch(18)
    f.request(at(18, 10), "clean the build directory")
    f.briefed(at(18, 10))
    refs["rm-refused"] = f.row(at(18, 10, 1), "refused", "rm -rf target")
    f.row(at(18, 10, 2), "report", "The host refused rm -rf target, so I left the build directory alone.")
    full_request = f.request(at(18, 15), "i prefer that you never run the full workspace tests")
    refs["full-pref"] = f.remember(
        at(18, 15),
        "preference",
        "agent",
        "The owner said: i prefer that you never run the full workspace tests",
        [f"journal:{full_request}"],
    )
    f.row(at(18, 15), "report", "I'll follow that once you accept it at F2.")

    nightly(19)
    watch(19)
    refs["full-rejected"] = f.decide(at(19, 9), refs["full-pref"], False)
    refs["paused"] = f.row(at(19, 18), "control", "the owner paused alice")

    nightly(20)
    watch(20)

    # Week three.
    nightly(21)
    watch(21)
    refs["resumed"] = f.row(at(21, 9), "control", "the owner resumed alice")

    nightly(22)
    watch(22)
    f.request(at(22, 10), "add a keep-green job for the supervise crate")
    refs["green-added"] = f.row(at(22, 10, 1), "job", "the owner added job keep-green (Keep supervise green), off")
    f.row(at(22, 10, 2), "job", "job keep-green turned on")
    f.row(at(22, 10, 2), "report", "The keep-green job runs cargo test -p supervise each night at 03:00.")
    f.terminal(
        at(22, 14),
        "run the coder-boundary tests",
        [("cargo test -p coder-boundary", 7400, 0)],
        "The coder-boundary tests passed, exit 0.",
    )

    nightly(23)
    keep_green(23)
    watch(23)
    refs["verse-request"] = f.terminal(
        at(23, 10),
        "run the verse tests in release mode",
        [("cargo test -p verse --release", 0, None)],
        "The command did not finish: cargo test -p verse --release.",
    )
    refs["verse-timeout"] = refs["verse-request"] + 2
    f.remember(
        at(23, 10, 30),
        "project",
        "agent",
        "verse's release-mode tests need more than the 20 minute command bound",
        [f"journal:{refs['verse-request']}"],
    )

    nightly(24)
    keep_green(24)
    watch(24)
    refs["t-125-request"] = f.task(
        at(24, 9, 15),
        "update the gym README's ledger section to name the lock file",
        "t-125",
        "g-67",
        "Name the ledger's lock file in the gym README",
    )
    refs["t-125-merged"] = f.merged(at(24, 15), "t-125")

    nightly(25)
    keep_green(25)
    watch(25)
    refs["t-129-merged"] = f.merged(at(25, 14), "t-129")

    nightly(26)
    keep_green(26)
    watch(26)
    refs["t-131-request"] = f.task(
        at(26, 10), "add a --json flag to coder activity", "t-131", "g-72", "Add --json to coder activity"
    )
    refs["t-131-merged"] = f.merged(at(26, 17), "t-131")

    nightly(27)
    keep_green(27)
    watch(27)
    refs["nightly-last"] = len(f.journal) - 1
    f.terminal(
        at(27, 12),
        "summarize this week's merged changes",
        [("git log --since=2026-09-21 --oneline", 640, 0)],
        "This week the owner merged t-125, t-129, and t-131.",
    )
    refs["nightly-count"] = jobs["nightly-check"]
    refs["nightly-last-fired"] = max(
        i + 1 for i, e in enumerate(f.journal) if e["text"].startswith("job nightly-check fired")
    )
    return f, refs


def dated(choices_day):
    month, day = choices_day
    return [f"september {day}", f"sep {day}", f"2026-09-{day:02d}", f"{day} september"]


def items(refs, fixture, digest):
    j = lambda pos: f"journal:{pos}"  # noqa: E731
    m = lambda ident: f"memory:{ident}"  # noqa: E731
    merged = ["t-101", "t-105", "t-108", "t-125", "t-129", "t-131"]
    others = lambda keep: [[t] for t in merged if t != keep]  # noqa: E731

    def item(ident, partition, question, truth, check_all, sources, none=None, rule=None):
        check = {"all": check_all}
        if none:
            check["none"] = none
        return {
            "id": f"memory/{ident}",
            "family": "memory",
            "kind": "recall",
            "state": {
                "fixture": NAME,
                "fixture_digest": digest,
                "as_of": AS_OF,
                "check": check,
                "sources": sources,
            },
            "question": {"text": question},
            "truth": truth,
            "partition": partition,
            "label_source": "author",
            "label_rule": rule
            or "the fact the cited fixture records state; an answer passes when it names every required term and no forbidden one",
        }

    out = [
        item("merged-sep-09", "calibration", "Which task did the owner merge on September 9?",
             "Task t-101, the fix for the flaky timeout in the coder-pty replay test.",
             [["t-101"]], [j(refs["t-101-merged"])], none=others("t-101")),
        item("release-branch", "calibration", "What did the owner ask you to remember about the release branch?",
             "That the release branch is cut on Thursdays.",
             [["thursday"]], [m(refs["release-note"])]),
        item("push-rejected", "calibration", "Which command did the owner reject on September 11?",
             "git push origin HEAD:main, when I tried to push the docs fix to main.",
             [["git push"]], [j(refs["push-rejected"])]),
        item("commit-preference", "calibration", "What preference about commit messages did the owner accept?",
             "Write commit messages in the imperative mood, under 72 characters.",
             [["imperative"], ["72"]], [m(refs["commit-pref"]), j(refs["commit-accept"])]),
        item("gym-failure", "calibration", "Which crate's tests failed in the nightly checks on September 15, and with what exit status?",
             "The gym crate's tests: cargo test -p gym exited 101.",
             [["gym"], ["101"]], [j(refs["gym-failed"]), j(refs["gym-failed-report"])]),
        item("relay-port", "calibration", "Which port did the owner tell you the staging relay listens on?",
             "Port 7447.",
             [["7447"]], [m(refs["relay-note"])]),
        item("tokio-rejected", "calibration", "Which change did the owner reject at the Merge station on September 17?",
             "Task t-118, the change that bumped tokio to 1.48.",
             [["t-118", "tokio"]], [j(refs["t-118-rejected"]), j(refs["t-118-request"])]),

        item("merged-sep-15", "development", "Which task did the owner merge on September 15?",
             "Task t-108, the fix for the gym allowlist test.",
             [["t-108"]], [j(refs["t-108-merged"])], none=others("t-108")),
        item("fmt-candidate", "development", "Which preference you proposed is still waiting for the owner's decision?",
             "Run cargo fmt before every commit; the owner said it on September 17 and hasn't accepted it yet.",
             [["cargo fmt"]], [m(refs["fmt-pref"])],
             none=[["imperative"], ["push to main"]]),
        item("nightly-job", "development", "What standing job did the owner add on September 9?",
             "The nightly-check job, which runs the atif and gym tests each night.",
             [["nightly-check", "nightly check"]], [j(refs["nightly-added"])]),
        item("watch-skipped", "development", "Why was the watch-issues job skipped on September 20?",
             "The owner had paused me, so the job was refused and skipped.",
             [["paused"]], [j(refs["watch-skipped"][0])]),
        item("lagrange-question", "development", "What did the owner ask about the verse-lagrange crate?",
             "Why its station-keeping test takes 40 seconds.",
             [["station-keeping", "station keeping"]], [j(refs["lagrange-request"])]),
        item("rm-refused", "development", "What did the host refuse to run on September 18?",
             "rm -rf target, when the owner asked me to clean the build directory.",
             [["rm -rf"]], [j(refs["rm-refused"])]),
        item("wasm-project", "development", "What did you record about how to check the everglade-web crate?",
             "It builds only for wasm32-unknown-unknown; check it with cargo check -p everglade-web --target wasm32-unknown-unknown.",
             [["wasm32"]], [m(refs["wasm-project"])]),

        item("merged-sep-26", "locked", "Which task did the owner merge on September 26?",
             "Task t-131, which added a --json flag to coder activity.",
             [["t-131"]], [j(refs["t-131-merged"])], none=others("t-131")),
        item("rejected-preference", "locked", "Which preference you proposed did the owner reject?",
             "Never running the full workspace tests; the owner rejected it on September 19.",
             [["full workspace"]], [m(refs["full-pref"]), j(refs["full-rejected"])]),
        item("pause-dates", "locked", "On which days did the owner pause you and resume you?",
             "The owner paused me on September 19 and resumed me on September 21.",
             [dated((9, 19)), dated((9, 21))], [j(refs["paused"]), j(refs["resumed"])]),
        item("nightly-count", "locked", "How many times had the nightly-check job fired by September 27?",
             f"{refs['nightly-count']} times; it was skipped twice while I was paused.",
             [[str(refs["nightly-count"])]], [j(refs["nightly-last-fired"])]),
        item("verse-timeout", "locked", "Which command did not finish on September 23?",
             "cargo test -p verse --release, when the owner asked for the verse tests in release mode.",
             [["cargo test -p verse"]], [j(refs["verse-timeout"])]),
        item("jev-note", "locked", "What did the owner ask you to remember about Jev thresholds?",
             "That Jev thresholds need a measurement document before code trusts them.",
             [["measurement"]], [m(refs["jev-note"])]),
        item("keep-green-fix", "locked", "Which crate did the keep-green fix that merged on September 25 touch?",
             "The supervise crate: task t-129 fixed its deadline test.",
             [["supervise"]], [j(refs["t-129-merged"]), j(refs["t-129-request"])]),
        item("waiting-change", "locked", "Which of your changes still waits at the Merge station?",
             "Task t-110, which documents the nightly-check job template.",
             [["t-110"]], [j(refs["t-110-waits"])]),
    ]
    return out


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def jsonl(rows):
    return "".join(json.dumps(r, separators=(",", ":")) + "\n" for r in rows).encode()


def main():
    fixture, refs = build()
    journal = jsonl(fixture.journal)
    memory = jsonl(fixture.memory)
    journal_sha = sha256(journal)
    memory_sha = sha256(memory)
    digest = sha256(f"{FIXTURE_SCHEMA}\njournal {journal_sha}\nmemory {memory_sha}\n".encode())
    os.makedirs(OUT, exist_ok=True)
    with open(os.path.join(OUT, "journal.jsonl"), "wb") as handle:
        handle.write(journal)
    with open(os.path.join(OUT, "memory.jsonl"), "wb") as handle:
        handle.write(memory)
    manifest = {
        "schema": FIXTURE_SCHEMA,
        "name": NAME,
        "agent": AGENT,
        "workspace": WORKSPACE,
        "from": fixture.journal[0]["at"],
        "as_of": AS_OF,
        "synthetic": True,
        "journal_rows": len(fixture.journal),
        "memory_entries": len(fixture.memory),
        "journal_sha256": journal_sha,
        "memory_sha256": memory_sha,
        "digest": digest,
    }
    with open(os.path.join(OUT, "fixture.json"), "w") as handle:
        json.dump(manifest, handle, indent=1)
        handle.write("\n")
    suite_items = items(refs, fixture, digest)
    canonical = json.dumps(suite_items, sort_keys=True, separators=(",", ":"), ensure_ascii=False)
    suite = {
        "schema": "openagents.gym.suite.v1",
        "name": NAME,
        "description": (
            "Gym interviews for a workshop agent (docs/verse/generative-agents.md, item 7), asked "
            "against the frozen synthetic fixture in alice-interview-v1/: three weeks of Alice's "
            "journal and memory. This version holds the memory category: questions whose answers "
            "code checks against the fixture. Each item's state pins the fixture digest, the "
            "interview time, the check (every `all` group needs one of its terms, and no `none` "
            "term may appear), and the fixture records that hold the answer."
        ),
        "created": "2026-10-06",
        "digest": sha256(canonical.encode()),
        "items": suite_items,
    }
    with open(os.path.join(HERE, f"{NAME}.json"), "w") as handle:
        json.dump(suite, handle, indent=1)
        handle.write("\n")
    print(
        f"{len(fixture.journal)} journal rows, {len(fixture.memory)} memory entries, "
        f"{len(suite_items)} items, fixture {digest[:12]}, suite {suite['digest'][:12]}"
    )


if __name__ == "__main__":
    main()
