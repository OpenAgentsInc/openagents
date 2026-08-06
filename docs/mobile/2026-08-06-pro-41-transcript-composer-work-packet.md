# Pro #41 transcript and composer work packet

- Status: complete; implementation, full repository verification, and Pro production canary passed
- Source: current owner direction to complete every open Pro issue; Pro issue #41
- Outcome: publish one cross-runtime semantic transcript projection and use it in Pro web plus the native mobile controller
- Repository: `OpenAgentsInc/openagents`
- Base: `6563cd93a8`
- Verification: generated-contract checks, focused contract/mobile tests, mobile typecheck, repository `pnpm run check`, then Pro production canary

## Repository work claim

```text
CLAIM
actor/session: codex-pro-41-transcript-composer
base: 6563cd93a8
worktree/branch: worktrees/openagents/pro-41-transcript-composer (detached)
scope: canonical semantic transcript rows, five acknowledgement dimensions, typed composer context and draft DTOs, native semantic feed/actions, durable SQLite drafts, bounds/minimap, and cross-runtime fixtures for Pro web/mobile plus future MCP and Omega Rust consumers
paths: packages/all-work-contract/definition/all-work-v1.contract.json; packages/all-work-contract/generated/**; packages/all-work-contract/fixtures/**; packages/all-work-contract/src/generated.ts; packages/all-work-contract/package.json; packages/all-work-contract/test/**; apps/openagents-mobile/src/controller/**; apps/openagents-mobile/src/outbox/**; apps/openagents-mobile/tests/controller-*.test.ts; apps/openagents-mobile/package.json; pnpm-lock.yaml; docs/mobile/2026-08-06-pro-41-transcript-composer-work-packet.md; generated assure-repo inventories when required
hot files: packages/all-work-contract/definition/all-work-v1.contract.json; packages/all-work-contract/src/generated.ts; packages/all-work-contract/package.json; apps/openagents-mobile/package.json; pnpm-lock.yaml
hot contracts: openagents.all_work_boundary.v1; openagents.work_transcript.v1; openagents.mobile_controller.v1; openagents.client_command_outbox.v1
verification: package generation/checks + focused contract/mobile tests + mobile typecheck + pnpm run check + Pro production canary
claimed_at: 2026-08-06T20:00:00Z
```

## Authority and constraints

The all-work boundary owns the portable DTOs. Pro normalizes provider/runtime
records into those rows before projection; web and mobile consume the generated
contract now, and future MCP/Omega adapters can do so without inferring
semantics from raw provider messages.
Admission, effect, turn, quiescence, and verification remain five independent
acknowledgements. A queued or pending acknowledgement is never rendered as a
completed result.

Typed file, terminal, review, and skill contexts contain public references and
bounded display metadata only. Draft persistence never contains credentials.
Approval and input decisions bind an exact request revision and expiry.
Loaded transcript windows and navigation markers remain bounded on every
surface.

### Claim amendment — atomic native drafts

The shipped mobile composer required the existing
`apps/openagents-mobile/tests/client-outbox.test.ts` to prove that a message
command and deletion of its matching draft share one exclusive SQLite
transaction. That test entered the claimed path set with the already-claimed
`apps/openagents-mobile/src/outbox/**` adapter. No package-level outbox contract
or delivery policy changed.

## Implemented result

- The all-work definition generates `WorkTranscriptPage` and
  `WorkComposerDraft` into strict Effect/TypeScript, JSON Schema, fixtures, and
  Rust validators. The Node-free `./projection` export is the client seam.
- Mobile renders that generated semantic union exhaustively, retains historical
  request rows, uses exact projected revisions/expiries for decisions, exposes
  completed-plan follow-ups, and keeps a 12-marker bounded outline.
- Native file/terminal/review/skill context and composer drafts persist in
  SQLite. Message admission atomically clears the matching draft.
- Contract, generated-file, mobile typecheck, and 90 focused mobile tests pass.
  The repository-wide gate also passes: 2,387 Vite Plus files (20,166 tests),
  23 Node suites (177 passing, one intentional skip), Rust conformance, policy
  guards, and drift inventories.
- Pro production deployment `dpl_Hyqo2ycABKy4zv8Ldc1XkZDftDYk` and authenticated
  canary `mobile-controller-canary-12d1ad53-20b1-47ad-b163-45034da3637f`
  passed. The client observed live generations 1–5, one-time replay, the
  semantic user/request rows, a stale-decision refusal without state change,
  and the accepted exact-revision response.
