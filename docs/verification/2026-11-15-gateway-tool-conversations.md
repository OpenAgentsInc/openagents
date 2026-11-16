# Shared gateway tool conversations

Source checkpoint for #13620, based on
`7b6c5a4ca89763c78a0fb6fd158ed254caeed582`. This extends the existing gateway,
Chat Completions adapter and rich chat DTOs. Published protobuf fields and the
existing durable-text route profile retain their identities.

The [contract](../protocol/gateway-tools-v2alpha1.md) records every bound and
ownership rule. Calls retain native IDs and complete original argument JSON.
Assistant calls and matching tool results serialize as provider-native history
for a subsequent model step. History rejects missing, foreign, repeated and
interleaved outcomes. Unsupported adapters refuse rich history explicitly.

One owned step assembles deltas and releases no calls before both a valid tool
finish and an independent successful transport completion. All eight physical
slots and exact string/output boundaries are exercised; no successful prefix
survives a malformed call, overflow or refused/truncated response. The result
error flag remains owned data, even though Chat's tool message has no separate
error field.

The codec reuses the existing JSON shape helpers and Responses JSON tree,
including decoded duplicate-key and Unicode validation. Giving that tree one
module owner resolves its `RS_BODY` collision with the public CLI without a
second parser or changes to either constant's meaning.

## Observed checks

Heavy commands run on the owned Linux pool. Earlier passing mutation results
are retained for unchanged procedure bodies.

| Check | Result |
| --- | --- |
| Shared tool wire first-pass gate | 122 checks; 151 mutants, 139 killed, 12 reviewed equivalents, zero unresolved survivors |
| Tool-history gate | 131 checks; 264 mutants, 251 killed, 13 reviewed equivalents, zero unresolved survivors |
| Tool-step gate | 136 checks; 398 mutants, 383 killed, 15 reviewed equivalents, zero unresolved survivors |
| Final gateway gate, including unsupported tool-type refusal | 166 checks; 428 mutants, 409 killed, 19 reviewed equivalents, zero unresolved survivors |
| Descriptor/canonical byte profile | Call, result and error result all pass against the existing `chat-turn.pb` descriptor |
| Final native fixture profile | Original evaluator plus debug/release, ordinary and ASan/UBSan all pass; 11 seconds on `oa-build-6` |
| Public CLI build | Passed, 97 seconds on `oa-build-4` |
| `psionic/tests/check_responses.sh` after JSON module ownership | 279 checks in each mode; HTTP/typed replay/SSE/limits/cancellation fixtures pass in native debug/release, ordinary/ASan/UBSan; book structure passes |
| Selected durable-chat API harness | Passed; PostgreSQL, ASan/UBSan, built signed CLI, native task/transcript adapters, health/migration checks and 33,286 NIP-98 comparisons with zero disagreements; 576 seconds on `oa-build-5` |
| Wire, tools and layer checks | Passed; final wire check has 395 registered shapes, no allowlist increase |

Equivalent sites retain local reasons: a wider decoding span still meets the
same semantic size bound; zero-length required fields still refuse; required
typed result members make one closed-object check redundant; an extra empty
pending slot cannot match a valid ID; assigning the current call count changes
nothing; and admitted nonempty object/array strings have at least two bytes.

The native fixture checks owned decoded strings after the source arena is
overwritten, failed transport withholding all calls, complete call/result
round trips, exact correlated continuation bytes, foreign result refusal,
every physical slot, maximum ID/name/argument/output extents, and malformed
duplicate-key and Unicode arguments. It invokes the shared codecs and the Chat
adapter directly; it starts no model or tool process. The existing native
qualification helper is unchanged.

An initial custom-suite invocation used the wrong check ABI. The check-only
emitter then explicitly refused dynamic borrowed strings (`E0413`); its
eligibility rules were preserved. The final explicit fixture uses the ordinary
product backend. A first product fixture imported the process transport and
failed linking its foreign symbols; loading the pure existing Chat adapter
instead fixed the dependency without introducing transport stubs.

The fixture lives with native tests in `coax/tests/gateway/`; the profile
explicitly runs its original evaluator checks before native compilation.
A generic suite mutation attempt at its original library location passed
172 checks but also mutated the fixture's own assertions and setup, retaining
39 such survivors. Those assertions are test code, never an execution policy
or product import. They were not marked as equivalent to hide that result.
Every changed product policy keeps the separate zero-survivor gates above.

The API snapshot contains the final shared JSON module ownership. The later
parser edit restricts an optional tool type to `function`; its separate final
gateway gate above passed. The harness's 72 CLI authentication checks passed.
The API and Psionic snapshot observations do not establish live provider
inference. A later unrelated main commit changes native chat controls; rebase
retains it without changing these library procedure bodies.

## Remaining integration

This checkpoint qualifies data ownership, provider serialization and complete
tool-step admission. It does not dispatch tools, validate a tool-specific schema
or establish current execution grants and retained effect receipts. The
existing `GwCall.finished` flag alone cannot seal a tool step: the transport
owner must supply a complete successful HTTP observation.

The legacy browser `ch_gather` in
`apps/openagents-web/chat/chat-hosted.coax` is an existing separate assembler;
its consumer must move to the shared `tool-step.coax` owner during integration,
retaining the existing command-table executor. It currently flattens results
into prompt text and does not consume the new correlated history.

The shared executor, durable rich transcript adoption, supported strict schema
semantics, provider accounts, durable server capture paging, expired Stop
renewal and a real staging coding task with an expected patch/test result remain
#13620 work. Its end-to-end acceptance boxes stay open. Production and
TestFlight remain deferred under the owner's full-roadmap release condition.
