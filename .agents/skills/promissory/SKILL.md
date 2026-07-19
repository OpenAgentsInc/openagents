---
name: promissory
description: Assault non-green product promises per the PROMISSORY runbook. Use when the user says /promissory, asks to flip/advance/assault product promises, wants the next non-green promise target selected and worked, or wants a fleet wave dispatched at the promise backlog. Modes - no args = supervisor (rank pool, dispatch or work top targets); a promiseId arg = assault that specific promise (owner-directed override); a number N = dispatch a wave of N fleet workers.
---

# PROMISSORY — Non-Green Promise Assault

The canonical, always-current procedure is
`docs/fable/2026-07-01-promissory-nongreen-assault-runbook.md` (repo root).
**Read it first, in full, every invocation** — this skill is the launcher,
not the law. If the skill and the runbook disagree, the runbook wins.

## Argument parsing

- **No args** → supervisor mode: rank the pool, then either dispatch a
  fleet wave (if fleet capacity is ready) or personally assault the top
  target.
- **A promiseId** (e.g. `khala_code.forum_hotbar.v1`) → single-target
  mode on that promise. This is explicit owner direction: it overrides
  the runbook's "already mapped by other work" exclusion (note the
  override in the claim issue).
- **A number N** (e.g. `10`) → supervisor mode, dispatch a wave of N
  fleet workers at the top N unclaimed targets.
- Anything else (e.g. a campaign hint like `throughline: mobile`) →
  treat as a temporary throughline override for scoring this run only.

## Scoreboard (mandatory, start AND end)

Print the registry scoreboard **before any work starts** and **again after
the run finishes**, so the user sees states flip. Compute it from the
canonical source (one-off script from the openagents repo root):

```sh
bun -e "
const { publicProductPromisesDocument } = await import('./apps/openagents.com/workers/api/src/product-promises.ts');
const d = publicProductPromisesDocument();
const c = {};
for (const p of d.promises) c[p.state] = (c[p.state] ?? 0) + 1;
console.log('registry', d.version, '| total', d.promises.length, '|', JSON.stringify(c));
"
```

- **Opening scoreboard:** registry version, total records, and the count
  per state (green / yellow / red / planned / degraded / withdrawn), plus
  the target(s) selected and their current states.
- **Closing scoreboard:** the same counts recomputed from the merged
  `main`, presented as a before → after diff — explicitly name every
  record whose state or blocker set changed during the run (e.g.
  "`foo.v1` planned → yellow. `bar.v1` cleared 2 of 3 blockers, still
  red") and the new owner-decisions-ready count in `NEEDS_OWNER.md`.
- In fleet/wave mode the supervisor prints the opening scoreboard once at
  wave start and the closing one after the last closeout merges. Long
  waves should also emit interim scoreboards as each PR lands.

## Operating summary (details and exact rules live in the runbook)

1. **Snapshot + rank.** Load the registry via
   `publicProductPromisesDocument()` from
   `apps/openagents.com/workers/api/src/product-promises.ts` (the
   scoreboard script above already does this — reuse its output). Apply the
   eligibility filter (runbook §2 — including *steer clear of promises
   already mapped by open issues/epics/roadmap lanes*: PROMISSORY hunts
   the hidden and overlooked, unless the user directed a specific target)
   and the scoring formula (§3, throughline-weighted. Current campaign:
   Khala Code launch).
2. **Claim atomically.** One promiseId per claim. GitHub issue titled
   `PROMISSORY: <promiseId>` — search open AND recently-closed first.
   lower issue number wins races (§4).
3. **Assault ladder** (§5): audit the record → decompose every blockerRef
   into BUILD / EVIDENCE / OWNER / EXTERNAL → implement fully in a fresh
   worktree from clean `origin/main` with tests and dereferenceable
   evidence → update the registry record + one note + version bump in the
   same PR per the concurrent-safe edit protocol (§7) → verify
   (record's own verification, relevant suites, `check:deploy`,
   promise-test pins) → merge to `main`, close the issue, write
   owner-gated residue to the workspace `NEEDS_OWNER.md`, release the
   claim, take the next target.
4. **Fleet dispatch** (supervisor/wave modes): use the runbook §8 pinned
   `$PYLON khala request --workflow codex_agent_task` template (or the
   `khala_fleet` MCP equivalent), one worker per target, refill freed
   slots with the next-ranked unclaimed target on every closeout, keep
   the dispatch ledger and exact token-row verification per
   `docs/fable/EXECUTION.md`.

## Hard guardrails (never violate, even under time pressure)

- **Never flip a promise to green** — green is owner-signed,
  receipt-first. The success metric is *owner-decisions-ready*: promises
  one owner action from green, batched in `NEEDS_OWNER.md`.
- planned→yellow / red→yellow only when the record's own `verification`
  criteria are met with cited evidence. Honest downgrades encouraged.
- Never weaken a gate/test/policy to clear a blocker. Never edit the
  green-count test pin to silence a failure. Never broaden public copy.
- One promise per claim/PR. Registry edits touch only your record + one
  note + the version constant.
- Isolated worker homes always. Never touch `~/.codex` or the live
  `~/.claude`. Public-safe prompts and evidence only.
