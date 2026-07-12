# OpenAgents audio contract

`@openagentsinc/audio-contract` is the canonical Effect Schema contract for persistent voice. It owns control-frame meaning, lifecycle truth, identity fencing, retention receipts, and the rule that transcript/model/TTS prose has no command authority.

## Effect versus Rust decision

Use Effect for the protocol and state machine. Those values join Desktop, server, UI projections, typed actions, receipts, and tests; one TypeScript schema gives those consumers a shared vocabulary and keeps policy near the rest of the OpenAgents application. Rust is acceptable—and preferable—for a native media loop where predictable latency, device APIs, and memory control matter. The Rust boundary is therefore intentionally narrow: it may validate media identity, sequencing, codec metadata, payload length, and digest, then move bytes. It may not define transcripts, commands, Sync rows, storage policy, retention consent, or outcome truth.

This is not a performance verdict against Effect. It is an authority decision: Rust can be replaced without changing product semantics, while changing the Effect contract changes the product. AUDIO-4 may extend the Rust media helper but must not widen its authority.

## Compatibility

Unknown versions, tags, kinds, excess fields, oversized payloads, identity mismatches, and sequence gaps fail closed. Additive optional fields are allowed in v1 only when their meaning is public-safe and old readers can ignore them. Renames, removals, changed semantics or bounds, discriminants, identity/fencing rules, or binary layout require `openagents.audio.v2`; silent downgrade is forbidden. The cross-language golden corpus is normative.

Production binary media frames begin with ASCII `OAA1`, a four-byte unsigned big-endian JSON-header length, the canonical UTF-8 JSON media header, then raw payload bytes. Raw media never enters Runtime Gateway projections, Khala Sync, logs, analytics, traces, or support bundles.
