# Episode 250 Claude readiness truth receipt

Date: 2026-07-12  
Issue: #8712

## Outcome

The Episode 250 live proof exposed a false-green edge: the selected named
Claude account had a local credential directory, so Fleet projected it as
ready even after the real provider refused the turn because the organization
had disabled Claude subscription access for Claude Code. The failed probe also
left an older zero-token local-session observation visible as `available`.

Pylon now persists only that bounded provider-disabled health state for the
opaque account ref. While it is present, account readiness exposes no Claude
execution capability and local-session usage is projected as missing. A
successful bounded provider probe clears the health record. Failed SDK results
do not record successful local-session usage, and raw provider text is never
placed in the public account projection.

## Verification

- `apps/pylon`: 29 focused tests pass with 106 assertions.
- `apps/pylon`: TypeScript typecheck passes.
- `packages/pylon-core`: TypeScript typecheck passes; existing Effect advisory
  diagnostics remain non-failing.
- Real `accounts usage --account claude-pylon-2 --refresh --json` reports:
  - readiness `provider_disabled`;
  - no capability refs;
  - local-session state `missing` with null usage;
  - only `blocker.pylon.claude_account.provider_disabled` for the refusal.
- Real `accounts list --json` reports the same named account as
  `provider_disabled`.

This corrects Fleet/readiness truth but does not close #8712: a successful real
Codex turn and Claude turn still require provider capacity/access.
