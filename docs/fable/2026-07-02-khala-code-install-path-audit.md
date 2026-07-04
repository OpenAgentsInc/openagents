# Khala Code Install Path Audit (INSTALL.md, README, live AGENTS.md)

**Date:** 2026-07-02
**Scope:** how a user — or a user's coding agent — installs OpenAgents
software today, with Khala Code as the product we now lead with; the served
agent docs at `openagents.com/AGENTS.md` and `openagents.com/INSTALL.md`; and
the repo-root install story on GitHub.
**Status:** audit + implementation in the same change (owner-directed). The
fixes land alongside this doc: a canonical root `INSTALL.md`, a prominent
README link, a Khala-Code-first rewrite of the served install guide, and an
AGENTS.md slim-down that moves two heavy reference blocks into new companion
files.

**2026-07-04 RL-2 update:** the public browser route
`https://openagents.com/code/download` now exposes Khala Code install truth
under the `khala_code.desktop_codex_wrapper.v1` copy gate: Codex CLI + login
required, public `npm install -g @openagentsinc/khala` CLI path available,
source build supported, and macOS DMG still marked as a pending public
artifact. `GET /api/public/khala-code/download-counts` is the only public
download counter for this page; it reads exact `khala_code_download_events`
rows or returns `counts: []` with blocker refs instead of synthesizing totals.

---

## 1. The triggering failure

Owner observation (2026-07-02, screen-recorded): telling a coding agent
"install https://github.com/OpenAgentsInc/openagents (Khala Code product)
from source" makes the agent run a **full `git clone`** of the monorepo. The
`.git` history alone is ~460 MB, and nothing about the repo tells the agent a
cheaper path exists. Historically we also directed people at
`https://openagents.com/AGENTS.md`, which is agent-*onboarding* documentation
(register, forum, earn) — its install guidance is Pylon-first and never
mentions Khala Code at all.

There is **no INSTALL.md or BUILD.md at the repo root**, and no file anywhere
that says "here is the quickest supported way to install each thing we ship."

## 2. Current install-story inventory

### 2.1 What exists

- **`apps/openagents.com/docs/live/INSTALL.md`** → served at
  `https://openagents.com/INSTALL.md` (static asset, synced to
  `apps/web/public/` by `scripts/sync-live-agent-doc.mjs`). Content: "Install
  & test OpenAgents Pylon (v1.0)" — Pylon headless node + Autopilot Desktop
  DMG. Written for the RC-tester era; **no Khala Code, no khala CLI, no
  qa-runner.**
- **`docs/live/AGENTS.md`** (1,640 lines, served with a pinned SHA-256 and
  content-substring tests in
  `workers/api/src/openagents-agent-onboarding-routes.test.ts`): the install
  callout in its header blockquote says "The agent path is Pylon" and defers
  to INSTALL.md. Khala Code — per the repo README, "the core product… its
  front door" — appears nowhere in the served agent docs.
- **Repo root `README.md`**: describes Khala Code and gives the Codex
  prerequisite (`npm install -g @openai/codex`, `codex login`) plus a
  workspace `bun install` under "Working In This Repo", but never the actual
  run command (`bun run dev:khala-code-desktop`) and never clone guidance.
- **`clients/khala-code-desktop/README.md`**: has a correct "Install And
  First Run" section — but you only find it after you've already cloned.

### 2.2 Verified install facts (2026-07-02)

- **Khala Code desktop** (`clients/khala-code-desktop`,
  `@openagentsinc/khala-code-desktop`, private): **source build is the only
  path** — no public installer, no DMG, no GitHub release, no
  `docs/DEPLOYMENT.md` entry (consistent with the yellow `khala_code.*`
  promise records). Run path from a fresh machine:
  `npm i -g @openai/codex` + `codex login` → clone → `bun install` at the
  **repo root** (workspace) → `bun run dev:khala-code-desktop` (root script →
  `build:ui && electrobun dev`). Electrobun packaging is macOS-focused.
- **Workspace coupling:** Khala Code depends on 7 workspace packages
  (`agent-runtime-schema`, `arbiter-effect`, `composer-state`, `khala-tools`,
  `mcp-contract`, `ui`, `design-tokens`) plus a git dep
  (`three-effect`). Bun resolves `workspace:*` against the checked-out
  workspace set, so **sparse checkouts are fragile** — the supported cheap
  path is a **shallow clone** (`git clone --depth 1`), which skips the ~460 MB
  history while keeping the full working tree `bun install` needs.
- **Khala CLI:** `npm install -g @openagentsinc/khala` — live npm version
  0.1.20 (repo package.json is at 0.1.21, unpublished). Node 20+ or Bun.
- **Pylon:** `npx @openagentsinc/pylon` — live npm version 1.0.5, macOS +
  Linux. The existing served INSTALL.md's Pylon content is accurate.
- **QA runner:** `@openagentsinc/qa-runner` 0.1.0 **is published** —
  `bunx @openagentsinc/qa-runner` works standalone.
- **Khala API:** free key via `POST /api/keys/free`, documented in AGENTS.md.

### 2.3 The served-doc plumbing (what an edit must respect)

- Canonical sources live in `apps/openagents.com/docs/live/`;
  `scripts/sync-live-agent-doc.mjs` copies the companion-file list to
  `apps/web/public/` (served as static assets; `AGENTS.md`, `AGENTS-CORE.md`,
  `HEARTBEAT.md`, `RULES.md`, `skill.json` also have explicit worker routes
  with `text/markdown` handling; `INSTALL.md`/`SURFACES.md` are asset-only).
- `openagents-agent-onboarding.ts` pins **SHA-256 hashes** of AGENTS.md and
  AGENTS-CORE.md plus a `LastUpdated` constant; the routes test asserts those
  and ~60 content substrings, and asserts `docs/live` ≡ `apps/web/public`.
- `scripts/check-live-agent-doc-links.mjs` (in `check:deploy`) requires the
  founder-transcript URL in AGENTS.md and live-checks the critical URLs.
- Copy/redaction gates (`redaction-regression.test.ts`,
  `training-run-public-copy-gate.test.ts`, `public-launch-copy-gate.test.ts`)
  scan `docs/live/AGENTS.md` (not SURFACES.md) for forbidden payment/training
  copy.
- Precedent for splitting: **SURFACES.md** was extracted from AGENTS.md
  ("split out … to keep this file small") and AGENTS-CORE.md is the compact
  tier. The pattern is established; AGENTS.md has since regrown to 84 KB.

## 3. Target state (implemented with this audit)

1. **Root `INSTALL.md` (new, canonical).** One file a user can aim any agent
   at. Owner-directed scope: **only Khala Code and Pylon** are listed as
   installable products for now. Khala Code first (prereqs, the
   **shallow-clone** command — measured: `--depth 1` gives a ~40 MB `.git`
   vs ~460 MB full — root `bun install`, `bun run dev:khala-code-desktop`,
   the optional `khala fleet connect` block, update path), then Pylon
   (`npx @openagentsinc/pylon`); explicit agent-facing notes (never
   full-clone, never sparse-checkout, never touch an existing `~/.codex`
   login). Autopilot Desktop, the qa-runner, and the Khala API key flow are
   deliberately not listed (Autopilot Desktop per owner direction; the
   others live in their own quickstarts). Update/recommendation churn lands
   here first, so pointers stay stable while content changes.
2. **README.md links it prominently** (top-of-file callout after the intro,
   plus the Khala Code and Working-In-This-Repo sections point at it).
3. **Served `docs/live/INSTALL.md` becomes Khala-Code-first** while keeping
   the accurate Pylon v1.0 section; the Autopilot Desktop DMG section is
   removed per owner direction (agents fetching `openagents.com/INSTALL.md`
   get the same story as GitHub; the root file is named canonical).
4. **Served AGENTS.md**: install callout and the "Run Or Test" section are
   rewritten around the INSTALL.md front door with Khala Code named first;
   two heavy self-contained reference blocks move to new companion files —
   `## Pylon And Local Compute` (~190 lines) → **`PYLON.md`**, and
   `## Autopilot Sites` + `## Site Commerce, MDK, And L402` (~165 lines) →
   **`SITES.md`** — each leaving a short pointer section that preserves the
   test-pinned safety lines (money-language rule, capability≠earning,
   launch-dashboard ref). Cuts AGENTS.md by ~20%.
5. **Plumbing:** new files added to the sync script; sync run; SHA-256 +
   LastUpdated constants and the routes test updated; PYLON.md/SITES.md added
   to the redaction and training-copy gates for scan parity with the content
   they inherit.

## 4. Invariants

- **URLs are promises:** `openagents.com/INSTALL.md` keeps resolving with
  the same role (install truth); AGENTS.md anchors that other docs cite
  (`#pylon-agent-smoke-path`, the Tassadar section) stay in place.
- **Claim discipline:** INSTALL.md states plainly that Khala Code has no
  public installer yet (source build; `khala_code.*` promises), that
  installing/running a node is a capability, not earning, and repeats the
  never-clobber-`~/.codex` rule.
- **Counter discipline:** `/code/download` may link the public counter, but the
  counter may only report exact grouped download-event rows. Empty rows,
  missing table, page views, or release-feed existence are not install totals.
- **No authority drift:** all moved AGENTS.md content moves verbatim in
  meaning; the pointer sections keep the owner-approval and
  no-payout-claim boundaries in the main file.

## 5. Follow-ups

- Ship a packaged Khala Code installer (DMG + release feed) and update
  INSTALL.md when the `khala_code.wrapper_product` promise moves; the doc is
  written so only the "Install" block changes.
- Publish khala CLI 0.1.21 or document the 0.1.19–0.1.21 changelog gap.
- Consider a worker route for `/INSTALL.md` (explicit `text/markdown` +
  funnel logging like `/AGENTS.md`) so install fetches are observable.
- Once a public DMG exists, instrument `khala_code_download_events` at the
  artifact edge and keep `/api/public/khala-code/download-counts` exact-only.
- Consider further AGENTS.md splits (Forum Rules → FORUM.md) if the file
  keeps growing.
