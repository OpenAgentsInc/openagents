# Draft: ordinal protobuf error precedence (#5869)

Design and check matrix only, based on origin/main
`486594f300e2ff301c759360783c90c1b8390012`. #5869 remains blocked by
#161a. This proposal is not an implementation or an accepted API decision.
The review did not run tests. Generated DTOs are unverified projections;
none of these checks authenticates an event, identity, or authority.

## Failure and proposed rule

The shared protobuf errors are *ordinal values*: OK=0, WIRE=1, LIMIT=2,
DUPLICATE=3, UTF8=4, REQUIRED=5, ALIAS=6 and VALUE=7. They are not flags.
For example, a carried LIMIT and fresh WIRE currently yield 2|1=3,
which falsely says DUPLICATE. Analogously WIRE|UTF8 becomes REQUIRED.

Proposed rule: return the first nonzero error at the earliest documented API
stage, preserving its exact u32 value. For a method receiving an error-bearing
DTO, the DTO's carried error wins and its wire is not read or decoded. If the
carried error is zero, retain the existing decoder's first error, including
strict versus partial behavior and its existing wire-order/duplicate/canonical
precedence. A message's already-validated field search or iterator then reports
its existing scanner/cursor error, if reached. For nested message setters or
appends, first reject the parent carried error, then the parent partial-check
error, then the child carried error, then the child's strict decode error,
then rewrite/capacity/alias errors, then final parent-validation error. Do not
OR ordinal values or infer a priority from numeric magnitude. A carried unknown
nonzero u32 remains unchanged; do not silently turn it into an existing code.

This improves error identity and skips an unnecessary decode on a carried
error. It does not waive the existing live/stable/physically-valid borrowed-view
premises for normal reads, establish native pointer provenance, or promise
that output scratch is unchanged after a rewrite followed by final validation.
Bytes may have been written even when the final parent check refuses; only the
successful returned view is published. Do not imply all failed writers are
transactional.

## Complete generated-path inventory

The following points are emitted by `coax/lib/proto/importer.coax` at the
reviewed hash. Review the whole generator again against the commit actually
implemented, including templates added by other work.

| Surface | Current behavior | Proposed scoped behavior |
| --- | --- | --- |
| Singular scalar/string/bytes/message getters (`G_getter`) | Eager `decode(msg.wire)` then OR carried/decode errors | Check and return carried first; otherwise decode and return its exact error, then find the field. Preserve default value and `present=false` on error. |
| Repeated iterators (`G_iterator` init) | Same eager decode/OR | Same order, preserve `done=true`, `has_value=false` on failed initialization. Cover bytes/string/messages and packed/unpacked scalar values. |
| Iterator `next` | Copies old iterator, clears `has_value`, returns for `error!=0 || done`; later scalar/scan errors mark done | Preserve existing next-stage behavior and the exact carried error. A constructed `error!=0, done=false` currently keeps `done=false`; do not silently change its API without separately specifying it. |
| Real oneof selectors (`G_api`, `_case_*`) | Eager decode/OR | Same carried-first rule; error result is not-present. Test synthetic optional separately: it is presence, not a real oneof selector. |
| Unknown-field reader (`G_api`, `_unknown`) | Eager decode/OR; `PB_Field` error defaults `done=false` | Same carried-first rule **and preserve error with `done=false`**. `verse/home.coax` uses `if !Home_HomeConfiguration_unknown(config,0).done {return false;}`; changing failure to `done=true` could be mistaken for clean end-of-fields. Success must preserve unknown order, wire span, cursor and existing invalid-cursor handling. |
| Nested message set/append (`G_writer`) | Parent carried error then parent partial check; child decode happens before OR with carried child error | Preserve parent-first order; check child carried error before decoding child, then return strict child decode error. Test both singular set and repeated append. Clear has no child input. |
| Encode, clear, scalar/string/bytes set/append | Already explicitly check parent carried error before checks/rewrite | Confirm parity; do not accidentally change successful bytes, scratch/alias/error ordering, partial construction or checked writer behavior. |
| Decode / decode_minimal / recursive `__check` | Return first existing check error; canonical flag and field budget affect order | Preserve unchanged except independently approved validation fixes. Existing error precedence for duplicate-known before bad wire and other schema checks must be characterized, not silently rewritten. |
| `PB_find`, `PB_scan`, `PB_rewrite[_checked]`, `PB_copy[_checked]` | Helpers return their own ordinal; rewrite checks metadata before overlap, then scans/capacity, and may later write | Audit separately for accidental OR; keep cursor, boundedness and unknown-field behavior; distinguish physical pointer validity and host overlap observation from metadata checks. Do not broaden the promise of failure-atomic scratch. |

A source search at this base found five emitted OR templates: getter, iterator
initializer, nested-message writer, oneof selector and unknown reader. The
actual generated surface has many instances per descriptor/field. A grep only
shows this base and does not prove that external generated artifacts or future
templates are covered.

## Check-first matrix

1. For each carried u32 value 1..7 plus an unknown sentinel (for example
   `0xdeadbeef`), pair it with (a) valid complete wire, (b) malformed wire that
   would independently return WIRE, and (c) an empty/incomplete required message
   that would return REQUIRED. For getter, repeated init, real-oneof selector,
   unknown reader and child set/append, assert the exact carried code, no
   invented ordinal, and each surface's defined `present`/`has_value`/`done`
   shape. An OR result that aliases another named error must fail the test.
2. With carried error zero, independently exercise a valid message, a missing
   optional field, valid-empty repeated fields, and fresh WIRE, LIMIT,
   DUPLICATE, UTF8, REQUIRED and VALUE cases. ALIAS is an existing carried or
   output/overlap result, not a decoder-produced error; test it in its actual
   writer/copy path. Check that strict/minimal decoding retains documented
   duplicate, bad-wire, invalid UTF-8, budget and canonical ordering.
3. Nested writer matrix: parent-carried vs parent-partial-error vs child-carried
   vs child-decode-error vs output capacity/alias failure, including two errors
   present at once. Check exact first-stage winner. On carried/parent/child
   early failures, verify scratch canaries remain untouched. Treat a later
   rewrite followed by final-validation refusal as unpublished scratch, not a
   promise that no bytes were modified. Validate singular set, repeated append,
   clear and real-oneof replacement separately.
4. Exercise iterator init and next, including existing-error/done states,
   malformed packed data, empty packed segments, mixed packed/unpacked values,
   exhaustion and the actual error-result shapes. For `_unknown`, verify carried
   error has `done=false`, malformed wire and invalid cursor preserve their
   current results, normal end has `done=true`, and ordered unknown fields keep
   exact bytes. Run the home configuration consumer regression because it checks
   `.done` directly.
5. Use paired nonzero-error test objects with deliberately inaccessible borrowed
   wire only in controlled native guard-page/ASan and Wasm host fixtures, where
   the ABI permits constructing the view without dereferencing it: the carried
   error must short-circuit before decoding. Such targeted tests do not establish
   general foreign-pointer safety. Also test a valid view whose bytes remain
   immutable throughout each call and returned-view lifetime.
6. Exercise existing `ios-home`, `haunt`, `voice`, Nostr event and FI descriptors,
   plus a compact independent fixture containing scalar, bytes, string, oneof,
   nested and repeated/packed fields. For normal success and missing optional
   values compare exact old/new emitted wire, unknown siblings and independent
   protoc encodings. Verify generated C ABI headers/layouts and run relevant
   Swift/iOS app and simulator paths. Standard protoc does not define or enforce
   these project-specific ordinal diagnostics; do not use it as an error oracle.
7. Provide Coax direct checks and full changed-authority mutation with zero
   survivors and no waivers. Include negative controls that deliberately invert
   each precedence, merge with OR, or decode before carried-error check. Run
   checked/release native and Wasm, malformed/alias sanitizer controls, relevant
   iOS, and the Python-free compiler bootstrap on exact recorded sources/tools.

## Compatibility and frozen references

Changing which nonzero error is returned for a DTO whose carried error and its
wire both fail is intentional observable API behavior. Wire format, valid
encoding/decoding, message layouts, names and successful output should remain
unchanged; byte-level parity and ABI claims must be measured, not assumed.
Document that consumers comparing erroneous combined ordinal values may need to
update, and do not represent this as a protocol-level migration or verification.
The book currently prints an illustrative `error=msg.error|checked.error`
example (`coax/book/ch15-protocol.md`); update that example and explain the
precedence when implementation lands. Inspect the actual shipped Home, Haunt
and Voice call sites and external consumers, including `.done`-only checks.

Keep `coax/tests/proto/import-baselines.json`,
`coax/tests/proto/presence-baselines.json` and any subsequently landed frozen
baseline files byte-for-byte unchanged. Record new source and compiler/tool
hashes and add a separately named current output characterization, not a
replacement historical baseline. Verify a structured, descriptor-aware exact
old-to-new transformation against every pinned profile and independent protoc
metadata; ensure counts and fields match and all non-target declarations are
byte-identical. Include deliberate malformed transformation controls and
preserve the prior compiler/descriptor outputs. A new characterization generated
from the candidate is regression evidence, not by itself an independent
correctness oracle. Update existing authored generator assertions carefully;
never auto-rewrite them from the implementation and claim parity by that alone.

The earlier event-test examples that assert `N__pb_LIMIT|N__pb_WIRE` are useful
reproducers of the bug, but must be converted to assert the specified carried
error when this task is implemented. Do not alter the in-flight #161a frozen
mutation graphs or silently move this blocked task ahead of its prerequisite.
