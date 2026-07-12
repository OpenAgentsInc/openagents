# Sol checked-in issue sources

- Class: index
- Status: current classification index
- Dispatch: no; use live GitHub issues and claims
- Owner: Sol roadmap
- Inventory: every non-README Markdown file in this directory is classified
  exactly once below

Checked-in sources preserve acceptance language, closure evidence, and
non-revival boundaries. They are not a second live issue database. Current work
comes from the [`master roadmap`](../MASTER_ROADMAP.md),
[open `roadmap:sol` issues](https://github.com/OpenAgentsInc/openagents/issues?q=is%3Aissue%20state%3Aopen%20label%3Aroadmap%3Asol),
and [`CLAIM_PROTOCOL.md`](../CLAIM_PROTOCOL.md).

## Live issue sources

These files correspond to open issues at this index snapshot. Their bodies may
contain historical status; refresh the live issue/comments before dispatch.

| Source | Live issue | Durable role |
| --- | --- | --- |
| [`app-program.md`](./app-program.md) | #8566 | Sole R0–R7 program parent |
| [`app-desktop.md`](./app-desktop.md) | #8574 | Desktop D0–D6/R5 track |
| [`app-mobile.md`](./app-mobile.md) | #8597 | Mobile R0–R7/R6 track |
| [`fc-cloud-codex.md`](./fc-cloud-codex.md) | #8547 | First accepted brokered Agent Computer/workroom |
| [`fc-4-hybrid-cloud.md`](./fc-4-hybrid-cloud.md) | #8636 | Local/managed routing through one claim registry |
| [`native-streamed-conversation-handoff.md`](./native-streamed-conversation-handoff.md) | #8676 | Real Desktop-to-physical-mobile continuation |
| [`conversation-fault-convergence.md`](./conversation-fault-convergence.md) | #8677 | Command/event/lifecycle fault proof |

## Closed proof and implementation sources

These are immutable checked-in issue-body/acceptance sources for closed work.
They are evidence, not ready leaves.

- [`confirmed-agent-timeline.md`](./confirmed-agent-timeline.md) — #8672
- [`desktop-codex-subagent-history.md`](./desktop-codex-subagent-history.md) — #8674
- [`desktop-codex-trace-acceptance.md`](./desktop-codex-trace-acceptance.md) — #8675
- [`desktop-effect-scope-topology.md`](./desktop-effect-scope-topology.md) — #8678
- [`desktop-runtime-agent-timeline.md`](./desktop-runtime-agent-timeline.md) — #8673
- [`desktop-runtime-conversation.md`](./desktop-runtime-conversation.md) — #8669
- [`desktop-runtime-gateway.md`](./desktop-runtime-gateway.md) — #8655
- [`desktop-session-controls.md`](./desktop-session-controls.md) — #8665
- [`desktop-session-loopback-policy.md`](./desktop-session-loopback-policy.md) — #8663
- [`desktop-session-pkce.md`](./desktop-session-pkce.md) — #8664
- [`desktop-session-recovery.md`](./desktop-session-recovery.md) — #8662
- [`desktop-session-vault.md`](./desktop-session-vault.md) — #8661
- [`desktop-sync-host.md`](./desktop-sync-host.md) — #8656
- [`desktop-visible-sync-conversation.md`](./desktop-visible-sync-conversation.md) — #8670
- [`fc-1-run-contract.md`](./fc-1-run-contract.md) — #8637
- [`fc-2-local-executor.md`](./fc-2-local-executor.md) — #8633
- [`fc-3-supervision.md`](./fc-3-supervision.md) — #8639
- [`fc-5-dogfood.md`](./fc-5-dogfood.md) — #8640
- [`fc-khala-inference.md`](./fc-khala-inference.md) — #8600
- [`fc-substrate.md`](./fc-substrate.md) — #8638
- [`local-first-identity.md`](./local-first-identity.md) — #8666
- [`mobile-session-pkce.md`](./mobile-session-pkce.md) — #8660
- [`mobile-session-recovery.md`](./mobile-session-recovery.md) — #8659
- [`mobile-session-vault.md`](./mobile-session-vault.md) — #8658
- [`mobile-sync-host.md`](./mobile-sync-host.md) — #8657
- [`mobile-visible-sync-conversation.md`](./mobile-visible-sync-conversation.md) — #8671
- [`native-authenticated-sync-hosts.md`](./native-authenticated-sync-hosts.md) — #8667
- [`native-conversation-continuation.md`](./native-conversation-continuation.md) — #8668

## Closed non-revival tombstones

These closed/not-planned sources preserve negative product boundaries. They are
not dormant epics. A real defect or newly authorized outcome gets a new issue.

- [`app-forum.md`](./app-forum.md) — #8635
- [`app-landing.md`](./app-landing.md) — #8595
- [`app-web-consolidation.md`](./app-web-consolidation.md) — #8634
- [`blueprint-correction.md`](./blueprint-correction.md) — #8642
- [`glass-ui-and-sarah-mobile.md`](./glass-ui-and-sarah-mobile.md) — #8646
- [`role-programs-and-colleagues.md`](./role-programs-and-colleagues.md) — #8643
- [`sarah-presentation.md`](./sarah-presentation.md) — #8610

## Architecture reference

- [`effect-native-electron-host.md`](./effect-native-electron-host.md) —
  reusable Electron host gap/reference associated with #8574; not a separate
  current issue source.

## Reconciliation rule

- Keep live state and claims in GitHub and one master projection.
- Keep durable acceptance and non-revival boundaries in checked-in sources.
- Keep proof in the [`receipt index`](../receipts/README.md) and immutable
  receipts with obvious final dispositions.
- Do not update every historical body to mirror a state transition.
- If a still-open acceptance contract materially disagrees with its live issue,
  reconcile before dispatch.

The ordered cleanup and retirement plan is
[`../2026-07-12-documentation-cleanup-audit-and-retirement-plan.md`](../2026-07-12-documentation-cleanup-audit-and-retirement-plan.md).
