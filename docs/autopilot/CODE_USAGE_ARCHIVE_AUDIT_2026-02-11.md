# Code Usage + Archive Audit (Autopilot Web/Mobile/Desktop)

Date: 2026-02-11

## Execution Status

Completed on 2026-02-11:
- Moved archive-safe directories out of this repo to:
  - `/Users/christopherdavid/code/backroom/archive/openagents/from-openagents-2026-02-11-safe-archive`
- Archived paths:
  - `apps/web-old`
  - `packages/social`
  - `apps/cloudflare-agent-sdk-demo`
  - `apps/nydus`
  - `apps/liteclaw-local-agent`
  - `apps/autopilot-worker.local.1770334210`
- During archive copy, obvious generated artifacts were pruned from the backroom destination (`node_modules`, `dist`, `.wrangler`, `playwright-report`, `test-results`, `.tanstack/tmp`, `*.tsbuildinfo`, `.DS_Store`) to keep archive history source-focused and push-safe.

## Scope

This audit answers: what code is currently in use by Autopilot surfaces, and what can be archived.

Surfaces audited:
- `apps/web` (Autopilot web + worker routes)
- `apps/expo` (Autopilot mobile)
- `apps/autopilot-desktop` (Autopilot desktop)

Method:
- Static import/reference tracing from the three surface roots.
- Workspace/package reference checks (`rg` across `apps`, `crates`, `packages`, `docs`).
- No behavioral assumptions without code evidence.

## Active Surface Dependency Map

### Web (`apps/web`)

Directly used local packages:
- `@openagentsinc/effuse`
- `@openagentsinc/effuse-flow`
- `@openagentsinc/effuse-panes`
- `@openagentsinc/effuse-ui`
- `@openagentsinc/hud`
- `@openagentsinc/dse`

Cross-app code imports (important):
- `apps/web/src/effuse-host/autopilot.ts` imports from `../../../autopilot-worker/src/effect/ai/*` and `../../../autopilot-worker/src/dseCatalog`
- `apps/web/src/effuse-host/contracts.ts` imports from `../../../autopilot-worker/src/tools` and `../../../autopilot-worker/src/dseCatalog`
- `apps/web/src/effuse-host/dseAdmin.ts`, `dseCompile.ts`, `dseJobs.ts`, `dsePinnedArtifacts.ts` import `../../../autopilot-worker/src/dseCatalog`
- `apps/web/tests/worker/dse-pinned-artifacts.test.ts` imports `../../../autopilot-worker/src/dseCatalog`

Conclusion:
- `apps/autopilot-worker` is not fully orphaned; it is used as a shared source library by `apps/web`.

### Mobile (`apps/expo`)

Cross-app coupling:
- `apps/expo/app/screens/FeedScreen.tsx:4` imports Convex API types from `../../../web/convex/_generated/api`

Navigation reality (currently reachable code, not dead):
- `apps/expo/app/navigators/AppNavigator.tsx` routes to `DemoNavigator`
- `apps/expo/app/navigators/DemoNavigator.tsx` includes tabs for `Feed`, `DemoShowroom`, `DemoCommunity`, `DemoPodcastList`, `DemoDebug`, `Profile`

Conclusion:
- A large amount of Ignite demo code is still active/reachable in mobile; it is not dead code today, but it is product-bloat candidate.

### Desktop (`apps/autopilot-desktop`)

Workspace-level active binary:
- `Cargo.toml:4` includes `apps/autopilot-desktop` as a workspace member.

Key coupling:
- `apps/autopilot-desktop/src/main.rs` uses `https://openagents.com/api/moltbook/api` as default proxy base.
- `apps/web/docs/CLOUDFLARE-API-ROUTING.md` documents a separate `openagents-api` worker on `openagents.com/api/*`.

Conclusion:
- `apps/api` appears to be an active backend surface for at least some desktop/API traffic and should not be archived blindly.

## Archive Candidates

### A. Safe Archive Candidates (high confidence)

1. `apps/web-old`
- Evidence: no references found from active surface code roots (`apps/web`, `apps/expo`, `apps/autopilot-desktop`, `crates/autopilot_ui`, `crates/autopilot_app`).
- It is currently used as historical reference in docs/spec notes only.
- Recommendation: move to `archive/` branch/tag or `apps/_archive/web-old`.

2. `packages/social`
- Evidence: no active imports/usage from web/mobile/desktop code.
- Only self-references in `packages/social/*` docs/package metadata.
- Recommendation: archive package or mark experimental and remove from active workspace expectations.

3. `apps/cloudflare-agent-sdk-demo`
- Evidence: referenced as pattern/demo in docs; no runtime imports from web/mobile/desktop.
- Recommendation: archive to `apps/_archive/` unless actively used for onboarding.

4. `apps/nydus`
- Evidence: tied to LiteClaw tunnel tooling/docs; no usage from Autopilot web/mobile/desktop runtime paths.
- Recommendation: archive with LiteClaw tooling or move under `apps/experimental/`.

5. `apps/liteclaw-local-agent`
- Evidence: LiteClaw tunnel utility only; no active usage from Autopilot web/mobile/desktop codepaths.
- Recommendation: archive or relocate to `apps/experimental/` with `nydus`.

6. Local-only stale directory: `apps/autopilot-worker.local.1770334210`
- Contents are cache artifacts (`node_modules`, `.wrangler`, `tsconfig.tsbuildinfo`).
- No references in repo code/docs.
- Recommendation: delete locally; keep ignored.

### B. Not Safe To Archive As-Is (currently in use)

1. `apps/autopilot-worker`
- Used directly by web via source imports (catalog/contracts/model adapters).
- You can archive only after extracting shared modules to a stable package (e.g. `packages/autopilot-shared` or `packages/autopilot-ai-adapters`) and updating `apps/web` imports.

2. `apps/api`
- Likely active for non-autopilot-web API routes (`openagents.com/api/*`) and desktop Moltbook proxy usage.
- Needs route-ownership confirmation before any archive action.

3. `apps/expo` demo surfaces
- Not dead code today: demo screens are mounted in `DemoNavigator`.
- Could be reduced, but that is a product refactor (not dead-code deletion).

## High-Value Cleanup Opportunities

1. Decouple web from `apps/autopilot-worker` source imports.
- Extract these currently shared files:
  - `apps/autopilot-worker/src/dseCatalog.ts`
  - `apps/autopilot-worker/src/tools.ts`
  - `apps/autopilot-worker/src/effect/ai/languageModel.ts`
  - `apps/autopilot-worker/src/effect/ai/openRouterLanguageModel.ts`
  - `apps/autopilot-worker/src/effect/ai/fallbackLanguageModel.ts`
- Then remove cross-app relative imports in `apps/web/src/effuse-host/*`.

2. Mobile slimming pass.
- Remove non-product Ignite demo tabs/screens from `DemoNavigator`.
- Keep `Feed`, `Profile`, and auth flow only (or whatever product requires).

3. Archive legacy web app.
- Move `apps/web-old` out of active app root to reduce confusion and accidental edits.

## Suggested Execution Order

1. Archive `apps/autopilot-worker.local.1770334210` immediately.
2. Archive `packages/social`, `apps/cloudflare-agent-sdk-demo`, `apps/nydus`, `apps/liteclaw-local-agent` (or move to `experimental/`).
3. Extract shared worker modules used by `apps/web` into a package, then decide fate of remaining `apps/autopilot-worker` runtime.
4. Run a product-directed Expo slimming PR.
5. Archive `apps/web-old` after final historical snapshot/tag.

## Notes

This audit intentionally distinguishes:
- Dead/unreferenced code (archive-safe now), vs
- Reachable but low-priority/product-bloat code (requires intentional refactor).

## Local Workspace Artifact Cleanup (non-source)

These are present locally and consume space but are not tracked source:
- `apps/api/target` (~1.5G)
- `apps/autopilot-worker.local.1770334210` (~458M)
- `apps/web-old/playwright-report` and `apps/web-old/test-results` (test outputs)

They can be removed at any time without changing repository source history.
