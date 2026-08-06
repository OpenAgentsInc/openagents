# Pro Convex mobile controller work packet

- Status: accepted and claimed
- Source: current owner direction to complete every open Pro issue; Pro issue #39
- Outcome: add the durable mobile command outbox required by Pro's Convex controller
- Repository: `OpenAgentsInc/openagents`
- Base: `48db404cbb55f44ce51bf2cce4fecc056a2f8c5d`
- Verification: package and mobile focused tests, mobile typecheck, then repository `pnpm run check`

## Repository work claim

```text
CLAIM
actor/session: codex-pro-39-mobile-outbox
base: 48db404cbb55f44ce51bf2cce4fecc056a2f8c5d
worktree/branch: worktrees/openagents/pro-39-mobile-outbox (detached)
scope: shared command-class/outbox kernel plus Expo SQLite adapter, quarantine, shell cache, and restart/reconnect exactly-once proof
paths: packages/client-command-outbox/**; apps/openagents-mobile/src/outbox/**; apps/openagents-mobile/tests/client-outbox.test.ts; package manifests/lockfile only if required; this work packet
hot files: pnpm-lock.yaml and package manifests only if required
hot contracts: openagents.client_command_outbox.v1; package workspace keys only if required
verification: focused tests + mobile typecheck + pnpm run check
claimed_at: 2026-08-06T11:36:43Z
```

### Claim amendment — production mounting

At `2026-08-06T11:56:45Z`, the expected path set expanded to include
`apps/openagents-mobile/src/app.tsx`. The repository's uncalled-production-
symbol gate correctly refused a test-only adapter. The app root now mounts the
outbox provider so SQLite initialization is part of the shipped lifecycle;
future controller screens receive that runtime instead of constructing a
second store. The generated `docs/assure-repo/surface-inventory.v1.json` and
`docs/assure-repo/false-green-candidates.v1.json` also entered the claim after
the repository guard required the new package and test surfaces to be
recorded. No other application path entered the claim.

## Authority and constraints

This packet implements the accepted Pro control-plane program on the supported greenfield mobile app. It does not restore a retired Khala client or OTA lane. The outbox stores normalized command bytes and public refs only. Credentials, tokens, prompts outside the exact command payload, provider payloads, native paths, and private traces are structurally excluded.

Delivery remains subordinate to target authority. A queued row, socket acknowledgement, or local cache entry is not a target outcome. Only the target's durable receipt settles a command. Live-control and destructive operations never auto-replay, and expired decisions fail closed before delivery.
