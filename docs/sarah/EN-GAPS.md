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

## Active demand (SQ-7) — status as of 2026-07-09 (catalog v26 vendored)

| Gap | Real screen demand | Upstream status | Notes |
|---|---|---|---|
| `Host` kind `media-video` | `/sarah` owned avatar WebRTC / LiveAvatar attach | **shipped** → [effect-native#67](https://github.com/OpenAgentsInc/effect-native/issues/67) (v26, upstream `941acc8`) | `hostKinds` gains `media-video`; typed `MediaVideo` + DOM `makeMediaVideoDriver` with `onElement` attach seam. **Sarah converted**: the avatar pane is an EN `MediaVideo` host; `avatar-session.ts` acquires the driver-owned `<video>` and only binds the stream. |
| Streaming transcript primitive | `/sarah` partial-utterance live transcript | **already shipped** → [effect-native#35](https://github.com/OpenAgentsInc/effect-native/issues/35) (`Transcript`, v17) + #26 stream region | Keyed messages with the closed `thinking/streaming/failed/done` status set update in place — partial utterances are message-body replacements. **Sarah converted** off `List(pinToEnd)`+`Card` onto `Transcript` (Card bodies keep the exact visual); status wiring starts when the brain emits partial events. |
| Mic state + audio level | `/sarah` push-to-talk / VAD UI | **waiting** → registered in upstream `GAPS.md` ([effect-native#66](https://github.com/OpenAgentsInc/effect-native/issues/66)) | Typed proposal: `MicIndicator` with closed `idle/live/denied/muted` state + 0..1 level; `Meter`/`Badge` (#39) compose an interim. Enters when Sarah actually wires PTT/level metering. |
| Handoff / checkout / receipt cards | `/sarah` tool-effect cards | **waiting** → registered in upstream `GAPS.md` ([effect-native#66](https://github.com/OpenAgentsInc/effect-native/issues/66)) | Today honestly composed from `Card`+`Text`+`Button` (catalog-blessed composition, not a workaround). BM-4 uses that composition for Actions and Code/Receipts; a typed card family enters on second-surface demand. |
| First-contact AI disclosure banner | `/sarah` + landing first paint | **covered** → `StatusBanner` ([effect-native#40](https://github.com/OpenAgentsInc/effect-native/issues/40), v16) | The catalog piece exists. Sarah's disclosure currently renders as the server-page `header.sarah-disclosure` above the EN mount; moving it onto `StatusBanner` is a layout-level EN adoption step (copy unchanged), tracked with the shell, not a catalog gap. |
| GraphFigure semantic affordances | `/sarah` Blueprint map graph | **waiting** → [effect-native#68](https://github.com/OpenAgentsInc/effect-native/issues/68) | BM-2 ships on the existing `GraphFigure` v26 model. Follow-up demand: domain-neutral badge/accent slot, keyed node entry animation, typed pin/datum chips for evidence refs, and an `evidence_backed` edge status aligned with `arbiter-effect`. |

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
- Upstream process: `OpenAgentsInc/effect-native` `GAPS.md` (catalog v26+)
- Vendored snapshot manifest: `apps/openagents.com/packages/effect-native-vendor.json`
- EN-2 issue: OpenAgentsInc/openagents#8572
- SQ-7 issue: OpenAgentsInc/openagents#8624
