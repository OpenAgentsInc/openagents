# Pack C Change Capture And Diff Review Artifacts

Date: 2026-06-12

## Scope

This is the Pack C change-capture record for #4833. It covers the minimal
typed artifact shape needed before PR readiness, writeback, market delivery,
or proof issues can cite changed repo state safely.

## Contract

Change captures carry:

- change ref
- repository ref
- worktree ref
- base/head refs
- file summary refs
- patch digest ref
- verification refs
- diagnostic refs
- review caveat refs
- authority receipt refs when writeback is required
- visibility and public-safety state
- freshness metadata and typed blocker refs

Change capture projections expose summary refs, digest refs, counts, status,
refs, and caveats only. They must not expose raw patches, raw file contents,
raw shell output, private repo data, local filesystem paths, provider payloads,
credentials, wallet/payment material, or raw prompts.

## Blockers

Pack C change captures block when:

- verification refs are missing
- patch digest refs are missing
- writeback-bound captures lack authority receipt refs
- worktree identity is stale or blocked
- public visibility is requested for non-public-safe captures

Blocked captures are still useful evidence, but they are not PR-ready and do
not satisfy writeback, acceptance, merge, payout, settlement, or public-claim
authority.
