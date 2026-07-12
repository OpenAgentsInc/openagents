# CUT-16A — Effect Native mobile execution-target selector receipt

- Issue: [#8717](https://github.com/OpenAgentsInc/openagents/issues/8717)
- Parent: [#8696](https://github.com/OpenAgentsInc/openagents/issues/8696)
- Date: 2026-07-12

## Landed contract

The active OpenAgents mobile app now reads the existing authenticated
`GET /api/mobile/model-preference` authority through a strict Effect Schema
boundary. The bearer remains inside the native host; returned state contains
only public-safe target labels, opaque account refs, readiness/reason facts,
and exact runtime targets. Malformed/excess response fields, unknown target
kinds, failed requests, and unresolved Auto choices fail closed.

The server-advertised concrete choices lower as follows:

- Khala/Gemini → `hosted_khala`, exact hosted target, model
  `gemini-3.5-flash`;
- Codex account → `codex_app_server`, `gpt-5.6-sol`, exact
  `codex:<accountRefHash>`;
- Claude account → `claude_pylon`, `claude-fable-5`, exact
  `claude:<accountRefHash>`.

The Effect Native composer renders accessible selected/disabled controls with
the server's public-safe label and readiness. Selection is persisted in the
canonical device-local draft as exact lane/provider/model/account/execution
refs. Reopen/restart retains a still-advertised exact target; removal or
revocation marks that same target unavailable without changing text or
attachments. There is no automatic substitution.

For a new turn, the exact persisted `lane` and `executionTargetId` enter
`runtime.startTurn`. Steering an already-running turn ignores the idle
selector and stays pinned to the confirmed active lane.

## Verification

- focused selector/composer/conversation/Home/accessibility gate:
  39 pass, 0 fail, 199 assertions;
- full `apps/openagents-mobile` suite: 120 pass, 0 fail, 631 assertions;
- mobile TypeScript: pass;
- catalog tests prove request-only bearer use and redacted errors/results;
- Home view tests prove the controls remain Effect Native nodes with bounded
  accessibility labels and exact target delivery.

No server change was required: the production route and account-readiness
projection were already live. This sub-issue deliberately owns no physical
acceptance gate; parent CUT-16 retains the physical/assistive close rule.
