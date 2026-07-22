# RLM and Full Auto transcript-roadmap audit

- Date: 2026-07-22
- Scope: the recursive engine `@openagentsinc/rlm`, its Full Auto consumer, and
  the transcript corpus at `docs/transcripts/`.
- Method: source review of the AI SDK repository and the monorepo, plus two real
  offline runs of the engine over a bounded transcript slice.
- Authority: this document is an audit. It is not a product promise. It is not a
  release decision. The demo output is a bounded example. It is not authority.

## 1. Verdict

- RLM to Full Auto integration status: **REACHABLE BUT NOT WIRED**.
- A complete host consumer exists at
  `apps/openagents-desktop/src/full-auto-recall.ts`. The unit test at
  `apps/openagents-desktop/src/full-auto-recall.test.ts` proves the consumer in
  isolation. No live Full Auto run loop calls it. The only non-test reference is
  a behavior-contract path entry in
  `apps/openagents-desktop/src/contracts/ux-contracts.ts:115`.
- A model-callable path exists through the `history_recall` host tool. That tool
  reaches RLM Tier D, and Tier S under host admission. The tool serves the
  interactive turn kernel. It does not serve the Full Auto provider lane.
- RLM can traverse the transcript corpus today only through the SDK inline
  corpus builder. No corpus source adapter reads a folder of transcript files.
- Both engine tiers run over a real transcript slice. See section 5.

## 2. What RLM is, concretely

RLM is an Effect-native engine that runs typed recall programs over an
authorized corpus source. The engine has two tiers.

- Tier D is deterministic traversal. It makes zero model calls. It supports the
  operations `Grep`, `OrdinalSlice`, and `InspectMetadata`. Evidence:
  `packages/rlm/src/interpreter/deterministic.ts:42`.
- Tier S is budgeted semantic recursion. A root model call must return one valid
  `RlmProgram` JSON object. The engine decodes that program with Effect Schema
  and runs its nodes. Evidence: `packages/rlm/src/engine/rlm.ts:279` for the
  root prompt and `packages/rlm/src/engine/rlm.ts:287` for the decode.

The program node set is a fixed graph vocabulary. It includes `CorpusOp`,
`Partition`, `Transform`, `ModelMap`, `RlmMap`, `ModelReduce`, and `Commit`.
Evidence: `packages/rlm/src/schemas/program.ts:17` and after. One root program
can partition a collection and map a model call or a child RLM call across the
parts. The engine does not take one root turn for each child. This is the
paper-faithful behavior that the README claims.

Inputs and outputs:

- Input is an `RlmRequest`. The request is `Deterministic` or `Semantic`. It
  carries a pre-authorized corpus input, limits or a budget, and an evidence
  policy. Evidence: `packages/rlm/src/schemas/request-result.ts`.
- Output is a terminal result. The result tag is `Completed`, `Partial`,
  `Refused`, or `Failed`. Each result carries validated citations, a usage
  record, and an honesty record. Evidence: `packages/rlm/src/engine/rlm.ts:172`
  and after.

Provider and model dependency:

- Tier D needs no model. The `rlmDeterministicLayer` refuses semantic work.
  Evidence: `packages/rlm/src/engine/rlm.ts:465`.
- Tier S needs an injected model plan. The plan supplies `completeRoot` and an
  optional `completeLeaf`. The SDK does not own credentials or account choice.
  The host injects the model. Evidence: `packages/rlm/src/engine/rlm.ts:27` and
  `docs/rlm/OPENAGENTS-CONSUMPTION.md` section 5.

Caps and guardrails:

- The SDK program identifier set is SDK-RLM-01 to SDK-RLM-08 plus SDK-RLM-04A.
  SDK-RLM-04A is the typed symbolic environment and the programmatic recursion
  gate. Evidence: `docs/ROADMAP.md:20` and `docs/rlm/PAPER-AUDIT.md:143` in the
  AI SDK repository.
- The budget bounds depth, iterations, model calls, tokens, subcalls, program
  nodes, fan-out, fan-in, concurrency, values, value bytes, environment bytes,
  inline output bytes, artifact output bytes, per-call tokens, observation
  characters, transcript characters, entries scanned, spans, and characters per
  span. Evidence: `packages/rlm/src/schemas/budget.ts:10`.
- The engine can require exact usage. When `requireExactUsage` is true, a model
  call that does not report exact tokens fails with
  `usage_required_but_unavailable`. Evidence:
  `packages/rlm/src/engine/rlm.ts:242`.

Safety claims, checked:

- No Python, REPL, `eval`, or shell path exists in the source. Operators are
  registered pure functions. The claim in the README holds against the source.
- No code-execution node exists. A model authors an `RlmProgram`, not code. The
  engine runs only the fixed operator set.
- The engine is Effect-native. It uses `Effect`, `Layer`, `Stream`, and
  `Schema` throughout. The claim holds.
- Results are cited candidates, not authority. The engine validates every
  citation against the corpus handle. Evidence:
  `packages/rlm/src/engine/rlm.ts:378`.

## 3. Full Auto integration, precise status

The Full Auto consumer module is complete. Read
`apps/openagents-desktop/src/full-auto-recall.ts`. It provides:

- run-scope resolution from the authoritative run registry only, at
  `full-auto-recall.ts:91`;
- a finite per-run recall budget with an idempotent ledger, at
  `full-auto-recall.ts:245`;
- a deterministic-first, admitted-semantic recall run, at
  `full-auto-recall.ts:454`;
- a bounded cited-candidate continuation fragment, at
  `full-auto-recall.ts:737`.

The module design is correct against the consumption contract in section 9 of
`docs/rlm/OPENAGENTS-CONSUMPTION.md`. It keeps Full Auto authority outside RLM.
It marks every recall as a cited candidate and never as verified.

The gap is the call site. A repository search finds no live caller of
`runFullAutoRecall` or `applyFullAutoRecallToContinuation`. The Full Auto
continuation framing in `apps/openagents-desktop/src/full-auto-mission.ts`
renders the continuation prompt without a recall step. The provider lane
dispatch in `apps/openagents-desktop/src/provider-lane.ts` and the headless
lanes do not reference recall. Therefore a live Full Auto run does not consult
RLM today.

The separate model-callable path is the `history_recall` host tool. The turn
policy registers it at
`apps/openagents-desktop/src/turn/desktop-turn-policy.ts:36`. The turn kernel
installs that policy at `apps/openagents-desktop/src/turn/desktop-turn-main.ts`.
The kernel is on by default. The rollback opt-out is
`OPENAGENTS_DESKTOP_AFS_TURN_KERNEL=0`. Evidence:
`apps/openagents-desktop/src/turn/desktop-turn-main.ts:196`. This path serves an
interactive owner turn. It is not the Full Auto run loop. So an interactive
agent turn can call RLM Tier D now, and Tier S under host admission. A Full Auto
agent cannot, because the Full Auto lane does not carry the host tool set and
does not call the recall consumer.

Summary by path:

- Interactive turn to RLM Tier D through `history_recall`: WIRED.
- Interactive turn to RLM Tier S through `history_recall`: WIRED, behind host
  admission.
- Full Auto run to RLM through the recall consumer: NOT WIRED. The consumer is
  built and tested. No live caller exists.

## 4. Can RLM traverse `docs/transcripts/`

The corpus is 268 Markdown files. The size is about 5.1 megabytes. The theme
guide is `docs/transcripts/README.md`.

The corpus source contract is `RlmCorpusSource`. Evidence:
`packages/rlm/src/corpus/source.ts:6`. The engine accepts any handle that the
source resolves. Two adapters exist in the monorepo:

- `apps/openagents-desktop/src/desktop-history-corpus-source.ts` reads the
  durable event log and neutral thread snapshots. Its scope kinds are `Thread`,
  `Run`, and `ThreadSet`. It has no folder or file scope.
- `apps/openagents-desktop/src/managed-rlm-corpus-policy.ts` reads a managed
  store.

Neither adapter reads a folder of Markdown files. So RLM cannot point at
`docs/transcripts/` through a production adapter today. The available path is the
SDK inline corpus builder `buildInlineCorpusInput`. Evidence:
`packages/rlm/src/corpus/handle.ts:167`. The caller reads the files, splits the
text, and builds inline entries. This path is a test and small-case path. This
audit used that path for the runs in section 5.

To make a first-class transcript source, a developer adds a corpus source
adapter that:

1. lists the transcript files under a fixed root;
2. reads each file with a bounded read;
3. splits the text into ordered entries with stable entry references;
4. assigns a content digest, a coverage manifest, and exclusions;
5. returns the generic immutable `RlmCorpusHandle`.

## 5. Real runs over a transcript slice

Both runs used episodes 248 to 255. The builder split the Markdown into
paragraph entries. The entry count was 1638. The runs used the published package
`@openagentsinc/rlm@0.2.0-rc.2` from the desktop application. The runs used the
Node 24 host with `tsx`. No production code changed.

### 5.1 Tier D deterministic run

The run used a `Grep` operation with a feature-signal pattern. The Tier D
interpreter uses a real regular expression. Evidence:
`packages/rlm/src/interpreter/deterministic.ts:72`.

Result:

- terminal tag: `Partial`, reason `cap_truncated`;
- model calls: 0;
- caps hit: `maxSpans` and `maxObservationChars`;
- validated citations: 40;
- findings: 40 cited spans.

The run produced real cited feature-signal spans from the transcripts. Examples,
with entry references:

- `entry.124`: "We need a conversation-first desktop, with the same work
  available for mobile, durable unattended work, and several coding agents
  composed as workers within one flow."
- `entry.148`: "We need to move away from traditional terminal interfaces and
  old IDEs. I envision a sidebar ... and a ChatGPT-style desktop app with
  history and widgets for managing long-running work."
- `entry.155`: "The follow-up identifies four material omissions from the
  current master roadmap ... conversation, native delegation, scheduling
  overnight work, and compute."

This is a real bounded demonstration of RLM traversal over the transcripts. The
output is a candidate feature-signal list. It is not a roadmap authority.

### 5.2 Tier S semantic run, scripted model, zero spend

This run used a scripted model plan. The `completeRoot` returned one fixed valid
`RlmProgram`. The program was `Grep`, then `Partition` into four parts, then
`ModelMap`, then `TransformJoinText`, then `Commit`. The `completeLeaf` extracted
one feature-signal sentence from each partition. The run made no provider call.
The run demonstrates the engine mechanics only. It is not a real model synthesis.

Result:

- terminal tag: `Completed`;
- model calls: 5, that is one root call plus four map calls;
- program nodes: 5;
- ModelMap calls: 4;
- validated citations: 8;
- committed synthesis, joined from the four map outputs:
  - "We need to ensure the harness provider selector is included"
  - "we need to never have again"
  - "we need to extend that resilience to Fable and Anthropic itself"
  - "we need to win it on identity".

Note on the Tier S grep operator. The Tier S `CorpusOp` grep escapes the
pattern. It supports a literal substring and a `*` glob only. Evidence:
`packages/rlm/src/interpreter/deterministic.ts:165` and its call at
`packages/rlm/src/program/execute.ts:117`. The Tier D grep uses a full regular
expression. A first attempt with a regular expression pattern found zero hits and
returned `Partial` with reason `invalid_citations`. A literal pattern found hits
and the run completed with validated citations. This difference is a real
usability trap for a program author.

## 6. Feasibility for the roadmap use case

The owner wants RLM to read the transcript corpus and to produce a roadmap of
missing features. This is feasible. The engine already runs over the corpus. The
gaps are adapters and admission, not the engine.

Ordered steps to make it real:

1. Add a transcript corpus source adapter. See the five points in section 4. Keep
   the adapter read-only. Give each entry a stable reference and an ordinal.
2. Choose a program shape for synthesis. A `Grep` then `Partition` then `ModelMap`
   then `ModelReduce` then `Commit` graph fits the corpus size. The `ModelReduce`
   node folds the mapped candidates into one ranked list.
3. Provide an admitted root and leaf model. Use the existing desktop model
   readiness and account policy. Pin a strategy reference. Set `requireExactUsage`
   to true for exact accounting.
4. Bound the budget. The corpus is large. Set fan-out, partition count, tokens,
   and inline output bytes to safe values. The default budget already forbids an
   artifact output.
5. Preserve citations. The `Commit` node must cite the grep hit value so the
   result validates. A program without a citation value returns
   `invalid_citations` and does not complete. This audit reproduced that failure.
6. Add a call site. For an interactive path, extend the `history_recall` scope so
   the tool can resolve a transcript corpus under host admission. For a Full Auto
   path, call `runFullAutoRecall` from the continuation framing, or add a
   dedicated transcript synthesis run outside a customer run.
7. Label the output. The result is a cited candidate. It is not a verified
   roadmap. Route any product claim through the promise and evidence gate.

Cost and quality caveats:

- The corpus is about 5 megabytes. A full semantic run over all 268 files needs a
  careful budget and a real cost estimate. This audit did not run a real provider.
- The consumption contract in `docs/rlm/OPENAGENTS-CONSUMPTION.md` section 11
  Phase F requires an evaluation gate before a public quality claim. That gate is
  not complete. So a produced roadmap stays a candidate until that evaluation.

## 7. Honest limits of this audit

- This audit did not run a real provider. It did not drive `codex exec`. The
  Tier S run used a scripted model to prove engine mechanics without spend. So
  this audit does not prove real synthesis quality.
- This audit ran the published package `0.2.0-rc.2`. The local working copy in
  the AI SDK repository is `0.2.1-rc.2`. The behavior evidence is consistent
  across the two. The line references in section 2 use the AI SDK working copy.
- The demo output is a bounded example over eight episodes. It is not a corpus
  conclusion and not a roadmap authority.

## 8. Document location

This document is at `docs/ai-sdk/2026-07-22-rlm-full-auto-transcript-roadmap-audit.md`.
The location is the AI SDK documentation area. Sibling documents are the AI SDK
README, the getting-started guide, and the package guide. This area has no
manifest classification gate. The Sol documentation area has that gate, so this
audit avoids it.
