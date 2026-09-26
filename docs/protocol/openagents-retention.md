# OpenAgents relay retention

The relay stores protocol records. It does not execute them. The
[implementation coverage](2026-09-26-nip-implementation-coverage.md) separates
supported role validation from ordinary event storage.

## What is kept

| Kind | Class | Retention |
| --- | --- | --- |
| `30180`, `30181`, `30182`, `30184`–`30186` | Addressable | Latest accepted event for each author, kind, and `d` tag. Replacement does not run a program or install a release. |
| `30190`, `30192`, `30193` | Addressable | Knowledge heads, offering heads, and individually addressed quest versions. A discovery update does not rewrite an immutable entry or accepted agreement. |
| `3184`–`3194` | Regular | Accepted releases, revocations, runs, private artifacts, evaluation publications, knowledge entries, offerings, and XP awards remain independent records, subject to deletion and expiration rules. |
| `25900`–`25920`, `26900`–`26920`, `27000`–`27020` | Ephemeral | Fan-out only. The relay does not keep CJ job state. |

Regular events are not a permanent archive promise. NIP-09 deletion, NIP-40
expiration, operator policy, and relay loss can make a record unavailable.
Hosts retain the inputs, terms, journals, and unresolved obligations needed for
recovery independently of an ephemeral request or a mutable discovery head.

Private capability policy (`30181` with `oa:cap-policy:private:v1`) and run
records (`3187`, `30186`) use author/recipient visibility. Shared private
artifacts (`3188`) also have explicit visibility and encrypted envelopes.
Unauthenticated or unrelated readers get no match through guarded history,
ID lookup, COUNT, search, or live delivery. Those API boundaries do not hide
routing metadata or provide protection from an operator with database access;
payload confidentiality depends on the encryption and recipient keys.

## What an acknowledgment means

`OK true` means the relay accepted the event for storage or fan-out. It does
not mean a worker admitted the job, a program ran, or a release was installed.

`EOSE` ends the stored portion of that subscription. It does not establish a
complete run journal, close a structural gap, or prove an effect finished.
Pagination limits and NIP-67 hints remain relevant. An authenticated NIP-RS
snapshot is a separate bounded contract, not a stronger meaning for ordinary
EOSE.

NIP-40 expiration drops delivery of an event. It does not stop a process the
worker already started or release its unresolved accounting obligations.

## Advertised roles

`NOSTR_RELAY_OPENAGENTS_PROFILES=true` adds `nip-cap-v1`, `nip-prg-v1`,
`nip-ext-v1`, and `nip-run-v1` to NIP-11 `supported_extensions`. Leave it unset
until the relevant admission checks cover this build. Names are not numeric
`supported_nips`, and this switch does not enable a labor host or client control.

Public CAP, PRG, and EXT bodies are parsed at admission. The relay checks
private envelopes without decrypting their contents. Keeping a valid encrypted
record does not implement the host contract inside it. Private policy and run
publication require the event author's NIP-42 authentication. Check the
[current coverage](2026-09-26-nip-implementation-coverage.md) before advertising
any additional application role.
