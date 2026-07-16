# Khala UI Astro pilot superseded disposition

- Class: disposition
- Date: 2026-07-15
- Status: superseded without implementation
- Dispatch: no; use [#8848](https://github.com/OpenAgentsInc/openagents/issues/8848)
- Parent: [#8844](https://github.com/OpenAgentsInc/openagents/issues/8844)
- Base: `c8844a7bab39c57d360509fbfd67fb9165e76aa2`

## Decision

#8848 cannot be implemented honestly against current `main`. It targeted a
static CSS/SVG projection in `apps/openagents.com/apps/astro`, required that
Astro app to remain authoritative, and explicitly excluded Start/TanStack
landing expansion. The named app and premise no longer exist.

Commit `abf4eaa311fd80c31fe966937e606cbae3fc977c` deleted the Astro app and
moved `/astro`, `/install`, and the public landing implementation into
`apps/openagents.com/apps/start`. Current `apps/openagents.com/INVARIANTS.md`
makes Start authoritative for `/astro` and defines `/tanstack` only as a
compatibility redirect to it.

Recreating `apps/astro` would violate current route authority. Moving #8848's
work into Start would violate its explicit non-goal, replace its static-output
budget with an unreviewed React/SSR budget, and silently broaden the issue.
The correct disposition is to close #8848 as superseded, not to report its
obsolete acceptance criteria as complete.

## Evidence

- #8848 was created at `2026-07-15T20:11:35Z`.
- `a7514ef6b9` split the public Astro and authenticated apps minutes later.
- `abf4eaa311`, authored at `2026-07-15T16:27:21-05:00`, then unified public
  routes in Start and removed every tracked file under `apps/astro`.
- `apps/start/src/routes/astro.tsx` now owns `/astro`.
- `apps/start/src/routes/tanstack.tsx` redirects `/tanstack` to `/astro`.
- Repository search finds no current `apps/openagents.com/apps/astro` path.

## Preserved boundaries

- No retired app, runtime, dependency, route, CSS, or asset was recreated.
- No Start landing code or public copy was changed.
- No root cutover or deployment was performed.
- The accepted Desktop and Forum Khala pilots remain unchanged.
- Motion and Canvas stay separately gated by #8849 and #8850.

## Verification

- current public-site, Cloud Run app UI, and client/server route-agreement
  suites: 3 files and 33 passing tests;
- Sol manifest: 123 classified documents;
- Sol policy, links, and `git diff --check`: pass; and
- repository history and current-path search confirm that no tracked
  `apps/openagents.com/apps/astro` implementation remains.

## Future activation

If the owner later wants Khala UI on the authoritative `/astro` page, open a
new Start-owned issue. It must audit the then-current interactive Desktop demo,
SSR/CSR and no-JS contract, shared Desktop workbench dependency, route output,
public copy, focus/zoom/forced-colors behavior, and `/tanstack` redirect. It
must not inherit #8848's static-Astro measurements or claim this superseded
issue as implementation proof.
