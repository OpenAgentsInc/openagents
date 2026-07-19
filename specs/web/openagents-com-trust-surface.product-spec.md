---
spec_format_version: "0.1"
title: "openagents.com: Public Trust Surface and Remote Supervision Client"
artifact_type: "prd"
spec_revision: 7
author: "OpenAgents"
created_at: "2026-07-17T22:03:50.000Z"
updated_at: "2026-07-19T00:00:00.000Z"
linked_github_repo: "OpenAgentsInc/openagents"
applies_to:
  - path: "apps/openagents.com/"
custom_sections:
  - id: "custom-owner-gates"
    label: "Owner Gates"
    after: "success_metrics"
  - id: "custom-receipts"
    label: "Receipts"
    after: "custom-owner-gates"
  - id: "custom-promise-links"
    label: "Promise Links"
    after: "custom-receipts"
tool_metadata:
  openagents_source_synthesis: "docs/teardowns/2026-07-17-full-catalog-synthesis-what-openagents-should-incorporate.md"
  openagents_source_transcripts: "docs/transcripts/200.md through docs/transcripts/255.md (Khala launch and counters, live money loop, referral, sell-in-public, Observer, trace views, proof-first projections, promise state machine, treasury/tipping, agent front door, market doctrine)"
  openagents_revision_3_note: "Rev 3 folds in back-catalog direction from episodes 200-237: the product-promise state machine exactly as defined in episode 234 (GREEN/YELLOW/RED/RED-Elected/PLANNED/WITHDRAWN, versioned promise slugs, human page plus programmatic agent registry, Forum as report path); the permanent I-AM-AN-AGENT homepage block and /AGENTS.md invariant (230); the public treasury page with donate flow and the Artanis pattern — an autonomous steward with bounded, receipted treasury spend authority (235); the BOLT12 agent tip flow (agent creates wallet and reusable offer from AGENTS.md instructions alone) and money-moderated Forum ranking (231, 235); episode-237 clearing-layer doctrine in positioning (accepted outcome as the atomic unit, confidence tiers as priced products, accepted outcomes per kilowatt-hour, 'the real product is the receipt that proves the wiring worked'); the open-lane stance and protocol refusals (Bitcoin/Lightning/Nostr only, never a token, no shitcoin acceptance); API-parity and agent-crawlable earnings/registry APIs (212, 224); and don't-break-userspace plus one-click data export as web-surface laws (204, 227)."
  openagents_admission_status: "roadmap-reconciled by docs/sol/MASTER_ROADMAP.md revision 119 as surface vision and target intent; implementation dispatch remains limited to live issues and exact accepted plans/work packets, with public promise, copy, settlement, and proof gates intact"
  openagents_revision_2_note: "Rev 2 folds in founder-stated direction from transcripts 238-255: the Khala public API surface with self-serve keys and per-request routing disclosure (242, 243, 244); the live tokens-served counter law — realtime, strictly monotonic, converging exactly to the ledger sum, with internal dogfood demand distinguishable from external demand (243); the /stats page with per-day token history and model-family mix (244); agents.md as the standing agent front door and the Forum as the agent community surface (238, 244); the seller path — run a Pylon — and the live money loop rendered legibly (238, 247); refer-once-earn-forever referral attribution on homepage, landing pages, and sites, with the affiliate program and sell-in-public revenue graphs (239, 247); Observer at openagents.com/observer with shareable CONFIRMED/REFUTED QA run views, videos, and exact accounting (252); /trace/{uuid} as the reusable public evidence grammar and the proof-first project board direction (252-notes, 253-notes); trace visibility tiers with pay-for-privacy and free-tier data-policy candor (242, 243, 245); benchmark publications as receipts-not-vibes with cost-per-accepted-outcome and latency percentiles (243); pricing as a thin margin over BYO tokens plus premium bulk services (255); the Verse visualization direction (240, 241, 243)."
  openagents_revision_4_note: "Rev 4 binds Cursor web/cloud-agent and Remote Control parity: start and supervise background work, search history, review changes and artifacts, intervene, rerun, and hand back to Desktop across optional local, owner-managed, or managed placement, without making the browser or cloud the canonical runtime or transcript owner."
  openagents_revision_5_note: "Rev 5 incorporates MemoHarness where web has legitimate authority: public verification and safe remote projection. Benchmark and comparison claims must bind provider, model, harness bundle, toolset, evaluator, environment, static/global/adapted class, and cache state. The trust ledger verifies harness release, adaptation, promotion, and rollback receipts; the data-flow matrix discloses experience capture, storage, retention, retrieval/training eligibility, deletion, and visibility. The browser may select only released compatible bundles/policies for remote launch and never receives raw bank contents, runs retrieval/optimization, mutates modules, promotes candidates, or gains execution authority."
  openagents_revision_6_note: "Rev 6 specifies the web projection for a Zed-quality IDE and the public code-share link contract. Signed-in supervision gains safe project tree/search/Problems/diff/proposal/test/task/artifact evidence. A versioned CodeShareBundle provides audience-scoped or deliberate public snapshot/live review with manifest verification, omissions/staleness, expiry/revocation/access audit, noindex-by-default public policy, and Desktop continuation, while excluding host paths, secrets, environments, raw terminals, private context/retrieval data, unselected repository content, and all mutation authority. Adds AC-22 through AC-28 and SM-10."
  openagents_revision_7_note: "Rev 7 binds signed-in supervision and CodeShareBundle rendering explicitly to IDE-14 of docs/ide/ROADMAP.md. Public and private code evidence uses shared Effect Schema contracts plus an allowlisted Tokyo Night semantic review projection, never a web-local DTO or executable Desktop theme. Web has no Monaco/Vim/editor authority and cannot promote a Desktop release rung. Adds AC-29 through AC-31 and SM-11 through SM-12."
  openagents_ide_architecture: "docs/ide/2026-07-18-zed-quality-ide-effect-rust-architecture.md"
  openagents_ide_roadmap: "docs/ide/ROADMAP.md (web ownership: IDE-14 CodeShareBundle and authenticated review projection)"
  openagents_ide_spec_crosswalk: "specs/IDE_ROADMAP_CROSSWALK.md"
  openagents_sibling_specs: "specs/openagents/cursor-capability-parity.product-spec.md, specs/desktop/desktop-trust-complete-workbench.product-spec.md, specs/mobile/mobile-any-host-fleet-controller.product-spec.md"
---

## Problem

The web surfaces of every audited competitor are marketing plus cloud-canonical
transcript custody, and their trust failures are public record: Cursor had two
pricing crises from opaque metering and a concealed base-model substitution.
Amp hides model identity behind mode names and holds transcripts cloud-only
with "unlisted" links that are internet-readable. Factory publishes mutually
contradictory data-flow pages. Command Code calls a hosted inference loop
"local" while posting an undisclosed device fingerprint. Almost everyone's
release chain is unsigned or checksum-only from the same origin. A developer
or team deciding whether to trust an agent vendor has nowhere to verify
anything: not what model ran, not what a run cost, not where data went, not
whether the binary they installed is what the vendor built. The same user
needs a browser surface that can actually supervise their fleet when they are
on a machine that is not theirs. And OpenAgents itself has public-economy
surfaces no competitor even attempts — a free collective-intelligence API, a
Bitcoin-paid verified-work loop, referral attribution, agent community — that
need a public home whose every number is backed by receipts, because "we are
not going to be making any big claims that are not sourced by evidence."

## Hypothesis

If openagents.com becomes the public trust surface — durable addressable
thread objects with owner-controlled receipted visibility, exact per-call
usage and model truth, live counters that converge exactly to the ledger,
shareable proof views for QA runs and traces, a dereferenceable public ledger
of release manifests and receipt verification, a published per-work-unit
data-flow matrix — and simultaneously the front door for the agent economy
(self-serve Khala API keys, the agents.md agent onboarding path, the run-a-
Pylon seller path, refer-once-earn-forever attribution, the Forum) and a full
remote-supervision client with the same typed command vocabulary as Desktop
and mobile, then trust-sensitive developers and teams will convert at
materially higher rates, agents will onboard themselves at machine speed, and
existing users will activate across surfaces — because openagents.com is the
only place in the market where an agent vendor's claims can be checked
instead of believed.

## Scope

```productspec-scope
in:
  - Meet the web, background-agent, automation-review, and Remote Control rows of `specs/openagents/cursor-capability-parity.product-spec.md`; Cursor cloud breadth is the floor while local-first custody and optional placement remain the stronger contract.
  - Own the authenticated-web and CodeShareBundle half of `docs/ide/ROADMAP.md` IDE-14 over IDE-13 portable project capabilities. Web may display the exact source release rung and gap state, but it cannot promote Files foundation, daily-use basic IDE, agent IDE, parity-candidate, or full-parity claims.
  - Launch bounded work or automations onto an explicitly selected reachable owner-local, owner-managed, OpenAgents-managed, or compatible audited-provider target, then search, monitor, inspect logs/diffs/artifacts, answer, approve, steer, queue, pause, stop, rerun, and hand the same session back to Desktop or mobile.
  - Present the thread as a durable, addressable, cross-surface work object: stable IDs and URLs, search across text, file, repository, author, and date, cross-references between threads, and remote control — while local-first custody holds and the web renders synced typed facts, never becoming the canonical transcript authority.
  - Make every visibility transition explicit and receipted: changing a thread or trace from private to shared shows the exact before and after audiences, requires confirmation, and records a receipt; no silent visibility expansion on workspace join, no ambiguous unlisted state, and an irreversible-copy warning before any public disclosure.
  - Ship remote supervision parity: an attention inbox, fleet and agent-graph views, approvals, questions, steer-and-queue controls, and continuation links that hand a session to Desktop or mobile without forking identity, history, or authority — deepening the same work rather than starting another chat.
  - Project the Zed-quality IDE safely for signed-in supervision: bounded multi-root tree, file/symbol/search, Problems, changed files, version-bound proposals/diffs, test/task outcomes, artifacts, bounded logs/excerpts, and code-to-agent causal links carry exact attachment/document/service/evidence generations, effective placement, staleness, omissions, and cached/degraded/unavailable truth without giving the browser a project process or raw root.
  - Decode every signed-in and public IDE projection from shared identified Effect Schema sources and derive web TypeScript types from them; constrained opaque refs, TaggedStruct/TaggedUnion variants, and entry decoding replace web-local raw interfaces or handwritten unions. Effect services and scoped layers own bundle compilation, audience policy, storage, verification, stream/cache lifetime, revocation, and teardown.
  - Render code, diff, diagnostic, selection, focus, staleness, omission, and verification roles through an allowlisted Tokyo Night semantic review projection derived from the initial Desktop theme contract. No executable Desktop/VS Code theme contribution crosses the boundary, and a public page never implies that deferred light/high-contrast/system Desktop theme parity or built-in Vim/editor authority exists in the browser.
  - Ship a first-class `CodeShareBundle` for deliberate code-work sharing, rendered initially as a typed code-evidence variant of the retained `/trace/{uuid}` grammar rather than a new top-level product route or second transcript surface. Each immutable revision names snapshot-versus-bounded-live mode, audience, creator, created/expiry/revoked times, safe session/project/run refs, an allowlisted tree subset, bounded syntax-highlighted excerpts, diffs/proposals/checkpoints/commits, Problems, tests/tasks, artifacts, bounded logs, agent causal links, effective runtime facts, receipt refs, content/evidence digests, omissions, staleness, and verifier metadata.
  - Make share visibility precise: private is default; named authenticated audiences, organization/team policy, expiring link access, and deliberate public publication are distinct receipted states. Public publication warns that copied content cannot be revoked, defaults to `noindex` until separately made discoverable, supports revocation of future access and access auditing, and never treats an unlisted URL as privacy.
  - Compile every share through a structural allowlist and source policy before publication. Absolute roots, filesystem topology outside selected display refs, environment variables, credentials, secrets, ignored/private files, raw terminals, raw prompts/transcripts/provider events, private context manifests, embeddings, retrieval queries/scores, private harness evidence, unselected repository content, and mutable capability tokens are forbidden; truncation and omitted counts are visible rather than silently dropped.
  - Let recipients verify and continue safely: a downloadable manifest proves every displayed item belongs to the same bundle revision and evidence generation; stable anchors cover files/ranges/hunks/Problems/tests/artifacts/agent turns; Open on Desktop carries only opaque refs and Desktop reauthorizes the current generation; a share page never exposes workspace, terminal, Git, model, harness, browser, or mutation authority.
  - Serve the Khala public API surface: the free OpenAI-compatible endpoint with self-serve keys, clear free-tier limits, and per-request routing disclosure — which backend model and orchestration path produced each response, with token counts — so routing is inspectable per message.
  - Publish live counters as ledger projections: the tokens-served counter updates in realtime, is strictly monotonic, never double-counts or moves backward, and converges exactly to the sum of exact receipted usage rows; internal dogfood demand is distinguishable from external demand so the surface never implies traction it does not have.
  - Publish /stats as the network's public instrument panel: total and per-day (non-cumulative) token history, model-family mix, and the verified-work counters of the live money loop.
  - Keep agents.md as the standing agent front door, permanently: the homepage always carries an I-AM-AN-AGENT block linking /AGENTS.md, the machine-readable onboarding path by which an agent can read one file, call the API, create a wallet with a reusable BOLT12 offer, join the Forum, and find paid work; API parity holds — everything the UI does is reachable through the documented API, so agents onboard themselves.
  - Operate the Forum as the economic coordination layer for agents and humans: coordinating in public, vetting claims against evidence, posting work for each other, and receiving Bitcoin tips, with money-moderated ranking (what people will pay to surface) as spam defense and signal — the Forum is discussion and discovery, never scheduler or authority.
  - Publish the product-promise registry with its full state machine: every promise a versioned slug carrying GREEN (confirmed with current evidence), YELLOW (partial or gated), RED (blocked or lapsed), RED-Elected (affirmative copy elected ahead of evidence), PLANNED, or WITHDRAWN, rendered as the human page and the programmatic agent registry, with the Forum as the report path when reality does not match the claim.
  - Host the public treasury page: visible balance, a donate flow, and the autonomous-steward pattern — a cloud agent with bounded, receipted treasury spend authority acting visibly on the Forum — with every spend dereferenceable.
  - Render the live money loop legibly: claim work, worker runs, validator replays, verified, both sides paid — as receipted public state, with the run-a-Pylon seller path ("How do I sell my compute, data, labor, verification? Run a Pylon") as a first-class onboarding journey.
  - Carry refer-once-earn-forever attribution: referral codes linked into the homepage, landing pages, and generated sites; durable referral binding on signup; accrual display backed by receipted rows; and the affiliate program presented with sell-in-public candor, including public revenue graphs when the owner publishes them.
  - Publish usage and model truth as product: every billable call resolves to provider, model, and cost in a routing receipt; budgets are visible before spend and reconciled against exact usage rows after; no silent model substitution, ever.
  - Publish benchmark and comparison numbers as receipts, not vibes: latency percentiles (p50/p90/p99, never the mean), cost per accepted outcome, and verification rates, sourced only from decision-grade real-seam runs — fixture runs are illustrative and never published as measurements.
  - Bind every benchmark, comparison, and adaptive-harness claim to the complete effective execution tuple: provider, model, `HarnessPolicyBundle` digest, toolset, evaluator, environment, static/global/adapted classification, experience-bank snapshot policy, and cold/warm adaptation-cache state. Results with different tuple members are separate measurements, never silently aggregated.
  - Host the public proof surfaces: Observer at its own route as the proof-design product page; shareable QA run views with honest CONFIRMED/REFUTED verdicts, videos, exact accounting, and a live board where nodes and edges light only when real receipts land; and the public trace view as the single reusable evidence grammar (agent, model, goal, verdict, cost, steps, stable anchors) rather than a second transcript viewer.
  - Grow the proof-first project board direction: public project pages generated from authority records (specs, packets, leases, verdicts, receipts) with generation timestamps and staleness, never manually editable and never an unreceipted percent-complete guess — the intended replacement class for issue-tracker coordination surfaces.
  - Publish a dereferenceable public trust ledger: release-set manifests and signing keys, the component compatibility ledger, receipt verification endpoints, and the product-promise registry, so third parties can verify artifacts and claims mechanically.
  - Extend the trust ledger to verify public-safe MemoHarness lineage: released bundle/module digests, compatibility claims, adaptation receipt signatures/digests, candidate promotion and rollback receipts, held-out evaluation refs, and the independent Blueprint release decision. A receipt proves the declared lineage and decision, not the truth of private experience content.
  - Publish trace visibility and data-policy candor: visibility tiers with named audiences, the free-tier trains-models data policy stated plainly, pay-for-privacy and confidential-compute options disclosed, and a per-work-unit data-flow matrix stating local reads, uploads, provider destinations, storage, visibility, retention, and training as separate facts consistent with observed behavior.
  - Make MemoHarness data use explicit in that matrix: whether a terminal run may compile an experience; where metadata and large evidence reside; which tenant/workspace/visibility/consent filters govern snapshot eligibility; whether an artifact is retrieval-eligible, training-eligible, or neither; retention; export; deletion/tombstone behavior; and whether any released aggregate pattern can cross workspace boundaries.
  - Project safe MemoHarness state into remote supervision: base/effective bundle digests, dimension-policy refs, adaptation state and redacted receipt ref, frozen bank-snapshot ref, compatibility, release state, and effective execution tuple. Web launch may select only released compatible bundles and admitted policies exposed by the target; the authoritative host performs retrieval, adaptation, and authority checks.
  - Ship an onboarding gradient measured in seconds: a zero-install command front door that stands up a paired supervising session with the pairing token confined to the URL fragment, import lanes that meet users inside their existing tool histories, and UI-first pairing, device-linking, and fleet-account connection flows.
  - Present pricing with margin candor: a thin transparent margin over the user's own tokens and subscriptions, premium services (bulk FastFollow runs, privacy, confidential compute) priced explicitly, and no pricing claim the receipts cannot back.
  - Pursue the Verse visualization as the public spectacle layer: live network traffic rendered spatially (requests fanned to Pylons and models) as a projection of the same receipted state the counters show, once those counters and receipts exist.
  - Keep honesty conventions product-visible: inert or unsupported configuration is labeled, degraded enforcement renders as degraded, and public counters reconcile to exact receipted rows.
out:
  - The web surface never grants desktop privilege, never holds canonical transcript custody, and never executes agent work in the browser.
  - No growth of legacy pages; the retained public product routes stay minimal, and copy changes remain behind the existing promise-registry copy gates.
  - No third-party analytics, tracking, or SaaS dependencies on the public surface.
  - No unlisted-link visibility state; every visibility state has a named audience.
  - No vanity metrics: no counter that cannot be reconciled to exact rows, no benchmark from fixtures presented as measurement, no implied external traction from internal demand.
  - Bitcoin, Lightning, and Nostr are the only rails: never a token, no shitcoin acceptance, no custodial treasury for users beyond small-balance agent wallets with sweep-out guidance.
  - No lock-in: one-click complete data export is a standing capability, and once a public surface works for users or agents it keeps working (don't-break-userspace binds the web surface too).
  - Pays-you economics copy (plugin royalties, trace monetization, paid free-tier usage) renders only per the promise registry's recorded states; planned promises are presented as planned.
  - No raw private experiences or patterns, prompts, transcript text, provider tool output, embeddings, retrieval queries or private scores, secrets, credentials, or filesystem paths in public-safe MemoHarness projections; no browser-side experience-bank mutation, retrieval/optimization execution, module editing, candidate self-verification/promotion, or authority expansion.
  - No Monaco, LSP/tsserver/DAP, Git/shell process, PTY, Rust helper, unsaved-buffer/undo custody, raw terminal stream, or general code editor in signed-in supervision or public shares; every IDE fact is a bounded host-produced projection.
  - No browser ownership or mutation of Desktop Vim enablement, mappings, modal state, editor settings, Monaco key handlers, theme selection, or editor-release-rung admission; Open on Desktop carries opaque refs and Desktop reauthorizes its own current editor configuration.
cut:
  - CUT-WEB-01: Public thread discovery feeds and leaderboards are cut; threads are shared deliberately or not at all.
  - CUT-WEB-02: An in-browser IDE or editor surface is cut; the web workbench is supervision and review, not editing.
  - CUT-WEB-03: A separate marketing microsite stack is cut; the trust ledger, the counters, and the product are the marketing.
  - CUT-WEB-04: Mutable public sandboxes, runnable terminals, edit/apply buttons, raw-repository mirrors, and unlisted-as-private links are cut from code shares; a later interactive cloud IDE would require a separate ProductSpec and authority model.
```

## Acceptance Criteria

```productspec-acceptance-criteria
- id: AC-1
  criterion: When a user opens a thread URL they are authorized for, the page renders the typed projection consistent with device truth, including a replay-to-live marker, and renders unreconstructable history as explicit transient-gap markers rather than fabricated continuity.
- id: AC-2
  criterion: When a user changes a thread's or trace's visibility, the flow displays the exact before and after audiences, warns that public disclosure is irreversible copying, requires explicit confirmation, and records a visibility receipt retrievable from the object.
- id: AC-3
  criterion: When a user inspects usage, every billable call resolves to its provider, model, and cost, and the public counters reconcile to the exact receipted rows backing them.
- id: AC-4
  criterion: When a third party fetches the trust ledger, release artifacts verify against the signed release-set manifest with the published pinned key, and the receipt verification endpoint returns a mechanical pass or fail for a presented receipt.
- id: AC-5
  criterion: When a new user runs the zero-install front door command, they reach a paired supervising session in one command plus one browser confirmation, and the pairing token never leaves the URL fragment or appears in server logs.
- id: AC-6
  criterion: When a user approves, answers, or steers from the web, the action produces the same typed durable outcome records as the equivalent Desktop or mobile action, and a continuation link opens the same session on another surface without forking identity.
- id: AC-7
  criterion: When a user reads the data-flow matrix for a work-unit type, the stated local reads, uploads, provider destinations, storage, visibility, retention, and training facts match audited behavior for that work-unit type.
- id: AC-8
  criterion: When the live tokens-served counter updates, it never moves backward or double-counts, and an auditor summing the exact usage rows for the covered period arrives at the displayed value.
- id: AC-9
  criterion: When a Khala API response is inspected through the routing-disclosure surface, it reveals the effective backend model and orchestration path and the token counts for that request.
- id: AC-10
  criterion: When a visitor arrives through a referral link and later signs up, the referral binding is durably recorded, attributed accruals render only from receipted rows, and the referrer can inspect the attribution trail.
- id: AC-11
  criterion: When a QA or assurance run completes, its shareable run view renders the CONFIRMED or REFUTED verdict, videos, and exact accounting from receipts, and any board element lights only when a real receipt landed.
- id: AC-12
  criterion: When a public project or stats page renders progress, every figure derives from authority records with a generation timestamp and staleness indication, and no manually editable or unreceipted percent-complete appears.
- id: AC-13
  criterion: When an agent fetches the programmatic promise registry, every promise carries its versioned slug, current state from the defined state machine, evidence references, and the Forum report path, and the human /promises page renders the same states without divergence.
- id: AC-14
  criterion: When the Cursor web and cloud-agent parity corpus runs, the browser can launch or resume bounded work, search session history, monitor background state, inspect logs, changes, and artifacts, answer or approve, steer or queue, pause or stop, rerun, and continue on another surface without opening Cursor or forking identity.
- id: AC-15
  criterion: When work is launched from the web, the target picker distinguishes owner-local, owner-managed, OpenAgents-managed, and compatible audited-provider placement and discloses harness, model, custody, reachability, cost, index/data flows, and retention before admission; the browser itself never gains workspace execution authority.
- id: AC-16
  criterion: When the selected owner-local target is unreachable or a managed target is not configured, the web renders the exact unavailable capability and recovery options without silently moving execution, copying the canonical transcript to cloud, or claiming the command was accepted.
- id: AC-17
  criterion: When a benchmark or comparison involving a harness is published, every result binds provider, model, HarnessPolicyBundle digest, toolset, evaluator, environment, static/global/adapted class, bank-snapshot policy, and cold/warm adaptation-cache state; materially different tuples render as separate cohorts and fixture results never become decision-grade measurements.
- id: AC-18
  criterion: When a third party verifies a MemoHarness trust-ledger entry, the endpoint mechanically checks bundle/module content digests, compatibility lineage, adaptation receipt integrity, held-out evidence refs, and independent Blueprint promotion or rollback receipt without exposing or claiming to validate private source-experience content.
- id: AC-19
  criterion: When MemoHarness state is viewed or controlled from web, the typed projection may contain only safe base/effective digests, dimension-policy refs, static/global/adapted class, adaptation state and redacted receipt ref, frozen snapshot ref, compatibility, release state, and effective execution tuple; schema decoding rejects private experiences, prompts, transcripts, tool output, embeddings, retrieval queries or scores, secrets, credentials, and filesystem paths, and no bank/optimizer/module/promotion command exists.
- id: AC-20
  criterion: When the data-flow matrix covers a MemoHarness-enabled work unit, it separately states terminal experience compilation, metadata and evidence destinations, snapshot scope and consent, retrieval versus training eligibility, retention, visibility, export, deletion/tombstone behavior, and any released aggregate cross-workspace use, and those statements match audited behavior.
- id: AC-21
  criterion: When a user launches work from web with Advanced harness controls, the target supplies only released compatible bundles and admitted adaptation policies, the browser submits immutable refs through the ordinary typed command path, and the target may fail closed before run creation; the browser never performs retrieval/adaptation or gains workspace execution authority.
- id: AC-22
  criterion: When a signed-in user supervises a coding session, the browser can inspect the safe bounded project tree, file/symbol/search results, Problems, changed files, proposal diffs, tests/tasks, artifacts, logs/excerpts, and code-to-agent links with exact attachment/document/service/evidence generations, placement, staleness, omissions, and cached/degraded/unavailable truth, while hosting no project process, raw root, or IDE database.
- id: AC-23
  criterion: When a user creates a code share, the compiler emits one immutable versioned CodeShareBundle whose allowlisted manifest binds snapshot or bounded-live mode, audience, selected tree/excerpts/diffs/proposals/checkpoints/commits/Problems/tests/tasks/artifacts/logs/agent links/runtime facts/receipts, content and evidence digests, omissions, staleness, creation/expiry/revocation, and verifier metadata before a URL is issued.
- id: AC-24
  criterion: When a private share changes to a named audience, expiring link, organization audience, or public state, the UI shows exact before/after readers and data, records a visibility receipt, warns that public copies cannot be revoked, keeps public pages noindex until separately made discoverable, and never labels an unlisted URL private.
- id: AC-25
  criterion: When any code share is compiled or fetched, structural redaction and forbidden-material tests prove it contains no absolute root, excluded topology, environment, credential, secret, ignored/private file, raw terminal, raw prompt/transcript/provider event, private context manifest, embedding, retrieval query/score, private harness evidence, unselected repository content, or mutable capability token; omitted and truncated content is counted visibly.
- id: AC-26
  criterion: When a recipient downloads the share manifest, every displayed anchor and artifact verifies against the same bundle revision and evidence generation; stale bounded-live updates are labeled, and an unavailable or failed verification never renders as current or verified.
- id: AC-27
  criterion: When a recipient selects Open on Desktop from a shared file/range/hunk/Problem/test/artifact/agent link, the URL carries only opaque safe refs, Desktop reauthorizes and resolves the current generation, and a missing historical generation opens an exact snapshot/diff or explicit unavailable state without granting the browser or link any workspace authority.
- id: AC-28
  criterion: When a share expires or is revoked, future access converges to a non-disclosing unavailable response and records the revocation while retained access-audit and public-copy limitations remain explicit; the share page has no edit, apply, Git, terminal, model, harness, browser-control, or other mutation path before or after revocation.
- id: AC-29
  criterion: When authenticated supervision or a CodeShareBundle decodes a tree, excerpt, Problem, diff, proposal, test, task, artifact, log, causal link, runtime fact, or receipt ref, web uses the same identified Effect Schema and constrained opaque refs as the authoritative project graph, derives its TypeScript types from that schema, rejects unknown/forbidden fields at entry, and has no web-local raw interface or handwritten union acting as a parallel contract.
- id: AC-30
  criterion: When code evidence renders in a private, audience-scoped, or public web view, syntax, diff, diagnostic, selection, focus, omission, stale, and verification roles come only from the allowlisted Tokyo Night semantic review projection, pass applicable contrast and non-color checks, and include no executable theme contribution; the page exposes no Desktop Vim state, key handling, theme selection, Monaco model, or editor mutation authority.
- id: AC-31
  criterion: When a web page or public statement describes IDE support, it identifies the projection as IDE-14 supervision or a versioned CodeShareBundle over exact admitted IDE-13 refs and displays source rung/staleness/verification facts; it never promotes a Desktop Files/basic-IDE/agent-IDE state into parity-candidate or full-parity status by inference.
```

## Success Metrics

```productspec-success-metrics
- id: SM-1
  metric: front_door_to_paired_activation_rate
  target: ">= 25% of front-door starts reach a paired supervising session"
  target_status: provisional
  target_owner: "owner"
  window: within 90 days of the onboarding gradient shipping
- id: SM-2
  metric: weekly_trust_ledger_verifications
  target: "baseline established, then growing month over month"
  target_status: provisional
  target_owner: "owner"
  window: rolling 30 days from trust-ledger availability
- id: SM-3
  metric: cross_surface_continuations_per_weekly_active_user
  target: ">= 2 per week"
  target_status: provisional
  target_owner: "owner"
  window: within 90 days of supervision parity shipping
- id: SM-4
  metric: usage_reconciliation_disputes
  target: "0 unresolved discrepancies between public counters and exact rows"
  target_status: committed
  window: rolling 30 days, continuously
- id: SM-5
  metric: share_of_new_signups_citing_verifiability
  target: "baseline established via onboarding survey"
  target_status: provisional
  target_owner: "owner"
  window: within 120 days of trust-ledger availability
- id: SM-6
  metric: external_tokens_served_per_day
  target: "growth trend in externally attributed (non-dogfood) tokens served, bent upward honestly"
  target_status: provisional
  target_owner: "owner"
  window: rolling 30 days, continuously
- id: SM-7
  metric: referral_attributed_signup_share
  target: "baseline established, then a growing share of signups carrying durable referral attribution"
  target_status: provisional
  target_owner: "owner"
  window: within 120 days of referral attribution shipping
- id: SM-8
  metric: cursor_web_remote_and_cloud_agent_journeys_completed_without_cursor_fallback
  target: "100% across the maintained web parity corpus"
  target_status: committed
  window: every release candidate
- id: SM-9
  metric: published_memo_harness_measurements_with_complete_effective_tuple_and_verifiable_lineage
  target: "100%"
  target_status: committed
  window: continuously
- id: SM-10
  metric: code_share_bundle_revisions_passing_manifest_integrity_redaction_audience_and_zero_authority_gates
  target: "100%"
  target_status: committed
  window: every publication and continuously for bounded-live shares
- id: SM-11
  metric: web_ide_projection_contracts_decoded_from_shared_effect_schemas_with_derived_types
  target: "100%; zero web-local parallel boundary contracts"
  target_status: committed
  window: continuously and every release candidate
- id: SM-12
  metric: authenticated_and_shared_code_evidence_roles_using_the_allowlisted_tokyo_night_semantic_projection
  target: "100% of initially supported review surfaces"
  target_status: committed
  window: every release candidate and publication
```

## Solution

The web app is three products on one typed substrate. First, a projection
client: the same generated protocol and command vocabulary as Desktop and
mobile, rendered for the browser, with custody staying local-first and sync
carrying typed facts. Second, a trust ledger: the public, machine-checkable
face of the receipts, manifests, counters, and promises the rest of the
system produces — release verification, usage truth, routing disclosure,
benchmark receipts, data-flow candor, shareable proof views for QA runs and
traces. Third, the economy's front door: self-serve Khala keys, agents.md
onboarding for agents, the run-a-Pylon seller journey, the Forum, referral
attribution, and the live money loop — every number a projection of receipted
rows. The onboarding gradient (zero-install command, fragment-token pairing,
import lanes) connects them: verification first, supervision seconds later,
participation after that. The Verse spectacle layer renders the same
receipted state spatially when the substrate is real.

For MemoHarness, web is verifier and safe controller, never optimizer or
memory host by implication. Shared Effect Schema projections expose immutable
bundle and receipt lineage. The receipt verifier recomputes public digests and
Blueprint release relationships. And benchmark views compare only identical
effective tuples. Authoritative Desktop or managed Effect services own
private retrieval, Cloud SQL metadata, private Cloud Storage evidence,
adaptation, terminal experience compilation, optimization, and release
resolution. Browser commands can name released refs but cannot acquire those
services' data or authority.

IDE supervision and sharing use another safe Effect projection, never an
in-browser IDE. The authoritative host compiles generation-bound project,
document, language, Git, task/test, proposal, artifact, and agent evidence into
bounded DTOs. For deliberate sharing, a server-side Effect service validates a
versioned `CodeShareBundle`, strips forbidden material structurally, stores
only the allowed manifest/artifacts, enforces audience/expiry/revocation, and
serves verification metadata. Monaco, LSP, Git, PTY, external runtimes, and
Rust helpers remain on the authoritative host. Desktop continuation treats
share refs as requests to reauthorize, not capabilities.
Those DTOs remain the shared identified Effect Schemas, with derived web types
and constrained refs rather than a browser-owned mirror. Effect services and
scoped layers own compilation, storage, verification, bounded-live streams,
revocation, and teardown. A safe Tokyo Night semantic subset renders review
evidence consistently without transferring theme code, Vim state, editor
settings, or any authority-bearing Desktop behavior.

## Strategic Positioning

Every competitor asks to be believed. None can be checked. Cursor, Amp,
Factory, and Command Code each failed publicly on exactly the dimensions this
surface makes verifiable. A public trust ledger is cheap to render once the
underlying receipts exist and is structurally hard for cloud-custody vendors
to copy, because their business models depend on the opacity it removes. The
economy surfaces extend the moat: no incumbent will publish
ledger-convergent counters, per-request routing disclosure, referral
attribution with receipts, or pay-both-sides verified-work loops — and the
proof-first project board direction points at replacing issue-tracker-class
coordination surfaces with intent-plus-proof projections priced around the
user's existing subscriptions.

The doctrine underneath is episode 237's clearing layer: the atomic unit of
the agent economy is the accepted outcome — scoped in advance, executed
wherever cheapest, graded against a rubric, recorded in a receipt, settled to
everyone who contributed — because "money only travels across a gap it can
verify." Confidence is priced (draft, verified, reviewed, bonded are
different products at different prices), the long-run metric is accepted
outcomes per kilowatt-hour, and "the real product is not the wiring. It is
the receipt that proves the wiring worked." This surface is where that
doctrine is publicly checkable, in the open lane: plural systems held
accountable through markets and receipts, paying the people — deflation plus
dividends — rather than pooling value at the top.

## Risks

- A trust ledger with gaps reads worse than none. Ship each ledger section
  only when its underlying receipts are real and continuously produced.
- Supervision parity on the web must not quietly turn the browser into a
  privilege escalation path. The projection-only boundary needs the same
  IPC-grade discipline as Desktop's renderer.
- Usage-truth commitments (SM-4, AC-8) create a standing operational
  obligation to reconcile counters. That cost is the product working as
  intended, but it must be staffed.
- Economy copy is regulated by the promise registry. A planned pays-you
  mechanic presented as live would be exactly the kind of unverifiable claim
  this surface exists to eliminate.
- Fragment-token onboarding depends on relay and pairing infrastructure from
  the mobile/desktop programs. Web sequencing cannot outrun them.
- The Verse layer is spectacle on top of receipts. Building it before the
  counters converge would invert the honesty ordering.
- Public adaptive-harness claims can become irreproducible marketing if the
  harness/evaluator/environment tuple or cache state is omitted. Incomplete
  tuples are unpublished, not footnoted away.
- Trust-ledger lineage can accidentally disclose private retrieval evidence.
  public receipt schemas must verify digests and decisions while keeping raw
  experiences, embeddings, queries, and scores private.
- Code shares can become accidental repository mirrors, secret leaks, or
  remote execution URLs if selection and authority are implicit. Immutable
  allowlisted manifests, structural forbidden-material scans, bounded content,
  precise audiences, noindex-by-default publication, revocation, access audit,
  and zero mutation commands are release-blocking.

## Open Questions

- Which trust-ledger section ships first: release verification (smallest
  dependency surface) or usage truth (highest user demand)?
- Do team workspaces get visibility-policy templates at launch, or is every
  share an explicit per-thread decision initially?
- How much of the supervision client is served to signed-out users viewing a
  shared thread (read-only projection versus none)?
- When does the proof-first project board graduate from OpenAgents' own
  projects to a general offering?
- Which MemoHarness aggregate patterns, if any, may be public artifacts, and
  what privacy evidence is required before their release?
- Should the initial code-share default be immutable snapshot only, with
  bounded-live shares admitted later after revocation, staleness, and audit
  behavior are proven under concurrent project generations?

## Related Artifacts

- Cursor parity contract and capability ledger:
  `specs/openagents/cursor-capability-parity.product-spec.md`
- Cursor product and local-state evidence:
  `docs/teardowns/2026-07-11-cursor-product-teardown.md`
- Zed-quality IDE projection, public share contract, and Effect/Rust boundary:
  `docs/ide/2026-07-18-zed-quality-ide-effect-rust-architecture.md`
- Canonical IDE roadmap and web IDE-14/CodeShareBundle boundary:
  `docs/ide/ROADMAP.md`
- Roadmap-to-spec and assurance traceability:
  `specs/IDE_ROADMAP_CROSSWALK.md`

- Roadmap reconciliation and AC-by-AC gap crosswalk:
  `docs/sol/MASTER_ROADMAP.md` revision 119 and
  `docs/fable/2026-07-17-surface-vision-gap-analysis-and-roadmap.md`
- Source synthesis: `docs/teardowns/2026-07-17-full-catalog-synthesis-what-openagents-should-incorporate.md`
- Competitor trust-failure evidence: `docs/teardowns/2026-07-11-cursor-product-teardown.md`,
  `docs/teardowns/2026-07-16-amp-code-teardown.md`,
  `docs/teardowns/2026-07-16-command-code-teardown.md`,
  `docs/teardowns/2026-07-16-factory-desktop-cli-teardown.md`
- Transcript sources: `docs/transcripts/230.md` (agent front door, five
  markets, flow of funds), `docs/transcripts/231.md` (Forum, money
  moderation), `docs/transcripts/234.md` (promise state machine),
  `docs/transcripts/235.md` (BOLT12 tipping, treasury, autonomous steward),
  `docs/transcripts/237.md` (clearing-layer doctrine, open lane),
  `docs/transcripts/238.md` (live money loop), `docs/transcripts/239.md`
  (refer-once-earn-forever), `docs/transcripts/240.md` +
  `docs/transcripts/241.md` (Verse direction), `docs/transcripts/242.md` +
  `docs/transcripts/243.md` + `docs/transcripts/244.md` (Khala API, counter
  law, /stats, routing disclosure, benchmark honesty),
  `docs/transcripts/247.md` (sell-in-public funnel),
  `docs/transcripts/252.md` (Observer, run views, trace grammar),
  `docs/transcripts/255.md` (pricing candor). Back-catalog laws from
  `docs/transcripts/204.md` (do-not-break-userspace), `212.md` (API parity),
  `224.md` (crawlable earnings transparency), `227.md` (no lock-in, export)
- Sibling surface specs: `specs/desktop/desktop-trust-complete-workbench.product-spec.md`,
  `specs/mobile/mobile-any-host-fleet-controller.product-spec.md`
- Public-claim authority remains the promise registry
  (`docs/promises/`, `/api/public/product-promises`).
- MemoHarness and Blueprint integration authority:
  `docs/research/2026-07-18-memoharness-blueprint-integration-analysis.md`

## Owner Gates

- All public copy changes remain behind the promise-registry copy gates with
  owner sign-off.
- Publication of signing keys and the receipt verification endpoint is an
  owner release decision.
- The data-flow matrix wording requires owner review before publication,
  since it is a standing public claim.
- Public MemoHarness benchmark copy, any cross-workspace aggregate-pattern
  publication, and the exact safe receipt/projection schema require owner or
  designated independent privacy review plus the normal promise-registry gate.
- Any team-workspace visibility defaults require owner sign-off.
- Referral/affiliate program terms, revenue-graph publication, and any
  pays-you economics promotion from planned to live are owner decisions
  bound to promise-registry state.
- The proof-first public project board route is an owner gate before it
  ships.
- Initial public CodeShareBundle publication, discoverability defaults,
  maximum content/retention bounds, and any organization-wide share default
  require owner and privacy review. Private per-object sharing never silently
  inherits a broader workspace default.

## Receipts

Planned receipt kinds this surface renders or verifies: visibility-transition
receipts, model/usage routing receipts, release-manifest verification
results, receipt-verification endpoint results, continuation-handoff records,
counter-reconciliation attestations, referral-attribution records, verified-
work payout receipts, public-safe HarnessAdaptationReceipts, harness release/
promotion/rollback receipts, immutable CodeShareBundle compilation and
verification records, code-share visibility/expiry/revocation/access-audit
records, and QA/assurance run receipts behind shareable run views. This
section plans kinds. Evidence lives in the receipt systems, not in this spec.

## Promise Links

None yet. Every public claim this surface makes (usage truth, verifiable
releases, data-flow candor, counter convergence, referral accrual, verified-
work payouts) must be registered in the promise registry with verification
gates before it appears in copy. SM-4 and AC-8 are written to be consistent
with the exact-rows law already governing public counters, and pays-you
economics remain planned-state promises until settlement evidence exists.
