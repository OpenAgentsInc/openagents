# Khala Chat Streaming Experience

The `/chat` surface follows Shad's streaming-chat rule: never move the reader
against their intent. The scene, transcript, and composer are separate layers.

## Interaction Rules

- The composer owns the blue frame. Transcript messages must not render inside
  the composer box.
- Submitting a turn is explicit intent to move. Place the new user turn near
  the top of the transcript viewport so the reply can stream into the space
  below it.
- Streaming deltas do not auto-scroll. If the reader is not at the live edge,
  new text can arrive offscreen without stealing position.
- Provide a visible return-to-latest control while a transcript exists so the
  reader can jump back to the live edge on purpose.
- Preserve context above the active reply. Avoid full-bottom pinning that hides
  the previous turn.
- Keep controls native and accessible: keyboard focus stays where the user put
  it, the textarea remains a real textarea, and live/retry/error affordances do
  not shift layout unexpectedly.

## Visual Rules

- The transcript is an unframed reading layer over the Khala scene.
- Assistant replies read as prose first; avoid putting every assistant token in
  a boxed card.
- User turns can use compact right-aligned chips, but they should not dominate
  the scene.
- The bottom composer is a single command surface: one blue border, no nested
  textarea border, and an icon send control.
