# Autopilot Task Delegation Packets

This directory stores tracked task packets that can be committed, pushed, and
sent to Autopilot after the programmatic Autopilot operating system is ready.

These files are not ordinary planning notes. Treat each task packet as launch
input for a future durable Autopilot goal/run.

## Background

Read this operator runbook packet before creating, editing, committing, or
dispatching task packets from this directory:

- `2026-06-04-programmatic-autopilot-operator-runbook.md`

That runbook establishes the operating model:

- the foreground coding agent acts as an Autopilot operator;
- Autopilot owns product implementation work;
- the foreground agent should repair only Autopilot infrastructure defects that
  block launch, execution, reporting, continuation, or visibility;
- implementation specs must live in tracked repo files, then be committed and
  pushed before dispatch;
- future launches should use preflight, provider reconnect gating, canonical
  callback contracts, callback retry/backfill, run continuation, and public/team
  observer surfaces rather than one-off manual shell recipes.

The older historical audit remains available at:

- `../2026-06-04-programmatic-autopilot-work-runbook-audit.md`

Use the audit for context. Use the runbook packet in this directory for current
launch mechanics.

## Dispatch Gate

Do not dispatch a packet from this directory until the runbook recommendations
needed for reliable delegation are complete enough for the requested work:

- `POST /api/operator/autopilot/preflight` or equivalent readiness check;
- provider reconnect gating before dispatch;
- canonical SHC callback contract and tests;
- safe callback retry/backfill by run ID;
- durable goal/run continuation relationship;
- public/team goal observer page that does not leak private delivery mechanics;
- operator checklist command or equivalent status summary.

Current implementation status as of the 2026-06-04 foreground session:

- preflight/checklist, callback retry, continuation, provider reconnect gating,
  and callback contract tests are implemented, committed, pushed, deployed, and
  passing focused API tests/typecheck/architecture guard plus full deploy
  checks;
- production checklist smoke passed for the ImageGen target run
  `11a4ff12-601b-48f3-b596-34f947bfc4bb`: database, team/project/agent
  metadata, provider account, GitHub writeback, SHC control, callback
  configuration, and callback lag were all `ok`;
- existing team/public observer primitives are usable, but a polished single
  "goal observer" browser page remains a product follow-up unless a task packet
  explicitly requires it.

If those are not ready, update the packet or record the blocker. Do not pretend
Autopilot is ready by manually performing the product implementation.

## Packet Structure

Each task packet should include these sections:

- Title: `Autopilot Task: <short task name>`.
- Status: queued, ready for dispatch, dispatched, blocked, complete, or
  superseded.
- Target repo and branch.
- Primary agent, team, project, and public/private visibility.
- Public route or observer link, if relevant.
- Dispatch gate and required preflight conditions.
- Objective in product terms, not operator mechanics.
- Current starting point, including already-completed work.
- Relevant repo files and historical reference files.
- Production/public links that are safe to show.
- Commit input for dispatch, including suggested commit message and launch
  payload fields.
- Autopilot work plan with bounded implementation slices.
- Safety rules for public projection and secrets.
- Acceptance criteria with tests, artifacts, commit/PR, and deployment notes.
- Suggested public run summary when the work is meant to be observer-visible.

Keep packets specific enough that an Autopilot runner can execute from the file
without hidden chat context.

## Commit And Push Contract

Before launching a task:

1. Ensure the packet is tracked in this directory.
2. Commit and push the packet and any prerequisite implementation/spec changes.
3. Reference the commit SHA and `taskSpecPath` in the Autopilot launch input.
4. Do not include secrets, provider tokens, callback tokens, OAuth material,
   local secret paths, private prompts, or raw runner payloads in the packet or
   launch payload.

The packet should be safe for the remote runner to read from the repository.

## Current Packets

- `adjutant-site-task-template.md`: template for generated Adjutant Site
  generation, adjustment, review, and deployment packets.
- `2026-06-04-programmatic-autopilot-operator-runbook.md`: canonical operator
  runbook packet for programmatic preflight, callback retry, continuation, and
  foreground-agent supervision.
- `2026-06-04-r10-pylon-campaign-continuation.md`: public Artanis/Pylon
  campaign continuation packet for releasing the next Pylon version, wiring it
  deeper into OpenAgents product surface, routing inference and fine-tuning work to Pylons, and using
  Bitcoin-backed accounting.
- `2026-06-04-cloudflare-containers-runner-backup-implementation.md`: runner
  backup implementation packet for adding Cloudflare Containers as a backup and
  burst lane.
## Done Packets

Completed packets are moved to `done/` rather than left in the active packet
list:

- `done/2026-06-04-customer-software-ordering-flywheel.md`
- `done/2026-06-04-effect-driven-chat-demo-page.md`
- `done/2026-06-04-gemini-image-generation-implementation.md`
- `done/2026-06-04-stripe-effect-service-implementation.md`
- `done/2026-06-04-thread-ownership-sidebar-separation.md`
