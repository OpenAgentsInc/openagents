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
