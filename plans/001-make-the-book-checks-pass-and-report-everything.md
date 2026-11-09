# Plan 001: The book checks pass on main, and one stale review no longer hides the rest

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report. Do not improvise. When done, update the status row for this plan
> in `plans/README.md`, unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**:
> `git diff --stat 76084e17a9..HEAD -- psionic/tests/book.py coax/book/capability-claims.json coax/book/capability-claims.py coax/book/check-structure.py`
> If any of these files changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding. On a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P1
- **Effort**: S to M
- **Risk**: LOW
- **Depends on**: none
- **Category**: dx
- **Planned at**: commit `76084e17a9`, 2026-11-09

## Why this matters

Every book check in this repository starts by asserting that a ledger of
reviewed "capability claims" is current. That ledger went stale on
2026-11-08, so every book check has failed on `main` since then, on its
first line. While it failed, real drift went unseen: 11 source listings in
the Aethyr book, one registry entry and three Coax book excerpts no longer
matched their sources. Those were fixed by hand on 2026-11-09, but the cause
remains. `psionic/check.sh`, the Psionic release gate, runs this check at
line 876, so that gate cannot pass either.

This plan does two things. It reviews and refreshes the stale claims. And it
changes the book check so a stale claim is reported together with the other
checks' results, not instead of them.

## Current state

Files:

- `coax/book/capability-claims.json`: the ledger. Each claim has an `id`, a
  `document`, a `scope` sentence, a `text_sha256` of the claim's prose, and
  an `evidence` list of `{path, sha256}`. A claim is stale when any evidence
  file's SHA-256 differs from the recorded one.
- `coax/book/capability-claims.py`: the checker. `problems(root)` returns a
  list of error strings. It has no update mode.
- `psionic/tests/book.py`: the book check for the Psionic, Aethyr, Nostr,
  Coder and Bitcoin books.
- `coax/book/check-structure.py`: the Coax book's check. It also runs the
  claims check.

The assertion that hides everything else, `psionic/tests/book.py:105-111`:

```python
def check(name="psionic"):
    book, programs, readme, least = BOOKS[name]
    spec = importlib.util.spec_from_file_location("coax_book_claims", ROOT / "coax/book/capability-claims.py")
    claims = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(claims)
    errors = claims.problems(ROOT)
    assert not errors, "reviewed capability claims: " + "; ".join(errors)
```

The current failures, from `python3 coax/book/check-structure.py`:

```text
psionic.execution.current: evidence changed; review psionic/book/SUMMARY.md
psionic.gpu.current: evidence changed; review psionic/book/SUMMARY.md
psionic.language.current: evidence changed; review coax/lib/compiler/wgsl.coax
coax.wgsl.current: evidence changed; review coax/lib/compiler/wgsl.coax
coax.wgsl.current: evidence changed; review coax/src/shader.cpp
coax.wgsl.current: evidence changed; review verse/everglade/gpu.bindings
```

When each evidence file was last reviewed, and what changed since:

| Evidence file | Claims | Last reviewed at | Commits since | Size of change |
| --- | --- | --- | --- | --- |
| `psionic/book/SUMMARY.md` | `psionic.execution.current`, `psionic.gpu.current` | `9c280f5f8b` | `11da31e23c` | 51 lines added |
| `coax/lib/compiler/wgsl.coax` | `coax.wgsl.current`, `psionic.language.current` | `09101c58f9` | `6adbb76c82`, `cdf2eedd13` | 115 added, 12 removed |
| `coax/src/shader.cpp` | `coax.wgsl.current` | `4fa20ba31b` | `6adbb76c82` | 15 added, 11 removed |
| `verse/everglade/gpu.bindings` | `coax.wgsl.current` | `8b76a0b0f3` | `c50fcbbf15`, `4c7669373d` | 5 added, 3 removed |

What is already known about these changes:

- `11da31e23c` added 25 bold part-name lines, such as
  `**II. Open weights early: Elephant and Mammoth**`, to
  `psionic/book/SUMMARY.md`. It added, removed and re-labelled no chapter.
- `cdf2eedd13` is titled "Kill wgsl.coax's surviving mutants so the Coax
  compiler publishes again". `6adbb76c82` is titled "Cull clusters on the GPU
  with a request array and two-phase Hi-Z (#13008)".

The claim text for `coax.wgsl.current`, in `coax/book/ch17-gpu.md:35-48`:

```text
A separate bounded graphics path is implemented:
`coax wgsl FILE --vertices A,B --fragments C,D --bindings F [--strip P]`
writes a WGSL module from ordinary checked Coax procedures. Its admitted
subset includes vectors, records, graphics entry wrappers and resource
bindings; unsupported entry shapes, bindings and layouts are refused.
Everglade's main graphics module is generated from `verse/everglade/gpu.coax`
with `gpu.module` and `gpu.bindings`, and its particle module from
`gpu-fx.coax` with `gpu-fx.module` and `gpu-fx.bindings`. Other realm shaders
still have their separately documented paths. This does not implement the
planned `@gpu`/`shader` syntax, a general CPU/GPU layout model, or a qualified
Psionic browser compute backend. Device and numerical evidence remains scoped
to each actual consumer.
```

The other claim blocks are at `psionic/docs/roadmap.md:3`
(`psionic.execution.current`), `psionic/book/PLAN.md:58`
(`psionic.gpu.current`) and `psionic/book/PLAN.md:1002`
(`psionic.language.current`). Each block lies between
`<!-- capability-claim: ID -->` and `<!-- /capability-claim -->`.

How earlier reviews were recorded: the reviewer appended one sentence to the
claim's `scope` string and replaced the stale `sha256` values. An existing
example from the `coax.wgsl.current` scope:

```text
Reviewed for #12949: caf5c6c80b adds scoped Mac host acceptance without widening compute qualification; 199402193d removes only a worker-notice workaround from the unchanged spatial oracle.
```

Conventions in this repository:

- Write prose in plain, active, present-tense sentences.
- Never edit files in the main checkout. Work in your own worktree.
- Python in this repository is tooling only. `psionic/tests/book.py` and
  `coax/book/capability-claims.py` are existing tools; edit them in place and
  add no new Python file.

## Commands you will need

| Purpose | Command | Expected on success |
| --- | --- | --- |
| Claims check alone | `python3 coax/book/capability-claims.py` | exit 0, no output |
| Coax book check | `python3 coax/book/check-structure.py` | exit 0 |
| Other books | `python3 psionic/tests/book.py --book NAME` for `psionic`, `aethyr`, `nostr`, `coder`, `bitcoin` | each prints a line ending `local links pass` |
| All of them | `scripts/check-books.sh --check` | exit 0 |
| SHA-256 of a file | `shasum -a 256 PATH` | 64 hex characters |

None of these builds a compiler or needs the network. Each runs in under a
minute.

## Scope

**In scope** (the only files you may modify):

- `coax/book/capability-claims.json`
- `psionic/tests/book.py`
- `plans/README.md` (status row only)

**Out of scope** (do not touch):

- The claim prose in `coax/book/ch17-gpu.md`, `psionic/book/PLAN.md` and
  `psionic/docs/roadmap.md`. If a claim's prose is wrong, that is a STOP
  condition, not an edit.
- `coax/lib/compiler/wgsl.coax`, `coax/src/shader.cpp`,
  `verse/everglade/gpu.bindings`, `psionic/book/SUMMARY.md`. You read them;
  you do not change them.
- `coax/book/capability-claims.py` and `coax/book/check-structure.py`.

## Git workflow

- Work in a worktree: `git worktree add ../openagents-plan-001 origin/main`.
  This repository forbids branches other than `main` and forbids edits in the
  main checkout.
- One commit. Message style from `git log`, for example:
  `Books: review the stale capability claims and report every book check`.
- Do not push unless the operator tells you to.

## Steps

### Step 1: Review each changed evidence file against its claim

For each row of the table above, read the change:

```shell
git diff 9c280f5f8b..HEAD -- psionic/book/SUMMARY.md
git diff 09101c58f9..HEAD -- coax/lib/compiler/wgsl.coax
git diff 4fa20ba31b..HEAD -- coax/src/shader.cpp
git diff 8b76a0b0f3..HEAD -- verse/everglade/gpu.bindings
```

Then read the claim's prose block and its `scope` string. Answer one question
per claim: **does the change make any sentence of the claim's prose false?**

Use these rules:

- `psionic/book/SUMMARY.md`: the claims `psionic.execution.current` and
  `psionic.gpu.current` are about which model and device profiles are
  qualified. A change that only adds headings, reorders nothing and changes no
  chapter's status label makes neither claim false.
- `coax.wgsl.current`: the claim is false if the diff adds a new kind of
  admitted WGSL input that the prose does not list (the prose lists vectors,
  records, graphics entry wrappers and resource bindings), adds the planned
  `@gpu` or `shader` syntax, adds a compute backend for Psionic in the
  browser, or removes a refusal. Changes that only add checks, kill mutants,
  or add bindings to Everglade's own module do not make it false.
- `psionic.language.current`: its scope says that "Metal graphics emission is
  a graphics path and does not qualify Psionic Metal or WebGPU compute beyond
  the bounded kernels named". It is false only if the `wgsl.coax` diff adds
  Psionic compute emission.

Write one sentence per claim that states what the commits changed and why the
claim still holds, in the style of the example in "Current state".

**Verify**: you have four sentences, one for each of
`psionic.execution.current`, `psionic.gpu.current`, `coax.wgsl.current` and
`psionic.language.current`, and each names the commits it covers.

### Step 2: Record the reviews in the ledger

In `coax/book/capability-claims.json`, for each of the four claims:

1. Append your sentence to the end of the claim's `scope` string, after a
   space. Start it with `Reviewed 2026-11-09:`.
2. For each stale evidence entry of that claim, replace its `sha256` with the
   output of `shasum -a 256 PATH`.

Change nothing else in the file. Keep its indentation and key order.

**Verify**: `python3 coax/book/capability-claims.py` exits 0 and prints
nothing.

### Step 3: Report the claims result with the other checks, not before them

In `psionic/tests/book.py`, function `check`, replace the early assertion:

```python
    errors = claims.problems(ROOT)
    assert not errors, "reviewed capability claims: " + "; ".join(errors)
```

with a deferred one. Keep the claims result in a local variable:

```python
    stale_claims = claims.problems(ROOT)
```

and, as the last statement of `check` before its `return examples` line, add:

```python
    assert not stale_claims, "reviewed capability claims: " + "; ".join(stale_claims)
```

The function prints its summary line (`... local links pass`) just before it
returns. Put the new assertion after that `print` and before the `return`, so
a stale claim still fails the check, but only after every structure, registry,
link and listing check has run and reported.

**Verify**: `python3 psionic/tests/book.py --book aethyr` prints a line ending
`local links pass` and exits 0.

### Step 4: Prove that a stale claim no longer hides other failures

1. Temporarily change one hex digit of one `sha256` in
   `coax/book/capability-claims.json`.
2. Run `python3 psionic/tests/book.py --book aethyr`.
3. Change the digit back by hand. Do not use
   `git checkout coax/book/capability-claims.json`: it would discard step 2's
   edits.

**Verify**: in sub-step 2 the output contains the line ending
`local links pass` AND then fails with `reviewed capability claims:`. After
sub-step 3, `python3 coax/book/capability-claims.py` exits 0 again.

### Step 5: Run every book check

**Verify**: `scripts/check-books.sh --check` exits 0.

## Test plan

No new test file. Step 4 is the regression check for the reordering. The
existing inventory test still passes:
`python3 psionic/tests/book_inventory_test.py` exits 0.

## Done criteria

All must hold:

- [ ] `python3 coax/book/capability-claims.py` exits 0
- [ ] `python3 coax/book/check-structure.py` exits 0
- [ ] `python3 psionic/tests/book.py --book NAME` exits 0 for `psionic`,
      `aethyr`, `nostr`, `coder` and `bitcoin`
- [ ] `grep -n 'assert not errors, "reviewed capability claims' psionic/tests/book.py`
      prints nothing
- [ ] `grep -c 'Reviewed 2026-11-09:' coax/book/capability-claims.json` prints `4`
- [ ] `git status --short` lists only `coax/book/capability-claims.json`,
      `psionic/tests/book.py` and `plans/README.md`
- [ ] `plans/README.md` status row updated

## STOP conditions

Stop and report back if:

- In step 1, a diff makes a sentence of a claim's prose false, or you cannot
  tell. Report the claim, the commit and the sentence. A person must decide
  whether the prose or the code is wrong.
- More claims are stale than the six lines listed in "Current state". Other
  agents push to `main` constantly. Review only the six listed; report the
  others by name.
- A book check fails in step 5 for a reason other than capability claims, for
  example `listing drift` or `registry source lacks listed files`. Report the
  exact message. Fixing book drift is not part of this plan.
- `check` in `psionic/tests/book.py` no longer ends with a `print` followed by
  `return examples`.

## Maintenance notes

- The ledger hashes whole files. Any commit to `coax/lib/compiler/wgsl.coax`
  makes two claims stale again. `capability-claims.json` changed in 140
  commits in 14 days for this reason. A follow-up worth considering: bind a
  claim to a narrower thing than a whole compiler source file, for example to
  the test file that pins the claimed behavior. That is a design decision for
  the owner and is not in this plan.
- `coax/book/check-structure.py` still stops the Coax book's check on a stale
  claim, because it reports all of its errors together already.
- A reviewer should read the four `scope` sentences. They are the only part of
  this change that needs judgment.
