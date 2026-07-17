# Fast Follow

Fast Follow is OpenAgents' standing, typed source of learning work. A project
declares which external systems it follows, the exact lessons it wants from
each one, the combinations in which those lessons matter, and the boundaries
that must survive adaptation. The declaration can continuously produce
research, gap analysis, issue proposals, and—after separate admission—bounded
implementation work.

Fast Follow is not a replacement for ProductSpec, AssuranceSpec, the issue
backlog, repository instructions, or Full Auto:

- ProductSpec owns product intent: **what should exist**.
- AssuranceSpec owns proof intent: **what would justify confidence**.
- FastFollowSpec owns learning intent: **what should be studied and why**.
- A roadmap, issue, accepted plan, or work packet owns sequencing and
  implementation admission.
- Full Auto owns unattended continuation of an already authorized local
  session. It may consume Fast Follow work; Fast Follow does not widen Full
  Auto authority.

The authored convention is a nearest-scope `FASTFOLLOW.md` beside the relevant
`AGENTS.md`. Its frontmatter and typed JSON blocks form the machine projection.
Nested projects may define their own file; the nearest file replaces the
parent unless a future format adds explicit, digest-pinned composition. There
is no implicit merge in format 0.1.

## Reading order

1. [`FAST_FOLLOW_SPEC.md`](FAST_FOLLOW_SPEC.md) — normative format, lifecycle,
   artifacts, work program, sharing boundary, and Full Auto composition.
2. [`../../FASTFOLLOW.md`](../../FASTFOLLOW.md) — the first OpenAgents
   FastFollowSpec, whose ordered initial program follows
   [`Amp in a Few Days`](../fable/2026-07-16-amp-in-a-few-days-on-openagents.md)
   before returning to the complete `docs/teardowns/` catalog.
3. [`../../specs/openagents/fast-follow.product-spec.md`](../../specs/openagents/fast-follow.product-spec.md)
   — product intent and acceptance criteria for the native system.
4. [`SUGGESTED_ISSUES.md`](SUGGESTED_ISSUES.md) — dependency-ordered,
   issue-ready implementation program.
5. [`fast-follow.schema.json`](fast-follow.schema.json) — JSON Schema for the
   canonical projection compiled from an authored document.

The workspace skill at
[`../../.agents/skills/fast-follow/SKILL.md`](../../.agents/skills/fast-follow/SKILL.md)
provides the agent operating method. `pnpm run test:fast-follow` validates the
OpenAgents seed's stable references, source/directive graph, capacity profiles,
ordered Amp program, Fable strategy binding, and complete teardown coverage.

## What works in the current checkout

Format 0.1 is a dogfood contract, not a claim that the native scheduler or
shared study service already exists.

The current OpenAgents Desktop Full Auto implementation already:

- exposes a per-session **Full Auto** composer toggle;
- persists the loop across renderer/app restart;
- binds it to one workspace and provider profile;
- uses repository `AGENTS.md`, README, docs, issues, and bounded
  ProductSpec/AssuranceSpec context as candidate sources;
- takes one concrete useful action per continuation; and
- stops after 20 successful automatic continuations.

Because the OpenAgents `AGENTS.md` now points Full Auto at `FASTFOLLOW.md`, an
owner can use Fast Follow tonight without a new binary:

1. Open a session rooted at the OpenAgents repository.
2. Turn on **Full Auto**.
3. Send one of these initial instructions:

   ```text
   Run the ordered initial_program from FASTFOLLOW.md at its default
   gap_analysis stage. Work on the first non-terminal Amp day directive,
   produce one evidence-grounded gap or candidate artifact per turn, and do
   not edit product code.
   ```

   ```text
   Run the Fast Follow implementation lane from FASTFOLLOW.md. Only consume
   an already admitted issue or work packet; implement and verify one bounded
   candidate per turn.
   ```

   ```text
   Use FASTFOLLOW.md as the fallback work source after higher-authority
   actionable backlog work.
   ```

For the five-worker shape in the owner brief, use three issue/backlog sessions,
one ordered Amp-program research session, and one implementation session scoped
to a separately admitted candidate from that program. Each session gets its own
Full Auto toggle and isolated claim/worktree when it mutates. This is a manual
allocation bootstrap. Current Desktop Full Auto serializes its own
continuations and is not yet a five-worker fleet scheduler; the native
portfolio policy and concurrent supervisor are specified but remain
implementation work.

## The four artifact layers

```text
public upstream snapshot
        │
        ▼
content-addressed StudyPacket ───────── shareable across projects
        │
        ▼
target-bound GapAssessment ──────────── private to the target by default
        │
        ▼
WorkProposal / issue candidate ──────── evidence-only until admitted
        │
        ▼
implementation + target-local proof ── ordinary repo authority applies
```

This is the token-reuse boundary. Many projects following the same OpenCode
revision and lesson set can reuse one public StudyPacket. Each project must
still perform its own target-bound gap analysis, policy reconciliation,
implementation admission, and proof. A cache hit never means “adopt this.”

## Why the well does not run dry

“Limitless work” does not mean manufacturing churn. A target can change, a
study can become stale, a previously rejected lesson can be reconsidered after
intent changes, a local implementation can drift, or a stronger oracle can
become possible. Each pass is content-addressed and may honestly emit
`no_material_delta` instead of an issue. Persisted negative dispositions and
dedupe keys prevent the system from reopening the same rejected idea forever.

## Khala and Blueprint direction

Fast Follow is a natural bootstrap corpus for Khala's typed Blueprint system:

- the fixed program controls resolution, pinning, research, evaluation,
  proposal, admission, implementation, verification, and closeout;
- semantic models operate inside typed research/planning modules, never by
  rewriting control flow;
- StudyPackets and target outcomes create reusable training/evaluation data;
- GEPA/DSPy-class optimization may later propose better module parameters
  against executed outcomes and cost; and
- no optimizer, packet, model, or agent may self-promote, dispatch, merge,
  release, spend, or change a public promise.

Format 0.1 does not require GEPA and does not revive a deprecated runtime. It
defines the evidence and candidate boundaries that a future optimizer can use
safely.
