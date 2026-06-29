# Autopilot Task: Mike M Counter Dashboard Site Generation

Status: ready for dispatch

Target repo: `OpenAgentsInc/openagents`

Target branch: `main`

Primary agent: `agent_adjutant`

Team: `team_openagents_core`

Project: `project_adjutant`

Visibility: team during generation; customer/public only after operator review.

## Assignment

- assignmentId: `adjutant_assignment_a779d4df16bd407ea90adb299752f989`
- assignmentKind: `site_generation`
- goalId: `agent_goal_6480cf25f0b84062985c71ff5de1d5a9`
- softwareOrderId: `software_order_dd2f2917274c4e64bf1d678127dd6fa6`
- siteId: `site_project_1c1769628bfd41dcb52547df72381468`
- site slug: `mike-m-dd2f2917274c4e64bf1d6781`
- eventual review URL: `https://sites.openagents.com/mike-m-dd2f2917274c4e64bf1d6781`
- first-batch policy: `public_beta_free`
- first-batch policy ID: `first_batch_payment_policy_57d1673b3128419092863d251de81b58`

## Customer Request

Mike M requested a clean single-page dashboard with multiple counters. Each
counter should support increment, decrement, and reset actions, persist its
state in `localStorage`, and use smooth number animations.

Treat this as a focused first-batch Autopilot Site delivery. The customer did
not ask for a marketing page, authentication, backend persistence, billing, or
multi-user collaboration.

## Approved Public Research

- researchBriefId: `adjutant_research_brief_afb8e7f44f5a4f0daa5437eeeb9c0881`
- enrichmentRunId: `exa_enrichment_run_d7369b8a5fbe476b82a1b6645745a9be`
- approved source cards:
  - `exa_enrichment_source_561581214ef84989964007adefd89d9d`
  - `exa_enrichment_source_b3f53ca4166d4c8e95e8410b5cab1945`
  - `exa_enrichment_source_f7d995f5f23c456f86a59c614690658d`
- public source URLs:
  - `https://github.com/Haseeb-MernStack/focusflow-productivity-dashboard`
  - `https://github.com/lakshyaelite/tally-counter-app`
  - `https://github.com/didoghosh143/Productivity-Dashboard`

Use the approved sources only as lightweight public context for client-side
dashboard, counter, Pomodoro/productivity, `localStorage`, and reset-control
patterns. The customer request is the implementation authority. Do not copy
source code from the references.

## Objective

Produce the first reviewable OpenAgents Site version for Mike's counter
dashboard request.

The result should be an actual usable one-page dashboard, not a landing page.
It should feel simple, fast, and useful on first load, with persistent counters
and clear controls that work without server state.

## Output Contract

- Use `adjutant_assignment_a779d4df16bd407ea90adb299752f989` as the work
  receipt.
- Produce a reviewable Site artifact suitable for `site_versions`.
- Emit an `openagents.adjutant.site_artifact_receipt.v1` payload so OpenAgents product surface can
  ingest the output into `site_project_1c1769628bfd41dcb52547df72381468`.
- Save the version as review-ready only if the artifact is responsive,
  accessible, functional, and free of secret-shaped material.
- If the output is only a partial concept, save it as internal/team review or
  record a blocker instead of marking it customer-review-ready.
- Include source notes for any images, fonts, libraries, or external facts.
- Prefer CSS-native UI and minimal generated assets. This task does not need
  stock photography or decorative illustration.

## Suggested Product Shape

1. Header: compact title, lightweight status summary, and a global reset action.
2. Counter grid: at least four named counters with stable card dimensions and
   clear increment, decrement, and reset controls.
3. Persistence state: values restore after refresh using `localStorage`.
4. Motion: number changes animate smoothly without layout shift and respect
   reduced-motion preferences.
5. Editing affordance: allow counter names or an active counter label to be
   adjusted if this can be done cleanly within the first slice.
6. Empty/error handling: handle missing, malformed, or cleared `localStorage`
   values gracefully.

## Safety Rules

- Do not expose secrets, provider grants, callback tokens, OAuth data, billing
  internals, private operator notes, raw customer-private data, or raw runner
  payloads.
- Do not claim server persistence, team collaboration, deployment state, or
  external integrations that the run did not actually implement.
- Do not use loose prompt keywords for routing or file selection. Use only the
  explicit software order, assignment, Site, and authorized file/source refs
  above.
- Do not deploy or widen access without operator review and the Sites launch
  checklist.
- User-facing copy should say `Autopilot` or `OpenAgents`, not `Adjutant`.
- Keep the interface professional and task-focused. Do not create a marketing
  hero or explanatory in-app tutorial.

## Acceptance Criteria

- A new `site_versions` row is created for
  `site_project_1c1769628bfd41dcb52547df72381468`, or the assignment records a
  customer-safe blocker explaining exactly what is missing.
- The saved version includes multiple counters with working increment,
  decrement, reset, and persisted refresh behavior.
- Smooth number animation is present and does not cause layout shift.
- The dashboard is responsive on mobile and desktop and uses accessible button
  labels or visible labels for icon controls.
- The customer-facing artifact does not expose internal runner logs,
  provider/account references, callback details, or source grants.
- If marked `customer_review_ready`, the stable Site URL and dedicated version
  URL are both reachable after operator deployment/review.
- The review-ready email path uses the typed `EmailService` ledger and includes
  the early-software reply note asking the customer to reply with bug reports or
  problems.
- The run summary records the exact tests, build checks, screenshot checks, or
  manual verification performed, plus any skipped checks and reasons.

## Suggested Public/Customer Summary

OpenAgents prepared the first review version of your local counter dashboard.
It includes multiple persisted counters with increment, decrement, reset, and
smooth number-change behavior. This is early beta software; please reply to the
review email with anything broken, confusing, or wrong so the next revision can
improve it.
