# 2026-03-18 Data Market CLI and Agent-Skill Audit

> Historical note: this is a point-in-time audit from 2026-03-18. Current product and architecture authority lives in `README.md`, `docs/MVP.md`, `docs/OWNERSHIP.md`, `docs/kernel/`, and the latest desktop/runtime code.

## Scope

This audit answers one practical question:

> can OpenAgents data-sale flows already be driven without bootstrapping the UI,
> and if not, what exact CLI and agent-skill shape should we build so an agent
> can package local material for sale and publish it truthfully?

Primary OpenAgents surfaces reviewed:

- `apps/autopilot-desktop/src/bin/autopilotctl.rs`
- `apps/autopilot-desktop/src/desktop_control.rs`
- `apps/autopilot-desktop/src/compute_mcp.rs`
- `apps/autopilot-desktop/src/data_seller_control.rs`
- `apps/autopilot-desktop/src/data_market_control.rs`
- `apps/autopilot-desktop/src/input/tool_bridge.rs`
- `apps/autopilot-desktop/src/openagents_dynamic_tools.rs`
- `apps/autopilot-desktop/src/app_state.rs`
- `skills/autopilot-data-seller/SKILL.md`
- `skills/autopilot-data-market-control/SKILL.md`
- `docs/plans/data-market-mvp-plan.md`
- `docs/plans/data-market-mvp-implementation-spec.md`
- `docs/headless-compute.md`

Primary Agent Skills references reviewed:

- `~/code/agentskills/README.md`
- `~/code/agentskills/docs/specification.mdx`
- `~/code/agentskills/docs/skill-creation/quickstart.mdx`

## Executive Verdict

Not yet.

OpenAgents now has a real conversational seller flow, real typed data-market
tools, real kernel authority writes, and a real `Data Seller` product surface.
But that truth is still primarily app-owned.

What exists today:

- a dedicated `Data Seller` pane with a seller-specific Codex session
- typed `openagents.data_market.*` tools for draft, preview, publish, grant,
  payment, delivery, revocation, and snapshot
- built-in seller skills that constrain Codex inside that lane
- kernel authority objects and Nexus routes for `DataAsset`, `AccessGrant`,
  `DeliveryBundle`, and `RevocationReceipt`

What does not exist today:

- no first-class `autopilotctl data-market ...` commands
- no `autopilotctl` support for pane input or pane actions
- no headless data-market runtime equivalent to the compute headless path
- no data-market MCP surface equivalent to `compute_mcp.rs`
- no repo-owned packaging skill whose main job is to turn local material into a
  saleable asset bundle and publish it from CLI

So the answer is:

- the underlying truth surfaces are already there
- the desktop product can already sell data conversationally
- but a no-UI CLI and agent-skill path is still missing

The right next move is not “teach an agent to click the pane.”

The right next move is:

1. add semantic `autopilotctl data-market ...` commands backed by the same
   app-owned seller logic,
2. add deterministic local packaging helpers for digest/provenance generation,
3. then add a repo-owned packaging-and-publication skill that uses those
   commands,
4. and only after that consider a dedicated headless data-market daemon or MCP
   server.

## Current Repo Truth

### 1. The seller workflow is real inside the app

The current app already has a meaningful seller state model in
`apps/autopilot-desktop/src/app_state.rs`.

`DataSellerDraft` already captures the practical publication contract:

- `asset_kind`
- `title`
- `description`
- `content_digest`
- `provenance_ref`
- `default_policy`
- `grant_policy_template`
- `grant_consumer_id`
- `grant_expires_in_hours`
- `grant_warranty_window_hours`
- `price_hint_sats`
- `delivery_modes`
- `visibility_posture`
- `sensitivity_posture`
- `metadata`
- `grant_metadata`

The draft also already computes readiness blockers, which means the repo has an
actual definition of “what facts are required before publication.”

This matters because the missing CLI is not blocked on figuring out what the
seller must say. The contract is already embodied in the seller state.

### 2. Typed seller tools already exist

The OpenAgents dynamic tool surface already includes a real data-market family
in `apps/autopilot-desktop/src/openagents_dynamic_tools.rs` and
`apps/autopilot-desktop/src/input/tool_bridge.rs`:

- `openagents.data_market.seller_status`
- `openagents.data_market.draft_asset`
- `openagents.data_market.preview_asset`
- `openagents.data_market.publish_asset`
- `openagents.data_market.draft_grant`
- `openagents.data_market.preview_grant`
- `openagents.data_market.publish_grant`
- `openagents.data_market.request_payment`
- `openagents.data_market.prepare_delivery`
- `openagents.data_market.issue_delivery`
- `openagents.data_market.revoke_grant`
- `openagents.data_market.snapshot`

This is already the right semantic layer for automation. The missing piece is
that these tools are currently exposed to the in-app Codex lane, not to
external CLI operators as a first-class control surface.

### 3. The current built-in seller skills are pane-scoped policy skills

The repo already contains:

- `skills/autopilot-data-seller/SKILL.md`
- `skills/autopilot-data-market-control/SKILL.md`

These are useful and correctly scoped for the in-app seller lane:

- they enforce draft-before-publish discipline
- they require explicit confirmation
- they require kernel read-back after mutation
- they keep delivery and revocation truthful

But they are not yet the skill the user is asking for.

They are primarily:

- conversational policy for a dedicated pane
- typed tool usage policy for a Codex session that already lives inside the app

They are not yet:

- a packaging skill for local files or folders
- a shell-first skill for `autopilotctl`
- a headless publication skill that can run without the UI surface

### 4. `autopilotctl` is real, but narrow

`apps/autopilot-desktop/src/bin/autopilotctl.rs` is explicitly a thin CLI
client for the running desktop-control runtime.

Today it can already do useful app-owned control work:

- query status and events
- control compute- and wallet-related flows
- list/open/focus/close/status panes
- interact with chat, buy mode, training, provider state, and other existing
  desktop-control routes

But for data-market and seller control it is still missing the important parts:

- no `data-market` subcommand tree
- no `pane set-input`
- no `pane action`
- no generic typed OpenAgents tool invocation surface
- no seller prompt submission surface

So `autopilotctl` can see the product shell, but it cannot yet drive the real
seller workflow.

### 5. Compute has the headless and MCP precedent

The compute side already shows the pattern OpenAgents should reuse:

- `docs/headless-compute.md` documents a real headless compute surface
- `apps/autopilot-desktop/src/compute_mcp.rs` exposes compute control over the
  same desktop-control contract

That precedent matters.

It means the correct question is not whether OpenAgents should have headless or
MCP-style data control. It already has that pattern for compute.

The correct question is whether the Data Market should first ship:

- semantic CLI commands,
- a data-market MCP server,
- a headless data-seller binary,
- or some combination.

For the MVP, semantic CLI should come first.

## Direct Answer: Is Data Sale Accessible Via CLI Right Now?

Only partially, and not in the way you want.

### What is accessible today

- the app-owned seller truth exists
- the data seller flow can be executed inside the app through the dedicated
  pane and its attached Codex lane
- `autopilotctl` can target the running desktop-control runtime and inspect
  panes
- `autopilotctl --json` already makes many control responses machine-readable

### What is not accessible today

- there is no first-class CLI path to draft a data asset
- there is no first-class CLI path to preview or publish a data asset
- there is no first-class CLI path to preview or publish a grant
- there is no first-class CLI path to request payment, prepare delivery, issue
  delivery, or revoke access
- there is no no-UI headless data-market runner comparable to
  `autopilot-headless-compute`
- there is no general agent skill that can package local materials and publish
  them through CLI

### Important practical distinction

There are two different “CLI” questions here:

1. Can a command-line client control the app-owned seller logic?
   Not yet in a first-class way.

2. Can an agent skill already instruct an agent how to package things for sale?
   Only partially, because the required packaging and publication commands do
   not yet exist as a stable shell-facing interface.

## What The Agent Skill Should Look Like

The `~/code/agentskills` spec is clear on the right format:

- one skill directory
- `SKILL.md` with frontmatter and instructions
- optional `references/`
- optional `scripts/`
- optional `assets/`

Given that format, the right skill is not a generic prompt bundle and not a
pane-clicking helper.

It should be a repo-owned operational skill with three responsibilities:

1. package local material into a deterministic saleable bundle
2. derive the required seller facts and publication inputs
3. call stable CLI commands to preview and publish through app-owned truth

## Recommended Skill Shape

Recommended new skill:

- `skills/autopilot-data-seller-cli/`

Recommended layout:

```text
skills/autopilot-data-seller-cli/
  SKILL.md
  references/
    packaging-contract.md
    policy-template-cheatsheet.md
    cli-workflow.md
    delivery-and-revocation.md
  scripts/
    package_data_asset.sh
    emit_listing_json.sh
    publish_asset.sh
    publish_grant.sh
  assets/
    listing-template.json
    grant-template.json
```

### `SKILL.md`

This should say, in plain operational terms:

- use this skill when asked to package local files, conversations, or data
  bundles for sale in OpenAgents
- first determine the asset family and packaging boundary
- compute canonical digest and provenance
- fill a structured listing draft
- preview before publish
- publish only after explicit user confirmation
- read back published asset and grant truth
- never claim saleability or publication from prose alone

It should also explicitly prefer semantic CLI commands over raw HTTP.

### `references/packaging-contract.md`

This should map local packaging to the seller draft contract.

At minimum it should define:

- required draft fields
- accepted asset families for the MVP
- how to generate `content_digest`
- how to generate `provenance_ref`
- how to choose `default_policy`
- how to choose `delivery_modes`
- how visibility and sensitivity postures interact

### `references/cli-workflow.md`

This should describe the exact shell workflow the skill will follow once CLI
support exists:

1. package local source material
2. emit listing JSON
3. `autopilotctl data-market draft-asset ...`
4. `autopilotctl data-market preview-asset`
5. user confirms
6. `autopilotctl data-market publish-asset --confirm`
7. optional grant/payment/delivery follow-on commands

### `scripts/`

The scripts should do the boring deterministic work the model should not be
re-inventing every turn:

- collect a local file or folder into a canonical package boundary
- compute bundle hash / digest
- emit a listing JSON draft
- optionally create a grant draft JSON
- call the stable CLI commands and parse JSON responses

The model should decide what to package and what policy to use. The scripts
should do the repeatable shell work.

### `assets/`

The templates should make it easy for both humans and agents to inspect or edit
the structured draft before publish.

## What Still Needs To Be Built

### 1. First-class `autopilotctl data-market ...` commands

This is the biggest missing piece.

Recommended initial command set:

- `autopilotctl data-market seller-status`
- `autopilotctl data-market draft-asset --file listing.json`
- `autopilotctl data-market preview-asset`
- `autopilotctl data-market publish-asset --confirm`
- `autopilotctl data-market draft-grant --file grant.json`
- `autopilotctl data-market preview-grant`
- `autopilotctl data-market publish-grant --confirm`
- `autopilotctl data-market request-payment --request-id <id>`
- `autopilotctl data-market prepare-delivery --request-id <id> --file delivery.json`
- `autopilotctl data-market issue-delivery --request-id <id>`
- `autopilotctl data-market revoke-grant --request-id <id> --action revoke|expire --confirm`
- `autopilotctl data-market snapshot`

These should all support `--json`.

This is the stable operator and skill surface the repo is missing.

### 2. A desktop-control path for typed data-market operations

There are two clean ways to do this:

- add explicit `DesktopControlActionRequest` variants for the data-market seller
  family
- or add a constrained desktop-control action that invokes allowlisted
  `openagents.*` tools with JSON arguments

For MVP, the important thing is not which internal route wins. The important
thing is that the external CLI surface is semantic and stable.

The agent skill should not need to fake clicks or guess pane input fields.

### 3. Deterministic local packaging helpers

Right now the seller tools assume the important identity inputs already exist:

- `content_digest`
- `provenance_ref`
- metadata
- asset family classification

There is no checked-in seller helper yet whose job is:

- take a local file, directory, or conversation export
- normalize the package boundary
- compute a digest
- emit provenance and metadata
- produce a structured listing draft

That helper needs to exist before the CLI+skill path becomes good.

### 4. A no-UI runtime story

If the goal is truly “I should not need to bootstrap the UI,” then the repo
still needs an explicit answer for where the seller logic lives when no window
is open.

There are three possible answers:

- keep requiring the desktop-control runtime, but allow the app to run in a
  no-window or background mode
- add a dedicated `autopilot-headless-data-market` binary
- add a data-market MCP surface and let external agents drive that

For the MVP, the most pragmatic sequence is:

1. make `autopilotctl data-market ...` real
2. let it target the same app-owned runtime first
3. then decide whether the right long-term headless surface is a binary, MCP,
   or both

### 5. A real packaging-and-publication skill

Only after the CLI exists should the repo add the new skill.

Otherwise the skill would have to:

- hit raw HTTP routes,
- depend on unstable pane poking,
- or pretend the packaging step is just prose.

That would be the wrong abstraction boundary.

## What Should Not Be Built

### 1. Do not make the skill a pane-clicking skill

The current `agentskills` format can absolutely tell an agent to run terminal
commands or inspect files.

It can also describe pane control.

But for this use case, the wrong solution is:

- `autopilotctl pane open data_seller`
- synthetic input poking
- synthetic pane actions
- implicit UI-state coupling

That is brittle, hard to version, and not the right public contract.

### 2. Do not bypass app-owned truth by going straight to Nexus from the skill

The raw kernel routes exist, but a direct-to-Nexus publication script would
skip the actual seller-product discipline:

- draft state
- preview posture
- explicit confirmation
- wallet and NIP-90 follow-on posture
- app-owned truth reconciliation

That might be useful for lower-level operators later, but it is not the right
first skill for the product you described.

### 3. Do not make “packaging” just mean “attach a title and a price”

The current seller draft makes it clear that packaging is not only metadata.

It also includes:

- content identity
- provenance
- policy
- delivery mode
- visibility posture
- sensitivity posture

The skill should own that packaging discipline explicitly.

## Recommended Build Order

1. Add `autopilotctl data-market ...` subcommands with JSON output.
2. Back those commands with app-owned typed data-market control, not pane clicks.
3. Add deterministic local packaging helpers that emit listing/grant JSON.
4. Add `skills/autopilot-data-seller-cli/` on top of that CLI.
5. Add a data-market MCP or headless binary if external non-Codex agents need a
   richer always-on control surface.

## Bottom Line

The repo is closer than it might look.

The hard domain work is already done:

- data authority objects exist
- seller draft semantics exist
- typed seller tools exist
- seller-specific Codex policy exists

What is missing is the operator-facing automation layer.

So the right next build is not “make the agent use the UI.”

It is:

- make the seller tool family accessible through `autopilotctl`
- add deterministic packaging helpers
- then add a packaging-and-publication skill that uses those commands

That will give OpenAgents a truthful no-UI seller flow without splitting the
system into a second unofficial control plane.
