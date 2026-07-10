# Terra session log — 2026-07-10

- Status: factual working log; not a product-claim or authority record
- Scope: `apps/openagents-desktop`, `apps/openagents-mobile`, and the shared
  Effect Native DOM/RN renderers where a native-equivalent lowering was needed
- Governance update: Sol's rev-22 roadmap makes Terra an explicit parallel
  execution lane. It may land ready low-collision #8574 leaves, while Sol
  retains P0, dependency order, hot-contract integration, and reconciliation.

## The request as it evolved

The owner wanted a desktop OpenAgents application that feels like the mobile
OpenAgents/SwiftUI surface, not a developer harness. The important feedback
was direct and useful:

1. A scripted reply is not a chat product.
2. A fake, hard-coded recent-chat list is not a sidebar.
3. Electron/debug/authority labels are implementation residue, not product UI.
4. “Looks like SwiftUI” cannot mean a few dark CSS effects; the shared Effect
   Native rendering contract needs a material/backdrop lowering analogous to
   the mobile SwiftUI Liquid Glass fallback.
5. The result must be minimal: chat history, conversation, and composer are
   the default surface. Everything else must earn its visibility.

## Landed work

| Commit | Outcome |
| --- | --- |
| `39b0e01928` | First Desktop fleet/chat surface. |
| `4b8bfe4e2f` | Bounded local-Pylon Fleet brief dispatch. |
| `47b62e4359` | First visual polish pass. |
| `6c48774608` | Neutral chat workspace rail; removed visible Sarah branding. |
| `a4d6348a08` | Host-owned chat completion bridge and persisted five-thread history. |
| `9df69b7391` | Removed the rectangular composer focus ring. |
| `398911ad3c` | Composer re-focuses after New chat and completed sends. |
| `8ed6d166fd` | Effect Native DOM adopts the shared OpenAI Apps SDK icon catalog; sidebar uses typed icons and left-aligned rows. |
| `acdb90f378` | Desktop authors a typed `BackgroundGradient` behind typed glass surfaces; the shared DOM renderer lowers the mobile-material analogue. |
| `d13bdd4ab0` | Removed rendered host/proof/workspace/Fleet controls, authority copy, Pylon status, and the composer Proof action. |
| `d1abe0e81e` | Added real desktop Project Home and bounded local-folder listing/read preview through fixed IPC. |
| `226aad0e72` | Sharpened the desktop material geometry after visual review. |
| `7d77150514` | Replaced mobile seeded conversations with an app-owned five-thread catalog, real new-thread minting, and recent-thread restoration. |
| `23aba8270a` | Added the mobile Khala mode: a typed public-orchestrator transcript/composer through the existing streaming Khala route; TestFlight build 114 is `VALID`. |
| `ee78dc1a2e` | Restored the owner-required `openagents-liquid-glass` SwiftUI module after a concurrent removal caused the iOS app to expose opaque React Native fallback controls; prepared native build 115. |
| Pending current mobile landing | Removed mobile named-persona/session/demo/local-catalog code; made the SwiftUI Liquid Glass composer the sole Khala input; prepared build 116. |
| Pending current desktop landing | Uses the exact checked-in OpenAgents mobile app PNG for the Electron window and macOS Dock, with a build-time byte-parity test. |

## What now works

- The renderer sends a bounded `{ threadId, message }` request through fixed,
  schema-validated Electron IPC.
- The Electron host stores conversations locally, keeps five recent threads,
  calls the configured OpenAgents model gateway with a host-held token, and
  returns a persisted assistant turn.
- If a model token or gateway is unavailable, the UI gives an honest system
  error instead of manufacturing an assistant reply.
- New chat and completed sends return focus to the composer without a visible
  rectangular focus outline.
- The shared OpenAI Apps SDK catalog is the Effect Native DOM icon source.
- The desktop's backdrop/material relationship is authored in the Effect
  Native view tree, rather than only in app-local CSS.
- The visible desktop default is intentionally minimal: sidebar conversations,
  the selected transcript, and the composer.
- Desktop Project Home and Files use a bounded host service: the user selects
  a local root, the renderer receives a capped root listing, and reads are
  capped and traversal-checked.
- Mobile persists only conversations created inside the app, keeps the newest
  five, and restores the selected conversation's own Sarah relationship and
  transcript. It never presents made-up recents.

## Verification run today

For the final minimal-surface change:

- `bun run --cwd apps/openagents-desktop typecheck`
- `bun test apps/openagents-desktop` — 32 passing tests
- `OPENAGENTS_DESKTOP_SMOKE=1 bun run --cwd apps/openagents-desktop smoke`

The Electron application was restarted after each pushed UI change so the
owner saw the current worktree build rather than a stale process.

For the mobile recent-thread delivery (`7d77150514`):

- `bun run --cwd apps/openagents-mobile typecheck`
- `bun run --cwd apps/openagents-mobile test` — 32 passing tests

## Lessons

### A styled placeholder loses trust faster than a missing feature

The first visible chat output was locally scripted. It made the UI appear to
work while answering unrelated prompts with the same text. The right repair
was not better canned copy; it was moving completion and persistence to the
host boundary and surfacing unavailable configuration honestly.

### Product chrome must answer a user question

`electron/darwin`, proof counters, local-workspace status, Pylon dispatch,
and authority boilerplate describe implementation mechanics. They may be
valuable in a diagnostic surface, but they pollute a default conversation
surface. The default should be quiet; diagnostics should be explicit and
separate.

### “Shared UI” must mean a shared semantic lowering

The mobile app's relevant idea is not just blur. It is a scene behind a
material, with semantic controls rendered on top. The desktop equivalent is a
typed backdrop plus a typed glass material lowered by the renderer—not a
collection of incidental CSS effects inside one app.

### Fast work still needs a tight feedback loop

The reliable loop today was: change one clear thing, typecheck, run the small
test suite, run the real Electron smoke, inspect a screenshot, push, restart.
It caught stale-process confusion and UI that passed structural tests but was
still visibly wrong.

## Remaining limitations

- Electron cannot literally run SwiftUI's iOS `.glassEffect`; the desktop uses
  the shared Effect Native material contract with the DOM's honest equivalent.
- The Fleet capability remains in internal code and host IPC, but the owner
  explicitly removed it from the default UI. It needs a separate, deliberate
  entry point before being shown again.
- The local thread store is intentionally a bounded desktop store, not yet a
  Sync-backed cross-device conversation authority.
- Actual model responses require a configured host token/gateway. No fallback
  response is fabricated.

## Mobile continuation

The same day, the mobile OpenAgents app gained the corresponding real
conversation catalog. Its native Liquid Glass shell remains SwiftUI-owned,
while the Effect Native program now hydrates a five-thread, app-owned on-device
catalog. New chat mints a fresh Sarah relationship, a recent row restores its
own bounded transcript, and titles come from the user's first message rather
than placeholder copy. It intentionally does not expose the desktop's raw
folder browser: on mobile, device-file access must begin with an explicit
user-selected document capability rather than silently treating the phone's
  filesystem as a workspace.
- The default mobile mode picker now includes Khala. It is a real call to the
  public generic orchestration route, but it is deliberately not a claim that
  a named backing model, Pylon, FleetRun, receipt, or Sarah relationship exists.

## Mobile SwiftUI regression and correction

Build 114 was technically valid in App Store Connect but was visually wrong.
The concurrent renderer cleanup removed the app-local SwiftUI module from the
native project. The JavaScript fallback then became the actual iOS chrome,
which is why the owner saw outlined, opaque controls instead of Liquid Glass.
That is not an acceptable degradation for a surface explicitly required to be
SwiftUI.

`ee78dc1a2e` restores the native module, its SwiftUI `GlassComposer`,
`GlassIconButton`, and `GlassPill` controls, and the app dependency that causes
the module to be embedded during iOS prebuild. The typed state and intent model
remains shared; the owner-visible iOS lowering is deliberately native SwiftUI.
Because an OTA cannot add a native module missing from an already-installed
binary, this correction requires build 115 rather than a build-114 OTA.

## Mobile reliability reset

The owner directed a cleanup after seeing two inputs on Khala: the floating
SwiftUI composer and an Effect Native composer inside the transcript. The
second composer is removed. The remaining native composer now owns the real
Khala text field, draft change, and submit events; it feeds the same typed
Khala intent boundary as the renderer rather than serving as a decorative tap
target.

Sol roadmap rev-24 also changes the acceptance path. The mobile client no
longer contains named-persona relationship state, client-side prospect/thread
persistence, SSE adapters, demo videos, purchase-sheet demo, or their tests.
Those server-side compatibility routes remain outside this mobile client. The
new client is intentionally smaller and truthful while R1/R2 identity and
Khala Sync authority are built.

## Execution record

- Desktop scope was claimed and progress-reported against OpenAgents issue
  #8574. Mobile scope was claimed and released against #8597.
- Each implementation commit was rebased onto the moving `main` tip before
  push when necessary; no force-push or overwrite of concurrent work was used.
- All claims in this log distinguish local persistence from Sync authority,
  and user-visible success from an unverified production model/fleet outcome.
- The current Desktop Settings slice is a bounded readiness/device-auth start
  path. Its smoke reaches awaiting-browser state only; a real owner Codex
  reconnect remains an owner proof gate and must not be marked completed here.
