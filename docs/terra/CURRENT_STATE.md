# Terra current product state

- Updated: 2026-07-10
- Status: shipped local baseline; this is not a claim of Sync, Fleet, provider,
  or production-model authority

## Shared product rule

The default surface is a quiet conversation product. A user sees their recent
conversations, the selected conversation, and the action they can take next.
Host, proof, policy, token, Fleet, and transport details are not decorative
chrome. They appear only in a dedicated, connected surface when they answer a
real user question.

## Desktop — `apps/openagents-desktop`

| Area | Current behavior | Boundary | Not yet claimed |
| --- | --- | --- | --- |
| Chats | Local five-thread store; create/open/send; assistant turns persist | Fixed schema-validated Electron IPC | Sync/cross-device thread authority |
| Completion | Sends bounded `{threadId, message}` through the Electron host | Host-held configured OpenAgents gateway token | A response without configured gateway credentials |
| Composer | Focus returns after New chat and completed sends; no rectangular focus outline | Renderer state + DOM focus management | Rich editor parity |
| Material | Typed backdrop and glass surface authored in Effect Native | Shared DOM lowering | Literal SwiftUI `.glassEffect` on macOS |
| Project Home | Real thread projection and local-root selection | Bounded Electron workspace service | Project/session Sync authority |
| Files | Root listing plus bounded, traversal-checked read preview | Fixed workspace IPC | Edit/save, diffs, terminal, arbitrary renderer filesystem access |
| Fleet | Existing local brief dispatch remains internal | Pylon host integration | A visible Fleet cockpit or a claimed FleetRun |

## Mobile — `apps/openagents-mobile`

| Area | Current behavior | Boundary | Not yet claimed |
| --- | --- | --- | --- |
| Liquid Glass | SwiftUI iOS 26 `.glassEffect`, material fallback on older iOS; RN host fallback elsewhere | `openagents-liquid-glass` native module | Desktop pixels or a fake native material |
| Chats | App-owned, persisted five-thread Sarah catalog; title from first user turn | `expo-file-system` document storage | Cross-device Sync |
| New/open chat | New chat mints a new Sarah relationship; recent selection restores that thread's bounded transcript | Typed Effect Native intents and Sarah client | Invented placeholder or canned thread state |
| Sarah turns | Existing production Sarah route and bounded SSE stream | Effectful mobile client -> typed state updates | Fleet/authority state inferred from chat text |
| Files | No folder browser | Mobile privacy/runtime boundary | Raw device filesystem browsing |

## Shared renderer status

- The Effect Native icon name set includes the OpenAI Apps SDK catalog names
  used by the desktop work.
- DOM lowers those names through the shared Apps SDK icon SVG source.
- React Native keeps a bounded glyph fallback for the same closed icon name
  set, so typed views remain portable while native SVG support is not a
  shipping dependency.

## Verification baseline

| Surface | Commands last run | Result |
| --- | --- | --- |
| Desktop | `bun run --cwd apps/openagents-desktop typecheck`; `bun test apps/openagents-desktop`; `OPENAGENTS_DESKTOP_SMOKE=1 bun run --cwd apps/openagents-desktop smoke` | Passed during this session (desktop suite reported 34 tests at latest run) |
| Mobile | `bun run --cwd apps/openagents-mobile typecheck`; `bun run --cwd apps/openagents-mobile test` | Passed: 32 tests |

## Deliberate exclusions

- No raw IPC from a renderer.
- No token, local path, arbitrary process, or terminal capability in a UI
  payload.
- No hardcoded assistant replies, fake recents, fake FleetRuns, or fabricated
  proof/account/provider state.
- No hidden mobile filesystem access. A future attachment flow begins with an
  explicit user selection and a reviewed native dependency decision.
