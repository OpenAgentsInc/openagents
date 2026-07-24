# Historical Rust and WGPUI harvest for Omega

- Class: historical source audit and current port recommendation
- Status: current harvest plan
- Date: 2026-07-24
- Audience: product, Omega, Nostr, provider, and assurance teams
- Historical source snapshot:
  `5c0579953db1a7cc998c5d3a8d7880f842032a8d`
- Removal commit: `f5919c766930d5913d67484660ff670dd92776fd`
- Current implementation rule: reimplement accepted behavior with native Zed GPUI
  and canonical `nostr-effect` contracts

## Decision

Harvest the historical Rust product model, interaction patterns, state machines,
fixtures, and failure lessons.

Do not port WGPUI as a framework.
Do not revive the Autopilot application as a second desktop architecture.
Omega already contains the real Zed GPUI substrate that the historical work
was trying to approximate.

The highest-value harvest is a native **Jobs and Markets** path in Omega.
It should combine the best historical NIP-90 buyer, provider, relay, payment,
receipt, and inspection behavior in one coherent GPUI surface.
It should be part of the current Buzz parity work, because a standard NIP-90
job is the clearest first proof that an external Nostr-native agent or provider
can do useful paid work without ACP.

Full historical product parity remains a useful destination.
This week should not recreate every historical pane.
It should prove one complete standard NIP-90 market loop and establish the
native component grammar that later compute, labor, data, skill, and trajectory
markets can reuse.

## Audit boundary and source truth

The audit covered all historical OpenAgents Rust generations that exposed
product UI, WGPUI framework behavior, Zed-inspired contracts, NIP-90 protocol
or runtime behavior, marketplace views, payment evidence, and provider tools.

The last complete Rust tree before the Bun and Effect rebuild is
[`5c0579953db1a7cc998c5d3a8d7880f842032a8d`](https://github.com/OpenAgentsInc/openagents/tree/5c0579953db1a7cc998c5d3a8d7880f842032a8d).
Commit
[`f5919c766930d5913d67484660ff670dd92776fd`](https://github.com/OpenAgentsInc/openagents/commit/f5919c766930d5913d67484660ff670dd92776fd)
removed that tree during the 2026-06-09 workspace rebuild.
The removal did not make the older implementation current authority.
It made it historical source evidence.

Two earlier generations also matter:

- `crates/autopilot_ui` last appears at
  [`b118d632797c1e83cf7ba8f82b386f66d601c07b`](https://github.com/OpenAgentsInc/openagents/tree/b118d632797c1e83cf7ba8f82b386f66d601c07b/crates/autopilot_ui).
  It contains the earlier canvas, editor, inbox, chat, wallet, identity, Pylon,
  marketplace, and NIP-90 submission experience.
- `apps/autopilot-desktop-wgpu` last appears at
  [`f6a29097242d77b38dd0c66439eea36d039c4f93`](https://github.com/OpenAgentsInc/openagents/tree/f6a29097242d77b38dd0c66439eea36d039c4f93/apps/autopilot-desktop-wgpu).
  Its migration material explicitly describes a Zed and GPUI-inspired entity,
  render, action, layout, and visual-test model.
- `apps/inbox-autopilot/daemon/src/bin/wgpui_background_demo.rs` is available
  at
  [`ad65b0b335f073f9b9635bf2288b5919df9a2914`](https://github.com/OpenAgentsInc/openagents/blob/ad65b0b335f073f9b9635bf2288b5919df9a2914/apps/inbox-autopilot/daemon/src/bin/wgpui_background_demo.rs).
  It is a useful visual reference, not a product authority.

Current code wins when this document and historical code differ.
The current NIP-90 protocol owner is [`packages/nip90`](../../packages/nip90/README.md),
which re-exports the canonical implementation from the sibling
`nostr-effect` repository.
The current OpenAgents web API has a bounded
[`buy-mode-dispatcher`](../../apps/openagents.com/workers/api/src/buy-mode-dispatcher.ts).
The historical Pylon provider files and historical relay application are not
present on current main.
The root repository rules state that the relay application is retired.
This harvest must not recreate it.

## Scale of the historical Rust product

The final historical snapshot was a large implementation, not a small visual
experiment.

| Area                                   | Files | Rust files | Rust lines | Rust tests | `unwrap` or `expect` sites |
| -------------------------------------- | ----: | ---------: | ---------: | ---------: | -------------------------: |
| `crates/wgpui-core`                    |     8 |          7 |      2,479 |         40 |                          8 |
| `crates/wgpui-render`                  |    11 |          5 |      2,698 |         12 |                          7 |
| `crates/wgpui`                         |   335 |        301 |     97,895 |        976 |                        213 |
| WGPUI family total                     |   354 |        313 |    103,072 |      1,028 |                        228 |
| `apps/deprecated/autopilot-deprecated` |   234 |        189 |    368,829 |      1,489 |                      2,843 |

The earlier `crates/autopilot_ui` generation contained 8 Rust files and 12,698
Rust lines.
Its compact size came from concentrating many product areas in a few files.
The final Autopilot generation expanded those areas into many modules, but it
also accumulated a 44,786-line `app_state.rs` and several modules above 12,000
lines.

The source volume proves that valuable product thinking and executable behavior
existed.
It also proves that a source-level port would carry substantial coupling,
duplicate framework code, and panic risk into Omega.

## What was actually borrowed from Zed

The historical code explicitly borrowed Zed and GPUI concepts.
It did not embed Zed's GPUI implementation.

| Historical concept         | Evidence                                                                                                                           | Omega disposition                                                                           |
| -------------------------- | ---------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------- |
| Typed actions              | WGPUI's action module says its action system was inspired by Zed GPUI                                                              | Use native `gpui::Action` and Zed action dispatch                                           |
| Entity and render model    | The WGPU migration plan describes `Entity<T>`, `Context<T>`, `Render`, and `RenderOnce`                                            | Use native GPUI entities and render traits                                                  |
| Flex layout helpers        | The migration plan names `h_flex`, `v_flex`, `flex_1`, `min_w_0`, overflow, and rem helpers                                        | Use Zed `ui` and GPUI layout APIs                                                           |
| Logical pixels             | WGPUI text documentation aligns its coordinate model with Zed                                                                      | Keep native GPUI pixels and text system                                                     |
| Keymaps and contexts       | WGPUI compared its manual registration, simple context strings, bubble-only routing, and single keystrokes with Zed's richer model | Use Zed inventory registration, key contexts, capture and bubble routing, and key sequences |
| Component and visual tests | Migration material proposes `TestAppContext` and visual-test ergonomics                                                            | Use GPUI test and visual-test support                                                       |
| Compact tool rows          | Tool cards describe Zed-style one-line rendering                                                                                   | Adapt the presentation to existing Omega agent UI components                                |

This distinction sets the main harvest rule.
When history contains a substitute for a Zed primitive, use Zed.
When history contains OpenAgents-specific product semantics, preserve and
reimplement them.

### Current native Omega substrate

The current Omega tree already contains the native systems that the historical
framework tried to approximate.

| Native area                   | Current Omega source                                                                                                                                                                                                                                        | Harvest use                                                                    |
| ----------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------ |
| Workspace and panes           | [`workspace`](https://github.com/OpenAgentsInc/omega/tree/6abbd28aca7142292b0223f198fb89c1b81e2e52/crates/workspace/src)                                                                                                                                    | Register Jobs and Markets as native items and panels                           |
| Lists and virtualized content | [`gpui` elements](https://github.com/OpenAgentsInc/omega/tree/6abbd28aca7142292b0223f198fb89c1b81e2e52/crates/gpui/src/elements)                                                                                                                            | Render job, event, provider, and relay histories                               |
| UI components                 | [`ui` components](https://github.com/OpenAgentsInc/omega/tree/6abbd28aca7142292b0223f198fb89c1b81e2e52/crates/ui/src/components)                                                                                                                            | Use native modal, menu, popover, scrollbar, notification, and tooltip behavior |
| Command palette               | [`command_palette`](https://github.com/OpenAgentsInc/omega/tree/6abbd28aca7142292b0223f198fb89c1b81e2e52/crates/command_palette/src)                                                                                                                        | Expose typed market and inspector actions                                      |
| Agent interaction             | [`agent_ui`](https://github.com/OpenAgentsInc/omega/tree/6abbd28aca7142292b0223f198fb89c1b81e2e52/crates/agent_ui/src)                                                                                                                                      | Adapt thread, message, permission, diff, terminal-tool, and status views       |
| Markdown and terminal         | [`markdown`](https://github.com/OpenAgentsInc/omega/tree/6abbd28aca7142292b0223f198fb89c1b81e2e52/crates/markdown/src) and [`terminal_view`](https://github.com/OpenAgentsInc/omega/tree/6abbd28aca7142292b0223f198fb89c1b81e2e52/crates/terminal_view/src) | Present provider output and open attached execution evidence                   |
| GPUI tests                    | [`visual_test_context`](https://github.com/OpenAgentsInc/omega/tree/6abbd28aca7142292b0223f198fb89c1b81e2e52/crates/gpui/src/app/visual_test_context.rs)                                                                                                    | Rebuild deterministic historical scenarios as native tests                     |

The target is an OpenAgents product layer that is native to this substrate.
It is not a compatibility wrapper around WGPUI.

## Historical component inventory

### Framework and generic components

WGPUI implemented buttons, divisions, dropdowns, modals, tabs, text, text
input, scroll views, and virtual lists.
It also implemented a command palette, context menus, frame clips, hotbar,
notifications, panes, resizable panes, tooltips, and a status bar.

These are not port candidates.
Omega already has native pane and dock management, workspace items, modal
layers, command palette behavior, lists, uniform lists, popovers, context menus,
dropdowns, scrollbars, tooltips, and notifications.
Replacing those systems with WGPUI code would move Omega away from Zed and
increase the future rebase cost.

WGPUI also implemented grid, moving, puff, dots-grid, heatmap, reticle,
ring-gauge, scanline, and signal-meter visuals.
The semantic visuals can inspire a native implementation when a live product
view needs them.
The render implementation should not move.

### Reusable product atoms

The historical atom set is a useful product vocabulary.
The atoms should become native Omega components only when a current surface
uses them.

| Group               | Historical components                                                                                                        | Disposition                                                            |
| ------------------- | ---------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------- |
| Agent and session   | Agent schedule, agent status, mode, model, parallel-agent, session status, streaming, thinking, tool, and tool-status badges | Adapt now through the existing agent UI and workroom roster            |
| Nostr and network   | Bech32 entity, event-kind, network, relay-status, threshold-key, and reputation badges                                       | Port now for identity, relay, event, and provider inspection           |
| Jobs and markets    | Bounty, job-status, market-type, content-type, trajectory-source, trajectory-status, skill-license, and earnings badges      | Port now as a shared Jobs and Markets vocabulary                       |
| Payment             | Bitcoin amount, payment-method icon, payment-status badge, and status dot                                                    | Port now with explicit source-quality and settlement authority         |
| Code and operations | Checkpoint, entry marker, issue status, pull-request status, resource usage, stack layer, tick event, and keybinding hint    | Reuse existing Zed status components first, then add missing semantics |
| Permission          | Permission button and daemon-status badge                                                                                    | Adapt through Omega's current permission and service-state surfaces    |
| Metrics             | APM gauge and goal-progress badge                                                                                            | Later, after the workroom and market loop have real measurements       |

Do not recreate every badge as an isolated type before its first use.
Start with shared semantic enums and theme tokens.
Render those semantics in the native component system.

### Cards, rows, and compact views

| Historical group     | Components                                                                                                                                             | Harvest decision                                                                                                                 |
| -------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------- |
| Identity and contact | Address card, contact card, message header, mnemonic display, and signing-request card                                                                 | Port address, contact, message, and signing concepts. Do not port mnemonic or raw-key display into normal UI                     |
| Agent operation      | Agent profile, session card, session search, terminal header, thinking block, tool header, permission bar, permission history, and permission rule row | Adapt to the existing Omega agent panel, thread, terminal, and permission systems                                                |
| Nostr operation      | Relay row, direct-message bubble, event entry actions, zap card, and invoice display                                                                   | Port relay and event evidence first. Add zap presentation when a current journey requires it                                     |
| Market operation     | Provider card, skill card, dataset card, balance card, payment row, and transaction row                                                                | Port provider, offer, payment, and receipt cards into Jobs and Markets. Hold public skill and dataset catalogs for a later slice |
| Code work            | Repository card, issue row, pull-request timeline item, diff header, and checkpoint restore                                                            | Map to native Zed project, Git, diff, and task truth. Preserve signed work references and receipts                               |
| Analysis             | APM comparison and session rows, collapsible section, model selector, and mode selector                                                                | Reuse only where a current product view needs them                                                                               |

### Composite product surfaces

WGPUI contained an agent state inspector, APM leaderboard, assistant and user
messages, Codex event cards, diff and terminal tool calls, direct-message
threads, event inspector, guidance cards, Markdown view, permission dialog,
receive and send flows, relay manager, schedule configuration, search tool
calls, threshold key manager, thread controls, thread entries, and zap flow.

It also contained code, message editor, metrics, terminal, thread feedback,
thread header, thread view, and trajectory sections.

Use these dispositions:

- Port the **event inspector**, **relay manager semantics**, **agent state
  inspector**, and **trajectory and receipt presentation** as native Omega
  product behavior.
- Adapt message, thread, Markdown, terminal, diff, permission, search, and code
  surfaces to existing Zed and Omega components.
- Reuse the send, receive, zap, and threshold-key interaction lessons after the
  sovereign signer and wallet authority are explicit.
- Hold APM dashboards and schedule configuration until real workroom data and
  policy exist.
- Do not port the WGPUI live editor.
  Omega already has Zed's editor and message editor foundations.

### Historical Autopilot panes

The final app registered agent, Apple adapter training, Apple Foundation Models
workbench, AttnRes lab, Buy Mode, buyer race matrix, CAD, calculator, Cast,
chat, Codex, coding project, contributor beta, credit, data buyer, data market,
data seller, earnings jobs, frame debugger, key ledger, local inference, log
stream, NIP-90 sent payments, presentation, project operations, provider
control, psionic remote training, psionic visualization, relay choreography,
relay connections, Rive, seller earnings timeline, settlement atlas,
settlement ladder, skill, Spark replay, Tassadar lab, voice playground, wallet,
and xtrain explorer panes.

The panes divide into four harvest classes.

| Class                     | Historical panes                                                                                                                                                                     | Decision                                                                                                 |
| ------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------- |
| Port now                  | Buy Mode, buyer race matrix, earnings jobs, NIP-90 sent payments, provider control, relay choreography, relay connections, key ledger, and the useful parts of settlement inspection | Consolidate into one native Jobs and Markets panel with tabs and inspectors                              |
| Adapt now                 | Agent, chat, Codex, coding project, project operations, log stream, and wallet evidence                                                                                              | Connect to native workrooms, agent UI, project truth, terminal, logs, signer, and bounded payment bridge |
| Port after the first loop | Data buyer, data market, data seller, seller earnings timeline, skill, credit, settlement atlas, settlement ladder, and Spark replay                                                 | Reuse the first market shell and canonical contracts. Add only after standard NIP-90 proof               |
| Reference or drop         | Training labs, CAD, calculator, Cast, frame debugger, presentation, Rive, voice playground, and experimental model labs                                                              | Require a separate current product decision. Do not carry them through parity by default                 |

The earlier `autopilot_ui` freeform canvas, draggable panes, resizing, panning,
and hotbar were coherent product experiments.
They are not a reason to replace Zed's workspace, dock, item, focus, and command
systems.
The useful behavior should appear as native Zed items and panels.

## Marketplace evidence

There were two different levels of marketplace implementation.
They must not be confused.

The WGPUI Storybook exposed compute, skills, data, and trajectory market types.
It showed provider cards, job states, reputation, earnings, skill and dataset
cards, and unified dashboard concepts.
Much of that layer was static demonstration data.
It is valid evidence for vocabulary, layout, and interaction intent.
It is not proof of relay, payment, delivery, or settlement behavior.

The final Autopilot NIP-90 implementation was materially deeper.
It contained buyer and provider flow state, live relay ingress, multi-relay
observation, NIP-89 handler announcements, requests, feedback, results, invoice
extraction, provider selection, wallet settlement, payment backfill, data
vending, and detailed evidence views.
Historical audits progressed from an incomplete March 4 state to a real buyer
and provider path by March 11.
By March 22, the repository recorded a relay-only paid data-selling journey
through listing, offer, access request, payment, contract, result, and byte
verification.

The harvest should take product and state-machine evidence from the final
Autopilot implementation.
It should take visual vocabulary from Storybook.
It should treat neither as current protocol authority.

## NIP-90 Rust harvest

### Protocol and request model

The historical core under `crates/nostr/core/src/nip90` implemented:

- job request kinds from 5000 through 5999
- job result kinds from 6000 through 6999
- job feedback kind 7000
- typed inputs, parameters, bids, relay hints, provider preferences, output
  requirements, amounts, and BOLT11 references
- request, result, and feedback event-template builders
- an OpenAgents data-vending profile for assets, listings, offers, grants,
  scopes, delivery modes, previews, delivery references, digests, and
  revocation

The core carried 43 tests.
The historical Nostr client also included a minimal relay-pool DVM helper for
submitting a request and awaiting a result.

Do not copy these protocol types into Omega.
Compare them against `nostr-effect` and add any accepted missing behavior to
the canonical package with fixtures and negative vectors.

### Provider lane

The 5,150-line historical provider lane modeled:

- preview, online, and degraded operating modes
- explicit relay health and catch-up state
- authenticated provider identity
- request admission, canonical parameters, output MIME types, and buyer
  responses
- publish roles, per-relay outcomes, and ingress updates
- standard compute capability and the data-vending profile
- NIP-89 handler announcement event kind 31990

This is a valuable behavior specification.
The provider loop belongs in the current contributor or Pylon runtime, or in a
bounded external provider.
It does not belong inside the GPUI process.
The old Pylon provider files are absent from current main, so the roadmap must
not claim that a live provider already exists there.

### Buyer and settlement flow

The historical buyer flow contained one app-owned snapshot with request,
active job, winner, payable, payment, timeout, and wallet-backfill states.
That model is worth preserving.
It is safer than rebuilding the journey from loosely related mutable reducers.

The UI exposed:

- a Buy Mode request and payment ledger
- provider race lanes and explicit winner or loser reasons
- result, invoice, payable, and settled states
- provider inbox, active-job, payout, and wallet metrics
- sent-payment counts, sats, fees, debits, time windows, and relay evidence
- relay health beside historical event and payment evidence
- exact Nostr actor, Lightning destination, event reference, payment pointer,
  payment hash, and source-quality fields

The loser reasons included no invoice, error-only, late result, over budget,
and lost race.
Those are useful product semantics and test cases.

### Payment and relay evidence

The historical payment model separated Nostr actors from Lightning actors.
It keyed attempts by payment pointer and tracked wallet authority, binding
quality, source quality, relay evidence, and time-window reports.
It deduplicated observations of the same event across relays.

Keep these rules:

1. A wallet or settlement receipt is payment authority.
2. Nostr carries the signed causal record and safe references.
3. A relay observation is not a separate payment.
4. Every derived payment state identifies its source and source quality.
5. Degraded, backfilled, or inferred state remains visibly labeled.
6. Exact event-to-relay evidence remains available for replay and inspection.
7. Secret preimages, raw credentials, and unbounded invoice material do not
   enter normal event or UI state.

### Protocol correctness lessons

Retain these historical corrections in the new contract and tests:

- A `p` tag expresses provider preference.
  It does not create strict provider exclusivity by itself.
- Targeted or private jobs need encryption, admission policy, and relay policy.
- The first result is not automatically the winner.
  Selection needs a non-error result and valid payable terms.
- Duplicate relay delivery must not duplicate a job, result, payment, or
  receipt.
- Result and feedback events need request and provider binding checks.
- Payment state needs restart-safe wallet reconciliation.
- Timeout, late result, loser error, and partial relay failure are normal
  states that need visible outcomes.
- One app-owned flow projection should drive the UI.
  Do not let individual panes become competing authorities.

## Native GPUI target

The first target should be a **Jobs and Markets** dock panel in Omega.
It should use native Zed workspace, pane, item, list, modal, menu, focus,
keyboard, Markdown, terminal, diff, notification, and permission primitives.

The panel should contain:

| Surface             | First behavior                                                                                                  | Historical source to harvest                             |
| ------------------- | --------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------- |
| Jobs list           | Attention, status, actor, market type, budget, age, and degraded state                                          | Job, market, provider, payment, relay, and status atoms  |
| Job composer        | Standard kind 5050 request, inputs, output, relays, budget, expiry, and provider preference                     | NIP-90 request builders and earlier submission form      |
| Job detail          | Signed causal timeline from request through closeout                                                            | Buy Mode, event inspector, and flow snapshot             |
| Provider race       | Offers, feedback, result quality, payable terms, winner, and loser reasons                                      | Buyer race matrix                                        |
| Payment and receipt | Bounded approval, wallet result, fees, receipt, and source quality                                              | Payment attempts, payment facts, and sent-payment report |
| Relay inspector     | Publish acknowledgements, ingress observations, duplicates, gaps, and failover                                  | Relay choreography and relay connections                 |
| Provider view       | Mode, identity, capability, inbox, active work, output, and payout state                                        | Provider control and earnings jobs                       |
| Evidence export     | Event identifiers, relay acknowledgements, content digests, payment receipt references, and verification result | Event inspector, key ledger, and settlement views        |

The first implementation can use tabs or a detail sidebar.
Do not recreate separate panes for every historical settlement visualization.
The data model should permit those projections later without making them
separate authorities.

## Required Week 1 proof

Use standard NIP-90 kind 5050 for the first external market journey.
The current canonical package already supports the kind and the existing web
dispatcher has a bounded foundation.
After that path works, reuse the shell for OpenAgents labor kinds and the
data-selling profile.

The required journey is:

1. The owner composes and signs a bounded request in Omega.
2. Omega publishes it to more than one admitted relay and records
   acknowledgements.
3. A provider that is not connected through ACP accepts the request.
4. Omega shows signed feedback, progress, output terms, and source relay
   evidence.
5. Provider selection rejects an error-only or non-payable first result.
6. The owner admits one capped payment through the bounded payment bridge.
7. Wallet evidence becomes the payment authority.
8. The provider publishes the result with correct request and provider
   bindings.
9. Omega verifies the result, records signed closeout or receipt references,
   and exports the causal evidence.
10. Restart and relay replay restore the same job without duplicate payment or
    completion.

If a real payment rail is not admitted during the week, run and label a
no-spend proof.
Do not claim paid-market parity from a simulated settlement.

The negative suite must cover duplicate relay events, an invalid signature,
an expired request, a `p`-tag mismatch, a result without payable terms, an
error-only first result, a late loser result, a loser error after the winner,
an over-budget offer, a payment-binding mismatch, partial relay failure, and
restart during payment.

## Port order after the first proof

1. Reuse the market shell for OpenAgents labor request, bid, award, progress,
   acceptance, and closeout kinds.
2. Port data listing, offer, access, delivery digest, and revocation behavior.
3. Add provider earnings, buyer spend, and source-quality reports.
4. Add public provider discovery, capability announcements, reputation, and
   bounded search.
5. Add skill, dataset, and trajectory market catalogs when their contracts and
   delivery authorities are current.
6. Add richer settlement and relay visualizations only when operators need
   them to diagnose real failures.

This order moves Omega beyond Buzz.
The workroom does not only coordinate attached agents.
It can discover, contract with, pay, verify, and retain portable evidence from
external Nostr-native providers.

## What not to port

Do not port:

- WGPUI renderer, scene, layout, text, focus, input, platform, clipboard,
  window, or executor layers
- WGPUI action registration, keymap, modal, pane, command palette, virtual
  list, editor, Markdown, terminal, or generic component implementations
- the freeform canvas or manual pane-bounds system as Omega workspace
  infrastructure
- the generic dynamic `UiTree` and `UiPatch` JSON protocol
- the 44,786-line Autopilot app-state monolith
- duplicate project, Git, file, editor, terminal, agent-thread, or permission
  models
- historical raw-key, seed-phrase, or mnemonic presentation in normal UI
- retired relay-service topology
- historical Pylon or provider presence claims that current source does not
  support
- Storybook state as proof of relay, payment, result, delivery, or settlement
- one component type for every old badge before a live surface needs it

Do not preserve historical module boundaries merely because they existed.
Preserve accepted state semantics, failure behavior, user jobs, and evidence.

## Assurance and implementation rules

- Freeze the canonical protocol and projection fixtures before UI lanes split.
- Put NIP-90 contract changes in `nostr-effect` and expose them through
  `packages/nip90`.
- Keep provider execution outside the GPUI process.
- Give the GPUI layer one typed flow projection and typed commands.
- Keep Zed project and editor state authoritative for code work.
- Keep wallet or settlement receipts authoritative for payment.
- Keep accepted signed Nostr events authoritative for portable job history.
- Record relay acknowledgement and observation separately.
- Test multi-relay deduplication, offline queueing, restart, replay, expiry,
  cancellation, and degraded sources before a parity claim.
- Use native GPUI tests, deterministic scheduler seeds, visual tests, and
  packaged human proof.
- Port historical golden scenarios and fixtures when their contracts still
  match current protocol.

The historical WGPUI repository had a substantial test DSL, Storybook, and
offscreen PNG capture system.
That investment is evidence that deterministic product-state fixtures matter.
Omega should express the same discipline with GPUI's existing test and visual
test tools.
The historical WGPUI test plan also contained aspirational cases, so test-plan
text alone must not count as executable evidence.

## Historical source map

Use these exact-revision sources during implementation review:

- [WGPUI action system](https://github.com/OpenAgentsInc/openagents/blob/5c0579953db1a7cc998c5d3a8d7880f842032a8d/crates/wgpui/src/action/mod.rs)
- [WGPUI action and keymap comparison](https://github.com/OpenAgentsInc/openagents/blob/5c0579953db1a7cc998c5d3a8d7880f842032a8d/crates/wgpui/docs/action-keymap-system.md)
- [WGPUI coordinate model](https://github.com/OpenAgentsInc/openagents/blob/5c0579953db1a7cc998c5d3a8d7880f842032a8d/crates/wgpui/docs/text-rendering-coordinate-system.md)
- [marketplace Storybook product](https://github.com/OpenAgentsInc/openagents/blob/5c0579953db1a7cc998c5d3a8d7880f842032a8d/crates/wgpui/examples/storybook/sections/products/marketplace.rs)
- [marketplace Storybook flows](https://github.com/OpenAgentsInc/openagents/blob/5c0579953db1a7cc998c5d3a8d7880f842032a8d/crates/wgpui/examples/storybook/sections/flows/marketplace_flows.rs)
- [WGPUI event inspector](https://github.com/OpenAgentsInc/openagents/blob/5c0579953db1a7cc998c5d3a8d7880f842032a8d/crates/wgpui/src/components/organisms/event_inspector.rs)
- [WGPUI relay manager](https://github.com/OpenAgentsInc/openagents/blob/5c0579953db1a7cc998c5d3a8d7880f842032a8d/crates/wgpui/src/components/organisms/relay_manager.rs)
- [WGPUI test harness](https://github.com/OpenAgentsInc/openagents/tree/5c0579953db1a7cc998c5d3a8d7880f842032a8d/crates/wgpui/src/testing)
- [historical NIP-90 core](https://github.com/OpenAgentsInc/openagents/tree/5c0579953db1a7cc998c5d3a8d7880f842032a8d/crates/nostr/core/src/nip90)
- [historical DVM client](https://github.com/OpenAgentsInc/openagents/blob/5c0579953db1a7cc998c5d3a8d7880f842032a8d/crates/nostr/client/src/dvm.rs)
- [provider NIP-90 lane](https://github.com/OpenAgentsInc/openagents/blob/5c0579953db1a7cc998c5d3a8d7880f842032a8d/apps/deprecated/autopilot-deprecated/src/provider_nip90_lane.rs)
- [NIP-90 compute flow](https://github.com/OpenAgentsInc/openagents/blob/5c0579953db1a7cc998c5d3a8d7880f842032a8d/apps/deprecated/autopilot-deprecated/src/nip90_compute_flow.rs)
- [buyer payment attempts](https://github.com/OpenAgentsInc/openagents/blob/5c0579953db1a7cc998c5d3a8d7880f842032a8d/apps/deprecated/autopilot-deprecated/src/state/nip90_buyer_payment_attempts.rs)
- [payment facts](https://github.com/OpenAgentsInc/openagents/blob/5c0579953db1a7cc998c5d3a8d7880f842032a8d/apps/deprecated/autopilot-deprecated/src/state/nip90_payment_facts.rs)
- [buyer race matrix](https://github.com/OpenAgentsInc/openagents/blob/5c0579953db1a7cc998c5d3a8d7880f842032a8d/apps/deprecated/autopilot-deprecated/src/panes/buyer_race_matrix.rs)
- [NIP-90 sent payments](https://github.com/OpenAgentsInc/openagents/blob/5c0579953db1a7cc998c5d3a8d7880f842032a8d/apps/deprecated/autopilot-deprecated/src/panes/nip90_sent_payments.rs)
- [provider control](https://github.com/OpenAgentsInc/openagents/blob/5c0579953db1a7cc998c5d3a8d7880f842032a8d/apps/deprecated/autopilot-deprecated/src/panes/provider_control.rs)
- [relay choreography](https://github.com/OpenAgentsInc/openagents/blob/5c0579953db1a7cc998c5d3a8d7880f842032a8d/apps/deprecated/autopilot-deprecated/src/panes/relay_choreography.rs)
- [data buyer](https://github.com/OpenAgentsInc/openagents/blob/5c0579953db1a7cc998c5d3a8d7880f842032a8d/apps/deprecated/autopilot-deprecated/src/panes/data_buyer.rs),
  [data seller](https://github.com/OpenAgentsInc/openagents/blob/5c0579953db1a7cc998c5d3a8d7880f842032a8d/apps/deprecated/autopilot-deprecated/src/panes/data_seller.rs),
  and
  [data market](https://github.com/OpenAgentsInc/openagents/blob/5c0579953db1a7cc998c5d3a8d7880f842032a8d/apps/deprecated/autopilot-deprecated/src/panes/data_market.rs)

## Final recommendation

Aim for full useful product parity with the historical Rust product over time.
Use the old source as a behavior mine, not as a dependency or architectural
template.

For this week, harvest the market vocabulary, NIP-90 flow snapshot, provider
race, payment authority, relay evidence, and event inspection into one native
GPUI Jobs and Markets slice.
Make a real standard NIP-90 external-provider journey part of the Buzz
full-core proof.

That path is more Nostr-centric than Buzz.
It makes portable work contracting, provider interoperability, multi-relay
recovery, bounded payment, and signed causal evidence native to Omega instead
of adding Nostr as a compatibility layer around attached agents.
