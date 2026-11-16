# An owned chat turn's native task

`openagents.chat.task.v1alpha1` associates one saved account/Project turn with
one immutable native execution Binding. The API commits it before preparation,
input upload or Start. A repeated worker claim reconciles that association;
it never chooses another computer or sends the same turn as a fresh task.

The [schema](../../proto/openagents/chat/task/v1alpha1/task.proto) adds a
separate contract. Published chat, router and native execution contracts keep
their identities. Every top-level field is required; the object is closed.
Each nested record is read and written with its primary codec.
Server router station IDs are the first 32 hexadecimal bytes of the full
enrolled host key; colliding prefixes refuse. Display names come from the
account's connection. The retained native report keeps its original hostname
ID and canonical bytes. Neither a friendly name nor a hostname selects a grant.

| Field | Meaning |
| --- | --- |
| `schema` | `openagents.chat.task.v1alpha1` |
| `binding` | Exact native task, turn, session, route, command, model/effort, input digest and original logical window |
| `decision` | Server's router decision, retaining requested model/effort profiles and refusal/fallback reasons |
| `report` | Canonical authenticated report used by that decision; its host, time and SHA-256 match Binding |
| `offer` | Selected account's actual ready SDK observation; Binding's exact model and effort must occur in it |
| `enrollment_sha256` | Digest of the admitted connection, checked against the current connection before each native effect or native status/transcript read |
| `selection_reason` | Complete UTF-8 explanation, 1–1024 bytes; actual choices never inherit guessed model defaults |
| `dispatch_decision` | Original decision projected to the native report's station ID and Binding's exact model/effort; every other decision field is retained |

The enrollment digest is SHA-256 of the UTF-8 bytes
`openagents.chat.enrollment.v1\nHOST\nGRANT\nEPOCH\nRELAY`. Host/grant are
fixed lowercase hexadecimal identities; epoch is its canonical decimal value.
It grants no authority. HOST still checks current device/grant/workspace rights.
Changing an enrollment cannot transfer an in-flight task to a replacement grant.
The owner can still read the immutable association after disconnecting; that
historical record grants no access to a native task or transcript.

The API captures the complete native Input with the same saved history and
Project snapshot used for hosted inference, during turn admission. It never
reconstructs native input from a Project that changed later. An absent native
input on a historical turn is not permission to fabricate one.

`GET /api/chat/turn-task/v1alpha1?id=TURN` reads the private association. Existing
task status, transcript, permission and Stop routes operate on its host/task.
The association is retained even when the computer disconnects. A turn may be
preparing while its native Task status does not exist yet; a connection failure
is not a confirmed failed/canceled task.

Preparation and Start keep their original Binding/command/window on recovery.
Once Start might have reached its native owner, recovery reconciles that same
task. It does not reroute, renew its window or repeat the turn with new IDs.
Background worker scans continue after a phone disconnects; native tasks keep
their independent resident lifetime. Account ownership and the exact enrollment
are rechecked independently of Project membership.

`GET /api/chat/native-input/v1alpha1?id=TURN` reads the saved primary native
Input. Historical turns without such a record return 404. Both reads require
the account that owns the turn and use private no-store responses. The CLI
exposes them as `chat native-input TURN` and `chat task TURN`.

The durable runner now produces this association for an ordinary chat selected
for native work. It obtains current signed reports, uses the shared typed
router, reads the selected engine's actual SDK offer, resolves an offered
model/effort pair and saves the association before Prepare, input upload or
Start. The first consumer uses the selected engine's `system` account;
multi-account selection and SDK catalog pagination remain required work.
Router preview fallbacks cannot authorize execution.

The queue also claims associated running turns. Each claim uses a fresh worker
fence while retaining the original native IDs, admitted bytes and command
window. It observes the original Task before retrying Start, normalizes status
through the primary codec and refuses regressing or conflicting revisions.
The native phase is retained before Start, so a lost reply never licenses a
different task. A current-enrollment refusal after possible Start records an
unknown outcome rather than a confirmed cancellation or a new dispatch.

Native work renews the queue lease while making bounded broker calls. An
unfinished observation releases its lease and schedules another claim after
five seconds. The per-claim work budget is five minutes, independent of the
resident task's lifetime. Fresh Prepare/upload/Start effects recheck funding,
account/Project admission and Stop; status, transcript and Stop remain usable
after funding is removed. Every broker call still checks the current enrollment
and the native owner independently enforces its grant.

The runner reads verified transcript captures through the shared `XhClient`
and inert Session reader, then appends only assistant text to the saved chat.
Tool calls, results and pending permissions remain available through the native
Task/transcript routes. Stop intent is retained before its native effect;
retries use the same command. Completion, failure and confirmed cancellation
retain their actual native outcome. Missing provider usage remains unknown.
An assistant projection beyond the saved chat byte limit fails explicitly;
the full native transcript remains available.

The mobile account/Project chat opens the saved association from **Run details**
and shows its actual computer, engine, model, effort and task state. Reconnect
reads that same task and never submits another Start. Tool activity uses the
shared inert Session reader and shared tool/notice components. Input and output
are selectable, copied in complete UTF-8 pages of at most 49,152 bytes, with
explicit previous/next controls. Every tool retains its native call ID.

The mobile reader keeps one private reader identity per selected task, a fresh
capture read identity and the exact next byte offset. A transient page failure
keeps that cursor; expired or invalid captures restart the read. All capture
bytes and their whole digest must match before publication. Two caller-owned
Session pools retain the previous valid history until a complete new history
decodes. The existing 4 MiB capture bound refuses larger histories explicitly;
it never approves from a successful prefix. Account reset clears the complete
capture, decode and both publication buffers.

Allow requires the current waiting status, current connection availability and
a complete verified capture of that exact status with a pending permission.
Deny and Stop remain available when history cannot be read. A permission reply
pins its original task revision and interaction before requesting entropy. An
unknown HTTP result retains the exact command bytes; explicit retry sends those
bytes again. Read-only reconciliation clears the local intent only on an
authoritative native outcome or advancement of that original interaction.
Chat Stop retains its separate saved turn intent even while a permission reply
is uncertain. It retires an approval/denial that has never been offered to HTTP
and holds a delivered response for reconciliation, so a queued permission
cannot be posted automatically after Stop. No local button press confirms
cancellation.

Foreground reads use the existing installed one-second wait service, including
zero-delta frames, and pause under navigation, settings and backgrounding. The
generated account program has a fixed 64 MiB memory allowance (including its
8 MiB Wasm stack and approximately 34 MiB state) and a 16 MiB script admission
cap. These limits are separate from native SDK allocations.

Server capture assembly is currently bounded and restarts on a later worker
claim; durable server paging, renewal of an expired Stop command, SDK catalog
pagination/multiple accounts, shared gateway tool continuation, real provider
execution and end-to-end qualification remain part of #13620. Native C and
installed-interpreter fixtures do not establish production rollout or a real
provider execution; that live qualification remains separate.
