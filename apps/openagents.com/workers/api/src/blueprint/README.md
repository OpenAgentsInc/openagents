# Blueprint Kernel Boundary

This directory is the OpenAgents product surface-owned Blueprint kernel implementation boundary.

It owns Effect-first TypeScript schemas, services, repositories, projection
helpers, fixtures, export adapters, and tests for the Blueprint Program kernel.
The deprecated Rust Blueprint workspace is reference material only.

## Layout

- `boundary.ts`: source-of-truth boundary manifest and initial module catalog.
- `schemas/`: Effect Schema definitions for Objectives, Program Types,
    Signatures, Module Versions, Program Runs, Continuation Decisions, Action
    Submissions, Source Authority, Context Packs, Release Gates, Optimizer
    Runs, and Simulation Branches. Implemented slices:
  - `schemas/objective.ts` models Objective Types, Objective Runs,
    accepted-outcome links, policy refs, allowed surfaces, and release gates.
  - `schemas/program.ts` models Program Types, Program Signatures, input/output
    schema refs, decode policy, evidence requirements, receipt requirements,
    tool scope, status, and risk class.
  - `schemas/module.ts` models Module Versions, module kinds, lifecycle status,
    release state, provenance, scorecards, release decisions, rollback anchors,
    and deprecation anchors.
  - `schemas/action-submission.ts` models approval-gated write proposals,
    dry-run state, approval state, execution receipts, failures, and terminal
    states.
  - `schemas/source-context.ts` models Source Authority records, Context Packs,
    source freshness, consent, confidence, classification, included/excluded
    context, and public/customer-safe projections.
  - `schemas/release-gate.ts` models eval fixtures, release gates, rollback
    posture, policy/review state, receipt evidence, scorecards, explicit
    decisions, and self-promotion blocking.
  - `schemas/optimizer-run.ts` models Optimizer Runs, retained failures,
    candidate module refs, scorecards, release gate refs, and evidence-only
    optimizer output.
  - `schemas/simulation.ts` models Simulation Branches, Scenario Forks,
    simulated-only effect isolation, and no-production-effect projections.
  - `schemas/program-registry.ts` models operator-safe Program Registry
    projections, run detail projections, and the future registry API seed.
  - `schemas/continuation-decision.ts` models between-turn continuation
    decision input/output envelopes, evidence-only authority flags, direct
    effect denial lists, source authority refs, receipt refs, and Program
    Signature linkage.
  - `schemas/continuation-decision-queue.ts` models customer/operator Decision
    Queue projections for pending continuation decisions, next orders,
    blockers, approvals, retries, account-failover needs, stop conditions,
    workroom/order/Site refs, evidence refs, and receipt refs.
  - `schemas/continuation-release-gate.ts` models continuation-specific
    release-gate evaluation results for Program Signature and Module Version
    promotion.
  - `schemas/continuation-mission-briefing.ts` models public/customer/team/
    operator Mission Briefing projections for Site and coding workrooms,
    including changed artifacts, evidence, verification, emails, blockers,
    costs, routes, acceptance requests, links, and next actions.
  - `schemas/mission-briefing-metric.ts` models Mission Briefing usefulness
    feedback, elapsed-time buckets, comprehension results, missing context,
    follow-up actions, safe projections, and aggregate counts.
  - `schemas/developer-package-contribution.ts` models reviewed developer
    package contributions for Program Signatures, Module Versions, context
    packages, outcome templates, and UI bindings without granting runtime
    authority.
  - `schemas/signature-contribution.ts` models non-authoritative Program
    Signature and Module Version contribution drafts with contributor refs,
    source refs, intended family, risk class, required fixtures, review state,
    rejection/promotion refs, and explicit denied runtime effects.
  - `fixtures/program-registry.ts` seeds the first Autopilot continuation
    Program Registry projection.
  - `fixtures/continuation-decision-fixtures.ts` retains public-safe
    first-batch continuation fixtures for successful continuation,
    unverified changes, failed repairs, summaries, missing context, account
    retries, stop, escalation, and prepare-review decisions.
  - `services/smoke-probe.ts` defines the Blueprint no-network smoke and
    deployed probe discipline with fake Effect layers and retained failure refs.
  - `exports/contract-export.ts` seeds JSON Schema/OpenAPI refs, event catalog
    entries, and receipt catalog entries for agent and Rust-side consumers.
- Future `repositories/`: D1-backed repositories for persisted Blueprint
  records.
- `repositories/program-runs.ts`: D1-backed Program Run evidence repository.
- `services/`: Effect services that enforce evidence-only and approval-gated
  behavior. `services/program-run-authority.ts` denies deploy, email, PR,
  spend, source-mutation, and public-claim effects from Program Run authority.
  `services/continuation-decision.ts` classifies completed or interrupted
  Autopilot turns into `continue`, `test`, `fix`, `summarize`,
  `request_context`, `retry_account`, `stop`, `escalate`, or
  `prepare_review` decisions while keeping direct effects behind Action
  Submissions. `services/continuation-decision-queue.ts` projects those
  decisions into operator-safe and customer-safe queue rows.
  `services/continuation-mission-briefing.ts` renders concise but drillable
  Mission Briefings from queue projections and public-safe refs without raw
  runner logs, raw emails, secrets, or raw timestamps.
  `services/mission-briefing-metric.ts` projects and aggregates whether those
  briefings are understood inside the two-minute target.
  `services/continuation-release-gate.ts` wraps generic Blueprint Release Gate
  predicates with continuation/autopilot target checks, rollback-anchor
  requirements, explicit operator decision checks, and self-promotion denial.
  `services/developer-package-contribution.ts` keeps developer package
  contributions evidence-only until review and release-gate promotion and
  explicitly denies deploy, spend, email, repository mutation, public posting,
  Site creation, and runtime dispatch authority.
  `services/signature-contribution.ts` keeps marketplace/community
  contribution drafts non-authoritative until review and release-gate
  promotion.
- Future `exports/`: generated JSON Schema and OpenAPI artifacts for agents
  and Rust consumers.
- `fixtures/`: retained fixtures and release-gate examples. The first seeded
  catalog is `fixtures/autopilot-continuation-signatures.ts`, covering
  continuation, test, fix, summarize, request-context, retry-account, stop,
  escalate, prepare-review, route-selection, research-policy,
  email-decisioning, and proof-projection signatures.

## Boundary Rules

- Program Runs are evidence, not write authority.
- Action Submissions are the write-side boundary.
- Public/customer/agent surfaces use projection helpers.
- Runtime promotion requires Release Gates.
- The kernel may export contracts to Rust/Pylon/Probe/Psionic/Nexus/Treasury,
  but it must not depend on deprecated Blueprint code.
