# Mobile Sarah command center — accepted plan

- Class: accepted owner implementation and release plan
- Status: complete
- Date: 2026-07-29
- Owner authority: current owner conversation
- Base commit: `cc52bfb7ccf8f3c855fc6c3f55195bb4da896552`

## Decision

Ship Sarah's admitted command-center capability in the supported OpenAgents
mobile app and upload the resulting native build to TestFlight. Mobile voice
uses the canonical owner-private Sarah thread and may inspect or dispatch
owner coding capacity and inspect or queue pause, resume, and stop intents for
an existing Desktop Full Auto run through the existing typed Sarah brokers.

The phone does not receive workspace, editor, shell, Git, credential, payment,
or device authority. The paired Omega bridge remains read-only. Repository
work is delegated to receipted coding workers and Full Auto remains
Desktop-authoritative; a queued intent is never reported as applied.

## Active implementation claim

```text
CLAIM
actor/session: principal.sol.mobile-sarah-command-center-2026-07-29
base: cc52bfb7ccf8f3c855fc6c3f55195bb4da896552
worktree/branch: openagents-worktrees/mobile-sarah-parity / codex/mobile-sarah-parity
scope: Add canonical-thread mobile Sarah voice delegation, tool activity, heartbeat/reconnect hardening, focused oracles, service deployment, and TestFlight build 128.
paths: packages/audio-contract/**; packages/khala-sync-server/**; apps/openagents.com/workers/api/**; apps/openagents-mobile/**; packages/behavior-contracts/**; docs/mobile/**
hot contracts: Sarah voice profile; owner thread authority; Sarah runtime tools; realtime sequencing; mobile release identity
verification: focused contract/server/mobile tests and typechecks; root pnpm run check; staging migration/deploy and live command receipt; signed simulator/physical-device voice smoke; App Store Connect VALID and IN_BETA_TESTING
claimed_at: 2026-07-29T21:00:00Z
```
