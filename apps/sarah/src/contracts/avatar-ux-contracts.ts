import {
  BehaviorContractSchemaVersion,
  type BehaviorContractRegistryDocument,
} from "@openagentsinc/behavior-contracts"

/**
 * Sarah avatar UX behavior contracts (SQ-4 #8621, epic #8610).
 *
 * Owner mandate 2026-07-09: "you need to catch all this shit YOURSELF in
 * automated QA so user never has these horrible failures." Every live-session
 * failure the owner hit on launch day lands here with the statement verbatim
 * and an oracle in the normal sweep or the synthetic-prospect e2e smoke
 * (apps/sarah/scripts/sarah-avatar-e2e-smoke.mjs), so a regression fails a
 * machine gate before it reaches a person.
 *
 * Human rendering: docs/sarah/SARAH_CONTRACTS.md.
 */
export const sarahAvatarUxContractRegistry: BehaviorContractRegistryDocument = {
  contracts: [
    {
      authorityBoundary:
        "Binds the owned avatar path in apps/sarah (mint route, owned-renderer speaking bridge, SSE bus). Grants no authority over the render service's internal frame scheduling; hydralisk owns frame truth. The greeting is a fixed line, not a brain turn — it may not invent pricing or claims.",
      blockerRefs: [],
      contractId: "sarah.avatar_greets_first.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "apps/sarah/src/services/owned-renderer.ts",
        "apps/sarah/src/server.ts",
        "apps/sarah/scripts/sarah-avatar-e2e-smoke.mjs",
        "issue:#8621",
        "issue:#8610",
      ],
      oracles: [
        {
          description:
            "Unit: speaking the greeting on a minted owned session publishes the fixed greeting on the SSE transcript bus and streams greeting PCM to the fake render service (speak chunks sharing one event_id, then speak_end); a TTS outage degrades soft (transcript still lands, session unharmed).",
          id: "avatar_greeting_speaks.unit",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/sarah/src/services/owned-renderer.test.ts",
        },
        {
          description:
            "E2E smoke (synthetic prospect against a live deployment): mint a real session and require the greeting transcript on the SSE stream within the deadline; fails loudly otherwise.",
          id: "avatar_greeting_deadline.smoke",
          kind: "script",
          mode: "e2e",
          ref: "apps/sarah/scripts/sarah-avatar-e2e-smoke.mjs",
        },
      ],
      productArea: "avatar surface + owned renderer",
      source: {
        channel: "sarah-production-conversation",
        statedBy: "owner",
        statedOn: "2026-07-09",
      },
      state: "enforced",
      statement:
        "It is not advancing beyond this. [...] I don't see anything else from her. Fix it now.",
      surface: "sarah",
      verification:
        "bun test src/services/owned-renderer.test.ts and src/contracts/avatar-ux-contracts.test.ts inside apps/sarah (normal sweep); bun apps/sarah/scripts/sarah-avatar-e2e-smoke.mjs against staging/production for the live gate.",
    },
    {
      authorityBoundary:
        "Binds the /sarah browser surface and the speak bridge. Browser SpeechRecognition is the v1 transport; a native owned-ASR lane may replace it without weakening this contract, provided speech still reaches the brain and the unavailable case stays typed and visible.",
      blockerRefs: [],
      contractId: "sarah.avatar_hears_speech.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "apps/sarah/src/ui/avatar-session.ts",
        "apps/sarah/src/server.ts",
        "issue:#8621",
      ],
      oracles: [
        {
          description:
            "Surface source oracle: the owned session wiring constructs browser SpeechRecognition, forwards final utterances to the speak bridge (serialized so fast talkers cannot interleave turns), restarts recognition when the browser ends it, and surfaces a typed fallback card when recognition is unsupported or mic permission is denied — asserted against the UI source so a refactor cannot silently drop the mic path.",
          id: "avatar_mic_wiring.unit",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/sarah/src/contracts/avatar-ux-contracts.test.ts",
        },
      ],
      productArea: "avatar surface + owned renderer",
      source: {
        channel: "sarah-production-conversation",
        statedBy: "owner",
        statedOn: "2026-07-09",
      },
      state: "enforced",
      statement: "She can't hear a fucking thing I'm saying. Fix it now.",
      surface: "sarah",
      verification:
        "bun test src/contracts/avatar-ux-contracts.test.ts inside apps/sarah; runs in the package test glob and the repo test:sarah sweep before pushes to main.",
    },
    {
      authorityBoundary:
        "Binds session admission across apps/sarah and the hydralisk render service compat surface. Watched sessions (connected WebRTC peer) are never evicted; only peer-less abandoned sessions yield the slot. Capacity truth stays with the render service.",
      blockerRefs: [],
      contractId: "sarah.avatar_slot_never_wedges.v1",
      enforcementTier: "smoke",
      evidenceRefs: [
        "apps/sarah/src/ui/avatar-session.ts",
        "apps/sarah/scripts/sarah-avatar-e2e-smoke.mjs",
        "hydralisk:hydralisk/avatar/session.py#evict_one_stale",
        "issue:#8621",
      ],
      oracles: [
        {
          description:
            "E2E smoke: after an abandoned mint (no WebRTC connect), a second mint must succeed by evicting the stale session instead of returning avatar_session_limit; the surface must also send a stop beacon on unload so abandonment is usually explicit.",
          id: "avatar_slot_eviction.smoke",
          kind: "script",
          mode: "e2e",
          ref: "apps/sarah/scripts/sarah-avatar-e2e-smoke.mjs",
        },
      ],
      productArea: "avatar surface + owned renderer",
      source: {
        channel: "sarah-production-conversation",
        statedBy: "owner",
        statedOn: "2026-07-09",
      },
      state: "enforced",
      statement:
        "i click start conversation and theres some session post or something that took forever and then gave {...avatar_session_limit...} wht the fuck fix it",
      surface: "sarah",
      verification:
        "bun apps/sarah/scripts/sarah-avatar-e2e-smoke.mjs against staging/production; hydralisk-side eviction verified live 2026-07-09 (two back-to-back mints, second evicted the peer-less first).",
    },
  ],
  schemaVersion: BehaviorContractSchemaVersion,
}
