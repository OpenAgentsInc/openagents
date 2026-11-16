# NIP-17 inbox verification

Implementation of [#13629](https://github.com/OpenAgentsInc/openagents/issues/13629).
The contract is [Nostr direct messages](../../protocol/nostr-direct-messages-v1.md).
The final device artifact is [build 1371](build-1371.md); upload and web
promotion receipts are recorded on the issue.

The implementation uses one Coax messenger and conversation view in the
native iOS app and `/messages`. Account-managed custody stays behind the
existing installed services and API. Device-held custody, public feed reads,
agent chats and environment sessions keep their existing owners.

## Completed checks

The receipts retain the source scope and distinguish compiler results from
reviewed constant equivalence.
Logs below live in the isolated `oa-arcade-dm` worktree's ignored `build/`.

| Check | Result | Evidence |
| --- | --- | --- |
| Actual PostgreSQL/API journal, custody, delivery and private read-position routes | 56 checks passed under ASan | `dm-api-final-focused.log` |
| Read-position wire codec | 147 mutants: 145 killed, 2 reviewed equivalents, no survivors | `dm-read-wire-qualified.log` |
| Read-position projection | 234 mutants: 228 killed, 6 reviewed equivalents, no survivors | `dm-read-projection-qualified.log` |
| Delivery observation state | 152 mutants: 150 killed, 2 reviewed equivalents, no survivors | `dm-delivery-state-transition-qualified.log` |
| Opened inbox admission and bounded chronology | 210 mutants: 207 killed, 3 reviewed equivalents, no survivors | `dm-inbox-final-qualified.log` |
| Public-key display adapter | 66 mutants: 58 killed, 8 reviewed equivalents, no survivors | `dm-public-key-qualified.log` |
| Changed installed program-size and HTTP admission rules | 14 mutants killed, no survivors | `dm-native-metadata-policy.log` |
| Immutable outgoing intent, recovery and sender-copy recognition | 244 mutants: 225 killed, 19 reviewed equivalents, no survivors | `dm-outgoing-recovery-qualified.log` |
| Extracted unsigned-rumor adapter | 63 mutants killed, no survivors | `dm-rumor-initial-qualified.log` |
| Extracted kind-10050 relay projection | 78 mutants killed, no survivors | `dm-receipt-baseline-and-relay-projection.log` |
| Extracted NIP-17 message projection, including exact rumor-size ceiling | 293 mutants: 287 killed, 6 reviewed equivalents, no survivors | `dm-message-projection-qualified.log` |
| Private read-marker API admission | 37 mutants: 36 killed, 1 reviewed equivalent, no survivors | `dm-read-policy-qualified.log` |
| Receipt cache | 256 mutants: 249 killed, 7 reviewed equivalents, no survivors | `dm-cache-final-qualified.log` |
| Journal replay | 513 mutants: 505 killed, 8 reviewed equivalents, no survivors; canonical shard records retained here | `dm-journal-merged-qualified.log`, `journal-1.rec`, `journal-2.rec` |
| Delivery receipt wire | 388 mutants: 380 killed, 8 reviewed equivalents, no survivors; canonical records retained here | `dm-delivery-wire-canonical-merged.log`, `delivery-1.rec`, `delivery-2.rec` |
| Compose intent | 96 mutants: 93 killed, 3 reviewed equivalents, no survivors | `dm-compose-final-qualified.log` |
| Store/custody coordinator | 536 mutants: 524 killed, 12 reviewed equivalents, no survivors; formally merged | `dm-store-final-merged.log`, `store-1.rec` through `store-4.rec` |
| Foreground/background scheduler | 604 mutants: 589 killed, 15 reviewed equivalents, no survivors; formally merged | `dm-scheduler-final-merged.log`, `schedule-1.rec` through `schedule-8.rec` |
| Authenticated relay coordinator | 953 sites: 925 killed, 28 reviewed equivalents, no unreviewed survivors | `relays-1.rec` through `relays-8.rec`, `relays-grouped-7.rec`; annotation provenance below |
| Messenger owner/flight coordinator | 513 sites: 504 killed, 8 compiler-marked equivalents, one separately reviewed folded constant (#13644) | `coordinator-1.rec` through `coordinator-4.rec`; scope notes below |
| Actual native recipient picker/composer and accessible delivery glyph | Passed on the isolated signed simulator | `dm-native-final-ui-tests.log` |
| Shared DOM adapter, including complete digest IDs, focus and field values | 19 checks passed | `dm-full-id-modules.log` |
| Compiled native/web/reference exchange, native cold history-row selection, exact committed-send retry | Passed, signed iOS simulator and real relay/API | `dm-id128-live-tests.log`, `dm-id128-live-reference.log` |
| Browser cold recovery through the real history row | Passed; both texts recovered | `dm-id128-cold-result.log`, [browser cold recovery](browser-cold.png) |
| Tools, wire registry and component layers | Passed | `dm-final-tools.log`, `dm-final-wire.log`, `dm-final-components.log`, `dm-final-layer.log` |

The receipt/coordination checks now also cover closed nested rows, independent
generation fences, all known open-refusal statuses, exact operation-hash
vectors, stale revision/offset replacement, native history dispatch, and the
independent lifecycle prerequisites. The shared coordinator's expanded ordinary
suite passed 1,105 checks (`dm-owner-gaps-expanded-tests.log`).

Initial mutation diagnostics used bounded shards and, for some runs, the
documented native mutation profile with a smaller evaluator slot budget.
Every check still runs; checks exceeding that budget must pass and run for
each mutant as native code. A timeout, failed baseline, or undecided site is
retained as incomplete evidence. None is treated as successful qualification.
Final journal/receipt passes also keep the compiler's killer hints for reuse.

Equivalent annotations name the specific redundant condition or unobservable
local cleanup they cover. They do not remove the production checks or cleanup.

## Live client work

The opt-in `NativeMessagingTests` integration case runs the normally compiled
Coax phone module through the installed native custody, journal, relay and HTTP
services. The browser uses the normally compiled `/messages` module. The
independent client uses `coax/tests/nostr/nip17_live_reference.py` and the
independent NIP-01/44/59/17 arithmetic in `nip17_reference.py`.

The fixture uses two disposable email accounts on a throwaway development API
and PostgreSQL database, plus a disposable reference key on
`wss://relay.openagents.com`. It never loads the owner's account, tokens, wallet
or identity backup. Development email codes stay in a private log; no email is
sent. The loopback proxy supplies disposable sessions to both SDK consumers.

The `--lose-seal-response` mode returns one 503 **after** the real server has
committed a seal, then checks that the retry's complete request is byte-identical.
It does not replace the custody, journal, cryptography or relay implementation.

The complete live case passed at 17:49 on November 15: native → web → native,
native cold recovery through the actual conversation row, then an independent
client's verified open and encrypted reply. Counts are exactly one body for
each message and exclude reply previews. The fixture confirmed both the lost
committed response and a byte-identical retry. A fresh browser also recovered
both messages by selecting the history row. The native test waits for the
actual recipient picker/title before interacting with it.

The fixture hit the development API's accumulated account-creation limit after
repeated interrupted test sessions. Only `auth.limit_counts` in the isolated
throwaway PostgreSQL database was reset before this final fixture run. No
production account, limit, key, inbox or service was changed.

## Defects corrected during verification

- Queued navigation now redraws the selected page after its current request
  drains; it cannot leave the previous private composer visible.
- A recoverable NIP-42 authentication closure no longer permanently marks an
  otherwise completed catchup as incomplete. Earlier genuine incompleteness
  remains retained.
- Native HTTP admission explicitly includes the two registered private metadata
  endpoints; custody and journal effects remain on their installed services.
- A send pressed beside an already queued SDK action retains its frozen intent
  until native entropy can be requested.
- An explicit uncertain-send retry can wait behind a relay effect without
  replacing that effect's request or generating another intent.
- Same-owner settings refreshes preserve current SDK requests and resume a
  paused messenger. They cannot silently change its owner.
- Shared DOM slot IDs now retain up to 128 bytes. The previous 64-byte bound
  truncated complete digest-based conversation IDs while drawing the full ID,
  so a visible history row could not dispatch its own selection. The fix also
  preserves complete field/focus IDs and prevents suffix collisions.

## Merged native regression checks

The app was regenerated on November 15 from the merged chat-composer and
Microduck/world sources at `46c331be3c`, retaining the native task controls.
The subsequent rebase onto `7b6c5a4ca8` retains the newer Codex catalog work;
those later changes do not change the iOS program or installed runtime. The native regression run covered 55 non-live cases. Two obsolete
assertions still attempted to search through the reference drawer’s already
removed field; those now check actual paging, retention of both computers, and
refusal of stale input. The focused correction passed both cases. Account
login, durable streaming/Stop, retained native task approval/retry, environment
commands, provider usage, key custody, feed and DM controls passed. The live
case is opt-in and its earlier successful separate run is recorded above.

The independent runtime oracle now allocates the same fixed 64 MiB carrier
as the installed runtime. Its old 32 MiB allocation failed before the first
frame against the current 39,273,640-byte account/task/inbox state. The Coax
state plus stack fits the installed limit; both app and extras now match the
independent WebAssembly frames and actions (`dm-final-runtime-oracle-memory.log`).
The installed limit was unchanged.

## API deployment

The isolated prepared API snapshot `6c8d5c9d29` differs from source
`1c4979ceee` only by the remote snapshot’s build ignore marker. Its verified
image is `sha256:b13b90c35331507df8d0cc375550cf9de96fb9cfb004642ad975b9069b8f23c1`.
Staging `oa-api-staging-00233-wj8` and production `oa-api-00170-5v8` passed
migrations and deployment smoke checks; both serve that same immutable image.
Both new POST routes also return 401 without authentication through the public
production origin (`dm-production-private-routes.log`). The API’s new
read-position policy had already passed the focused PostgreSQL and ASan suite. No production user keys or plaintext messages were loaded for
verification.

## Signed program resource bounds

Build 1371 has been archived and exported locally with two Xcode jobs. It
retains the current 1370 native composer/transcript styling. The exact archive resources are retained separately from
working build products under `~/.openagents/releases/arcade-dm-1371/`.
All world modules link without missing dynamic bindings.

The existing CNU1 per-resource ceiling remains 8 MiB. The installed loader now
uses `coax.native.program-parts.v1`: an 8 MiB first part and an optional second
part, validated by one Coax admission rule, joined as bytes before a single
UTF-8 decode. The package contains all four tails, including empty tails.
Non-executed WebAssembly parity artifacts remain in the app archive and are
omitted from the 21-resource signed update. The published CNU1 wire contract,
its 32 MiB total ceiling, and its 24-resource ceiling are unchanged.

The parts admission suite qualified 27 mutants (25 killed, two precisely
reviewed equality equivalents); the changed installed policy qualified 41
(39 killed, two reviewed equivalents), both with zero survivors. All 15 native
loader/package tests passed, including a multibyte UTF-8 character split across
the resource boundary. Independent WebAssembly and joined JavaScript app/extras
frames/actions match. Evidence: `dm-program-parts-qualified.log`,
`dm-parts-installed-policy.log`, `dm-program-parts-test-summary.json`, and
`dm-program-parts-runtime-oracle.log`.

## Coordinator scope records

The retained owner/flight records come from the same local source checkpoint
`0ff3b891c1` (original local checkpoint `ba21d7c0bd`). The rebase retained
these units byte for byte. Parts 1, 3 and 4 selected the entry unit with `--unit`; part 2
selected the entry-source procedures with `--root-only`, retaining all dependency
checks. These select the same 513 ordered entry-source sites, but the compiler
includes the scope spelling in its fingerprint. Their original fingerprints and
scope fields are retained; they are not presented as one formally merged record.
No source behavior or check was removed to combine the coverage. The subsequent
changes strengthen dependent oracles and do not change these entry procedures.

## Compiler diagnostic limitation

The coordinator's fixed-array `.count` receiver-index mutation is explicitly
reviewed in source: the receiver is not evaluated and both forms have the same
800000-byte type-level count. Its compiled native mutation switch is folded
away. One dependent secp256k1 table property exceeds the evaluator's traversal
budget, so the compiler labels this site undecided despite the equivalent
annotation. This is tracked in [#13644](https://github.com/OpenAgentsInc/openagents/issues/13644).
It is not recorded as a compiler-qualified kill or used to suppress any other
undecided site. All live security/ownership guards remain checked.

## Final qualification provenance

The store and scheduler canonical records formally merge at source
`33369b6eac` (original local checkpoint `3a7157b96e`); the rebase retained their
Nostr units and dependencies byte for byte. The original full relay records
at that same source had one surviving inner `error==NE_OK && verified` node.
Those flags co-vary in `ne_signed_fast`. The compiler changes the existing
left-associated AST node; it does not reparse source operator precedence.
The outer canonical-decode and public-key-equality AND nodes remain required
and are independently exercised by the signed wrong-key AUTH fixture.

The only subsequent relay edit adds precise equivalent explanations, with no
AST or check change. The affected part 7 was run at `63d35e217d` (original
`4d3952319b`): 116 killed, three reviewed equivalents, no survivors.
`relays-grouped-7.rec` preserves its different fingerprint. The original
and supplementary records are not presented as one formally merged receipt.
Every other native surviving diagnostic was reviewed; the strengthened tests
now kill previously hidden relay-count, slot-zero, list-ID, refused-ACK,
reset, HTTP-admission, cursor and priority changes. No product guard was
removed or rewritten for mutation scoring.

## Native integration after final rebase

On build 1371, `CoaxNativeTranscript` passed all 16 checks and
`CoaxNativeMessaging` passed 57 checks with one opt-in live-network case skipped.
This retains main's newer composer, message bubbles, tool disclosure and copy
behavior. The earlier separate live exchange remains recorded above; no claim
of a physical-device run is made. All five world modules link without missing
symbols. API, registry, component, dependency and tool checks are recorded
above. The production POST endpoint smoke and exact signed OTA resource
verification use public transport without loading an owner's private data.

## Release receipts

The API deployment is recorded above. The exported, signed client artifact,
runtime and resource hashes are in [build 1371](build-1371.md). The 1361
archive was superseded before Apple upload and is retained only as
[tooling evidence](build-1361.md). Deployment/upload acceptance is recorded on
[#13629](https://github.com/OpenAgentsInc/openagents/issues/13629), separately
from qualification and Apple processing. Physical-phone verification remains
an owner action in `NEEDS_OWNER.md`.
