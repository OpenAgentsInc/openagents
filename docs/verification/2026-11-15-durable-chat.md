# Durable chat implementation, #13619

## Current checkpoint

Protocol, transactional API adapters, resident-worker dispatch/streaming and CLI
adoption are implemented. **#13619 remains open.** Native adoption, installed
mobile integration, live staging inference and the remaining restart/lost-worker
and legacy-continuation integration cases are still required.
No new TestFlight build or signed app update is published.
The final release waits for #13619–#13623 and the complete Arcade port.

The actual HTTP/CLI milestone now passes **41 checks**: save a queued turn,
recover a missed wake with the real runner, observe an actual provider delta
before completion, resume after its saved event cursor, reconcile a repeated
dispatch without another model call, Stop a running transport and retain its
partial answer, reopen and continue private Project chats through the built,
NIP-98-authenticated CLI. The fake provider observes exactly four calls across
the HTTP and CLI scenarios, including their retries; the existing inference
ledger meters each actual attempt once. This is real HTTP/Postgres/runner
integration with a deliberately delayed provider, not live Gemini or an installed
mobile flow.

The selected remote API command (`API_TESTS_ONLY=durable_chat_checks.coax,test_inference.py`)
completed successfully in **777 seconds**. It also passed the existing
inference/credit/drawdown cases, 72 common CLI-auth checks, 77 animation API checks,
schema matching for all 212 migrations, and the NIP-98 differential's 33,286
cases. The separately selected Project regression passed all 29 checks after
the shared context loader was factored out. Neither is a claim that the complete
API suite or #13619's installed acceptance is finished.

Shared `ctr_turn_read` and `ctr_page_apply` restore snapshots and apply whole
event pages atomically through the existing reducer. The CLI's `chat` domain
uses them for list/show/new/send/turn/events/follow/stop and legacy continuation.
Project instructions and selected knowledge still use the existing context
assembler; its byte-budget profile is shared without importing Project access
policy into Chat. The layer gate remains at 30 existing context crossings;
no allowlist was raised. The new budget helper kills all five of its mutants.

Changed input assembly: **257 mutants, 250 killed, seven reviewed equivalents,
zero unclassified survivors**. Stream adapter: **292 mutants, 286 killed, six
reviewed equivalents, zero unclassified survivors**. These cover independent
usage-presence bits, single-byte text/context, exact bounds, UTF-8/graphemes,
escaped provider strings, partial/error/unknown outcomes and cancellation.
The resource-reader audit covers all three changed procedures: **337 mutants,
329 killed, eight reviewed equivalents, zero unclassified survivors**. Its
independent malformed-field, coupled-cursor, stale-page, exact encoded-capacity
and historical-message cases retain atomic refusal and full text. Reader
validation is not inferred from the producer passing. Changed queue/grant
policy kills all **58** mutants; the renamed lifecycle transition kills all
**49** changed-scope mutants. Previously passing untouched scopes are retained.
The final assembled service passes **280 checks** in the evaluator. The final
generated account/Project units compile and link together, and both C adapters
pass strict syntax/warning checks. The moved Project wrapper has no remaining
mutation site; the shared context-budget implementation's five sites all die.

Initial integration failures were fixture errors: the disposable email allowlist
omitted the new addresses, the direct runner replay omitted `Nostr` in its
authorization header, the CLI assertion searched separate JSON deltas for one
concatenated answer, and the snapshot assertion expected instruction text rather
than its saved hash. All were corrected before the 41-check pass. Mutation
baselines also needed explicit evaluator budgets for the actual large legacy
and fully escaped reply fixtures; their assertions and byte ceilings were kept.

The API remains request-owned during a runner's inference request; phone reads
own no execution. Deployment configuration requests one always-CPU runner and a
360-second API request timeout. No service or app deployment has been performed
at this checkpoint.

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

## Earlier protocol checkpoint: next implementation boundary

Reuse the existing resident `oa-agent` service, whose CPU is allocated between
requests. An API child forked after a 202 on the request-throttled API service is
insufficient. Persist queue/input/claims/events first, fence dispatch before I/O,
recover committed queued work after a lost wake, and classify a lost dispatched
attempt as unknown. Then connect the same snapshots/events to CLI and native
chat, verify actual staging inference/reconnect/Stop and legacy continuation,
and only then complete #13619 and move to #13620.

## Earlier service and storage checkpoint

The additive `service.proto` owns control replies, conversation summaries/index,
event pages, runner requests and jobs. The resource codec also writes the
previously defined Turn, Message and Conversation records. Exact fixtures check
every envelope against the generated descriptor. No frozen conversation field
was changed. Internal page composers take fragments from these owned codecs.

The queue policy keeps claims queued until the current live dispatch fence is
admitted. An expired queued claim can be replaced; its original receipt remains
stable. A lost dispatched attempt is eligible only for unknown recovery. The
migration adds account-qualified foreign keys, a unique active conversation
turn, operation/ordinal/event uniqueness and lifecycle rows for all five tables.
Routes and the resident-worker adapter are still pending.

Validation retained for these scopes:

- All **144 codec checks passed**. The complete-reply check was subsequently
  strengthened and rerun alone: a 131,072-byte reply consisting entirely of
  six-byte JSON escapes encodes, decodes completely, fits its exact encoded
  buffer and refuses one byte less.
- Resource codec: **811 mutants, 800 killed, zero survivors, 11 reviewed
  equivalents**. Shared JSON helpers: **50 mutants, 49 killed, zero survivors,
  one equivalent**. Queue policy: **37 mutants, 36 killed, zero survivors, one
  equivalent**. Project wrapper change: **3 mutants, all killed**. These passing
  product scopes were retained when only test/documentation text changed.
- Updated independent native reader oracle: **2,865 cases**, no disagreement,
  compiled from the actual final readers with non-recovering UBSan.
- Pinned protobuf generation and compatibility/module/profile checks passed.
  Wire, layer and tool checks passed without increasing an allowlist.
- Selected API harness: **15 actual Postgres storage checks passed** for
  ownership/Project foreign keys, active-turn/operation/ordinal/event uniqueness,
  retained claim fences, account lifecycle registration and purge isolation.
  The regenerated snapshot matches all 212 migrations. The common NIP-98
  differential checked 33,286 cases without disagreement; the common CLI auth
  slice passed all 72 checks. This is the selected storage run, not the full API
  suite or a live durable-chat route test.

The initial resource mutation run had 75 survivors; independent ceiling,
malformed-peer, identity and writer expectations resolved its real gaps. Reviewed
equivalents describe duplicated bounds, impossible decoded states or JSON lengths
and exact identity checks after bounded decoding. They are not permission to
ignore later survivors.

Testing found that the shared UTF-8 helper's 64 KiB buffer could not validate a
full 128 KiB chat reply. Its bounded capacity now covers that span; the published
private-text Project profile retains its own 64 KiB limit. The first selected API
run stopped on the Project helper's legacy boundary assertion; that wrapper was
corrected before rerunning. No passing API-storage result is implied by that
earlier failed run.

Deployment integration must account for the runner's current minimum of zero
instances. CPU is already allocated outside requests, but queue recovery also
needs an available scanning instance. No deployment was changed at this
checkpoint; setting the minimum and verifying lost-wake/restart recovery belongs
to the actual worker integration.
