# Public coherence benchmark — specification proposal

- Date: 2026-07-21
- Status: proposal (owner-directed intent, no deploy authority)
- Audience: product reviewers and future implementers
- Related methods: [rubric](./conversation-thread-coherence-rubric.md),
  [deterministic screening](./deterministic-coherence-screening.md),
  [flywheel](./coherence-flywheel.md)
- Tracking issue: [#9160](https://github.com/OpenAgentsInc/openagents/issues/9160)
- Result authority: analysis only. A new public route needs product
  admission through `docs/promises/` and the Sol roadmap before launch.

## Owner intent

Show a public graph of conversation coherence. Compare the coherence of
Claude Code and Codex conversations with OpenAgents conversations. Show
trend lines of coherence improving across versions. Let users contribute
their own traces to the benchmark for grading.

## What the benchmark publishes

The public record is the assessment, never the conversation. Each point
on the graph is a `CoherenceAssessment`:

```yaml
assessment_id: <stable public ref>
metric_revision: coherence-screen-v1 | coherence-rubric-v1
toolchain: codex-cli | claude-code | openagents-desktop | openagents-mobile | other
surface_version: <app or CLI version, when known>
trace_ref: <public /trace/{uuid} ref, or `private_local` for aggregates>
transcript_digest: <sha256 of the graded transcript revision>
graded_at: <timestamp>
grader: deterministic_screen | model_rubric | independent_review
score: 0-100
grade: A | B | C | D | F
disposition: pass | needs_correction | fail | inconclusive | screening_pass | needs_review
signal_counts: { profanity: 0, correction: 0, interrupt: 0 }
dimension_ratings: <rubric D1..D8, model or reviewer grades only>
gates_failed: [G1..G6]
```

Privacy rules are absolute: no raw prompts, no quotes, no secrets, no
private paths, no customer data. `transcript_digest` binds the score to
one frozen revision so a re-grade is detectable. Contributed traces are
already public by the contributor's own action (see below).

## The graph

One public page renders, from the assessment dataset:

1. **Toolchain comparison.** Score distribution per toolchain (box or
   violin per source, with counts). The 2026-07-21 local baseline is the
   seed shape: Codex CLI and Claude Code both at mean 90.6 with about
   14.5 percent flagged for review.
2. **Trend lines.** Mean score and needs-review rate over time, one line
   per toolchain, and one line per OpenAgents `surface_version` so a
   release visibly moves the metric.
3. **Gate incidents.** Count of hard-fail gate assessments over time.
   This line must go to zero and stay there.
4. **Metric revisions.** Vertical markers where `metric_revision`
   changed. Trend claims never cross a marker silently.

The graph makes the hill-climb public: the claim "conversations are
getting more coherent" becomes a chart anyone can re-derive from the
published records.

## Contribution path

Reuse the existing trace infrastructure instead of a new upload system:

1. A user publishes a conversation as a public trace through the
   existing ATIF trace ingest, which serves it read-only at
   `/trace/{uuid}`. Publication is the user's explicit action and
   consent. The ingest's existing redaction rules apply.
2. The user marks the trace benchmark-eligible (a typed flag on the
   submission, or a follow-up API call with the trace ref).
3. The grading pipeline freezes the trace revision, runs the
   deterministic screen, runs the model rubric grader, and appends a
   `CoherenceAssessment` to the public dataset.
4. The contributor and everyone else see the point on the graph, linked
   to the public trace ref.

Contributed traces from external toolchains (a user's own Claude Code or
Codex session) enter through the same path once converted to the public
trace format. The local grader
(`scripts/grade-conversation-coherence.ts`) already parses both local
formats, so the converter is bounded work.

## Integrity rules

- **Two grader tiers.** Deterministic screen results are reproducible by
  anyone from the public trace. Model rubric results name the grader
  model and `metric_revision`. Headline claims use independent review
  tier only.
- **No selection bias by the operator.** OpenAgents-published
  assessments include every eligible run in the window, not a curated
  subset. Contributed traces are labeled as self-selected.
- **No authority transfer.** A benchmark score is not assurance,
  release, or promise evidence. The #8979-class gates keep their own
  evidence chain.
- **Adversarial input.** Contributed traces are untrusted data. The
  pipeline parses them in a sandboxed job, enforces size bounds, and
  never executes content.

## Implementation shape (proposal)

- Dataset: an append-only public table (Cloud SQL) plus a public JSON
  export endpoint under the existing API surface.
- Grading job: a scheduled Cloud Run job consuming benchmark-eligible
  traces, writing assessments.
- Page: one public route (candidate: under the existing trace surface,
  for example `/trace/coherence`) rendering the four charts from the
  export. Route admission, copy, and promise wiring go through
  `docs/promises/` and the roadmap before anything ships.

## Non-goals

- No leaderboard of individual users.
- No grading of private conversations server-side. Private material is
  graded locally with the CLI, and only aggregates may be shared.
- No claim that a screening score equals coherence. The rubric stays the
  semantic authority.
