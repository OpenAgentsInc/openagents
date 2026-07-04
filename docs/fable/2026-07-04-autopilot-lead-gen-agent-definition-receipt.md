# Autopilot Lead Gen agent definition receipt

Status: LG-7 source receipt for issue
[#8268](https://github.com/OpenAgentsInc/openagents/issues/8268), recorded
2026-07-04. This is a source-level/dogfood receipt, not a public availability
claim.

## What landed

- Standing definition:
  `agent_definition.autopilot.lead_gen.v1`
  (`apps/openagents.com/workers/api/src/autopilot-lead-gen-agent-definition.ts`).
- Product-promise record: `autopilot.lead_gen.v1`, state `planned`.
- Behavior contracts:
  `lead_gen_agent.drafting_only_toolset.v1` and
  `lead_gen_agent.no_send_without_approval_receipt.v1`.
- Dogfood customer #1 config:
  `lead_gen_config.openagents.customer_001.v1`
  (`customer.openagents.dogfood`).
- Recorded dogfood receipt:
  `receipt.autopilot_lead_gen.openagents.dogfood.20260704`.

## Definition boundary

The v0 definition runs on the existing background-agent substrate:

- `lane=own_pylon`
- weekday cron trigger: `trigger.autopilot.lead_gen.weekday_discovery`
- manual run trigger: `trigger.autopilot.lead_gen.manual`
- BA-B4 budget caps: `maxRunSeconds=1800`, `maxRunsPerDay=1`,
  `maxCreditsPerDay=0`
- BA-B5 run history route:
  `/v1/agent-definitions/agent_definition.autopilot.lead_gen.v1/runs`

Per-customer state is config, not a fork of the agent definition. The run
payload carries the customer ICP, analyzer config, target-discovery config,
template family, caps, source attribution ref, operator inbox, and approval
gate.

## Toolset boundary

Allowed:

- customer config read
- target-discovery config read
- LG-1 agent-readiness batch run
- LG-5 report draft
- LG-4 sequence-entry draft
- business-pipeline draft entry
- operator-inbox escalation
- receipt write
- Forge `git:receive-pack` only for the existing own-Pylon dispatch path

Denied:

- `tool.openagents.email.send`
- `tool.openagents.email_sequence.send`
- `tool.openagents.email_sequence.activate`
- `tool.openagents.apollo.sequence.send`
- `tool.openagents.apollo.emailer_campaigns_approve`
- `tool.openagents.apollo.emailer_campaigns_add_contact_ids`
- `tool.openagents.apollo.emailer_messages.*`
- Forge git admin

The receipt and run payload both record `sendAuthority.allowed=false`.
Outreach can leave the system only after a separate LG-4 approval receipt.

## Dogfood receipt

OpenAgents is customer #1 for the v0 definition:

- `targetDiscoveryConfigRef`:
  `target_discovery.openagents.agent_ready_businesses.v1`
- `analyzerConfigRef`: `analyzer.agent_readiness.ora_style_default.v1`
- `templateFamilyRef`: `template_family.lead_gen.report_led_sequence.v1`
- `sourceRef`: `apollo_agent_readiness_openagents`
- `operatorInboxRef`: `operator_inbox.autopilot.lead_gen.openagents.v1`
- `approvalGateRef`: `approval_gate.lead_gen.openagents.lg4.sequence_send.v1`

The Ora idea adapted here is the useful agent-readiness stance: public URL
scoring, cached/structured scan inputs, and an agent-readable rubric. This repo
does not claim an ora.ai integration or an Ora score; the analyzer config ref is
an OpenAgents-owned, public-safe readiness-rubric pointer.

## Remaining gates

`autopilot.lead_gen.v1` stays planned until these exist:

- a live customer run receipt,
- a separate LG-4 approval receipt for any send,
- customer-result receipts,
- owner-signed receipt-first green upgrade.

This source receipt grants no Apollo credential, contact reveal, email send,
sequence activation, spend, payout, settlement, or customer-delivery authority.
