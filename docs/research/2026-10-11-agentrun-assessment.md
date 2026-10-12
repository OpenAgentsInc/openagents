# AgentRun assessment: a tested workflow DSL we can mine for NIP-PRG

Source: <https://github.com/Parcha-ai/agentrun>, cloned to `~/work/agentrun`,
reviewed at `1fcdae6` (workspace `0.1.0-beta.12`). Build and test results
below were reproduced on this machine, 2026-10-11.

## What it is

AgentRun is a workflow language for an agent the host already runs. A
workflow is a JSON document (or TypeScript builder output) of typed nodes.
The interpreter executes it and calls back into the host for every model
call and side effect; the host keeps tools, models, permissions, budgets,
and delivery. It is from the TypeSafe/Pi ecosystem (Parcha Labs / Grep.ai):
typed decisions are Jev System One calls, and its agent integration
targets `@earendil-works/pi-coding-agent`.

Six packages, one version:

| Package | Role |
| --- | --- |
| `@parcha/agentrun-dsl` | Types, `validateWorkflow`, `runWorkflow`, dry run, inspection, CLI. One dependency: `typebox`. |
| `@parcha/agentrun-jev` | `createJevRunner()`, the `runJudge` adapter over `@typesafe-ai/sdk`. |
| `@parcha/agentrun-pi` | The Pi extension (`/agentrun`) and a Pi-backed `runNode` runner. |
| `@parcha/pi-durable-disk` | Durable pi runs on an Archil FUSE disk: claim, fence, lease, supervisor. Linux-only. |
| `@parcha/pi-browser`, `@parcha/pi-vision` | Held private; built and tested, not published. |

Node kinds: `chain`, `code` (trusted in-process JS transform), `agent`,
`decide`, `extract`, `report`, `artifact`, `map`, `parallel`, `loop`,
`escalate`, `judge`, `pick`, `sift`, `route` (by judgment or by a stored
`valuePath`), `call` (`tool`/`executor`/`shell` with deadline, retry
classes, and `poll`), and `workflow` (child invocation with isolated
state and an explicit output contract). ~30k lines of source, ~37k lines
of tests across 191 test files.

## Verified locally

- `npm ci --ignore-scripts`, `npm run build`: clean on Node 24.
- `packages/dsl` and `packages/pi`/`pi-browser` suites pass. One timing
  flake under parallel load ("deadline elapsed before effect admission"
  in `runtime-observers.test.mjs`) passed on rerun.
- `packages/pi-durable-disk`'s `npm test` **fails and then hangs on
  macOS**: its fence is pinned through `/proc/self/fd`, which is
  Linux-only by design (`src/env.ts` refuses cleanly without it), but the
  local-mode tests do not platform-skip, so the fixture exits
  `CLAIM_UNMOUNTED` and the suite stalls on open handles. CI is
  ubuntu-only, so they likely have not seen it. Worth an upstream bug
  report; not a product defect for its Linux target.

## Assessment

This is real engineering, well above the usual agent-framework bar.

**Strengths.**

- **A Lean 4 model of the interpreter** (`spec/lean/`): real proofs — a
  frame theorem over node writes, parallel-merge order-independence, loop
  termination, desugar equivalence — plus a JSON conformance corpus run
  against *both* the Lean model and the TypeScript implementation, a
  validator sweep that collects every workflow the test suites validate
  and checks the Lean validator accepts them all, and an axiom audit.
  They record counterexamples and dispositions in a findings table
  (F1–F9) and disclaim honestly: "conformance pins agreement, not
  equivalence."
- **Adversarial test method**: planted-secret sentinel tests that go red
  on a leak planted in a tool result, log line, receipt, or filename;
  symlink rejection; prototype-pollution guards; SIGKILL-mid-dispatch
  tests proving a paid effect dispatches exactly once.
- **Disciplined failure semantics**: `EffectOutcomeUnknownError` retains
  a settlement promise for an admitted effect still pending at cutoff;
  late-settled results are never committed or delivered; concurrent
  failures surface as `AggregateError` with every uncertain effect;
  parallel branches that write the same state key fail instead of
  silently overwriting; `$host` is a reserved state channel no workflow
  can write.
- **Honest security posture**: code nodes run `new Function` in-process
  and they say so. The denied-globals lint is a real lexical scanner that
  skips strings, comments, regexes, and templates — and is still
  documented as "a lint for untrusted authors, never a boundary."
- **Minimal dependencies**: the core has exactly one (`typebox`).
- **Limits documentation** (`docs/guide.md#limits`) is the best part of
  the docs: each guarantee and non-guarantee is stated plainly
  ("confidence is a model output, not an empirically calibrated accuracy
  guarantee"; "a completed run alone does not prove file delivery").

**Weaknesses.**

- Three weeks of public history, beta, API explicitly unstable. The repo
  is an export of a private "harness" monorepo (PR numbers reach #210 on
  65 commits; `pi-browser` is read-only here).
- The macOS test failure above.
- No sandbox and no turnkey durable scheduler in the core — both are
  deliberate host-owned boundaries, but adopting the DSL leaves you
  owning them.
- `code` nodes are JavaScript strings: convenient there, not portable.
- Decision quality is Jev's calibration story; `runJudge` is an
  interface, but the ecosystem assumption is System One.
- One or two authors, as far as the public history shows.

## Why it matters here

[NIP-PRG](../../nips/openagents/NIP-PRG.md) already specifies the same
idea — "a program is a typed workflow interpreted by a host" — with
`decide` and `delegate` step kinds, pinned digests, and child
composition. The Rust runtime in `crates/coder`'s `program` module is
partial: six step kinds and child composition work; `invoke` admission
and relay fetching are unbuilt. [Program
authority](../coder/guides/program-authority.md) is the admission half
(`Grant::meets`, effect derivation, the ATIF `program_authority` call).

AgentRun is essentially NIP-PRG shipped in TypeScript with different
transport (host adapters instead of NIP-CJ/NIP-RUN lanes). We cannot use
the code — it is TypeScript — but its semantics, validator rules,
failure taxonomy, and test method are a complete, working reference for
finishing ours in Rust.

## What to harvest

Ordered by leverage.

### 1. The conformance-corpus method — steal this first

Their strongest idea is not the DSL, it is the method: an executable
model, a JSON corpus recorded from the implementation keyed by execution
path (`root/steps/0/branches/1/item/3/...`), and a sweep that runs every
workflow the test suites ever validate through both validators.
Applies directly to:

- **ATIF** (`crates/atif`): a corpus of writer→reader cases recorded from
  the implementation would pin reader/writer agreement across versions
  the same way `spec/lean/conformance/` pins theirs.
- **The program interpreter** (`crates/coder`'s `program` module): record
  scripted-adapter runs of every step kind as JSON cases; replay them in
  CI against the Rust interpreter. Their corpus format
  (`spec/lean/conformance/*.json`: `workflow`, `input`, `script` of
  adapter answers keyed by execution path) is a proven shape — port it
  verbatim.
- **NIP-CJ job lifecycle and the quota ledger**: the same trick catches
  transport/ledger drift: script the adapter answers, record the run,
  diff the events.

Whether we want a Lean model is a separate question — the value that
transfers without Lean is the corpus and the sweep, which cost little
and pin the semantics permanently.

### 2. Effect outcome discipline for `crates/actors` and paid effects

Their `call` node contract is the tightest statement of the problem our
`actors` runtime, `coder-scheduler` ledger, and `pay-ledger`/`coder-labor`
paid effects all share:

- An admitted effect still pending at cutoff is **never retried
  automatically**; it throws `EffectOutcomeUnknownError` carrying a
  `settlement` promise. Late-settled results are reported
  (`effect.late_settled`) but never memoized, committed, or delivered.
- Retry classes are closed and declared: `timeout`, `http_5xx`,
  `http_429`, `connection`, `exit`; `exit` retries only when the author
  opted in.
- `poll: {until, fail_when, interval_s, deadline_s}` is a first-class
  contract — poll `deadline_s` must be ≥ `deadline_s`, and a host must
  reject fresh admission after the original deadline (their F8 finding:
  an adapter that ignores this can poll forever).
- `produces` names files the effect promises; the engine verifies them
  after the effect.

Their paid-effect acceptance test (`packages/pi-durable-disk/test/
acceptance-app.test.ts`) is the test to port: SIGKILL the process
mid-dispatch, reopen, assert the cut call comes back `interrupted`, the
API saw each charge exactly once, and no model turn repeated beyond the
one in flight — plus a forged-receipt negative control.

### 3. Validator and state-merge rules for the program interpreter

Portable semantics worth lifting into our step-kind validation:

- **Static reachability + runtime `requires`**: the validator checks path
  reachability structurally; `requires` re-checks non-empty values at
  execution because static analysis cannot predict model output (their
  F1 — documented, not fixed).
- **Parallel write conflicts**: branches that write the same top-level
  key fail `parallel_write_conflict`, including a scalar-vs-array mix on
  a `$host` key — that one was a real order-dependent bug their Lean
  model found (F7). Unchanged copies are not writes.
- **Reserved `$` state keys**: `$host` is host-owned, checkpointed with
  state, isolated per map item/branch/child, excluded from path-less
  output projection, and merged key-by-key across branches (arrays
  append in branch order). Our equivalent is whatever channel carries
  host metadata beside a run's domain state.
- **Judge sidecars**: a `judge` writes the typed value at `as` and the
  raw answers with confidences at `<as>$answers`. Once a decision is in
  state, `route` by `valuePath` — never ask the model again for a choice
  the state already holds.
- **Node `metadata`**: opaque to the engine, never enters state, events,
  prompts, or idempotency keys, but is hashed into the document digest.
  Clean model for host-owned markers on a NIP-PRG step.

### 4. Bounded verification with repair drives

A generative node declares `verify: {out, maxDrives, override.below}`: a
typed judgment checks each submission, a `review` callback returns the
rejection to the same adapter session for repair, and an exhausted
budget throws `WorkflowVerificationError` — never silently accepted.
This is the pattern to copy wherever a `decide` step gates a `delegate`
step's output: the rubric and thresholds stay host-owned so a candidate
cannot rewrite its own verifier.

### 5. The judge question-shape convention

Flat schema → question set: boolean → Noul, string enum → Choice,
leveled integer → Score; descriptions carry the question, criteria carry
choice meanings. `sift` asks items × questions in one request and splits
in order when it exceeds `maxQuestionsPerRequest` or a state-bytes cap
(4-way, first failure cancels). If `crates/jev` callers do not already
share a schema→questions convention, theirs is proven and documented in
`docs/guide.md` and `packages/dsl/skills/author/references/jev-decisions.md`.

### 6. Test and release practices to adopt

- **Planted-secret sentinel tests**: a leak scanner swept over every
  tool result, log line, receipt, and filename; green on clean, red on
  each planted leak. Directly applicable to relay payloads, ATIF traces,
  and gym trace bundles.
- **Vendored-facade pinning**: their Stagehand facade test asserts the
  vendored copy is byte-for-byte the upstream commit its manifest names;
  drift fails. We vendor upstream NIPs (`nips/official`) — same check.
- **Generated dependency inventory**: `docs/dependencies.md` is generated
  from the lockfile with licenses; `cargo about`/`cargo deny` gives us
  the same artifact.
- **Package verification**: tarballs are installed into an empty consumer
  in CI — the crate-graph equivalent is `cargo publish --dry-run` plus a
  downstream compile check.

### 7. What not to port

- **`code` nodes as language strings.** Rust has no `new Function`
  escape hatch, which is a feature. Our equivalent is named, pinned
  Wasm/plugin operations — exactly what NIP-PRG already says
  ("contains no arbitrary executable expressions"). AgentRun's honesty
  about the boundary is a good reminder of what we avoided.
- **The `new Function` denied-globals lint.** In Rust, untrusted
  transforms have a real sandbox story (Wasm); do not re-create a lint
  that pretends to be one.
- **JS-specific seams**: structured-clone semantics, getter/proxy traps
  in JSON copies, `undefined` edge cases. Rust's `serde_json::Value`
  removes the whole class.
- **Anything coupled to Pi or Archil** beyond the failure semantics
  noted above.

## Bottom line

AgentRun validates NIP-PRG's premise with working code and is the best
published reference for the interpreter semantics our `program` module
still needs: reachability-and-`requires` validation, parallel merge
conflicts, reserved host state, bounded verify-with-repair, and the
conformance corpus that pins all of it. Port the semantics and the test
method to Rust; leave the JavaScript behind.
