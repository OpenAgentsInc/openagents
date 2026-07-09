# Effect Native gaps filed by Sarah SM-2

Sarah's voice shell currently ships as a zero-React DOM implementation under
`apps/sarah/src/ui` while Effect Native component inventory catches up.

Demand for EN-2 (#8572) / GAPS.md:

- mic capture button with live/idle/denied states
- audio level meter (VAD-adjacent)
- streaming transcript list (user/assistant/modality)
- AI disclosure banner as first-class component
- handoff / checkout card primitives

Until those exist in the EN catalog, Sarah must not invent parallel React UI.

## Filed by the AV-5 avatar surface (#8598, 2026-07-09)

- `Host` kind `media-video`: a catalog-blessed host kind for live media
  elements (WebRTC/`<video>` attach targets). The avatar video currently
  mounts in a sibling container outside the EN tree because the host-kind
  set is closed (`code-editor|terminal|canvas`).
- Streaming transcript primitive: role-tagged, auto-pinned message list with
  live-append semantics (built today from `List(pinToEnd)` + `Card`; a typed
  transcript component would carry chunked/partial-utterance updates).
- Audio/level indicator (mic state, speaking state) — needed once push-to-talk
  or level metering lands on the avatar surface.
