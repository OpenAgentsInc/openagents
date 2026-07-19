# OpenAgents Mobile parity ledger

- Updated: 2026-07-10
- Destination: `apps/openagents-mobile` on Expo, React Native, SwiftUI Liquid
  Glass, and Effect Native
- Program authority: Sol roadmap rev-24. Mobile is a purpose-built fleet
  supervision and continuity client, never a miniature desktop workbench

## Current truthful baseline

| Desktop behavior | Mobile-native equivalent | Status | Guardrail |
| --- | --- | --- | --- |
| Neutral conversation entry | Persona-neutral Khala conversation | Landed baseline | No named-persona relationship or local thread authority |
| Real response/error | Public generic Khala stream with typed failure state | Landed baseline | No canned assistant reply |
| New chat | Clear the current in-memory Khala transcript | Landed baseline | Does not mint a fictional session or durable reference |
| Minimal composer | One real SwiftUI Liquid Glass composer | Build 116 correction | No duplicate Effect Native composer in the transcript |
| Official typed icon names | Shared Effect Native icon catalog | Landed | Native shell uses SF Symbols. Renderer remains typed |
| Cross-device conversations | Khala Sync projection | Not started in mobile | Local state never claims to be authority |
| Fleet supervision | Activity, attention, control, receipts, handoff | Not started in mobile | Requires authoritative Sync/Fleet contracts |
| Desktop editor/terminal | No mobile equivalent | Deliberately absent | Mobile does not pretend to be a coding workbench |

## Removed by the owner’s rev-24 decision

The mobile application no longer includes named-persona UI, relationship or
prospect state, local session/thread persistence, named-persona SSE adapters,
video/demo assets, or the price-sheet demo. Server compatibility routes remain
outside the client. Removing these local surfaces is not a claim that the
underlying server contracts have been deleted.

## Khala boundary

Khala calls the public streaming route at `/api/khala/chat`. The server owns
orchestration and any backing-lane routing. The mobile client renders the reply
but does not label a serving model, Pylon, tool, verifier, receipt, account,
or economic result. It creates no durable mobile authority.

## Next acceptance proof

On a physical phone using build 116, type and submit a real Khala turn through
the sole native SwiftUI composer. Verify no second input appears in the
transcript and that switching/new-chat cannot surface named-persona/demo/local
catalog residue. Then the correct next product work is R1/R2 authenticated
identity and Khala Sync continuity—not a replacement local cache.
