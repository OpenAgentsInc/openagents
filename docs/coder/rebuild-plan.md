# Coder terminal rebuild plan

This document proposes how the Coder terminal gets rebuilt in this
repository, and how the two-tool agent behind it is shaped. It describes
what has landed, what comes next, and the questions that decide the rest.

The current product direction and integration issue map are in
[Coder as a Decision Router consumer](coder-as-decision-router-consumer.md).
Use that document for the flagship consumer roadmap; this rebuild plan
retains the terminal's earlier design context.

## Context and boundary

The private `coder` repository holds a mature terminal: a renderer-agnostic
UI core (`coder-ui-core`) plus a ratatui surface (`coder-terminal`). The
`bender` repository holds a two-tool agent: `Classify`, a TypeSafe System
One model that turns session state into typed judgments, and `Generate`, a
generative model that synthesizes text, patches, and plans.

This repository is open source. The rule for everything that moves over:

- Reimplement the design here; do not copy files wholesale.
- The `Classify` side is already public: `crates/jev` is the Rust SDK for
  TypeSafe's System One API, and Jev calls go through `api.typesafe.ai`
  with a user's own `TYPESAFE_API_KEY`.
- The `Generate` side hits a private backend. This repository gets a
  trait, a request/response contract, and a stub implementation. The
  endpoint, credentials, prompts, and the service itself stay out.
- No private file paths, internal URLs, ticket numbers, or secrets land in
  source, tests, fixtures, or docs.

## What has landed

`crates/coder-terminal` is the first piece — a fresh implementation of the
terminal's visual core, not a port:

- `Intensity` — the four-step amber ladder (`Quarter`, `Half`,
  `ThreeQuarters`, `Full`) over a near-black field. One hue; tone carries
  every distinction.
- `Ladder` — maps an intensity to the terminal at hand: exact RGB under
  truecolor, the nearest 256-color cube entry otherwise, `Modifier::DIM`
  for the faint half when `NO_COLOR` is set.
- `Editor` — the composer's renderer-agnostic editing model: grapheme-step
  caret motions, word motions and kills, soft-wrap on word boundaries, a
  scroll window of 2–8 rows, prompt history with stash-and-restore.
- `Composer` — the framed input box: hairline frame, ` > ` prompt gutter,
  status/location rail on top, token rail on bottom, block caret.
- `handle_key` — terminal keys onto the editor: readline chords,
  `Enter` submits, `Alt-Enter`/`Ctrl-J` inserts a newline, `Up`/`Down`
  walk wrapped rows then history.
- `examples/shell` — a runnable loop: scrollback above, the composer at
  the foot. Submitted drafts echo into the scrollback; the stub reply
  marks where the agent plugs in.

Verify with `cargo test -p coder-terminal` and
`cargo run -p coder-terminal --example shell`.

`crates/coder` is the agent on top of it — a live conversation, not a
mock:

- `classify` — the question set and routing table in one module. One
  question over the structured state (`task`, bounded `transcript`):
  `action` (Choice: `respond` / `clarify` / `end_conversation` / `none`).
  The set is `coder-turns-v2`; v1's `needs_code`, `risk`, and `progress`
  are retired by
  `docs/decision-models/2026-09-20-coder-question-baselines.md`. The
  router halts on missing answers and reads the argmax choice; `none`
  answers unrouted.
- `generate` — the `Generate` trait plus `ResponsesDoor`, a streaming
  client for any Open Responses endpoint: `POST {base}/v1/responses`,
  `input_text`/`output_text` message items, `response.output_text.delta`
  events into a sink, usage out of `response.completed`. `Door::from_env`
  reads `CODER_DOOR_URL` (default the public Vercel AI Gateway),
  `CODER_MODEL`, and `CODER_DOOR_KEY`/`CODER_AI_GATEWAY_KEY`; no key means
  a `StubGenerate`, so the shell runs with no credentials.
- `agent` — the two-phase turn: `classify` returns a `Verdict` the
  terminal draws inline before `reply` streams the answer.
- `coder` (the binary) — the terminal itself: scrollback with word-wrap
  and hanging indents, the judgment rendered as dim lines under each user
  turn, the reply streaming at `ThreeQuarters`, the spinner in the prompt
  cell while a turn runs, tokens in the bottom rail. Runs with
  `cargo run -p coder`.

## The two-tool agent

The agent is a loop with two model calls, each a narrow contract.

### Classify

`Classify` sends one structured state object and a map of typed questions
to Jev, and gets one typed answer per question — a `Choice` with
probabilities and confidence, a `Noul` probability, or a `Score` on an
ordered rubric. It generates no text, so it cannot fabricate. The loop
uses it to pick the next action, gate risky steps, and judge whether the
task is done. Batched questions over one state run in parallel and cost
little, so the question set should grow to what the routing table needs,
not stay minimal.

Design rules carried over from an audit of bender's `Classify`:

- Every `Choice` includes a no-match option. Choice probabilities sum to
  one, so without a `none` the model must name an action even when none
  fits — that is what produced runaway `apply_edit` and `search_code`
  loops in bender.
- Select rather than generate wherever the answer is a closed set. File
  paths, candidate edits, and line anchors are `Choice` questions over
  real candidates, not generated strings.
- Send structured state — named JSON fields, not a concatenated string —
  and inject the index and observations the questions need instead of
  letting the model request them.
- Keep the question set and its thresholds in one reviewable module
  beside the routing table that consumes them.

### Generate

`Generate` produces text: code, diffs, explanations, plans. The public
crate owns only the contract:

```rust
pub trait Generate {
    async fn generate(&self, request: GenerateRequest)
        -> Result<GenerateResponse, GenerateError>;
}
```

`GenerateRequest` carries the task, the action `Classify` selected, and a
bounded slice of session state. `GenerateResponse` carries the produced
text or patch plus whatever structured fields the contract grows. The
crate ships a stub that returns a canned response, which keeps the shell
and tests working with no backend. The real client — endpoint, auth,
transport, streaming — lives in the private repository and implements the
same trait.

This split is the point of the architecture: `Classify` is fast, cheap,
and typed, so the loop asks it on every step; `Generate` is expensive and
private, so the loop calls it only when `Classify` routes to
`generate_answer` or `apply_edit`.

### The loop

1. The shell collects state: the task, the transcript, the file index,
   the last tool results.
2. `Classify` answers the question set: next action, confidence, risk,
   progress, blocked, repeats.
3. The router applies thresholds: act on the choice, halt on `none` or
   missing answers, confirm on high risk, stop on `task_complete`.
4. A deterministic tool runs (`read`, `search`, `build`) or `Generate`
   is called through the trait.
5. The result folds into state; the loop returns to step 1.

## What comes next

Ordered, each independently shippable:

1. **Tools.** `read_file`, `search` (literal string over a bounded
   directory), `run_build` (allowlisted commands), `apply_edit`
   (line-anchored, not generated `old_string`). A question that routes to
   them is scored on a harvested suite before it is asked; the re-specified
   `risk` in the question baselines record is the proposal.
2. **A richer question set.** `is_blocked`, `repeats`, and file selection
   as `Choice` over the real index, per the design rules above.
3. **Selection, mouse, and paste** in the editor — the pieces the basic
   box skipped.
4. **Syntax highlighting** through the intensity theme — the other half
   of the original's visual identity.
5. **Markdown rendering** for replies — the terminal should draw
   generated structure, not raw markers.

## Open questions

- **The `Generate` contract.** Text in, text out, or a streaming event
  protocol (tokens, tool calls, structured patches)? Streaming matters
  for the terminal's feel but doubles the contract's size.
- **Where the private client lives.** A separate private crate the shell
  links behind a feature flag, or an env-var-configured binary the shell
  spawns and talks to over stdio? The second keeps the private code fully
  out of the dependency graph.
- **Auth for `Generate`.** Does the open crate define the credential
  shape (env var name, header name) while the private side defines the
  endpoint, or does even that stay private?
- **Question set scope.** Bender's set was six questions; the audit says
  a dozen cost little. Which questions does the first milestone actually
  route on — and which `none` outcomes does each need?
- **Thresholds.** Confidence floors and the repeat-suppression rule need
  numbers measured on this agent's data, not borrowed ones. What is the
  labeled set that tunes them?
- **State shape.** How much of the repository goes into the `Classify`
  state — the file index, the symbol index, a bounded observation list?
  Each token is multiplied by every question.
- **Terminal backends.** Crossterm only, or is a second surface (the
  event-source choice) expected soon enough to shape the `handle_key`
  seam now?
- **Licensing of the look.** The amber palette and ladder are reimplemented
  here from the design, not copied. Is that sufficient, or does any piece
  need an explicit relicense note?
