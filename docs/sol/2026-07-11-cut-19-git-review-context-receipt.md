# CUT-19 Git review and composer-context closure receipt

- Date: 2026-07-11
- Issue: [#8699](https://github.com/OpenAgentsInc/openagents/issues/8699)
- Status: complete on `main`
- Closing commit: `6ff137e5c0`
- Parents: [#8566](https://github.com/OpenAgentsInc/openagents/issues/8566), [#8574](https://github.com/OpenAgentsInc/openagents/issues/8574)

## Outcome

Desktop can inspect staged and unstaged repository state, open bounded typed
file/hunk diffs, explicitly discard one safe tracked worktree change, and attach
one reviewed diff to the next composer turn without giving the renderer Git
argv, an absolute root, or ambient repository authority.

The status result carries opaque repository, HEAD, and exact status snapshot
refs. Every diff and discard request echoes the repository/status refs. A
concurrent HEAD, index, or tracked-worktree change invalidates the request and
forces visible refresh before mutation.

## Safety contract

- Diff uses fixed `git diff --no-ext-diff --no-textconv` argument vectors and
  relative paths after `--`.
- Binary, secret-shaped, unavailable, and larger-than-120-KB output is rejected
  before renderer projection or provider context.
- Unified output is parsed into bounded typed hunks and rendered with the shared
  Effect Native `DiffView`.
- Discard is a two-step inline confirmation and applies only to an unstaged,
  tracked, non-conflicted path with no staged change for that path.
- The only discard mutation is fixed `git restore --worktree -- <path>`; there
  is no reset, checkout, untracked deletion, arbitrary argv, or automatic
  commit/push/PR publication.
- Composer context is explicit, visible, removable, and next-turn-only. Provider
  delivery labels diff contents as untrusted data rather than instructions.
- Receipts and public-safe action errors contain no root, diff, credential, or
  raw stderr.

The pre-existing #8712 commit/push/branch/issue/PR surface remains intact. It is
not new CUT-19 automation and remains outside this issue's close claim.

## Verification

Post-rebase validation on `6ff137e5c0`:

- `bun run --cwd apps/openagents-desktop typecheck` — pass
- contract, Git panel, and shell typed suites — 105 pass, 0 fail, 561
  expectations
- Node 25 real temporary-repository corpus — 3 pass, covering:
  - dirty diff and typed hunk projection;
  - exact stale-snapshot discard refusal and confirmed discard;
  - binary, secret-shaped, and oversized diff refusal;
  - rename, submodule, detached HEAD, real merge conflict, and no-repository
    states
- full built Electron smoke — pass with a private temporary Git repository,
  real diff render, discard confirmation/cancel, composer attach/remove, every
  current EP250/#8712 journey, and lifecycle teardown `active: 0`

Bun 1.3.11 currently drops successful child-process stdout in the older
`git-github-host.test.ts` process context. The real repository matrix therefore
runs under Node's TypeScript stripping instead of skipping or weakening the
assertions; the production Electron/Node host is independently green in the
built smoke.

## Remaining ownership

CUT-20 owns workspace-bounded PTY terminals and local preview lifecycle. CUT-21
and later leaves own provider/runtime breadth and settings. Portable repository
materialization and cross-host session movement remain separate remote-first
work; CUT-19 context is host-local and is not uploaded to Khala Sync by default.
