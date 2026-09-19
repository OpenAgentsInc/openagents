# Lev roadmap

**Status:** proposed, for review. No issues are filed. Each item below is
written in the shape this repository files them, so an accepted item can be
opened without rewriting. The sequence starts with measurement rather than
code, because the design in [`architecture.md`](architecture.md) depends on
behavior nobody in this workspace has measured yet.

Read [`README.md`](README.md) first for what Lev is,
[`apple-fm-surface.md`](apple-fm-surface.md) for what is already known, and
[`calibration.md`](calibration.md) for the rule that governs step 5 onward.

## Decisions before step 1

Six decisions are the owner's, and four of them change what gets built.

**D1 — Swift sidecar, or Rust FFI?** This workspace has built the sidecar
twice, in `psionic` and in `openagents`, and both audits concluded the
localhost boundary was right. It also means a Rust-only workspace acquires a
Swift build step, a signed helper, and a supervised child process.
*Recommendation: the sidecar, with the helper's code signature checked
before launch from the first commit, which is the discipline the earlier
lane only reached at commit forty.*

**D2 — Reimplement, or depend on `psionic`?** `psionic-apple-fm` is about
6,200 lines of exactly the contract Lev needs. `AGENTS.md` says sibling
repositories are reference material and that a design carried over is
reimplemented here and named in the commit message.
*Recommendation: reimplement in `crates/lev`, cite `psionic` in the commit
message, and keep the type names aligned so the two stay comparable.*

**D3 — Does Lev own the door, or does this repository?** Lev is an
implementation of the System One contract, and `crates/jev` and `crates/kev`
already live here. The Apple bridge expertise lives in `psionic`.
*Recommendation: here. Lev is a decision model, not an Apple integration.*

**D4 — Base model only in v1, or an adapter too?** An adapter needs Apple's
adapter training toolkit, which is not on this machine, and it commits the
project to retraining on Apple's release schedule.
*Recommendation: base only through step 7. Treat step 8 as a separate yes.*

**D5 — Who reads Apple's terms?** Serving the on-device model to third
parties through the mesh, for payment, is a licensing question with a yes or
no answer. Nothing in [`mesh-plan.md`](mesh-plan.md) proceeds without it.
*Recommendation: answer it before step 9 is scheduled, not when it blocks.*

**D6 — What does a Lev door do without a distribution?** This one is
settled by the contract rather than by preference, and it is worth seeing
before step 2: `jev::NoulAnswer` carries exactly one field, the probability.
`ChoiceAnswer` and `ScoreAnswer` both require `confidence`. So an estimator
that produces no distribution cannot serve any of the three question types
through `/v1/systemone`.
*Consequence: L1 constrained argmax is an internal fast path for callers
that want a typed choice with no probability. It is never a door path. The
door serves L2 or L3, or it refuses.*

## The sequence

| # | Issue | Builds | Proved by |
| --- | --- | --- | --- |
| 0 | [Lev] Apple FM decision-model roadmap and tracking | — | the items below close |
| 1 | [Lev 1] Behavior measurement harness and the record | a throwaway harness, a committed record | numbers for every unknown in `apple-fm-surface.md` |
| 2 | [Lev 2] `crates/lev`: contract types and the schema compiler | `api.rs`, `render.rs`, `schema.rs` | every question type compiles to an admitted schema; refusals typed |
| 3 | [Lev 3] The bridge seam and question isolation | `bridge.rs`, the helper per D1 | kev's isolation probe passes against live hardware |
| 4 | [Lev 4] Estimators: seeded ensemble and argmax | `estimator.rs` | seeds reproduce; resolution reported; L1 and L2 agree on argmax |
| 5 | [Lev 5] Suites and the calibration map | `calibrate.rs`, suite fixtures, a record | a fitted map scored on a disjoint partition |
| 6 | [Lev 6] `lev-serve`: the door and its refusals | `serve.rs`, `src/bin/lev_serve.rs` | a `crates/jev` client round-trips against it unmodified |
| 7 | [Lev 7] The band readout | schema extension, a second map | L3 agrees with L2 inside tolerance, at one call |
| 8 | [Lev 8] The adapter lane *(gated on D4 and the toolkit)* | dataset conversion, export, signature pinning | a base-versus-adapter gate on the same suite |
| 9 | [Lev 9] Comparison and disposition | a card in `docs/lev/` | Lev, kev, and Jev scored on the same items |
| 10 | [Lev 10] Mesh row *(gated on D5)* | catalog row, probes, door routing | a Lev worker clears a behavioral floor |

Steps 1 through 4 produce no probability anyone may use. Step 5 is the first
one that does, and step 6 is the first one a caller can reach.

---

## [Lev] Apple FM decision-model roadmap and tracking

Build a third implementation of the System One contract, answered by Apple's
on-device foundation model, ending at a `lev-serve` binary that a
`crates/jev` client reaches with a `base_url` change and a calibration record
behind every probability it reports.

Background: `docs/lev/` — `architecture.md` for the design and why kev's
mechanism does not port, `apple-fm-surface.md` for what this workspace
already established, `calibration.md` for the measurement rule,
`mesh-plan.md` for the fleet path that follows.

Rules:

- Lev is not Jev and it is not kev. No probability leaves the door without a
  fitted calibration map recorded against the serving base signature.
- The model proposes, the host acts and reports. A typed answer is a
  proposal, never evidence that anything happened.
- The state is data, never instruction. Policy goes in session instructions;
  caller state goes in the prompt, marked as data.
- Rust only, no TypeScript, no GitHub-billed automation. Any Swift helper is
  signed and its signature is checked before launch.
- Every issue lands with its tests and its docs update.

## [Lev 1] Behavior measurement harness and the record

Part of [Lev].

Nothing in this workspace has measured Apple's model as a decision model.
Measure it before designing against it.

Build a throwaway harness that drives the live runtime on this machine and
answers, with numbers:

- Is greedy decoding deterministic run to run, and across process restarts?
- Does a fixed seed reproduce a sample exactly, within a session, across
  sessions, and across restarts?
- How does sampling spread over a constrained option set relate to item
  difficulty, on items with known answers? This is the premise of the L2
  estimator and the one result that can invalidate the whole design.
- What is the context limit for a state plus one question, and what error
  comes back at the boundary?
- Latency against state length and question count, and whether a
  per-question session pays the full state cost every time.
- How often do guardrails fire on ordinary decision-shaped inputs, and does
  declaring the content-tagging use case change that?
- Does the runtime report token counts that are worth putting in a `usage`
  object?

Acceptance:

- `docs/lev/measurements/<date>-behavior.md` commits the numbers, the exact
  commands, the OS build, and the base model signature.
- The harness is rerunnable and says so; it does not have to be pretty.
- `apple-fm-surface.md`'s "what stays unmeasured" section is emptied or
  explains what could not be measured and why.
- If sampling spread carries no signal about difficulty, say so plainly in
  the record and stop the sequence for a design review.

## [Lev 2] `crates/lev`: contract types and the schema compiler

Part of [Lev]. Depends on nothing; can run beside step 1.

Build the crate and the parts that need no hardware:

- `SystemOneRequest` with `state` as string, object, or array; `model`; and
  the questions map, with `Noul`, `Choice`, and `Score` matching the
  contract's criteria shapes and bounds.
- One renderer that flattens structured values into labelled text, shared by
  every later path so training and serving never see different text.
- The schema compiler: each question type to a runtime generation schema
  whose enum is exactly the admitted option set, per the mapping table in
  `architecture.md`.
- Answer types byte-compatible with `crates/jev`'s `answers.rs`.
- The typed refusal set, including `uncalibrated`, mapped from the runtime's
  error codes.

Acceptance:

- `cargo test -p lev` compiles every question in a fixture corpus to a
  schema, and rejects out-of-bound requests with typed refusals.
- Option text containing schema-shaped content cannot change the admitted
  set, proved on adversarial fixtures.
- No network and no hardware in the test path.

## [Lev 3] The bridge seam and question isolation

Part of [Lev]. Depends on Lev 2 and D1.

Reach the runtime, per D1, and answer one question at a time:

- Availability, model info, and typed unavailable reasons surfaced as
  refusals.
- Session creation with instructions, and a prompt carrying the state as
  marked data.
- Guided generation against a compiled schema.
- Generation options: greedy, and random with a seed.
- One session per question, created fresh, never reused across questions in
  a request.
- If a helper process is used, its code signature is verified before launch
  and diagnostics are captured rather than discarded.

Acceptance:

- kev's isolation probe passes: a secret in a sibling question stays at
  chance, the same secret in the state is found. An implementation that
  reuses a session fails this test, and a test proves the test works.
- A constrained Choice returns only admitted options across the fixture
  corpus.
- Every runtime error code maps to a typed refusal; none reaches a caller as
  a 500.

## [Lev 4] Estimators: seeded ensemble and argmax

Part of [Lev]. Depends on Lev 3 and the seed results from Lev 1.

- L1: one greedy call, returning the choice and no distribution. Internal
  only, per D6.
- L2: `N` seeded samples, an empirical distribution over the option set, the
  seeds recorded with the result, and the resolution `1/N` carried
  explicitly rather than rounded away.
- The Noul, Choice, and Score derivations from the distribution, identical
  to kev's formulas, including `confidence = (p_max − 1/K) / (1 − 1/K)`.

Acceptance:

- Re-running an L2 estimate with the same seeds reproduces the distribution
  exactly, or the record from Lev 1 already showed it cannot and the crate
  refuses to claim reproducibility.
- L1's choice equals L2's argmax on the corpus, or the disagreement rate is
  recorded.
- A distribution is never reported at a precision finer than `1/N`.

## [Lev 5] Suites and the calibration map

Part of [Lev]. Depends on Lev 4. This is the first step whose output anyone
may act on.

- Adopt kev's frozen-suite format: checksummed, with separate training,
  calibration, development, and locked-test partitions and pinned dataset
  revisions. Reuse kev's `evals/transfer-v4` items so Lev, kev, and Jev are
  scored on the same things.
- Fit a calibration map from the L2 raw signal to a probability, on the
  calibration partition, as a binned reliability table or an isotonic fit.
- Score it on the evaluation partition: accuracy, ten-bin ECE, NLL, Brier,
  confident-error rate at `p ≥ 0.9`, and resolution.
- Run the mechanism probes as behavioral tests: isolation, order
  sensitivity, option-set integrity.
- Commit the calibration record defined in `calibration.md`, including the
  base signature and OS build it is valid for.

Acceptance:

- One question family has a fitted map, scored on a partition disjoint from
  the one it was fitted on.
- The record refuses to validate when the serving base signature differs
  from the fitted one, proved by a test.
- The numbers are published whether or not they are good.

## [Lev 6] `lev-serve`: the door and its refusals

Part of [Lev]. Depends on Lev 5.

- `POST /v1/systemone` returning answers shaped exactly as `crates/jev`
  decodes them, plus `usage` limited to what the runtime honestly reports.
- `GET /v1/models` reporting the model id, base signature, OS build,
  estimator, question types, bounds, and the calibrated families.
- Typed refusals: `model_unavailable`, `uncalibrated`, `too_many_options`,
  `branch_too_long`, `guardrail`, `unsupported_guide`,
  `adapter_incompatible`, `invalid_request`.
- `extensions.estimator` carrying the estimator, `N`, the seeds, the
  calibration record reference, and the base signature.

Acceptance:

- A `crates/jev` client with `base_url` pointed at `lev-serve` round-trips
  `noul`, `choice`, and `score` unmodified.
- A question outside every calibrated family refuses with `uncalibrated`
  rather than returning a number.
- Every refusal case returns a typed error, not a 500.

## [Lev 7] The band readout

Part of [Lev]. Depends on Lev 6.

Add the ordered certainty-band field to the compiled schema, fit a second
map from band to probability on the same partitions, and serve it as the
one-call estimator.

Acceptance:

- L3's calibrated distribution agrees with L2's inside a stated tolerance on
  the evaluation partition, or the divergence is recorded and L3 is not
  promoted.
- L3 costs one call, measured.
- The band set and its map are in the calibration record.

## [Lev 8] The adapter lane

Part of [Lev]. **Gated on D4 and on obtaining Apple's adapter training
toolkit, which is not on this machine.**

- Convert suite records to the toolkit's dataset shape: JSON Lines, one
  message array per line, `response_format` on the user message for the
  guided-generation records, assistant last.
- Train and export a `.fmadapter` package; validate its inventory, metadata,
  and the Core ML blob-storage container layout before use.
- Pin `baseModelSignature` and refuse on mismatch.
- Re-fit the calibration map against the adapter, because the old map does
  not transfer.

Acceptance:

- A base-versus-adapter gate on the same suite, with both scored.
- A package with valid metadata but a malformed container is rejected, and a
  test proves it.
- An adapter whose signature does not match the running base refuses at
  attach time.

## [Lev 9] Comparison and disposition

Part of [Lev]. Depends on Lev 7, and on Lev 8 if it ran.

Score Lev against `kev-0.5b`, the kev previews, and hosted Jev on the same
suite items, using kev's own comparison method. Publish a card in
`docs/lev/` with the accuracy, calibration, mechanism, and runtime numbers,
and the failures recorded rather than omitted — the pattern kev's previews
already set by shipping with their release screen marked failed.

Then answer the question the program exists to answer: is Lev good enough,
on measured workloads, to put in front of anything? A clear no is a
successful outcome for this step.

Acceptance:

- One table, same items, four columns.
- A written disposition: which workloads Lev is admitted for, which it is
  refused for, and what would change the answer.
- `README.md`'s status line updated to match reality.

## [Lev 10] Mesh row

Part of [Lev]. **Gated on D5 and on a yes from Lev 9.** See
[`mesh-plan.md`](mesh-plan.md) for the catalog row, the behavioral floor
that replaces artifact digests, signature-scoped replication, and the
question-level fan-out that Lev's per-question cost makes natural.

## What this sequence does not cover

- Any claim that Lev is calibrated for a workload it was not measured on.
- Paid inference on Apple's model, which waits on D5.
- Replacing `crates/jev` or `crates/kev`. Three implementations of one
  contract is the point; a caller picks by `base_url`.
- Changing anything under `docs/jev/` or `docs/kev/`. The kev port is in
  flight under its own issues, and this program stays out of its way.
