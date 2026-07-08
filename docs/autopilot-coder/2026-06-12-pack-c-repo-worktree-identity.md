# Pack C Repository And Worktree Identity

**STATUS: HISTORICAL — point-in-time record (accurate as of its
date). Not current direction; consult MASTER_ROADMAP.**


Date: 2026-06-12

## Scope

This is the Pack C repository/worktree identity record for #4832. It covers
the minimal typed identity needed before future delivery, writeback, and
market evidence can cite a repository or worktree safely.

## Contract

Repository identity snapshots carry:

- repository ref
- host
- owner/name
- visibility
- trust tier
- default branch
- pinned commit ref
- remote digest ref
- data-scope refs
- caveat refs

Worktree identity snapshots carry:

- workspace ref
- worktree ref
- branch ref
- base/head commit refs
- cleanliness
- sandbox profile ref
- retention policy ref

Snapshots carry `generatedAt`, `observedAt`, `staleAt`, `ageMs`,
`freshness`, `status`, and typed blocker refs. Stale snapshots are evidence,
not fresh authority.

## Boundaries

Repository/worktree identity is evidence only. It does not grant writeback,
merge, acceptance, payout, settlement, provider mutation, or public-claim
authority.

Public or agent-readable identity projections must reject private remotes,
private repo content, raw prompts, raw shell material, local filesystem paths,
credentials, provider payloads, wallet/payment material, and customer-private
data before projection.

Branch refs must be parseable safe Git refs. They must not contain shell
fragments, path traversal, lockfile suffixes, or ambiguous Git ref syntax.
