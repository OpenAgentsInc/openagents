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
| Application icon | Exact OpenAgents mobile PNG is copied into the desktop runtime build | Electron window and macOS Dock receive that built asset | A separate or approximate desktop brand mark |
| Project Home | Real thread projection and local-root selection | Bounded Electron workspace service | Project/session Sync authority |
| Files | Root listing plus bounded, traversal-checked read preview | Fixed workspace IPC | Edit/save, diffs, terminal, arbitrary renderer filesystem access |
| Settings | Bounded Codex account-readiness view and isolated Pylon device-auth start | Fixed renderer-argument-free IPC; never default `~/.codex` | Completion of a real owner browser authentication; headless smoke proves only awaiting-browser state |
| Fleet | Existing local brief dispatch remains internal | Pylon host integration | A visible Fleet cockpit or a claimed FleetRun |

## Mobile — `apps/openagents-mobile`

| Area | Current behavior | Boundary | Not yet claimed |
| --- | --- | --- | --- |
| Liquid Glass | SwiftUI iOS 26 `.glassEffect`, material fallback on older iOS; one native `GlassComposer` is the only visible Khala input | `openagents-liquid-glass` native module embedded in build 116 | A React Native outline fallback or a duplicate composer |
| Khala conversation | Persona-neutral generic streaming conversation at `/api/khala/chat`; New chat clears only the in-memory local transcript | Server-owned `openagents/khala` routing; typed native-composer intents | Sync, Fleet/account authority, receipts, local cache authority, or backing-model disclosure |
| Named-persona path | Removed from the mobile application source, assets, tests, and state model under the rev-24 pause | Existing server routes remain compatibility substrate outside this client | A named-assistant front door or local relationship catalog |
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
| Desktop | `bun run --cwd apps/openagents-desktop typecheck`; `bun test apps/openagents-desktop`; `OPENAGENTS_DESKTOP_SMOKE=1 bun run --cwd apps/openagents-desktop smoke` | Passed through the Settings slice; Sol's receipt records 58 tests plus real Electron smoke |
| Mobile | `bun run --cwd apps/openagents-mobile typecheck`; `bun run --cwd apps/openagents-mobile test`; iOS archive/export | Passed: 20 tests after the persona-neutral correction; build 116 signed upload is accepted and awaits ASC build-record ingestion. Build 115 remains `VALID` but contains the removed path and duplicate composer. |

## Deliberate exclusions

- No raw IPC from a renderer.
- No token, local path, arbitrary process, or terminal capability in a UI
  payload.
- No hardcoded assistant replies, fake recents, fake FleetRuns, or fabricated
  proof/account/provider state.
- No hidden mobile filesystem access. A future attachment flow begins with an
  explicit user selection and a reviewed native dependency decision.
