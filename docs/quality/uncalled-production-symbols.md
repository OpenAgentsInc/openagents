# Code that ships, passes its tests, and is never called

Two guards, both in `check:fast`:

- `scripts/uncalled-production-symbol-guard.mjs` fails the build when code is
  proven real by a test and called by nothing in production.
- `scripts/cli-invocability-guard.mjs` fails the build when the entry point
  itself cannot run — a package-script CLI that rejects the `--` pnpm forwards.

## Why this rule exists

In one overnight session (the spend audit is
[`docs/afteraction/2026-07-31-codex-019fb495-overnight-spend-audit.md`](../afteraction/2026-07-31-codex-019fb495-overnight-spend-audit.md))
ten separate defects were found that share one shape: **the code existed, it
shipped, its tests passed, and no production path ever reached it.** Among them:

- `removeParticipant` had zero production callers, so stale room members were
  never retired.
- The self-provision hooks were never passed to `makeOmegaNostrSessionService`,
  so `OMEGA_NOSTR_SELF_PROVISION_ENABLED` was an unreachable kill switch.
- `recordSarahLiveKitParticipantJoin` was reachable only from its own test, so
  the `duplicate_participant_refused` release gate could not be satisfied by any
  credential — the refusal it implemented was not on a live path.
- `removeSarahLiveKitRoomMember` still has no non-test caller today, so
  `member_removed` and `membership_changed` cannot fire.
- Every hang trace ever written was empty, because deleting a UI package orphaned
  the only caller of `set_trace_enabled`.
- A minting script emitted the wrong ref shape, so every bearer it ever produced
  was refused 403 — and its tests asserted the same wrong shape, so the suite was
  green.

None of these were caught by types, lint, coverage, or CI. That is the point: a
passing test is exactly what hides them. The test is the only caller, so every
tool agrees the code is fine, and the reviewer sees a green tick over a function
that can never run.

`scripts/sarah-participant-join-authority-guard.mjs` was the first response — it
names three participant-admission authorities and requires each to keep a
production caller. That works, but it only protects symbols someone already
thought to list, which is the wrong end of the problem. This guard generalizes
the rule instead of growing the list.

## The rule

A finding is code that **a test references and production does not**.

1. **Exported value.** An exported `function` / `const` / `let` / `class` is
   flagged when a test or fixture references it, no production file other than
   its own declares or references it, and its own file never names it again.
   The last clause matters: a module that wires its own export into another
   export it ships is live, and only the declaration itself appears exactly once.

2. **Service-interface member.** A function-typed member of an exported
   `interface` is flagged when a test dot-accesses it and no production file
   anywhere does. Declaring a member and implementing it are both declarations;
   only a `.member` access is a call. Rule 1 cannot see this shape, because the
   interface member and its class implementation sit in one file and look like
   two mentions of a live symbol. `removeParticipant` is exactly this case.

Code that **nothing** references — not even a test — is ordinary dead code and is
deliberately not flagged. It is lower stakes, far noisier, and already the domain
of unused-export tooling. The whole signal here is the contradiction in "tested
but uncalled": someone cared enough to specify the behavior, and it still cannot
happen.

Exports a framework calls by convention (`default`, `GET`, `loader`, and the rest
of `CONVENTION_EXPORTS` in the guard) are skipped. Their caller is a router or a
host process, so "no named reference" proves nothing about them.

## What to do when it fails

The failure names the symbol, its file, the tests that reference it, and the
three things that satisfy it:

1. **Call it from the live path.** This is almost always the right answer, and it
   is the answer in every one of the ten defects above.
2. **Delete it, together with its test.** Behavior nothing reaches is not an
   asset. Git keeps it.
3. **Record it as an intentional exception** in `allowed` in
   `scripts/uncalled-production-symbol-baseline.json`, with a reason that names
   who will call it and when.

An `allowed` entry with no reason fails the guard, and so does one that outlives
its finding. An exception nobody can explain is indistinguishable from the defect
it claims to be an exception to.

## The debt ledger

`inheritedDebt` in the baseline holds the findings that already existed when the
guard landed — **1768 of them** on `4adee7da8a`. That number is not a target and
not an approval; it is the measured size of this defect class in the repository.
The ledger may only shrink:

- a new finding not in the ledger fails the build;
- a ledger entry that stops flagging must be removed
  (`node scripts/uncalled-production-symbol-guard.mjs . --prune`);
- `--prune` can only remove entries, never add them, so a fresh uncalled symbol
  cannot be laundered into the ledger with one command. Seeding is a separate
  flag that refuses to run against a ledger that already exists.

Two of the ten original defects are still live inside that ledger and are worth
fixing rather than inheriting:
`apps/openagents.com/workers/api/src/sarah-livekit-room-authority.ts#removeSarahLiveKitRoomMember`
and
`packages/khala-sync-server/src/sarah-livekit-room-authority-store.ts#SarahLiveKitRoomAuthorityStore.removeParticipant`.

The largest benign category in the ledger is test infrastructure that lives on a
production path — fakes, in-memory stores, conformance suites. Those are genuinely
only called by tests. The honest fix is to move them under a test path rather than
to widen the rule.

## Proof that it catches the real thing

A guard that cannot fail on known-bad input is the disease it is meant to cure. It
was run against the historical broken state of three of the ten defects, and
against the commits that fixed them:

| Defect | Broken at | Guard | Fixed at | Guard |
| --- | --- | --- | --- | --- |
| `recordSarahLiveKitParticipantJoin` | `c5b595a420` | flags | `94d49d8bab` | clean |
| `omegaNostrSelfProvisionEnabled`, `reserveOmegaNostrSelfProvision` | `e2ed217430` | flags | `267ca8719d` | clean |
| `SarahLiveKitRoomAuthorityStore.removeParticipant` | `81e0970f6e` | flags | never fixed | still flags |

Reproduce any row with a scratch worktree:

```sh
git worktree add --detach /tmp/proof c5b595a42037ee6fa8f44e5a7632962ff5ffde11
node scripts/uncalled-production-symbol-guard.mjs /tmp/proof --list | grep recordSarahLiveKitParticipantJoin
```

## The sibling guard: an entry point that cannot run

`gate-observation-cli.ts` was tested, documented, and had never once been
invocable by its documented command. pnpm forwards the `--` separator into argv
verbatim — verified against pnpm 11:

```
$ pnpm run echoargs -- --row x
["--", "--row", "x"]
```

Its parser threw `unsupported or incomplete argument --` before doing any work,
which is why the receipts directory it writes to did not exist. `acceptance-cli.ts`
had the same hole; `failure-matrix-cli.ts` had always skipped the token.

`scripts/cli-invocability-guard.mjs` requires every **package-script** CLI that
rejects an unrecognized argument to skip a bare `--`. The scope is deliberately
narrow — only files a `package.json` script actually executes, because an Electron
main process or a library that reads argv for its own reasons is never reached
through `pnpm run`. The fix is always one line:

```ts
if (value === "--") continue
```

It carries 14 inherited findings and the same ledger mechanics.

Writing this guard demonstrated why the historical proof is not optional. Its
first draft **did not flag the defect it was written for**, twice: the rejection
pattern matched only "unknown argument" and missed "unsupported or incomplete
argument", and the tolerance pattern read `next.startsWith("--")` as evidence the
token was handled, when that line proves the opposite. Both were found by running
the guard against `c1a54c919d` and watching it pass. A guard is only worth what it
fails on.

| Defect | Broken at | Guard | Fixed at | Guard |
| --- | --- | --- | --- | --- |
| `gate-observation-cli.ts`, `acceptance-cli.ts` | `c1a54c919d` | flags both | `bf837d2808` | clean |

## What it deliberately does not catch

- **Transitively dead code.** The guard is leaf-level. When
  `recordSarahLiveKitParticipantJoin` was dead, the store method it called looked
  healthy, because it had a real production caller — inside the dead function.
  Catching that needs reachability from a route table, not a search for one name.
- **Vocabulary with no implementation.** `duplicate_participant_refused` existed
  only as an entry in a gate-observation list and a receipt. No test asserted it
  and no route emitted it, so there was no symbol to flag. That is a different
  rule: a declared outcome no code can produce.
- **Class and object-literal methods generally.** Only members declared on an
  exported `interface` are covered.
- **Types.** A type with no runtime caller cannot ship a silent behavioral hole.
- **Documented commands that name a script nobody defines.** Measured: 6 real
  cases, buried in ~180 candidate matches that were mostly English prose after
  the word `pnpm`, third-party documentation vendored under `docs/reference/`, and
  commands written relative to their own package rather than the root. Making it
  precise needs code-fence extraction and package-relative resolution. Left
  unbuilt rather than shipped noisy.
- **Non-TypeScript surfaces.** The Rust instances (the empty delegate-tool
  description, the empty hang traces) are outside its reach; `cargo`'s `dead_code`
  lint does not catch them either, because their tests reference them.
- **A wrong value in code that is called.** The spend ceiling that always read `0`
  because `SUM(bigint)` returns a string ran on a live path every time. That is a
  contract-typing defect, not a reachability one.
