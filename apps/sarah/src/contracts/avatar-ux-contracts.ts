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
        "Binds session admission across apps/sarah and the hydralisk render service compat surface. Every successful browser mint owns exactly one idempotent authoritative server stop independent from local media teardown. Watched sessions (connected WebRTC peer) are never evicted; only peer-less abandoned sessions yield the slot. Capacity truth stays with the render service.",
      blockerRefs: [],
      contractId: "sarah.avatar_slot_never_wedges.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "apps/sarah/src/ui/avatar-session.ts",
        "apps/sarah/src/ui/avatar-session.test.ts",
        "apps/sarah/scripts/sarah-avatar-e2e-smoke.mjs",
        "hydralisk:hydralisk/avatar/session.py#evict_one_stale",
        "issue:#8621",
      ],
      oracles: [
        {
          description:
            "Adversarial one-slot browser lifecycle oracle: constructor/acquire/start/attach/EventSource and owned peer-constructor/acquire/offer/remote-description/EventSource/peer failures each issue exactly one authoritative stop; successful cleanup permits the next mint, while stop 503 remains typed cleanup-unconfirmed and forbids remint. Post-handle attach/disconnect/peer terminals synchronously block the shared client replacement gate before cleanup settles and produce zero additional mint requests unless exact cleanup confirms. Repeated or late handle.stop joins that same proof, and false/throwing beacons use one keepalive fallback.",
          id: "avatar_slot_release.unit",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/sarah/src/ui/avatar-session.test.ts",
        },
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
        "bun test src/ui/avatar-session.test.ts src/contracts/avatar-ux-contracts.test.ts inside apps/sarah; bun apps/sarah/scripts/sarah-avatar-e2e-smoke.mjs against staging/production; hydralisk-side eviction verified live 2026-07-09 (two back-to-back mints, second evicted the peer-less first).",
    },
    {
      authorityBoundary:
        "Binds only browser-observable Sarah video health and its Effect Native presentation. A decoded frame on a live MediaStream video track grants one short browser-local transport lease; it grants no admission, capacity, provider, reservation, or cost truth. Text and any exact-scope Fleet authority remain independent and available while video recovers.",
      blockerRefs: [],
      contractId: "sarah.avatar_media_truth_never_frozen_live.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "apps/sarah/src/ui/avatar-media-health.ts",
        "apps/sarah/src/ui/avatar-session-attempt-gate.ts",
        "apps/sarah/src/ui/avatar-start-deadline.ts",
        "apps/sarah/src/ui/avatar-stop-deadline.ts",
        "apps/sarah/src/ui/avatar-video-latch.ts",
        "apps/sarah/src/ui/main.ts",
        "apps/sarah/src/contracts/fleet-continuity-projection.ts",
        "issue:#8610",
      ],
      oracles: [
        {
          description:
            "Deterministic fake-clock/video oracle: no decoded frame never becomes LIVE; a frame on a live video track leases LIVE only until bounded expiry; burst frames renew that internal expiry without projecting state at frame rate; the next frame after stale recovers; requestVideoFrameCallback has a currentTime-advance fallback; hostile clocks/listeners cannot emit invalid leases; and stop removes every callback, timer, and listener.",
          id: "avatar_browser_media_lease.unit",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/sarah/src/ui/avatar-media-health.test.ts",
        },
        {
          description:
            "Effect Native surface oracle: a text-live conversation with stale media renders VIDEO RECONNECTING plus one typed Reconnect video action, explicit accessible fallback copy, and leaves the composer and Fleet surface present; VIDEO LIVE requires the fresh typed lease variant.",
          id: "avatar_media_status_surface.unit",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/sarah/src/ui/surface.test.ts",
        },
        {
          description:
            "Attempt-fence and bounded lifecycle oracles: rapid reconnect actions remain single-flight; an older async completion cannot replace a newer attempt; hung start/stop work reaches a typed deadline, refuses replacement without wedging cleanup, and disposal permanently rejects late completion or restart; pending video-element acquisition is rejected on disposal.",
          id: "avatar_media_reconnect_fence.unit",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/sarah/src/ui/avatar-session-attempt-gate.test.ts",
        },
        {
          description:
            "Deadline and lifecycle units prove a never-settling start after a successful stop releases the interaction transition, visibly blocks replacement, fences its late handle, and runs that handle through bounded cleanup before retry is admitted.",
          id: "avatar_media_start_deadline.unit",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/sarah/src/ui/avatar-start-deadline.test.ts",
        },
        {
          description:
            "Deadline units independently prove stop success, failure, and timeout outcomes are bounded and non-throwing while retaining eventual stop truth after the deadline.",
          id: "avatar_media_stop_deadline.unit",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/sarah/src/ui/avatar-stop-deadline.test.ts",
        },
        {
          description:
            "Disposable video-latch units prove pending and future media-host acquisition rejects with fixed copy after surface disposal, so unmount cannot strand an avatar start awaiting a removed host.",
          id: "avatar_media_video_latch.unit",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/sarah/src/ui/avatar-video-latch.test.ts",
        },
      ],
      productArea: "avatar browser media health",
      source: {
        channel: "openagents-codex-thread",
        statedBy: "owner",
        statedOn: "2026-07-09",
      },
      state: "enforced",
      statement:
        "LIVE media requires an actual recent video-frame/transport lease, goes stale on bounded expiry, and exposes an explicit reconnect-media action/status while text and Fleet controls remain available.",
      surface: "sarah",
      verification:
        "bun test src/ui/avatar-media-health.test.ts src/ui/avatar-session-attempt-gate.test.ts src/ui/avatar-start-deadline.test.ts src/ui/avatar-stop-deadline.test.ts src/ui/avatar-video-latch.test.ts src/ui/surface.test.ts src/contracts/avatar-ux-contracts.test.ts inside apps/sarah; runs in the package and repo test:sarah sweeps.",
    },
    {
      authorityBoundary:
        "Binds the Sarah clip tier: the closed shippable catalog in apps/sarah/src/services/opener-clips.ts, the /sarah/api/clips routes, the mint greeting:\"client_clip\" option, and the browser clip layer. License law is part of this contract: only the raw MIT Hallo2 512\u00b2 renders are servable; the CodeFormer-derived *-sr.mp4 variants (S-Lab 1.0, non-commercial) are unrepresentable in the catalog and must never ship. The clip carries owner-approved scripted lines only \u2014 it grants no pricing or claims authority. Clip failure always degrades to the live/TTS greeting path (never dead air, never a double greeting).",
      blockerRefs: [],
      contractId: "sarah.avatar_opens_with_shippable_opener_clip.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "apps/sarah/src/services/opener-clips.ts",
        "apps/sarah/src/ui/avatar-clip-layer.ts",
        "apps/sarah/src/ui/avatar-session.ts",
        "apps/sarah/src/server.ts",
        "apps/sarah/scripts/sarah-avatar-e2e-smoke.mjs",
        "docs/sarah/2026-07-09-oav-quality-strategy.md",
        "issue:#8610",
        "issue:#8605",
      ],
      oracles: [
        {
          description:
            "Unit: the clip catalog contains only MIT Hallo2 renders (no *-sr filenames representable), the clips route serves video/mp4 with immutable caching and range support while refusing unknown names, a greeting:\"client_clip\" mint returns the opener clip and publishes ONLY the transcript line (no TTS request \u2014 no double greeting), a mint without an available clip falls back to the TTS greeting, and /api/avatar/greet restores the TTS greeting for clip-playback failures.",
          id: "avatar_opener_clip_tier.unit",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/sarah/src/services/opener-clips.test.ts",
        },
        {
          description:
            "Unit: the browser clip layer plays the opener immediately, fades in on playback, holds the final frame until live media is ready before crossfading out, plays canned clips over the live stream, drops the clip on user barge-in, degrades muted-autoplay and unplayable clips to the typed fallback callbacks, and is destroyed on session teardown.",
          id: "avatar_clip_layer.unit",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/sarah/src/ui/avatar-clip-layer.test.ts",
        },
        {
          description:
            "E2E smoke (live deployment): the clips manifest lists the shippable tier with no SR variants, the opener clip URL serves real MP4 bytes with immutable caching, and a greeting:\"client_clip\" mint returns the opener clip while the greeting transcript still lands on SSE within the deadline.",
          id: "avatar_opener_clip.smoke",
          kind: "script",
          mode: "e2e",
          ref: "apps/sarah/scripts/sarah-avatar-e2e-smoke.mjs",
        },
      ],
      productArea: "avatar surface + clip tier",
      source: {
        channel: "owner-directive",
        statedBy: "owner",
        statedOn: "2026-07-10",
      },
      state: "enforced",
      statement:
        "Get the Hallo2-quality pre-rendered clip technology working in the LIVE web /sarah surface ASAP \u2014 actually fucking working. [openers-v2 playback verdict, verbatim: \"those v2s are much better - opener-05-show-you-hallo2.mp4 is for example close to shippable so proceed in that direction.\"]",
      surface: "sarah",
      verification:
        "bun test src/services/opener-clips.test.ts src/ui/avatar-clip-layer.test.ts src/contracts/avatar-ux-contracts.test.ts inside apps/sarah (normal sweep); bun apps/sarah/scripts/sarah-avatar-e2e-smoke.mjs against staging/production for the live gate.",
    },
  ],
  schemaVersion: BehaviorContractSchemaVersion,
  version: "2026-07-10.7",
}
