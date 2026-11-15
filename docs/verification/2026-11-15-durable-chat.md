# Durable chat implementation, #13619

## Current checkpoint

Protocol and shared client/server primitives are implemented. **#13619 remains
open.** Durable storage/claim/recovery, resident-worker integration, metered live
streaming, API routes, mobile and CLI adoption and installed end-to-end evidence
are still required. No new TestFlight build or signed app update is published.
The final release waits for #13619–#13623 and the complete Arcade port.

The [new protocol](../protocol/account-chat-turn-v2alpha1.md) defines rich
conversation/turn records, ordered content variants, submission and Stop,
explicit nullable usage and resumable events. The initial codec admits text
only and preserves the frozen v1 transcript/link contracts. Non-text schema
variants do not imply runtime support.

Checked Coax policy separates operation creation/replay/conflict, independent
admission facts, sequence advancement, dispatch, requested cancellation and
terminal unknown outcomes. The shared reducer preserves partial text and usage,
rejects gaps/wrong identities, ignores duplicate deltas and refuses overflow
without changing its prior cursor. SSE frames use the existing gateway parser
and its new bounded writer. Decoded views borrow an explicit caller arena.

Projects' bounded JSON/string/integer helpers were moved into
`coax/lib/json/shape.coax` and reused through behavior-preserving wrappers;
durable chat consumes the same helpers. There is no second JSON parser.

## Evidence

- Remote lifecycle suite: 7 checks; all 73 policy mutants killed. This file is
  unchanged since that passing run.
- Final shared wire/reducer suite: **124 checks passed**, including UTF-8 and
  encoded/decoded bounds, nullable fields, malformed events, all protocol
  spellings, source revisions, Stop, replay/gaps, cancellation and partial unknown
  outcomes. Actual native tests exercise both the codec and shared SSE parser.
- Independent Python standard-library JSON/Unicode oracle: **2,512 native ABI
  cases**, no disagreement, against the actual Coax-generated readers compiled
  with UBSan and non-recovering undefined-behavior checks. The first oracle run
  exposed its own missing unhashable-type guard; the corrected run passed.
- Shared JSON authority helpers and changed Project helper wrappers: 50 mutants,
  49 killed, zero unmarked survivors, one documented equivalent. Widening a digit
  bound to admit a colon still reaches `jv_int`'s own digit refusal.
- Changed SSE writer: 62 mutants, 46 killed, zero unmarked survivors, 16 documented
  equivalents. After the initial prefix append, its offset is -1 or at least 4,
  so the tested zero/one threshold changes have identical behavior.
- Exact submit/Stop/text-event/usage-event fixtures match the shared codec and
  generated protobuf descriptor. Pinned protoc 29.3 / Python protobuf 5.29.3
  generated the owned and selected descriptors and passed published compatibility,
  namespace, module and consumer-profile checks.
- Wire registry, layer and tool checks passed; no allowlist was raised.

The complete changed-procedure audit, including authority dependencies,
examined 1,137 sites: 1,071 killed, 50 documented equivalents and 16 remaining
survivors confined to the test-only escaped-key fixture constructor. Independent
literal/syntax controls were added for that constructor, followed by a targeted
check-unit audit: **39 mutants, 37 killed, zero survivors, two documented
equivalents** (an object member can never be node zero). Product procedures are
unchanged since the complete audit; their passing results were retained rather
than rerun. The final native reader was regenerated after the product changes
and passed the independent oracle and all 124 checks.

The earlier 160/119/54-survivor reports were intermediate findings, not passing
gates. Stronger independent wire/enum expectations and malformed-peer tests
resolved real gaps. Equivalent sites retain explicit arguments about duplicated
decoder/validator bounds, fixed-array capacities or impossible zero-length JSON
objects. No unclassified survivor remains in the verified scopes.

Detailed logs and the generated native reader are retained locally under
`~/.openagents/verification/durable-chat-2026-11-15/`. The corpus is synthetic;
no real account content or credentials are in the repository.

## Next implementation boundary

Reuse the existing resident `oa-agent` service, whose CPU is allocated between
requests. An API child forked after a 202 on the request-throttled API service is
insufficient. Persist queue/input/claims/events first, fence dispatch before I/O,
recover committed queued work after a lost wake, and classify a lost dispatched
attempt as unknown. Then connect the same snapshots/events to CLI and native
chat, verify actual staging inference/reconnect/Stop and legacy continuation,
and only then complete #13619 and move to #13620.
