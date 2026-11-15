# Durable chat dispatch through the native task owner

2026-11-15. Source checkpoint for
[#13620](https://github.com/OpenAgentsInc/openagents/issues/13620), following
the admitted input and association work in
`894d63d77db82fe7733d565a71139e322cd4c0cd`. The final HTTP snapshot was based on
`9beb63b50c5989ab8c9d545299bfc7265282475b`; subsequent upstream iOS/gallery
changes do not alter the qualified API sources.

An ordinary saved chat with connected computers now classifies native work,
obtains current authenticated reports, uses the existing typed router and
reads the selected engine's actual SDK offer. The exact offered model and
effort are retained in the immutable association and saved turn before any
native Prepare, upload or Start. Preview fallback decisions cannot authorize
execution. Ordinary hosted turns retain the existing inference path.

The runner sends the original admitted Input through the primary preparation
and chunk codecs. It retains a Start phase before the effect, then reconciles
the same task on a new queue claim. It never reroutes after an uncertain Start.
Status observations retain the complete binding and monotone revision, with
identical normalized bytes required at an equal revision. Current account
ownership, enrollment and native grants remain independent of Project access.

Each claim has a fresh worker fence; bounded broker calls renew its lease.
An unfinished native turn returns to the existing queue. Native task lifetime
is independent of that queue worker or phone connection. Stop intent is
retained before its native effect. Fresh execution still requires admission;
observation and Stop remain available after funding is removed.

The saved chat uses verified transcript captures through the shared `XhClient`
and inert Session reader. Only assistant text projects into the existing chat
events, with exact-prefix delta checks. Tools and permission prompts remain
in the primary native transcript. Oversized assistant projection fails
explicitly while the full task transcript remains available. Actual native
model/provider/effort are saved; unavailable usage stays unknown.

## Qualification

| Gate | Observation |
| --- | --- |
| New worker authority | 224 checks; 203 changed-procedure mutants, 198 killed and five reviewed equivalents, zero unresolved survivors |
| Primary descriptor/codec oracle | 24/24 checks, including binding a real offered pair to complete admitted Input and refusing unavailable preview fallback |
| Shared transcript projection | 257/257 checks; 104 changed-procedure mutants, 103 killed and one existing reviewed codec equivalent, zero unresolved survivors; every capture adapter reached |
| Primary TaskRequest C exports | 154/154 checks; 78 changed-procedure mutants, 77 killed and one reviewed equivalent, zero unresolved survivors |
| Initial PostgreSQL/HTTP/built CLI snapshot | 106/106 durable chat cases; complete selected ASan/UBSan API harness passed in 522 seconds |
| Final PostgreSQL/HTTP/built CLI snapshot | 118/118 durable chat cases, including Stop after funding removal, denial and replacement-enrollment refusal; 72/72 CLI authentication and 77/77 animation API cases; complete selected ASan/UBSan harness passed in 685 seconds |
| Database and signatures | All 215 migrations and schema snapshot match; 33,286 NIP-98 differential cases, zero disagreements; route goldens and all 24 room lifecycle checks passed |
| Wire registry | 390 rows, one existing allowlist line, zero failures; upstream servo additions retained during rebase |
| Layers/tools | Passed: 30 existing context loads, zero layer failures or increases; 2,034 tracked tools with no missing reason header; normal push gates run before landing |

The controlled native SDK fixture runs as a separate broker process and uses
the production preparation/task stores, primary status and transcript codecs,
and shared Session log. It commits Start and a waiting permission prompt,
then drops the first Start reply. New signed worker claims recover the same
task. The built CLI reads it, approves it, replays the retained approval and
the saved chat receives its complete final assistant text and actual model
choice. Before the separate Stop/deny cases, independent effect files prove
exactly one Start and approval across the lost replies and repeated claims.

This is controlled SDK process evidence, not provider inference or an installed
mobile flow. Existing unchanged protocol compatibility and admission/association
mutation results are recorded in
[the input checkpoint](2026-11-15-chat-native-input.md); they are not rerun here.
TaskRequest's new C exports use its sole existing codec with unchanged bytes.

The first pre-push correctly refused the capture suite: its adapter entry
points lacked direct unit coverage and skipping the first transcript part
survived. Added checks exercise all four adapters, first-item assistant text,
complete/incomplete paging, exact binding/hash correlation, empty output,
decode failure and output overflow. The focused mutation gate then passed.
Product procedure bodies are unchanged by this test-only correction, so the
passing sanitized HTTP snapshot was not repeated.

## Remaining issue scope

#13620 stays open, with its end-to-end acceptance boxes unchecked. Installed
mobile task/tool and permission controls, shared gateway tool continuation,
strict structured output, native SDK catalog pagination/account selection and
a real staging coding task with the expected patch/test result remain.

The first native dispatch consumer selects the engine's `system` account.
Transcript assembly is bounded and starts again on a later worker claim;
durable large-capture paging and renewal of an expired Stop command remain
required work. No production deployment or installed native execution is
claimed. TestFlight remains deferred until the owner's full Claude-port and
Arcade goal is complete.
