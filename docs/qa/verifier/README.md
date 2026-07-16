# Independent verifier — no agent accepts its own work (QA-5, #8910)

The transcripts are explicit (docs/transcripts/253-notes.md: "executor
self-verification or self-acceptance" is CUT; MP-AC-09: "An independent
verifier can reproduce the declared checks and attach a verdict. Executor
completion and passing tests cannot self-promote the packet to accepted.").
This directory owns the standing recipe: given a completed work unit (a
commit on `main` plus the closing issue comment's claimed evidence), a
DIFFERENT agent re-runs the claimed proofs from a clean checkout,
adversarially probes at least one claim, and produces a typed verdict.

Program context: epic #8904. The QA observer (`docs/qa/observer/README.md`)
watches live surfaces; this verifier judges completed work units. Verdicts
are acceptance **evidence** for the maintainer/coordinator — never merge,
release, promise, or public-claim authority.

## Pieces

| Piece                      | Path                                                                                  |
| -------------------------- | ------------------------------------------------------------------------------------- |
| Types + verdict logic      | `scripts/qa-verify-registry.ts`                                                       |
| Executor                   | `scripts/qa-verify.ts` (`pnpm run qa:verify`)                                         |
| Tests                      | `scripts/qa-verify.test.ts` (`pnpm vp test --run --root . scripts/qa-verify.test.ts`) |
| Claims + verdict artifacts | `docs/qa/verifier/results/qa-verify-issue-<n>-<sha12>[.claims].json`                  |

## The split: judgment vs mechanics

Parsing an implementer's free-text closing comment into runnable claims
requires judgment — that part is THIS recipe, executed by the verifier agent.
Everything mechanical (clean scratch checkout at the claimed commit, install,
re-running proofs, applying/restoring adversarial mutations, honest states,
the typed verdict artifact) is `scripts/qa-verify.ts`.

## The recipe (execute verbatim)

### 0. Independence gate

You must not be the implementer. Read the issue's `CLAIM` comment and closing
comment; the actor/session that implemented the work unit goes into the
claims file as `implementer`, and you pass your own session id as `--actor`.
The executor REFUSES (exit 2) when they match — do not work around it by
renaming your session. If you cannot establish who implemented it, record
what you know in `source` and leave `implementer` unset (the gate then rests
on your honesty; say so in your report).

### 1. Pin the work unit

- `--issue <n>`: the closed issue whose claims you are verifying.
- `--commit <sha>`: the integration commit named in the closing comment
  ("merged to main in <sha>"). The executor resolves it and pins the scratch
  checkout there; the claims file must name the same commit.

### 2. Parse the claims (judgment)

Read the closing comment(s). Extract every checkable claim and map each to a
typed entry:

- **Named tests** ("24/24 tests", "14 new tests in X.test.ts") → a
  `command` claim: `pnpm vp test --run --root . <file>` from the scratch
  root (per-package tests may use `pnpm --dir <pkg> run <script>` with `cwd`).
- **Named smokes** (`pnpm run smoke:...`) → a `command` claim, with any
  documented build prerequisite as a `setup` step. Read the smoke's header
  comment first — it usually names its prerequisites.
- **Committed artifacts/receipts** ("committed artifact X.json") → a
  `file_exists` claim.
- **Owner-gated / env-gated proofs** ("with the admin token", "prod Stripe
  key") → a `command` claim with `requiredEnv` naming the var (runs when the
  env is present, honestly unverifiable when not), or an `attested` claim
  with the exact reason when no mechanical re-run exists at all.
- **Point-in-time claims that cannot be replayed** (a specific past
  production run, a full multi-hour suite you are deliberately bounding) →
  `attested` with the exact reason. Never silently drop a claim — every
  claim in the comment gets a row.

### 3. Design at least one adversarial probe (judgment)

Pick a guarded behavior the claims assert ("exits nonzero on high/critical
drift", "401 on every route without the bearer"). Find the minimal source
mutation in the shipped code that breaks exactly that behavior, and cite the
implementer's own named test as the probe command. The executor applies the
mutation in the scratch copy, expects the cited proof to FAIL (exit
nonzero), and restores the file. Outcomes:

- proof fails against the mutation → `verified` (the proof really guards it);
- proof still passes → `failed` → **reject** (the claimed proof does not
  guard what it says);
- mutation anchor missing → `unverifiable_here` (source drifted; pick a new
  anchor).

An accept REQUIRES at least one verified adversarial probe — re-running
green tests alone never accepts.

### 4. Write the claims file

`docs/qa/verifier/results/qa-verify-issue-<n>-<sha12>.claims.json`, schema
`openagents.qa_verifier_claims.v1` (see `scripts/qa-verify-registry.ts` and
the committed demos in `results/`). Include `source` (which comment, which
timestamp) and `implementer`. Commit it — the claims mapping is part of the
evidence.

### 5. Run the executor

From the repo root:

```sh
pnpm run qa:verify -- --issue <n> --commit <sha> \
  --claims docs/qa/verifier/results/qa-verify-issue-<n>-<sha12>.claims.json \
  --actor <your session id>
```

It creates a scratch `git worktree` detached at the claimed commit, runs
`pnpm install --prefer-offline`, runs `setup` steps, then every claim, and
writes the verdict artifact beside the claims file. `--keep-scratch`
preserves the scratch for debugging; `--scratch-dir` pins its location.

macOS note: if `vp` fails with a dlopen/codesign error inside the scratch,
add the standard codesign setup step (see the #8907 demo claims file) — it
re-signs the Vite Plus native binary in the scratch checkout.

### 6. Interpret the verdict

- `accept` (exit 0) — every re-run claim verified, ≥1 command claim and ≥1
  adversarial probe verified. Unverifiable-here claims are listed and are
  NOT covered by the accept.
- `reject` (exit 1) — a claim was re-run and contradicted (test failed,
  artifact missing, or an adversarial mutation went uncaught). File the
  rejection back to the issue/coordinator with the artifact; the rejection
  must be actioned, not shrugged off.
- `unverifiable-here` (exit 3) — nothing failed, but this environment could
  not establish the accept conditions (owner-gated env, unbuildable scratch,
  no adversarial probe possible). NEVER auto-accepted; route to an
  environment that can run the missing proofs or to the owner.

Reclassification rule: when a `command` claim fails for a provably
environmental reason (missing GUI/display, missing local credential, dlopen
signing), you may reclassify it to `attested` with the exact failure reason
(keep the original output tail in the reason) and re-run. Never reclassify
an assertion failure — that is a reject.

### 7. Commit and hand off

Commit the claims file and the verdict artifact. The executor prints the
ready-to-post issue comment; hand it to the coordinator/maintainer. **The
verifier does not post the verdict to the issue and does not close/reopen
issues** — acceptance stays with an authority distinct from both the
implementer and the verifier's mechanical run.

## Wiring

- **QA-1 swarm findings fixes:** a fix for a swarm finding is accepted only
  after this recipe runs against its integration commit; the coordinator
  posts the verdict comment on the finding issue.
- **Issue closeouts (standing):** any agent may run this against a closed
  issue's claims; a `reject` verdict reopens the conversation with the
  artifact as evidence.
- **Full Auto tie-in (future L6/L7):** a Full Auto lane's completed turn can
  be marked "pending independent verification" — the loop continues while a
  different lane runs this recipe behind it; the verdict artifact is the
  gate for marking the unit accepted.

## Demos (first real verdicts)

Committed under `results/`:

- `#8907` (QA observer) @ `08096cae24` — claims re-run + adversarial probe
  on the high-severity exit gate; admin-token claim honestly env-gated.
- `#8886` (Full Auto control surface) @ `0353b307fa` — named test file
  re-run, `pnpm run smoke:full-auto-control` against real Electron, and an
  adversarial probe on the bearer-auth guard.
