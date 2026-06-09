# Autopilot Task: Chefgroep Site Generation

Status: ready for dispatch

Target repo: `OpenAgentsInc/openagents`

Target branch: `main`

Primary agent: `agent_adjutant`

Team: `team_openagents_core`

Project: `project_adjutant`

Visibility: team during generation; customer/public only after operator review.

## Assignment

- assignmentId: `adjutant_assignment_eeed1c3135b247f496fdd43f06975080`
- assignmentKind: `site_generation`
- goalId: `agent_goal_6480cf25f0b84062985c71ff5de1d5a9`
- softwareOrderId: `software_order_57593c2c60c54d25a140588633e3b318`
- siteId: `site_project_1eef2f6861e54dab8b8c5e3283277a48`
- site slug: `onlinechef-57593c2c60c54d25a1405886`
- eventual review URL: `https://sites.openagents.com/onlinechef-57593c2c60c54d25a1405886`
- first-batch policy: `public_beta_free`
- first-batch policy ID: `first_batch_payment_policy_d6336cc5e6c946a4b0624bc9c8064baf`

## Customer Request

OnlineChef submitted a full Chefgroep.nl site remake/design-system request for
the OnlineChefGroep ecosystem. The submitted brief asks for a high-craft Vite,
React, Tailwind, Framer Motion, and Three.js site that presents Chefgroep as an
operating system for autonomous operations.

Customer-provided direction includes:

- visual metaphor: precision kitchen plus operating-system interface;
- tone: calm mastery, layered depth, composable elegance, transparent and
  verifiable;
- primary headline direction: `The operating system for autonomous operations.`;
- emphasis on 38 subsystems, 2,860+ tests, public auditability, skill
  intelligence, worker/runtime fabric, data platform, security, observability,
  and OpenChefGroep services;
- target pages: home, OS/architecture, skills, runtime, platforms, cases,
  services, about/contact, privacy, terms, and polished not-found;
- warm neutral tech palette with teal or amber accents;
- Source Serif 4 display, Inter/system body, mono stats/code;
- motion and 3D should communicate structure, support reduced motion, and
  degrade gracefully on mobile and low-power devices;
- accessibility target: WCAG 2.2 AA;
- performance target: high Lighthouse score, lazy-loaded 3D, no visual bloat.

The order references a private GitHub repository,
`OnlineChefGroep/chefgroep.nl`, selected on branch
`chore/translate-frontend-english`. Current operator GitHub credentials could
not read that private repo, so this run must not claim to have inspected or
modified private source unless the Autopilot environment has explicit
authorized access. Treat private-repository access and writeback as separate
authority gates.

## Approved Public Research

- researchBriefId: `adjutant_research_brief_5645efb3c56b4bf0865e156cf4f34a37`
- enrichmentRunId: `exa_enrichment_run_91d6b55d240e42e9b14f3896b3a939e4`
- approved source card:
  `exa_enrichment_source_a731a35f600143fa83c5800e573aae85`
- public source URL: `https://chefgroep.nl/`
- source summary: the live public Chefgroep site presents ChefGroep as the
  operating system beneath OnlineChefGroep, with agent runtime, verified data
  platform, operations layer, 38 subsystems, 2,860+ green tests, and ecosystem
  layers for runtime, memory/context, TUI, data, workers, skills, and
  operations.

Rejected enrichment sources for this assignment:

- `https://github.com/deheerhoreca` was rejected as not sufficiently relevant.
- `https://designmd.ai/chef/corpscale` was rejected as not sufficiently
  relevant.

## Objective

Produce the first reviewable Chefgroep OpenAgents Site version from the
submitted design brief.

If private source access is unavailable, generate a standalone static review
Site artifact from the brief and record a customer-safe blocker explaining that
repo import/writeback requires authorized source access. Do not push branches,
open pull requests, or imply the customer repository was modified unless the
run records the required GitHub authority receipts.

## Output Contract

- Use `adjutant_assignment_eeed1c3135b247f496fdd43f06975080` as the work
  receipt.
- Produce a reviewable Site artifact suitable for `site_versions`.
- Emit an `openagents.adjutant.site_artifact_receipt.v1` payload so OpenAgents product surface can
  ingest the output into `site_project_1eef2f6861e54dab8b8c5e3283277a48`.
- Save the version as review-ready only if the artifact is presentable,
  responsive, and free of secret-shaped or private-repo material.
- If the output is only a partial concept, save it as internal/team review or
  record a blocker instead of marking it customer-review-ready.
- Include source notes for any images, fonts, libraries, or external facts.
- Prefer generated/procedural visuals, permissively licensed assets, or
  source-attributed public assets. Do not copy copyrighted site imagery or
  customer-private assets unless the run has explicit authority.
- Keep the first result focused: a polished public review page can be a static
  concept site. Do not attempt a full private-repo rewrite unless source access
  and writeback authority are present.

## Suggested Site Structure

1. Hero: Chefgroep as the operating system for autonomous operations, with an
   OS fabric/kitchen-orchestration visual that is not decorative filler.
2. Proof band: 38 subsystems, 2,860+ tests, public auditability, repository or
   verification links when safe.
3. Architecture: runtime, skills, worker fabric, data platform, security,
   observability, and platform adapters.
4. Skills and runtime: show how recipes/skills compose into agent work.
5. Services and cases: make the business offering concrete without overclaiming
   live deployments.
6. Contact/CTA: invite review, collaboration, or a scoped implementation
   conversation.

## Safety Rules

- Do not expose secrets, provider grants, callback tokens, OAuth data, billing
  internals, private repository contents, raw customer-private data, or raw
  runner payloads.
- Do not claim repository writeback, deployment, compatibility, performance,
  test counts, or source inspection that the run did not actually verify.
- Do not use loose prompt keywords for routing or file selection. Use only the
  explicit software order, assignment, Site, and authorized file/source refs
  above.
- Do not deploy or widen access without operator review and the Sites launch
  checklist.
- User-facing copy should say `Autopilot` or `OpenAgents`, not `Adjutant`.
- If the work is blocked by private repository access, record
  `customer_input_needed` with a clear next action instead of fabricating a
  Site/import result.

## Acceptance Criteria

- A new `site_versions` row is created for
  `site_project_1eef2f6861e54dab8b8c5e3283277a48`, or the assignment records a
  customer-safe blocker explaining exactly what source access or scope is
  missing.
- Any saved version includes a concise change/result summary and asset/source
  notes in public-safe metadata.
- The customer-facing artifact does not expose private repo material, internal
  runner logs, source grants, callback details, or provider/account references.
- If marked `customer_review_ready`, the stable Site URL and dedicated version
  URL are both reachable after operator deployment/review.
- The review-ready email path uses the typed `EmailService` ledger and includes
  the early-software reply note asking the customer to reply with bug reports or
  problems.
- The run summary records the exact tests, build checks, screenshot checks, or
  manual verification performed, plus any skipped checks and reasons.

## Suggested Public/Customer Summary

OpenAgents prepared the first Chefgroep Site review artifact from your submitted
design brief. This is early beta software; please reply to the review email
with anything broken, confusing, or wrong so the next revision can improve it.
