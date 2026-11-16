# Durable native transcript capture and expired Stop recovery

Checkpoint for [#13620](https://github.com/OpenAgentsInc/openagents/issues/13620).
The queue retains the primary transcript request before its read and every
admitted primary page before requesting another. A fresh worker reconstructs
the same owned `XhClient` from those bytes and their original acceptance times.
Its next request retains the read ID, digest and exact byte cursor. No Session
or borrowed descriptor is serialized and no transcript parser is duplicated.

One per-task reader replaces the previous reader churn. A claim reads at most
eight new pages. Partial expired, malformed or mismatched captures retire and
restart under that reader with a new read ID; different captures never share a
prefix. Complete publication still requires the whole digest and Session
decode, current enrollment and a fenced text append. A terminal chat waits for
a capture of that terminal native state. A crash between append and cache
retirement reconciles through the existing exact-prefix delta check.

Stop retries its retained primary command first. A native receipt reconciles
even after that command expires. Only an expiry refusal followed by a new
native status read can retain a replacement attempt; timeout, denial, conflict,
already-retained Stop and terminal state cannot do so. All prior Stop command
bytes remain in the private account/turn audit table. The task, original Start
and permission intents retain their identities.

## Qualification

Heavy checks ran on the owned Linux pool. Imported check counts overlap.

| Gate | Result |
| --- | --- |
| Worker/Stop policy | 229 checks; 51 changed-procedure mutants, 48 killed, three reviewed equivalents, zero unresolved survivors |
| Shared capture recovery | 260 checks; 60 changed-procedure mutants, 59 killed, one reviewed equivalent, zero unresolved survivors |
| Fresh database | 217 migrations; schema and migration hashes agree |
| Sanitized PostgreSQL/API/built CLI fixture | 126 durable-chat checks, zero failures; the complete selected API harness passes in 650 seconds |
| Existing signatures | 33,286 NIP-98 differential cases, zero disagreements |
| Existing API controls | 72 CLI-auth and 77 animation API checks pass, alongside route goldens and database invariants |

The native SDK is a controlled separate process using the production
preparation/task/capture stores and primary codecs. Its large transcript spans
claims: the first retains exactly eight pages and publishes no partial answer;
the next resumes and retires the whole cache. Authoritative completion likewise
waits across two claims and saves the exact final text once. The expired Stop
fixture seeds original primary command bytes, observes the native expiry
refusal, retains both attempts, then reconciles confirmed cancellation through
another claim. Stop remains usable after funding removal. Existing ownership
and replacement-enrollment refusals remain passing.

Receipts are in `/tmp/oa-chat-task-artifacts/`: `native-recovery-policy3.log`,
`native-recovery-policy3-detail.log`, `native-recovery-migrations.log` and
`native-recovery-api1.log`. Initial mutation runs found weak boundary assertions;
the final cases cover equality, exact integer-window limits, foreign/malformed
records, future cache acceptance times, complete terminal capture identity and
capacity bounds. Reviewed equivalents are independent primary-codec refusals
and an internal command construction that cannot fail after its preflight.

These are controlled native process and real database/HTTP/CLI observations.
They do not establish live provider inference, independent provider accounts,
installed mobile staging execution or the issue's complete end-to-end outcome.
#13620 remains open. No production deployment or TestFlight upload is included.
