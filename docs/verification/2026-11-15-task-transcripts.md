# Account task transcripts

2026-11-15. Source qualification for
[#13620](https://github.com/OpenAgentsInc/openagents/issues/13620), following
account task control commit `183866be3b0abdb8c293f3d5abfaf8868ad27782`.

An account can now read an already admitted native task's complete captured
transcript through the public API and OpenAgents CLI. The resident supplies
the original Session JSONL, including full tool inputs, matching results and
held permission notices. The shared client verifies the entire capture before
publishing an inert Session or replacing an export. The sole TaskStatus still
owns lifecycle and permission identity.

This checkpoint does not complete the issue. Automatic account router dispatch,
offered-model resolution, installed mobile task controls/rejoin, the shared
gateway's complete tool loop and a real provider patch/test task on staging
remain acceptance work. Production and TestFlight are deferred under the
owner's release condition.

## Contracts and effects

The [transcript contract](../protocol/task-transcript-v1alpha1.md) defines a new
request, capture and page. It widens neither frozen TaskRequest nor SessionLog.
Every shape is registered with one codec. Exact fixtures independently hash a
UTF-8 JSONL prefix and split its pages at a real character boundary.

The resident uses its existing signed/encrypted HOST path, current grant and
task owner. A viewer has one immutable capture with a renewable 30-second
lease. Refresh replaces only that viewer's capture. Separate viewers retain
their own bytes and captured statuses while the native task changes. Bounds
are four readers per grant, 16 globally, 4 MiB per capture and 64 MiB total.
Expired ephemeral readers and empty grant directories are reclaimed; active
foreign leases are never evicted. Native task logs and execution receipts are
outside that cleanup tree.

The account API preflights current exclusive enrollment and conversation/turn
ownership before asking the broker for content. It checks the requested host,
task, reader, capture, digest, byte offset and whole immutable Binding. Cached
content cannot bypass revoked native access. Private responses are uncached.
The CLI uses the existing signed account transport. Manual page reads retain
their reader/read IDs and offset across reconnect; verified exports are
atomically replaced with mode 0600 only after complete digest and inert decode.

The existing Session writer now exposes an explicit overflow refusal for
complete-prefix owners. A failed task checkpoint preserves the earlier log
and retains a conservative failed/unknown status before closing its native
owner. An initial empty prefix is saved before provider effects. Fresh
permission responses are refused once Stop is requested; exact prior receipts
remain reconcilable.

## Gates

Heavy commands ran on the owned Linux build pool.

| Gate | Observation |
| --- | --- |
| New descriptor/profile and exact byte oracle | Passed; 86 owned schemas and all 30 frozen compatibility checks |
| Filesystem transcript oracle | 33/33; immutable paging, separate grants/viewers, UTF-8, lease/capacity/reclamation, 0700/0600 modes and a full 270 KB escaped tool result |
| Signed native task smoke | 88/88; real local relay, resident, native Claude/Codex SDK children, permission/file effects, Stop, restart and revocation |
| Existing signed HOST smoke | 49/49 |
| Host native unit checks | 1,343/1,343 |
| Shared environment/Projects C ABI and process oracle | Passed under the API harness's ASan/UBSan link; primary transcript page was 17,959 bytes for 28,483 captured bytes; inherited SIGCHLD ignore was restored |
| Built CLI, signed HTTP, actual API/DB and broker child | 50/50 task checks; 71/71 existing history/account/Project/routing checks; complete selected API harness passed in 486 seconds |
| Shared transcript client changed-procedure gate | 252 checks; 593 mutants, 565 killed, 28 reviewed equivalents, zero survivors |
| Transcript wire changed-procedure gate | 166 checks; 394 mutants, 373 killed, 21 reviewed equivalents, zero survivors |
| Account task policy changed-procedure gate | 172 checks; 396 mutants, 375 killed, 21 reviewed equivalents, zero survivors |
| Repository gates | tools-check, wire-check and layer-check passed; 365 registry rows, no allowlist increase |

The native smoke recovers held Claude and Codex permissions through the same
signed transcript method used by the broker. Completed views retain matching
call IDs and full inputs/results; Claude's following assistant text records its
continuation. Cancellation and owner-loss views retain their authoritative
canceled/unknown state. Native observations still count exactly four Claude
prompts and two Codex starts across retries and restart. Transcript reads add
no model or tool effects. Revocation blocks both status and transcript reads.

The HTTP/CLI flow reads a first page, resumes the next exact offset, retries
that page, verifies a full export, refreshes the same bounded viewer, refuses a
changed Binding and preserves the earlier export after refusal. Separate-account
and foreign-turn tests check the broker's transcript audit counter to prove
refusal occurs before capture delivery. The fixture's provider content is
synthetic; the signed native smoke separately qualifies actual SDK transport
and filesystem effects. Neither is an external model call or deployment.

Initial mutation findings exposed missing assertions for lease monotonicity,
duplicate-byte rejection, exact capacity, unpublished/malformed captures and
missing or unrelated permission notices. Checks were strengthened in the
shared owners. Secondary encoding refusals and equal lease assignments have
reviewable equivalent annotations explaining their preceding bounds; no
allowlist was expanded and product behavior was not rewritten to fit mutation.
Only checks and equivalent annotations changed after the passing HTTP/native
integration run. The earlier passing filesystem/profile results were retained
because their product bodies and fixtures did not change.

Maximum-size paging through production traffic limits, continuous mobile
refresh, installed external provider cancellation and live staging execution
are not qualified by these bounded fixtures. The 4 MiB limit is a retention
bound, not a throughput promise. A refused, stale or incomplete export never
replaces a verified one.
