# Forge NIP-34 admission and projection receipt

- Date: 2026-07-26
- Issue: [OpenAgentsInc/openagents#9245](https://github.com/OpenAgentsInc/openagents/issues/9245)
- Scope: local service and contract acceptance
- Result: passed

## Authority result

The bare Git repository is the only Git object and ref authority. A repository
is created only after a signature-verified kind 30617 announcement reaches the
private membership boundary for an invited actor. Ordinary Smart HTTP requests
never create a repository. Names are scoped by tenant plus repository and each
admitted owner is bounded by `FORGE_GIT_MAX_REPOSITORIES_PER_OWNER`.

The projector verifies the Nostr signature, normalizes NIP-34 announcement and
coordinate references, and checks the required Git objects before it produces
state. A kind 30618 records exact old, new, and ref facts. `receive-pack` admits
only the matching current fact. A successful bare-Git receive marks its state
applied and creates a durable relay-outbox row; relay publication is therefore
derived and retryable, rather than a second ref authority.

## Negative acceptance

- An unadmitted Smart HTTP repository returned the typed
  `forge_git_repository_not_admitted` refusal and no bare repository was
  provisioned.
- A head move with no matching maintainer-signed 30618 fact was rejected by the
  receive hook as `forge_git_signed_state_required`.
- An event naming an unavailable object returned `purgatory`; it created no
  state projection. Purgatory expiry is 30 minutes, or 20 minutes for pointer
  and pull-request refs. Reconciliation projects first and marks resolved only
  after projection succeeds.
- Unclaimed `refs/nostr/<event-id>` records receive a 20-minute GC deadline;
  a claimed event cancels that deletion path.

## Commands

```text
pnpm --dir apps/forge-git-service typecheck
pnpm --dir apps/forge-git-service test
pnpm --dir apps/openagents.com/workers/api exec vitest run \
  src/forge-invite-membership-routes.test.ts src/forge-invite-policy.test.ts \
  src/forge-domain-repository.contract.test.ts
pnpm exec vp test --run packages/khala-sync-server/src/migrate.test.ts
git diff --check
```

All listed commands passed on the issue worktree before this receipt was
committed. This is not a public `/git` cutover receipt; transport infrastructure
and load-balancer publication remain FORGE-02 scope.
