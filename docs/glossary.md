# Glossary

Terms this repository implements or specifies. A term appears here only if
code or a document in this repository defines it.

**Status** tells you what backs the term:

- **Implemented**: code in `crates/` defines it, and tests cover it.
- **Designed**: a document or a NIP in this repository specifies it, and no
  code implements it yet.

Terms that belong to other repositories are out of scope, including terms
from the reference material in `~/work/coder`.

## The contract

| Term | Status | Definition |
| --- | --- | --- |
| System One | Implemented | The request contract all three decision models answer: `POST /v1/systemone`. You send one state and a map of typed questions. You get one typed answer per question, with probabilities. The service generates no text. The schema identifier is `openagents.systemone.v1`. |
| State | Implemented | The single document every question in a request reads. Questions do not see each other's answers. |
| Question | Implemented | One typed request for a decision. A question is a `Noul`, a `Choice`, or a `Score`. |
| `Noul` | Implemented | A question that asks whether a statement holds. The answer is a probability from 0 to 1. |
| `Choice` | Implemented | A question that asks which of several named options applies. The answer names one option and carries a distribution over all of them. The caller supplies the options in the request. |
| `Score` | Implemented | A question that asks which level applies on an ordered rubric. The answer carries a weighted position (`score`, `Σ i · p_i`), a legend, and a distribution; the categorical level an evaluator scores is the distribution's argmax, ties resolving to the last level listed, per [`decision-models/2026-09-20-score-contract.md`](decision-models/2026-09-20-score-contract.md). Serving a `Score` requires the levels to be ordered for the model, which [`decision-models/2026-09-19-score-ordinality.md`](decision-models/2026-09-19-score-ordinality.md) measures door by door. |
| Door | Implemented | One endpoint that answers the contract. `crates/jev` is the client for every door; you select one by `base_url`. |
| Door identity | Implemented | The fields that say which door produced a row: the door name and version, the adapter, the estimator, and the OS build. Before door identity existed, calibration records named only an OS build, which is identical across doors on one machine. |
| Refusal code | Implemented | A typed reason a door declines to answer, returned in the response body. A refusal carrying a code is the door's own answer and stays in the denominator. A failure carrying no code is a harness failure and produces no row. |
| `uncalibrated` | Implemented | The refusal a door returns when a caller requires probabilities for a family that has no admitted calibration map. |

## The three implementations

| Term | Status | Definition |
| --- | --- | --- |
| Jev | Implemented | The hosted door, run by TypeSafe on closed weights. Metered per request. `crates/jev` is the Rust SDK. |
| Kev | Implemented | The local door in `crates/kev`: a trained pointer head and LoRA adapter over a frozen open base. Four checkpoints exist — `kev-0.5b`, `kev-0.6b`, `kev-4b`, and `kev-8b`. |
| Lev | Implemented | The on-device door in `crates/lev`, which answers the contract through Apple's `FoundationModels` framework. Apple's runtime returns no logits, so Lev estimates distributions instead of reading them. |
| Packed sequence | Implemented | Kev's prefill layout. The state and every question occupy one sequence, so the state is encoded once for all questions in a request. |
| Block-causal mask | Implemented | The attention mask that isolates questions in a packed sequence. Each question attends to the state and to itself, and not to other questions. |
| Pointer readout | Implemented | Kev's answer mechanism. A trained head points at option tokens the caller supplied in this request, rather than selecting from classes fixed at training time. |
| Delimiter hardening | Implemented | Training that makes Kev treat delimiter-like text inside a state as data. Lev cannot use this technique, because Apple's guardrails refuse delimiter-fenced state. |
| Bridge | Implemented | The Swift helper in `swift/lev-bridge` that reaches `FoundationModels`. It speaks line-delimited JSON over stdin and stdout. Build it with `./scripts/build-lev-bridge.sh`. |

## Estimating a distribution

| Term | Status | Definition |
| --- | --- | --- |
| Estimator | Implemented | The method Lev uses to derive an answer when the runtime exposes no probabilities. |
| L1 | Implemented | One greedy call. Returns a choice and no distribution. |
| L2 | Implemented | `n` seeded samples, counted. Resolution is `1/n`, so eight draws answer in steps of 0.125. |
| L3 | Implemented | A constrained certainty band the model reports alongside its answer. |
| Raw signal | Implemented | What an estimator observes before calibration. The type is deliberately not named a probability, because a count of samples is not one. |
| Seed block | Implemented | The range of seeds an L2 estimate draws. Block `b` draws `b * n` through `b * n + n`, so two blocks never share a seed. Running the same block again reproduces the same arithmetic and is not a fresh trial. For independent evidence, ask for a block you have not drawn. |
| Certainty band | Implemented | A coarse confidence level the model reports. On the base model the band is constant. A trained band varies and separates correct answers from incorrect ones. |

## Calibration

| Term | Status | Definition |
| --- | --- | --- |
| Calibration map | Implemented | A fitted table that converts a raw signal into a probability. A map is fitted per family and per door. A map fitted against one door does not serve another. |
| Band-conditioned map | Implemented | A calibration map fitted separately within each certainty band. On Lev this reduced log loss where a pooled map did not help. |
| Admission gate | Implemented | The rule in `gym::calibrate::admit` that decides whether a map may be served. A map must reduce ECE by at least 10% on items it was not fitted on, must not increase log loss, and may increase Brier by no more than 10%. |
| Confident error | Implemented | A wrong answer returned with high probability. Counting these separates a model that is wrong from a model that is wrong and certain. |
| Calibration record | Implemented | The committed artifact holding a fitted map, its metrics, its verdict, and the door identity it was fitted against. The schema identifier is `openagents.gym.calibration_record.v1`. |

## The Gym

| Term | Status | Definition |
| --- | --- | --- |
| Gym | Implemented | The measurement and control plane in `crates/gym`. It stores results, judges comparisons, and refuses comparisons it cannot make. It reads and writes local files and depends on no service. |
| Row | Implemented | One door's answer to one item, with everything needed to attribute it: door identity, suite digest, question digest, gate, estimator, perturbation, and latency. New rows use `openagents.gym.eval_row.v2`, which records the selected answer; historical `v1` rows remain readable without rewriting their receipts. |
| Store | Implemented | The append-only result file. A row is appended as it is produced, so an interrupted run keeps what it measured. |
| Receipt chain | Implemented | The hash chain over rows in a store. Each row seals the row before it, so an edited or reordered row is detectable. The store seals a row over the value it reads back as, because writing an `f64` exactly and parsing it approximately produces different digests. |
| Chain fault | Implemented | What a chain verification reports when it fails: `Edited` if a row's content changed, `Resequenced` if the order changed. |
| Suite | Implemented | A set of labelled items with a content digest. Changing an item changes the digest, so a changed suite is a different measurement rather than a moved number. |
| Suite digest | Implemented | The hash over a suite's items. A comparison across two suite digests is refused. |
| Partition | Implemented | Which third of a suite an item belongs to: `calibration` to fit on, `development` to iterate on, and `locked` to spend once. Every item of one state sits in one partition. |
| Locked read | Implemented | A recorded spend of the locked partition. The schema identifier is `openagents.gym.locked_read.v1`. |
| Question set | Implemented | The question text for a suite's families, digested separately from the items. Rewording a question changes the question digest and leaves the suite digest alone, which makes a reworded question a candidate rather than a different suite. |
| Gate | Implemented | A rule that judges a comparison. Gates live in `crates/gym/gates/` and carry their thresholds, each threshold's basis, and a digest. |
| Gate digest | Implemented | The hash over a gate. A verdict names the gate digest that produced it, so tightening a threshold produces a new rule rather than new history. |
| Bound | Implemented | One threshold in a gate, with a record of where the number came from. |
| Basis | Implemented | What a bound rests on: a measurement, a derivation, or a judgment. A bound whose basis is a pending measurement cannot pass. |
| Verdict | Implemented | The three-valued result of judging a comparison: `failed`, `unverifiable`, or `passed`. A gate reports `unverifiable` when it has no floor for a metric, and `unverifiable` outranks `passed`. |
| Noise floor | Implemented | The spread a metric shows when nothing changes. On `support-v2` the seed-block standard deviation of accuracy is 0.0197, so a two-door comparison needs 0.056 to clear two sigma. |
| Headroom | Designed | The accuracy a partition leaves available. A partition scoring 0.975 at baseline has 0.025 of headroom, which is below the floor, so it cannot host a comparison whatever you test on it. |
| Perturbation | Implemented | What varies between two runs of the same door on the same items: the estimator, the draw count, the seed block, the option order, and the question digest. Two runs at the same perturbation are a repeat, which the store refuses. |
| Flip rate | Implemented | How often a door changes its answer when option order changes. A three-option `Choice` has six orders and fifteen distinct pairs, and a flip rate measured over one pair is a different statistic from one measured over all fifteen. |
| Label source | Implemented | What evidence an item's label rests on. `outcome` means the label was read mechanically from what happened next. `author` means a person read the item and decided. A suite that mixes the two without recording which is which loses the distinction permanently. |
| Comparison | Implemented | What two sets of rows may be read as: a door comparison when the items and question text match, a question-text comparison when the items match and the text differs, and nothing at all when the items differ. |
| Regression check | Implemented | `gym regress` compares a door against its own previous run at the same perturbation and suite digest. It refuses the comparison when either has moved. |

## Coder

| Term | Status | Definition |
| --- | --- | --- |
| Coder | Implemented | The agent in `crates/coder`. `classify` routes each turn through a decision model, and `generate` answers through an Open Responses door. |
| Coder Terminal | Implemented | The terminal interface in `crates/coder-terminal`: the amber intensity ladder, the framed composer, and the shell they draw. |
| Shell round | Implemented | One cycle of the shell loop. The model proposes a plan, the terminal runs the commands, and the outcomes return for the next judgment. |
| Plan | Implemented | A reply that is one JSON object carrying the schema version, commands, and the reason for each, rather than prose. A reply is a plan only if it is one whole object under the supported version, so a reply that quotes one as an example is prose. |
| Permit | Implemented | What the host permits one turn to do, built from the route and the operator's setting before the turn generates anything. A permit narrows and never widens, and a turn without one runs no commands whatever its reply asks for. See [`coder/shell-loop.md`](coder/shell-loop.md). |
| Job | Implemented | One program run under one supervisor, in `crates/supervise`. The job owns its process group: a deadline or a cancelled caller terminates the group and reaps the direct child before the job reports, and stdout and stderr are held to their caps as they are read. Unix only. See [`coder/subprocesses.md`](coder/subprocesses.md). |
| ATIF | Implemented | The Agent Trajectory Interchange Format, at version `ATIF-v1.7`, in `crates/atif`. A trajectory records a session as ordered steps so a tool can read it. `Call.extra` carries decision-model calls, which makes a System One call first-class in a trace. |
| Trace | Implemented | One Coder session as an ATIF document, written to `~/.openagents/traces/` as the session runs. A session is one terminal invocation. Steps append and the document is rendered on read, so a session that is killed still reads back. `CODER_TRACE=off` turns recording off. See [`coder/traces.md`](coder/traces.md). |
| Decision call | Implemented | A question put to a door, recorded in a trace as a `Call` carrying `openagents.decision-call.v1`: which door answered, the state and questions that went out, the typed answers, the digest of that state, and the route the host made of them. |

## Capabilities and programs

| Term | Status | Definition |
| --- | --- | --- |
| Capability | Designed | A granted ability. A capability that no grant declares is offered to no run. |
| Executor | Designed | The implementation that performs an agent session, whether the built-in runner or an external agent reached through an adapter. |
| Delegation | Implemented | One bounded task handed to one executor and recorded as an ATIF `Call` named `delegate`, in `crates/coder` (`delegate.rs`). A fan-out runs them concurrently under a stated bound. A refusal the executor declares, a bound that expired, a non-zero exit, and a process that never spawned are four outcomes rather than one. See [`coder/delegate.md`](coder/delegate.md). |
| Program | Designed | A state machine of named steps with per-step bounds, specified by [NIP-PRG](../nips/openagents/NIP-PRG.md). A program carries no code, commands, or prompts, composes by reference under bounds that narrow and never widen, and says nothing about where it runs. It is a general primitive and is not specific to an agent or a product. |
| Step kind | Designed | What one step of a program does: `query`, `check`, `decide`, `delegate`, `program`, or `module`. The registry is open. A host that does not recognize a kind refuses the program, because a program whose unknown steps are skipped is a different program. |
| Bounds | Designed | The limits a program's step states and an executor promises to keep. A host refuses a step whose bounds it cannot enforce rather than running it unbounded. Under composition, bounds narrow and never widen. |
| Module | Designed | A WebAssembly module a program's `module` step runs, named by content hash. The hash is required and the sources are hints, so the place bytes come from cannot decide what runs. |
| Module announcement | Designed | An optional `30183` event saying where a module's bytes can be found and what it requires. A locator, not an authority: it cannot change what a program runs, because the program names a hash. |
| Capability manifest | Implemented | A document that says how to drive an executor: transport, detection, bounds it enforces, bounds it ignores, whether it sees the repository, and who pays. Read from `capabilities/` today and published as Nostr `kind:30180` later. |
| Capability probe | Implemented | Running a manifest's `detect` on this machine, in `crates/coder`'s `capability` module. It resolves an absolute path rather than trusting `PATH`, and answers in three states. |
| Presence | Implemented | What a probe found: **present**, **absent**, or **present and unavailable**. Absence is not an error — the capability is not an option. The third state is an executor that is installed, detected, and refusing this context. |
| Program registry | Implemented | The programs a host resolved, read from `programs/` in `crates/coder`'s `program` module. The read records the Nostr filter it would have sent beside the answer it got from disk. |
| Task source | Implemented | Where a `query` step's work comes from, named in the program by slug and resolved from `sources/` in `crates/coder`'s `source` module. A program names a source and never a command, so a machine decides what the lookup reads. `request`, the work the request carried, is built in. |
| Selection | Implemented | What one `query` step looked up: the work that runs, the order it is in, what was dropped and why, and every path more than one selected item touches. A lookup answering with more than `max_results` truncates or refuses, as the step's `on_overflow` says, and the trace records which. |
| `cannot_enforce` | Designed | The bounds an executor accepts and silently ignores. A host refuses a delegation whose requirements intersect this list, because an executor that drops a bound is more dangerous than one that refuses it. |
| Operator policy | Designed | A signed document that says which capabilities an operator prefers, how wide a fan-out may go, and what to never use. Published as Nostr `kind:30181`. |

## Nostr

| Term | Status | Definition |
| --- | --- | --- |
| Relay | Implemented | The Nostr relay in `crates/nostr-relay`: one binary and one Postgres database, serving `relay.openagents.com`. |
| NIP-CJ | Designed | Coder jobs. Ephemeral `25900`, `26900`, and `27000` events carry a request, a result, and feedback between a terminal and a fulfillment worker. Payloads use NIP-44 and sockets authenticate with NIP-42. |
| NIP-PRG | Designed | Programs. Addressable `30182` events carry a state machine of named steps with per-step bounds. A host that does not recognize a step kind refuses the whole program rather than skipping the step. |
| NIP-CAP | Designed | Capabilities. Addressable `30180` and `30181` events carry capability manifests and operator policies. |
| `nips/` | Implemented | Pinned copies of the official NIPs and the extension NIPs. `nips/manifest.json` records the upstream commits. The `coder/` lane is authored here and is not synced. |
