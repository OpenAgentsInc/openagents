# OpenAgents Agent Contract

## Scope

This repository is the new OpenAgents Bun and Effect monorepo.

Preserve `docs/transcripts/`. It is the retained transcript archive from the
previous repository shape.

## Autonomous Loop: Constant Motion (owner mandate)

When running the autonomous AFK loop (`/loop`, see
`docs/autopilot-coder/2026-06-13-afk-autonomous-loop.md` — read it every
iteration), the **top operating rule is CONSTANT MOTION**:

- **Never sit idle. Never sleep on a minutes-long timer.** Do real work every
  moment the loop is active. There is always more work (active product
  integration, the issue backlog, the terminal-agent-systems well, the clarity
  sweep) — "nothing to do" is never true.
- **Long `ScheduleWakeup` idle waits are banned.** Keep working in the SAME turn:
  finish a unit → immediately start the next. If you must yield, prefer a fanout
  whose watcher re-invokes you instantly; only if truly unable to proceed this
  instant, use a SHORT wakeup (≤120s), never minutes.
- **Blocked on the owner? Pull other work.** Write a clear `NEEDS-OWNER:` note
  and immediately continue on a non-blocked item. An owner-gated step never
  stalls the loop. The owner's reply interrupts and takes priority, but you do
  not wait for it.

## Repo Layout

- `apps/openagents.com/` owns the `openagents.com` product surface, including
  the current Autopilot, Forum, Sites, and public proof implementation
  material.
- `apps/openagents-world/` is the Cloudflare Worker + Region Durable Object
  home for live Verse world projection, presence, local interaction,
  interest-scoped fanout, world WebSocket transport, D1 projection rows, queue
  markers, and DO alarm expiry. New world-backend work belongs there, using
  Effect, Effect Schema, D1, hibernatable WebSockets, and the shared world
  packages below.
- `packages/world-contract/` is the shared Effect Schema contract home for
  public-safe world rows, commands, deltas, cursors, moderation decisions, and
  WoC-style read-model projection types.
- `packages/world-client/` is the shared desktop/web Verse world client that
  mirrors snapshots and deltas into a read-only `WorldReadModel`.
- The old self-hosted SpacetimeDB `openagents-world` module was deleted during
  the Cloudflare Verse World cutover. Do not re-clone, regenerate bindings for,
  or add production world features to that path; port useful historical schema
  or reducer ideas into the Cloudflare/Effect world service instead.
- `apps/forum/` owns the forum extraction target for
  `openagents.com/forum`. The live Forum routes stay inside the
  `openagents.com` Worker for now because they share auth, D1, payment
  receipt, and public projection boundaries.
- `apps/pylon/` owns the Pylon contributor app imported from the standalone
  Pylon repository. It bundles the former Probe runtime as
  `@openagentsinc/pylon-runtime`.
- `apps/nostr-relay/` owns the Nostr relay surface.
- `packages/probe/` owns the Probe runtime imported from the standalone Probe
  repository.
- `packages/nip90/` owns the NIP-90 protocol library for the compute, data,
  and labor market rails.
- `docs/promises/` owns product-promise records, launch-promise source sets,
  verification gates, copy gates, and user/agent report templates.
- `docs/refactor/` owns migration plans, cutover notes, and architectural
  cleanup records for this repo reset.
- `docs/transcripts/` owns the retained transcript archive for episodes
  001-234 of the build series, with a theme guide in
  `docs/transcripts/README.md`.
- `docs/tassadar/` owns the Tassadar research essays on exact-execution
  LLM computers and verification by replay.
- `docs/autopilot-coder/` owns Autopilot Coder status audits, smoke runbooks,
  and the paid L402 boundary notes.
- `docs/forum/`, `docs/nostr/`, and `docs/research/` own dated audits for
  those areas.

## Live Public Reference Surfaces

- Agent onboarding instructions: <https://openagents.com/AGENTS.md>
- Product promises page: <https://openagents.com/docs/product-promises>
- Agent-readable promise registry:
  <https://openagents.com/api/public/product-promises>
- Product Promises Forum:
  <https://openagents.com/forum/f/product-promises>
- Strict bug form:
  <https://github.com/OpenAgentsInc/openagents/issues/new?template=strict-bug.yml>

## Deploying & Releasing

- **`docs/DEPLOYMENT.md` is the single hub for every deploy / publish / release.**
  Read it first for any of: deploying the `openagents.com` Cloudflare Worker,
  publishing Pylon to npm, cutting a Pylon or Autopilot Desktop release (incl. the
  signed/notarized macOS DMG), the `updates.openagents.com` OTA feed, or the mobile
  app. It indexes the per-surface runbooks (the sources of truth), the one-line
  recipe for each, the GitHub release-tag convention, and where the signing
  secrets live (`~/work/.secrets/` + GCP Secret Manager, project `openagentsgemini`).
- Signing/notarization details live in `apps/oa-updates/docs/release-signing-runbook.md`
  (ed25519 release key + the `HQWSG26L43` Apple Developer ID) — read before any signed
  release. Publish/deploy only from a clean `origin/main`; RCs are pre-releases and
  never take the stable `latest` badge.

## Working Rules

- Read `INVARIANTS.md` before changing authority, routing, payment,
  projection, or public-claim surfaces.
- For work under `apps/openagents.com/`, also read
  `apps/openagents.com/AGENTS.md` and `apps/openagents.com/INVARIANTS.md`.
- **Leave it cleaner than you found it — clean up as you go, every phase.** When you
  touch an area and find pre-existing breakage (failing tests, lint, type errors,
  doc-coverage/OpenAPI/AGENTS.md drift, stale refs, dead code), **fix it even if you did
  not cause it** rather than stepping around it or deferring. Nothing accumulates: every
  phase, branch, and PR lands with the full relevant test suite **and** `check:deploy`
  green — not "green except the pre-existing reds." If a pre-existing failure is genuinely
  too large or out of scope for the current change, fix what is cheap and **explicitly
  flag the rest** (in the report, and a tracking issue if it will persist) — never
  silently leave a red, and never describe a partially-green run as clean.
- Keep new TypeScript implementation work on Bun, Effect, Effect Schema, and
  Foldkit where `apps/openagents.com` already uses it.
- Do not reintroduce the old Cargo or Tauri workspace unless the user asks for
  explicit historical compatibility work.
- **Mobile build/ship policy (owner mandate): NO Expo/EAS cloud.** For
  `clients/mobile/AutopilotRemoteControl`, native iOS `.ipa` compiles **locally
  on this Mac** (`expo prebuild` → `xcodebuild`/`fastlane`) and TestFlight upload
  is **Apple-native `xcrun altool`** (ASC key in `.secrets/appstoreconnect.env`).
  JS-only changes ship **OTA via our own `updates.openagents.com`**
  (`apps/oa-updates/scripts/publish-ota.sh`), never `eas update`/u.expo.dev.
  Never run `eas build` / `eas submit` / `eas update`. The `expo` CLI itself
  (`expo install`/`export`/`prebuild`) stays. Runbook:
  `clients/mobile/AutopilotRemoteControl/TESTFLIGHT.md`.
- Route new user-facing and agent-facing product claim systems through
  `docs/promises/` before broadening copy.
- Keep Claim Your Agent public identity flows tweet-first where possible:
  use the shared owner-claim/X verification routes, the friendly
  `Verifying my agent ... Code: ...` copy, and public tweet-author binding
  rather than adding a parallel identity-verification path.
- Keep product-promise report intake Forum-first. Agents and users should post
  loose reports, product-promise gaps, feature commentary, and discussion in
  the Product Promises Forum.
- GitHub issues are only for concrete, reproducible bugs that satisfy the
  strict bug issue form. Blank issues are disabled, and malformed or loose
  reports should be rejected by the issue form or moved back to the Forum.
- Do not commit secrets, dependency caches, build output, `target/`, `dist/`,
  `node_modules/`, or local runtime state.
- Before publishing ANY npm package from this repo, read
  `apps/pylon/docs/npm-publishing-runbook.md`. The scope is
  `@openagentsinc/` (never `@openagents/`), the auth token lives in
  workspace `.secrets/npm-publish.env`, `bun publish` is broken against
  npmjs (use `bun pm pack` + `npm publish <tarball>`), Pylon pre-stable
  releases publish under `--tag rc` only, and registry-CDN propagation
  makes fresh publishes look 404 to bun for minutes — the runbook covers
  all of it.
- Keep Git operations scoped to this repository when working here.
- Do not put individual people’s names in commit messages, commit trailers, or
  other committed metadata unless the user explicitly asks for a legally or
  historically required attribution. Use neutral product, team, source,
  operator, or role wording instead.
