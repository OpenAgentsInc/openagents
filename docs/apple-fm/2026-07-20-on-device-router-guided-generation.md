# On-device router with guided generation (2026-07-20)

## Owner directive

The on-device model is a router. It does not answer the user when a delegate
agent is connected. It chooses the agent that handles the message. The host
dispatches the chosen agent. The model answers directly only when no delegate
agent is connected.

## Routing policy

- `claude` (its Fable model) takes high-concept work. This includes strategic
  thinking, planning, architecture, analysis, design, and general questions.
  `claude` is the default when no other role fits.
- `codex` takes coding tasks of medium-to-high difficulty.
- `grok_acp` takes simple mechanical tasks. This includes bulk string renames
  and trivial edits.

## Guaranteed shape

The route shape is guaranteed. The Swift bridge uses guided generation. A
runtime `DynamicGenerationSchema` constrains the `candidate` field to the
connected-agent set. Constrained sampling can only emit an admitted candidate.
The bridge assembles the full route JSON. The model cannot return a malformed
route. This replaces the earlier free-text JSON approach, which was a parsing
risk.

## Flow

1. `apps/openagents-desktop/src/turn/apple-fm-prompt.ts` builds the router
   preamble when one or more delegates are connected. The preamble supplies the
   policy. It does not supply a JSON template. The guided schema owns the shape.
2. `apps/openagents-desktop/src/turn/desktop-apple-fm-provider.ts` passes the
   ready delegate candidates to `host.runTurn`. A non-empty candidate set
   switches the bridge to guided route generation.
3. `packages/apple-fm-runtime/src/client.ts` adds the `route.candidates` field
   to the bridge request. `packages/apple-fm-runtime/src/wire.ts` mirrors the
   field.
4. The Swift bridge
   (`apps/pylon/swift/foundation-bridge/Sources/foundation-bridge/main.swift`,
   with an identical copy under
   `packages/apple-fm-runtime/native/foundation-bridge`) runs the guided path
   and returns the well-formed route JSON.
5. The existing AFS-02 decoder and AFS-04 router accept the route and dispatch
   the agent.

## Direct-answer fallback

The model answers the user directly only when no delegate agent is connected.
The direct-answer path keeps the honesty base and the ambient-context block, so
the model can speak to the environment and identity.

## Verification

The guided path was tested against the live on-device bridge. Each request
returned a well-formed route for an admitted candidate. Coding requests routed
to `codex`. Planning and general requests routed to `claude`. Simple mechanical
requests routed to `grok_acp`.
