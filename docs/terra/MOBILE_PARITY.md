# OpenAgents Mobile parity ledger

- Updated: 2026-07-10
- Destination: `apps/openagents-mobile` on Expo, React Native, SwiftUI Liquid
  Glass, and Effect Native
- Principle: preserve native interaction and privacy boundaries while matching
  the user-visible desktop behavior where that behavior makes sense on a phone

## Why mobile is not a desktop clone

The desktop can safely expose a user-selected local workspace through an
Electron host service. A phone has a different permission model, screen size,
and native runtime. Mobile parity therefore means the same trustworthy product
facts—real conversations, honest actions, durable local continuity—not a
miniature file manager or a copy of desktop diagnostics.

## Capability ledger

| Desktop behavior | Mobile-native equivalent | Status | Guardrail |
| --- | --- | --- | --- |
| Recent five chats | Persisted five-thread Sarah catalog | Landed | App-owned document storage only |
| New chat | Fresh Sarah prospect/session mint | Landed | No cloned or fake session |
| Open a chat | Restore the selected thread relationship + bounded transcript | Landed | Each selected row restores its own thread |
| Real response/error | Existing Sarah route, bounded SSE, typed unavailable state | Landed baseline | No canned assistant reply |
| Khala orchestrator | Public generic Khala mode in the same native picker | Landed baseline | Server owns `openagents/khala` routing; no invented backing lane or Fleet outcome |
| Minimal default chrome | Native Liquid Glass shell, typed Effect Native content/drawer | Landed | No developer/transport/Fleet counters in default chrome |
| Official typed icon names | Shared Effect Native icon catalog | Landed | RN fallback remains closed and typed |
| Project home | Conversation catalog is the current mobile home | Landed baseline | No fictional project data |
| Desktop local Files | Explicit import/attachment catalog | Not started | Requires user choice and native-runtime review |
| Desktop review/editor | Read-only attachment/detail surface | Not started | Must have a real selected document source |
| Desktop Fleet cockpit | Purposeful mobile supervision surface | Deferred | Requires authoritative Pylon/Sync state |

## Persistence behavior

1. The existing Sarah session file preserves the current relationship and its
   bounded settled transcript.
2. The recent-thread file stores at most five valid thread records, newest
   first.
3. A record title is the user's first message, clipped to a bounded length;
   it is not generated marketing copy or a synthetic summary.
4. Selecting a record resets the transient surface and restores that record's
   own prospect reference, thread ID, and transcript.
5. Disk failures are soft: the current live conversation remains usable, and
   no false persistence confirmation appears in the interface.

## Khala mode boundary

Khala mode is intentionally a separate, stateless generic conversation. Its
turns call the already-public streaming route at `/api/khala/chat`; the server
rebuilds the Khala instruction and performs orchestration. The mobile app
renders the returned answer but does not label a particular serving model,
Pylon, tool, verifier, receipt, or economic result. It does not reuse Sarah's
prospect persistence. This preserves the transcript distinction documented in
episodes 242–245: one Khala model surface can orchestrate a pool, but routing
and authority belong behind the endpoint.

## Next mobile acceptance test

On a real device, create more than five distinct chats, restart the app, and
verify that exactly the newest five appear in the drawer, each restores its
own transcript, and a new chat does not inherit a prior prospect/thread ID.
This is the next proof before adding any attachment or Fleet surface.

For Khala specifically, the next device receipt is a real turn through build
114 while the mode picker and native glass chrome are visible; build validity is
not a pixel/owner-acceptance claim.
