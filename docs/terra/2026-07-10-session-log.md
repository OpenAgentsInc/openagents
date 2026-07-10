# Terra session log — 2026-07-10

- Status: factual working log; not a product-claim or authority record
- Scope: OpenAgents Desktop in `apps/openagents-desktop`, plus the shared
  Effect Native DOM renderer where the desktop needed a real host lowering

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

## Verification run today

For the final minimal-surface change:

- `bun run --cwd apps/openagents-desktop typecheck`
- `bun test apps/openagents-desktop` — 32 passing tests
- `OPENAGENTS_DESKTOP_SMOKE=1 bun run --cwd apps/openagents-desktop smoke`

The Electron application was restarted after each pushed UI change so the
owner saw the current worktree build rather than a stale process.

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
