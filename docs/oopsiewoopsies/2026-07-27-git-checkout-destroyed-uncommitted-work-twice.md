# `git checkout` destroyed my own uncommitted work, twice in one hour

**2026-07-27, Omega session (omega#120, omega#121).** Written because the owner
asked for it after watching a night of it, and because the failure is not
interesting on its own — the *pattern* under it produced six distinct defects in
about four hours, and four of them reached his screen.

---

## The headline incident

I use **mutation testing** to prove a check is not vacuous: break the thing the
check is about, watch the check fail, put it back. The loop I wrote was:

```bash
mutate  crates/agent_ui/src/agent_panel.rs   # edit the file
run                                          # cargo test -p omega_deltas
git checkout -- crates/agent_ui/src/agent_panel.rs   # "undo the mutation"
```

`git checkout -- <file>` does not undo *the mutation*. It restores the file to
the index, discarding **every** uncommitted change in it. That file also carried
the fix I had spent the previous hour writing — the actual subject of the work —
which was not committed yet.

So the loop's last line deleted the fix. It printed nothing alarming. The tests
that ran *after* the restore were run against a tree that no longer contained the
change I believed I was verifying.

**I then did it again, about twenty minutes later, in the same session, on the
same file, for the same reason.**

### The tells I walked past

1. `git checkout` printed `Updated 0 paths from the index` on one iteration. That
   is the message for "nothing to restore" — the change was already gone. I read
   the words and did not act on them.
2. The mutation runs that followed showed **two** checks failing where I expected
   one. I attributed the extra failure to the mutation. It was the loss.
3. `git status --short` at the end of the loop showed three of my five modified
   files missing from the list. I printed that output and did not read it.

Three independent signals, in my own terminal output, in one loop. What defeated
all three is that I was looking for a specific answer (did the check fail?) and
everything else in the output was scenery.

### Why the second time is worse than the first

The first is a gap in knowledge about a command. The second is a process defect:
after the first loss I *knew*, I redid the work by hand, and then re-ran a loop I
had not changed. I fixed the damage and not the cause — which is precisely the
thing this document exists to stop, and which shows up again in section 3 below.

### The fix

**A mutation harness must operate on a committed tree.** Then `git checkout`
means exactly "undo the mutation", because there is nothing else in the file to
lose. That is a one-line precondition and it makes the dangerous command safe
instead of asking anyone to remember it is dangerous.

Where committing first is genuinely not wanted, the undo must be a snapshot the
harness itself took:

```bash
cp "$F" "$F.premutation"     # snapshot
...mutate, run...
cp "$F.premutation" "$F"     # restore exactly what was there
```

This is what I switched to for the remaining mutations, and it is what I should
have started with. `cp` cannot destroy work it did not take a copy of.

**Rule, stated so a machine can enforce it: never use `git checkout`,
`git restore`, or `git stash` to undo an edit in a tree that has uncommitted work
you care about.** They operate on *files*, not on *your last edit*, and they have
no idea which changes were yours.

---

## The same session's other failures, because one incident is not an audit

### 1. A mutation that silently never applied

```python
a = 'format!("{}\u{2026}", choice.name())'   # not a raw string
```

Python read `\u{2026}` as a broken unicode escape and raised `SyntaxError`. The
mutation never touched the file. The suite then ran against **unmutated source**,
printed `208 passed`, and I recorded that as "mutation did not break the check" —
a conclusion with no evidence behind it in either direction.

- **Why:** a shell pipeline that keeps going after a step fails, and a
  verification step whose only output is the *next* step's output.
- **Fix:** the mutation asserts it applied (`assert a in s`) and prints
  `mutated`; the runner does not run unless it saw that. Raw strings, or the
  pattern in a file rather than in a heredoc inside a heredoc.
- **General shape:** *a verification step that can no-op silently verifies
  nothing, and reads exactly like success.*

### 2. A check satisfied by its own comment

The check for "the executor label marks a pending choice" read the whole rendered
function and asserted it contained `if connecting` and an ellipsis. It passed
with the feature **completely removed**, because:

- the *comment* I had written explaining the ellipsis contains the ellipsis, and
- the tooltip's own unrelated guard contains `if connecting`.

It was reading my prose. This is the exact shape of `OMEGA-DELTA-0048`, which
held `workspace.show_toast(` for months while that toast rendered nowhere —
checking for a *line of source* rather than the *behaviour it describes*.

- **Fix:** the check now extracts the single `let label` binding, strips comment
  lines, and asserts on that. Two mutations watched failing.
- **What caught it:** mutation testing. Nothing else would have. A check written
  and never mutated is a check with an unknown truth value.

### 3. Fixing the symptom the owner named instead of the cause

The owner reported Shift-Tab "only cycling to Codex". The log showed the
selection advancing correctly on every press, so I concluded the **label** was
stale and changed it to display the *choice* rather than the *attachment*.

That was true of the control and false about the application. An hour later he
selected Exo, was shown "Exo", asked "who are you", and **Codex answered**. My
change had converted a visible bug into a lying interface — the one surface that
had been honest now wasn't, and three other surfaces (the thread title, the
composer placeholder, the reply itself) were all still telling the truth.

- **Why:** I verified the fix against the thing he complained about (the label
  moves) instead of against the thing he wanted (the executor changes).
- **Fix in the product:** the clamp described below, plus a label that reads
  `Exo…` while the choice and the attachment disagree, so there is no state in
  which it names an executor with nothing separating *is* from *will be*.
- **General shape:** *when someone says a control does not work, the acceptance
  test is the effect, not the display.* A display fix that makes the display
  agree with a broken effect is worse than the bug.

### 4. Guessing at layers before reading the log

Three wrong hypotheses about which layer was at fault before I read
`~/.omega-play/logs/omega-dev.log`. The log had the answer sitting in it the
whole time, in a shape that admitted no other reading: three
`OMEGA-DELTA-0115: a person chose …` lines, each followed by an ACP connection
and by **nothing else** — no attach line, meaning the router had never run.

- **Fix:** read the log before forming a hypothesis, every time. It costs one
  command. Three wrong guesses cost the owner an hour and two builds.
- Worth noting the logging that made this findable was already there. Structured,
  delta-numbered log lines at decision points are what turned a four-hour
  question into a two-minute one, *once I looked*.

### 5. A global invariant that broke a documented promise

The first version of the real fix clamped **every write** to the panel's stored
agent, so zero base always sat on Omega's router. It shipped, and the owner's
next launch said:

```
Failed to Launch
no thread found with ID: SessionId("019fa242-ecee-7ae3-9740-efa288e10983")
```

`OMEGA-DELTA-0118` promises that *a thread reopens under the executor that
recorded it*, and the panel restores the last thread's own agent at launch to
keep that promise. My clamp rewrote a Codex thread's agent to the router on the
way in; the router had no route record for a session it had never opened, fell
through to the native loop, and the native loop has no such session.

- **Why:** I treated one field as carrying one kind of event. It carries two —
  *what a new thread should use* and *what an existing thread was* — and the
  invariant is only true of the first.
- **Fix:** the clamp moved to the accessor a new thread is built from. A reopened
  thread keeps what it was recorded under.
- **Prevention:** before putting a global rule on a field, enumerate the events
  that write it, and **grep the delta ledger for existing promises about that
  same field**. 0118's promise was written down. I did not look.

---

## What actually prevents this

Ranked by how much they do not depend on me remembering anything.

1. **Mutation harness precondition: committed tree, or a `cp` snapshot.** Removes
   the whole class. Cheap, mechanical, no judgement required.
2. **Mutations assert they applied.** A mutation that cannot prove it changed the
   file must abort the run rather than let the suite report on unmutated source.
3. **Every new check gets mutated before it is trusted.** Not a sample — each
   one, individually. This session, 1 of 6 new checks was vacuous, and it was
   vacuous in a way no amount of re-reading would have shown me: I wrote both the
   check and the comment that satisfied it.
4. **Read the log before the hypothesis.** One command, and this session it was
   decisive twice.
5. **Grep the delta ledger for the field or surface being changed.** The promise
   I broke was already written down in the repository, in the file whose whole
   purpose is to hold promises like it.
6. **Acceptance is the effect, not the display.** For UI defects specifically:
   the test is "did the thing happen", never "does the control now look right".

Points 1–3 are the ones that belong in a script rather than in a document, for
the reason `script/omega-preflight` exists at all: a rule that lives in prose is
enforced by whoever happens to read the prose, and this document is prose.

---

## The honest summary

Six defects. One of them — the `git checkout` — was pure carelessness with a
sharp tool, repeated after it had already cost me an hour. Three were
verification that looked like verification and was not: a mutation that never
applied, a check that read its own comment, a fix accepted against the wrong
acceptance criterion. Two were reasoning ahead of evidence: guessing at layers
with the log unread, and imposing an invariant without checking what the
repository already promised about the thing I was constraining.

The common thread is not carelessness. It is **accepting a green signal without
asking what would have made it red.** Every one of these produced a
success-shaped output — a passing suite, a moving label, a clean restore — and in
each case nothing about that output distinguished "this worked" from "this never
ran".
