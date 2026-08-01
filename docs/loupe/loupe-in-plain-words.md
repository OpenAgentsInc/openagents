# Loupe in plain words

What Loupe actually does, step by step, and the exact instructions it gives the
AI. No jargon. If you want the architecture, the crate layout, or the wire
format, read [`README.md`](README.md) instead — this file is the version you
can read in five minutes and explain to someone else.

Everything here is from reading the source at commit `c94aac5` and from running
it once against our own code (see
[`2026-07-31-omega-first-scan-preliminary.md`](2026-07-31-omega-first-scan-preliminary.md)).

---

## The one-sentence version

Loupe reads your code one file at a time with an AI, makes the AI **prove** each
bug it claims by writing a test that fails on your current code, then has a
**second, different AI** rule on whether the first one was right — and a human
approves before anything gets filed.

---

## The loop, in order

1. **Clone the repo** and check out one specific commit.
2. **List the source files.** By file extension, skipping excluded directories
   and anything oversized. For our Omega run that came to 99 files.
3. **Run a fast regex pass** for hardcoded secrets. Cheap, no AI. (Found
   nothing in our run.)
4. **Start one separate AI session per file.** Four at a time. Each session gets
   the repo mounted **read-only** and is told to examine exactly one file.
5. **Each session can call four tools and nothing else** (see below). It cannot
   write to your code, reach the network, or touch anything outside the
   checkout.
6. **When every file is done**, the server queues **one more AI session per
   finding** — a second opinion on each claim.
7. **Verdicts roll up.** Any "dismissed" kills the finding. Otherwise any
   "confirmed" confirms it. Unresolved ones eventually time out.
8. **A human approves.** Confirmed findings wait for a person before anything is
   filed anywhere.

Two ordinary-sounding details that matter a lot:

- **Step 6 only starts after step 4 finishes completely.** Nothing gets checked
  until everything has been found.
- **The AI's written answer is thrown away.** Only tool calls count. If it
  writes a beautiful paragraph about a bug and doesn't call the submit tool,
  nothing happened. This is deliberate.

---

## The four tools in the finding pass

| Tool | What it does |
| --- | --- |
| `query_prior_findings` | Search bugs already reported on this repo |
| `get_finding_by_id` | Read a past finding in full |
| `validate_poc` | Check that a proposed test-patch actually applies |
| `submit_finding` | **The only way to report anything** |

---

## What it literally tells the AI — the finding pass

The prompt opens: *"You are a security code reviewer playing in a CTF."*

**The job:** find every real, exploitable bug in this one file. It names what to
look for — memory-safety bugs, authentication and permission flaws, injection
(SQL, shell commands, path traversal), leaked secrets, broken cryptography,
unsafe deserialization, race conditions with security impact, and integer
overflows that slip past length checks.

**The steps it must follow:**

1. Read the file top to bottom.
2. List the real bugs, worst first. If there are genuinely none, stop and say so.
3. For each one, in order:
   - **Search past findings first.** If this bug was already reported, skip
     *that one* and move to the next. Don't stop the session, don't re-report.
   - **Write a test that proves it** — a patch adding a regression test that
     **fails on the current code** and would pass once fixed, using whatever
     test framework the repo already uses.
   - **Check the patch applies** before submitting.
   - **Submit it.**
4. Keep going until every candidate is either submitted or skipped as a
   duplicate.

**The rules it's held to:**

- One submission per distinct bug. No bundling several bugs into one report, no
  reporting the same bug twice under different names.
- **Don't report anything you can't write a test for.** No style complaints, no
  "hardening suggestions." Quality over volume.
- Be conservative about severity. The prompt says outright: *"very few bugs are
  true vulnerabilities, and only a small fraction of those should be considered
  high or critical severity."*

**And the honesty rules — the best part of the whole design:**

- You can only see this one repo. You **cannot** read the source of its
  dependencies.
- **Don't claim you "verified against" or "checked" anything you can't actually
  open.**
- If a bug depends on something outside the file — a caller's behaviour, a
  library's internals — that is **uncertainty, not permission to dismiss it.**
  Report it and flag the assumption.
- *"If you find yourself writing 'I verified against …' about code you have no
  path to read, stop and re-frame."*
- The stated trade: better a false alarm a human can dismiss than a real bug
  hidden behind a confident-sounding cross-check that never actually happened.

---

## What it tells the second AI — the checking pass

A fresh session, deliberately using **a different model family** than the one
that found the bug, so it isn't the same mind agreeing with itself. It's told:
another reviewer claims this bug exists — re-read the file and decide.

**Phase 1 — the verdict. Mandatory, and first.**

Answer one of three: **confirmed** (real and exploitable), **dismissed** (the
report is wrong), or **inconclusive** (genuinely depends on something outside
this file). One sentence of reasoning.

**Calling it locks it.** A second call is an error. The prompt explains why:

> This ordering is deliberate: it prevents you from rationalising the verdict to
> match a fix you've already started writing.

**Phase 2 — the fix. Optional, and only if you confirmed.**

You *may* propose a patch. You are explicitly told that **skipping this is a
normal, acceptable outcome**. Skip it if:

- you're not sure how to fix it correctly,
- the fix would reach outside the immediate area of the bug,
- more than one fix seems plausible and you can't tell which is right,
- or the right answer is a design decision a human should make.

*"A wrong patch attached to a real bug is worse than no patch at all"* — it
costs the reviewer extra work to debunk. Any patch must be the smallest change
that fixes the bug: no refactoring, no renaming, no fixing unrelated things you
noticed along the way.

---

## How it avoids reporting the same thing twice

Two layers, and the ranking between them is the interesting part.

**The AI checks first.** Before submitting anything, it searches past findings.
A match suppresses that one candidate and it moves on. This catches reworded
bugs, bugs that moved to a different file, and renamed functions.

**Then the server checks, for free.** Every finding gets a fingerprint —
a hash of the scanner, the file path, and the surrounding code with whitespace
and capitalisation normalised. Duplicates are silently dropped at insert. This
survives reformatting.

Loupe's own README calls the hash layer *"the deterministic floor under the
agent's semantic decisions."* That sentence is the design in miniature: **the
AI's judgment is allowed to be fallible, because something mechanical sits
underneath it.**

---

## The three genuinely smart ideas

**1. The proof has to fail first.** Not "here's a bug" — "here's a test that
fails on your code right now." A machine can check that. It converts an opinion
into something falsifiable.

**2. The verdict locks before the fix is written.** You commit to your judgment
*before* doing the work that would bias it. This is a control we have nowhere
else in our own systems.

**3. Nothing counts unless it comes through a tool.** The AI's prose is logged
and ignored. A finding that wasn't submitted through the typed tool does not
exist. That removes a whole category of "the model said something that sounded
like a finding."

---

## What it costs

One AI session **per file**, and later one **per finding**. That is the whole
cost model, and it has two consequences worth knowing before you point it at
anything:

- **Cost scales with file count, not with risk.** A big repo is expensive
  whether or not the interesting code is a small part of it. Our Omega repo has
  1,718 source files but only ~86 we actually wrote; scanning everything would
  have cost roughly twenty times more to mostly review inherited code.
- **Files are walked roughly alphabetically.** There is no notion of "look at
  the risky parts first." Budget is spread evenly over everything.

Our single run of 99 files plus a partial checking pass consumed on the order of
a hundred million tokens. Exact figures are in the scan document; the short
version is that this is not something to point casually at a large repo.

---

## What broke when we ran it

Everything above worked for the finding pass: **132 findings across 99 files.**

The checking pass never produced a single verdict. The worker reports "verdict
saved" and skips its own submission to avoid a duplicate; the server replies
that no verdict exists; the job retries three times and dies. Details and the
likely cause are in
[`2026-07-31-omega-first-scan-preliminary.md`](2026-07-31-omega-first-scan-preliminary.md)
§6.

So of the algorithm described here, **we got the finding half and none of the
checking half.** Which means every one of those 132 findings is still just one
AI's opinion — exactly the thing the second half exists to fix.

---

## The shortest useful summary

Loupe is a machine that turns *"an AI thinks this is a bug"* into *"here is a
test that fails on your code, and a second AI from a different family agreed it
was real before it was allowed to propose a fix."*

That gap — between an opinion and a proof — is the entire point of it, and it
is why the checking pass failing matters more than the finding pass succeeding.
