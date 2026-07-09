# Effect Native gaps filed by Sarah (SQ-7 / #8624 · EN-2 / #8572)

Sarah's surfaces must **not** invent a parallel design system. Missing catalog
pieces are demand for Effect Native — tracked here and in
`docs/effect-native/DEMAND_REGISTER.md`.

## Demand discipline

1. Name the real screen (public-safe).
2. Record the gap here + in the monorepo demand register.
3. File/update upstream `OpenAgentsInc/effect-native` GAPS / issues.
4. Vendor only after catalog version bump + renderer conformance.
5. Convert Sarah shell workarounds only after the component lands.

## Active demand (SQ-7)

| Gap | Real screen demand | Upstream status | Notes |
|---|---|---|---|
| `Host` kind `media-video` | `/sarah` owned avatar WebRTC / LiveAvatar attach | [effect-native#66](https://github.com/OpenAgentsInc/effect-native/issues/66) | Host kinds today: `code-editor\|terminal\|canvas` only |
| Streaming transcript primitive | `/sarah` partial-utterance live transcript | [effect-native#66](https://github.com/OpenAgentsInc/effect-native/issues/66) | Today: `List(pinToEnd)` + `Card` composition |
| Mic state + audio level | `/sarah` push-to-talk / VAD UI | [effect-native#66](https://github.com/OpenAgentsInc/effect-native/issues/66) | Idle / live / denied + level |
| Handoff / checkout / receipt cards | `/sarah` tool-effect cards | [effect-native#66](https://github.com/OpenAgentsInc/effect-native/issues/66) | Typed card intents, not free-form DOM |
| First-contact AI disclosure banner | `/sarah` + landing first paint | [effect-native#66](https://github.com/OpenAgentsInc/effect-native/issues/66) | Must stay first-class, not ad hoc Text |

## Filed by AV-5 avatar surface (#8598, 2026-07-09)

Historical detail (kept for provenance):

- `Host` kind `media-video`: catalog-blessed host for live media elements
  (WebRTC/`<video>`). Avatar video currently mounts in a sibling container
  outside the EN tree.
- Streaming transcript: role-tagged, auto-pinned, chunked/partial updates.
- Audio/level indicator once PTT lands on the avatar surface.

## Non-goals

- No Sarah-local React component library.
- No unversioned one-off primitives in monorepo vendored EN packages.
- No closing SQ-7 until these are **filed upstream** and linked here (component
  implementation may remain open upstream).

## Links

- Monorepo demand register: `docs/effect-native/DEMAND_REGISTER.md`
- Upstream process: `OpenAgentsInc/effect-native` `GAPS.md` (catalog v25+)
- EN-2 issue: OpenAgentsInc/openagents#8572
- SQ-7 issue: OpenAgentsInc/openagents#8624
