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

Current implementation captures native Input at admission and validates reads
of retained associations. The routing/model-choice helpers and association
admission are implemented, but no producer yet inserts an association or
dispatches a native task from an ordinary chat. The preparation, Start and
recovery paragraphs above specify the required next integration. Installed
mobile controls, real provider execution and end-to-end qualification remain
part of #13620.
