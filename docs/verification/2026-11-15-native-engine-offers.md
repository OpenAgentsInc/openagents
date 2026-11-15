# Live native engine offers

2026-11-15. Source checkpoint for
[#13620](https://github.com/OpenAgentsInc/openagents/issues/13620), following
transcript commit `eee1645bb70180c88319347145c0fe24097befa7`.

The router's family labels cannot be pinned as exact native model identities.
The older environment catalog is a static display snapshot. The new
[offer contract](../protocol/engine-offer-v1alpha1.md) reads the selected
account's actual SDK list without sending a task prompt. Signed HOST, the
account API and the public CLI share its one codec.

The native owner reuses the resident's four SDK slots, native account probes,
Claude/Codex initialization, polling and closure. Warming, ready and failed
observations have a fixed 30-second lifetime. Completed probes release the
SDK slot; an ephemeral 16-record cache preserves exact probe identity without
creating a task, session log or execution receipt. Polling does not extend
retention. Scope is checked on every read and while warming. Profile removal
is blocked while an SDK probe or admitted task still owns that profile.

The account bridge requires the verified owner of an exclusive enrolled
connection and correlates the whole offer request and host before returning
private uncached metadata. It does not require a task to exist and does not
grant execution authority. `openagents tasks models HOST --engine ENGINE
--provider-account ACCOUNT` waits for the typed observation using the existing
account profile and NIP-98 transport. The engine/account flags are script
controls; mobile gains no picker.

## Fidelity and qualification

Strict native Claude discovery retains exact IDs and supported efforts without
adding the engine's internal effort aliases. Native IDs remain opaque,
including long-context variants; the frozen Binding still determines which
IDs that execution version can select. Neither SDK publishes a truncated
string pool, unknown effort set or oversized list as a complete ready catalog.
Strict Codex discovery currently refuses a list with a continuation cursor.
That refusal is explicit and sends no task; completing shared SDK pagination
remains required before claiming the larger catalog profile.

| Gate | Observation |
| --- | --- |
| Owned descriptors and protocol checks | Generated the new owned/profile descriptors; owned schemas, consumer closures, pins and all frozen compatibility checks passed |
| Offer byte/descriptor oracle | Six independent exact request/warming/ready/failed fixtures passed recursively against the owned descriptor and sole codec |
| Offer wire authority | 150 checks; 368 changed-procedure mutants, 366 killed, two reviewed equivalents, zero survivors |
| Account task authority | 182 checks; the same 368 changed-procedure mutants passed with zero survivors |
| Both resident/Coder entry points | C generation passed with the existing 250-million-step compiler budget |
| Final signed native integration and host units | 99/99 signed task checks, 49/49 existing HOST checks and 1,358/1,358 native host checks; 265 seconds on the build host |
| Initial API, database and signed built CLI | 58/58 task cases and 71/71 existing history/Project cases; complete selected API harness passed in 615 seconds before the Nostr rebase |
| API and signed built CLI after the checksum repair | Combined API built and applied all migrations; 58/58 task cases passed; the selected ASan/UBSan/PostgreSQL harness completed in 486 seconds (487-second remote command) |
| Tools, wire and layer gates | Passed after the Nostr foundation rebase; 384 registry rows, no new allowlist entry or context exception |

Heavy commands run on the owned Linux build pool. The final native job records
SHA256 identities for both engine adapters, the native offer/task/profile
owners, the shared environment slots, fixture driver and signed task harness.
This prevents an earlier cached SDK build from qualifying later source edits.
All nine source hashes recorded by the final native job match the checkout.
Two added checks require strict adapters to retain SDK order, reject an
overlong Claude effort list and refuse a malformed Codex continuation cursor.
Curated display catalogs keep their existing ordering behavior. The full
signed task and legacy HOST journeys passed again after these adapter edits.

The first API run after the Nostr rebase compiled the combined API but stopped
at an upstream migration checksum mismatch before running HTTP cases. The
[separate record repair](2026-11-15-migration-0206-checksum.md), commit
`97e4369fb6`, preserves the shipped SQL and checker; the default check passed
against corrected main (214 migrations, zero failures).
The subsequent actual API run passed its native broker process oracle,
migration/schema checks, all 58 task HTTP/CLI cases and the harness's common
authentication/route regressions. The API fixture sources were unchanged by
the final native adapter review; the signed native run qualifies those SDK
edits independently. Full SDK pagination remains outside this checkpoint.

The native journey reads live Claude and Codex catalogs, reuses an identical
ready observation, refuses changed engine reuse and an unavailable profile,
then feeds the returned exact model IDs into the real signed preparation and
Start path. Its existing permission/file effects, transcript reconnection,
Stop and restart cases remain required. Actual provider fixtures count four
Claude prompts and two Codex starts across retries; warm-ups add neither.
A separate actual partial SDK response must fail with no model list or turn.

The API process oracle links the shared environment and Project wrappers under
ASan/UBSan, inherits SIGCHLD ignored, receives a typed 459-byte offer from a
separate broker child and verifies the disposition was restored. The HTTP/CLI
cases cover both engines, exact supported efforts, missing profiles, changed
reply identity, malformed input and cross-account refusal before the broker's
offer audit counter changes. Existing task and history tests remain in the run.

Initial validation found a fixture that incorrectly assumed Codex offered one
model and a boundary test whose reused catalog was already invalid before its
count check. The native assertion now checks both offered Codex models. A
fresh 34-case catalog boundary matrix proves count admission independently.
Only the zero-length effort decoding checks have equivalent annotations: an
empty effort is also refused by the final required identifier validation.
The shared fixture driver now retains child stderr for fatal bootstrap
diagnostics, without changing successful protocol stdout.

## Remaining issue scope

No acceptance checkbox is closed by this checkpoint. Server-owned automatic
routing, resolving its family/profile against live offers, retaining the final
Binding and task association with the admitted account/Project turn, installed
mobile task controls/rejoin, the shared gateway tool loop and a real staging
patch/test task remain #13620 work. Full SDK pagination remains explicit
discovery work. Static display catalogs cannot stand in for a live ready offer.

These are source and fixture qualifications, not external provider inference
or deployment. TestFlight remains deferred until the Claude-port issues and
remaining Arcade port satisfy the owner's release condition.
