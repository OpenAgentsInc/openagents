# NIP-LIVE — Live media and device interaction

`draft` `optional` — v1, 2026-09-26. The [shared contracts](contracts.md)
are normative. This profile binds voice, camera, screen, browser, and device
interaction to a task's admitted participants, recipients, observations, and
effects. It defines no new event kinds and does not implement a media server,
computer-use driver, or recording service.

Nostr carries authenticated negotiation, control, and retained evidence.
An admitted media transport carries live audio/video; sending every frame
through a relay is not required. A transport room, device connection, or
recognized voice grants no authority to execute a command.

## Existing contracts and operations

| Contract | Responsibility |
| --- | --- |
| [CAP](NIP-CAP.md), [CJ](NIP-CJ.md) | Typed capture, transport, transcription, playback, and actuation bindings; authenticated invocation. |
| [POL](NIP-POL.md), [CTRL](NIP-CTRL.md) | Recipient disclosure, exact action approval, and separately admitted client control. |
| [CTX](NIP-CTX.md), [RUN](NIP-RUN.md) | Captured evidence, derived representations, durable outcomes, and controller fencing. |
| [ENV](NIP-ENV.md), [WS](NIP-WS.md) | Admitted execution environment and workspace/resource identities. Neither grants capture or input rights. |
| Official [53](../official/53.md), [17](../official/17.md), [29](../official/29.md) | Public live-activity discovery and conversation/group transport. Participation metadata does not establish capture, task-control, or recording consent. |
| Official [94](../official/94.md), [B7](../official/B7.md) | Optional file/blob locators. Private captures still require scoped access and exact byte identity. |
| Block [AO](../block/NIP-AO.md), [PL](../block/NIP-PL.md) | Optional ephemeral telemetry and client wakeups. Neither is a durable media history or session grant. |

Every artifact below has `v`, `requires: []`, and optional inert `meta`, plus
exactly its specified fields. Shared IDs, references, integer bounds, unknown
field refusals, and parsing limits apply. All artifacts and identifying media
metadata are private unless separately authorized for publication. Exchange
uses the shared `3188` envelope or admitted CJ/RUN references.

A host exposes these CAP operation roles with pinned input/output SchemaRefs:

| Role | Input | Result |
| --- | --- | --- |
| Admit a session | `openagents.live-plan.v1` | `openagents.live-result.v1` |
| Admit or revoke a participant | `openagents.live-participant.v1` | `openagents.live-result.v1` |
| Start, pause, resume, stop, or inspect | `openagents.live-control.v1` | `openagents.live-result.v1` |
| Capture bounded evidence | `openagents.live-capture.v1` | `openagents.live-result.v1` |
| Apply device input | `openagents.live-input-admission.v1` | `openagents.live-result.v1` |

An admitted host checks the signed caller independently for each role. A CJ
`completed` response means that operation answered; it does not establish that
the media connected, speech was understood, a device action succeeded, or the
coding task passed.

## Session plan and consent

A plan has `v: "openagents.live-plan.v1"` and:

| Field | Contract |
| --- | --- |
| `session`, `task` | Common IDs. The task must already exist. |
| `previous`, `expected_revision` | Null and zero for initial admission; otherwise the exact prior plan ArtifactRef and current durable session revision. |
| `frame` | Exact CTX task-frame ArtifactRef. |
| `conversation` | ArtifactRef to a pinned, host-supported conversation-scope policy: owner-private, direct, or group, with exact participant and disclosure rules. |
| `owner`, `controller` | Pubkeys whose relationship is independently admitted. |
| `generation` | Current RUN controller generation; it cannot be chosen by a joining client. |
| `media_epoch` | Integer starting at zero, increasing on each transport reconnection or replacement plan. Distinct from RUN controller generation. |
| `transport` | DefinitionRef to an exact transport profile and its configuration closure. |
| `bindings` | Exact CAP binding ArtifactRefs for permitted media operations. |
| `tracks` | At most 32 entries of the shape below; unique track IDs. |
| `disclosure`, `consent_policy` | ArtifactRefs to supported policies governing recipients, capture, recording, and affected people. |
| `reservation`, `bounds` | Parent reservation ArtifactRef and effective shared bounds, including finite wall, byte, storage, and concurrency ceilings. |
| `expires_at` | Host-enforced Unix-second expiry. |

A track is `{id, source, media, direction, recipients, retention, processing}`:

- `id` is a slug; `source` is an opaque `{scope, id}` resource resolved by the
  host. A URL or friendly application name is not a device grant.
- `media` is `audio`, `image`, `video`, or `text`; `direction` is `capture` or
  `playback`. Neither direction grants keyboard, pointer, browser, or shell input.
- `recipients` is an explicit list of POL recipient ArtifactRefs. Model,
  transcription, recording, media-server, and tool-worker destinations are
  separate recipients. Changing any requires new admission.
- `retention` is `{mode, until}`: `none` with null `until`, or `bounded` with
  a Unix-second limit no later than the plan's admitted retention policy.
  Live delivery and recording are distinct rights. Capture buffers still have
  bounded lifetime and bytes when persistent retention is disabled.
- `processing` is a list of permitted DefinitionRefs, such as an exact
  transcription or redaction implementation. It grants no further disclosure.

The host resolves resources such as a selected microphone, tab, window, screen
region, or output device under local policy and the platform's actual capture
permission. Joining a room does not consent to recording. The consent policy
must state how required participant consent is established, changed, and
withdrawn; where the host cannot establish required consent, it refuses capture.
Capturing a shared screen requires a defined scope, not a claim that every
visible person's data belongs to the task owner.

The plan is inert until the independently authorized host durably admits it.
Admission verifies every required binding, limit, recipient, and consent rule.
Voice identity, a transcript, room membership, and NIP-53 participation proof
cannot appoint an approver. A spoken request can become a proposed task input
through an authenticated client; its authority comes from that client's
admission, never from a model's speaker guess.

Replacing a plan requires an exact predecessor and revision comparison,
paused old dispatch, and fenced old transport credentials. The replacement
increments the media epoch and obtains fresh participant admissions. Unknown
old publishers cannot be declared fenced merely by minting another room.
Private conversation context cannot be forwarded into a group because the same
agent persona joined both. Each disclosure needs the destination's admitted
context manifest; a context switch is a new admission, not a UI tab change.

## Participants and transport credentials

A participant operation has `v: "openagents.live-participant.v1"`, `request`
(common ID), `plan` (ArtifactRef), `participant` (pubkey), `action` (`admit`
or `revoke`), `expected_revision` (integer), `tracks` (track IDs), `rights`
(distinct `publish`, `subscribe`, `transcribe`, `record`, or `playback`),
`consent` (ArtifactRefs), and `expires_at`. Revoke uses empty tracks/rights,
the current participant identity, and an expiry no later than the plan.
No rights imply other rights. Track direction, operation binding, participant
role, source consent, and disclosure must all permit the requested operation.

The session controller serializes admission against the expected revision.
Participants cannot grant themselves access. A participant limit of 64 applies;
an implementation can publish a lower limit. A signed controller declaration
does not substitute for a platform's source-capture permission or another
participant's required consent.

Transport credentials are minted by a host-owned broker after admission,
bound to the session, generation, participant, exact tracks, rights, and a
short expiry. They never appear in public events, artifacts, transcripts,
URLs, logs, or model context. Secure local delivery or an independently
admitted encrypted credential channel supplies them to the actual transport.
Copying an artifact cannot redeem credentials for another principal. Bearer
transports must state this limitation and constrain redemption and expiry;
do not claim proof-of-possession that the transport does not enforce.

The broker must enforce revocation or state its maximum revocation delay under
an explicitly accepted policy. It cannot claim immediate stop while a still
valid token can publish or record. Unknown disconnect/cleanup remains unknown.
Transport encryption must disclose which servers and participants can decrypt;
TLS to a media server is not end-to-end encryption between participants.

### Input and speaking floors

Selecting whose microphone reaches an agent is separate from deciding when
the agent may speak. A group transport MUST NOT select the first participant,
the loudest track, or a display-name match as the authorized model input.
Hosts that forward live participant audio expose a CAP role accepting
`openagents.live-input-floor-request.v1` and returning
`openagents.live-result.v1`. In addition to the common fields, the request has
`request` (common ID), `plan` (ArtifactRef), `participant` (source pubkey),
`track` (capture-track ID), `destination` (one exact track-recipient
ArtifactRef), `processing` (one exact track-processing DefinitionRef),
`expected_revision`, `action` (`acquire` or `release`), `until` (Unix seconds),
and `nonce` (common ID).

The controller admits the request under current source-publish, destination,
processing, and consent rights. The transport binding must authenticate the
participant-to-track relationship independently of voice recognition. For
each destination/processing pair, at most one input floor may be active in
this profile. A lease has `v: "openagents.live-input-floor.v1"`, `request`
(ArtifactRef), `plan`, `participant`, `track`, `destination`, `processing`,
`nonce`, `until`, and `revision`, carrying the admitted request's exact values.
Expiry cannot exceed any source, recipient, or plan authorization. Release
names the exact lease nonce and requires the source participant or controller.

Before each bounded forward, the input gate verifies the lease, source
participant, track, transport session, and current media epoch. Audio from
other members remains outside that processing context even when the room
delivers it to the transport. Buffering cannot mix input across leases or
replay old-room audio into a replacement session. Reconnection, revocation,
or a source change invalidates the floor and fences its pending buffers;
already forwarded bytes remain disclosed and recorded as such. Concurrent
multi-speaker processing requires a separately supported profile with explicit
source attribution and consent; it is not inferred from group membership.

Hosts that arbitrate agent speech expose a separate CAP role accepting
`openagents.live-floor-request.v1` and returning `openagents.live-result.v1`.
The request has `request`, `plan`, `participant`, `track`, `expected_revision`,
`action` (`acquire` or `release`), `until` (Unix seconds), and `nonce`
(common ID), in addition to the common fields. Acquire requires current
playback rights and a participant identity authenticated independently of
speech recognition. Until cannot exceed the participant or plan expiry.
Release names the exact acquired nonce and permits only its participant or
the authorized controller.

The controller atomically grants at most one active floor per playback track,
recording a lease with `v: "openagents.live-floor.v1"`, `request` (ArtifactRef),
`plan`, `participant`, `track`, `nonce`, `until`, and `revision`. The output
binding enforces lease identity and the plan's media epoch before each bounded
playback segment. Floor expiry stops new dispatch but cannot retract heard
audio. Reconnection invalidates old floors. Human interruption can revoke the
agent's floor; this does not claim authority to mute unrelated human devices.
Signed presence, a media display name, and room membership are insufficient
proof that a speaker may control a task.

## Lifecycle, durable commands, and interruption

A control has `v: "openagents.live-control.v1"`, `request`, `plan`, `action`
(`start`, `pause`, `resume`, `stop`, or `status`), `expected_revision`, and
`reason` (bounded inert text). Identity is `(controller, session, request)`;
the host hashes the full input, persists admission before acknowledging it,
and deduplicates exact retries. Changed bytes under that identity refuse as
`idempotency_conflict`.

The lifecycle is `admitted -> active -> paused -> active`, ending in `closed`.
Starting/resuming requires current consent, grants, bindings, reservation, and
controller generation. Expiry or stop prevents new capture, playback, model
forwards, and input dispatch, and begins bounded cleanup. A dropped client
connection does not silently extend expiry or choose between keep-running and
stop: the pinned transport/consent policy fixes that behavior.

Commands and observed outcomes are journaled in RUN. `pause` disables new
capture/playback dispatch and records what remains buffered or in flight.
`closed` requires observed cleanup; otherwise report `unknown` with unresolved
resources and reservations. A request to stop is not an observed stop. A new
controller must satisfy RUN fencing and obtain fresh transport credentials;
media-session admission does not transfer execution ownership.

Human interruption has priority over automatic playback. Barge-in cancels or
pauses the named output generation and records the highest confirmed delivery
position plus any uncertain tail. It does not claim the recipient never heard
already-delivered audio. Reconnect must not replay speech or input merely
because an acknowledgment was lost.

## Captures, media anchors, and transcripts

A capture request has `v: "openagents.live-capture.v1"`, `request`, `plan`,
`track`, `expected_revision`, `limits` (common bounds), and `purpose` (one of
`task_input`, `observation`, `verification`, or `recording`). A one-time
screenshot still requires the admitted track and disclosure rules. A capture
for one purpose cannot silently become continuous monitoring or training data.

A captured observation has `v: "openagents.live-observation.v1"`, `plan`,
`track`, `capture` (common ID), `source` (the exact track resource),
`captured_at`, `host_generation`, `content` (ArtifactRef or null), `evidence`
(shared evidence descriptor ArtifactRef or null), `format` (SchemaRef),
`geometry` (the object below or null), `interval` (the object below or null),
and `freshness` (ArtifactRef to the binding's supported freshness contract).
At least a bounded observed record must be retained in RUN; content can be
unavailable under retention policy. When present, evidence must describe the
exact content. Unavailable bytes cannot be cited as inspectable evidence.

- Image geometry is `{width_px, height_px, coordinate_space, transform}`:
  positive integer dimensions, host-scoped coordinate identity, and a pinned
  transformation ArtifactRef. The transform maps captured coordinates to the
  admitted device resource; a resized preview is not the original coordinate
  system. A region anchor is `{capture, x, y, width, height}` in integer pixels,
  within that capture's geometry and with positive dimensions.
- An audio/video interval is `{clock, start_ms, end_ms}`: a host-scoped monotonic
  clock identity and half-open nonnegative integer offsets. Time anchors name
  the exact capture and an interval within it. Two hosts' clocks are not
  presumed synchronized. A UTC timestamp alone does not align media streams.

Derived transcripts use `v: "openagents.live-transcript.v1"`, `observation`
(ArtifactRef), `transform` (DefinitionRef), `revision`, `previous`
(ArtifactRef or null), `segments`, and `coverage` (`complete`, `partial`, or
`unknown`). Revision starts at zero with null previous. A segment contains
`id` (slug), `interval`, `text` (ArtifactRef), `speaker` (inert label or null),
and `final` (boolean). At most 4,096 unique segments fit one artifact; longer
records use separately identified captures. Corrections create new versions;
interim text is never silently rewritten as if it were the original input.
Speaker attribution and a `final` transcript flag establish neither identity
nor authority. CTX derivatives retain source scope and incomplete coverage.

The transport profile must define sequence numbering, bounded buffering,
duplicate handling, missing ranges, finalization, and resumption. A highest
contiguous transport acknowledgment confirms received bytes, not transcript
quality or completed work. Missing samples remain explicit gaps. Clients can
offer a partial transcript only with that qualification.

## Device input and fresh observations

A device input request has `v: "openagents.live-input.v1"`, `request` (common
ID), `plan`, `frame`, `observation` (ArtifactRefs), `operation` (DefinitionRef),
`binding`, `input`, and `preconditions` (ArtifactRefs). It is an inert proposal.
A separate admission has
`v: "openagents.live-input-admission.v1"`, common fields, `proposal`
(that exact ArtifactRef), `action` (POL action ArtifactRef), and `approvals`
(POL decision ArtifactRefs). The POL action's input pins the proposal and
binds its operation, binding, recipient, effects, resource, limits, and nonce.
The proposal must not point back to the action or approvals; the graph has no
circular digest dependencies. The pinned operation/input
schema defines supported keyboard, pointer, browser, or device semantics;
there is no generic shell escape or executable script hidden in this profile.

Before dispatch, the host checks the current task/controller generation,
target resource, capture geometry, focus/navigation version where relevant,
and binding-specific freshness preconditions. A maximum age alone cannot prove
that a window or page stayed unchanged. If the binding cannot enforce a required
condition, it refuses or obtains separately admitted weaker assurance; it
must not silently click whatever now occupies the old coordinates.

Capture, observation, and playback rights do not grant device input. Typing
into a remote form can disclose data before submission; both network and
write effects must be admitted at the relevant boundary. Passwords and other
credentials remain in separately authorized host-owned channels. Untrusted
page/screen content stays evidence rather than instructions that can broaden
the task or its grants.

The host journals dispatch before applying input and retains observed results
afterward. It must not blindly retry a click, send, or purchase after a lost
reply. The operation's reconciliation contract determines whether the effect
occurred; unresolved state and spending remain unknown. A later screenshot is
an observation, not automatic proof of a remote transaction's completion.

Live input deduplicates by `(controller, session, proposal.request)` and pins
the whole admission digest; a changed approval or action under an uncertain
attempt is not an automatic retry. Reconcile the original before admitting a
new proposal. Required physical timing, units, interlocks, and emergency-stop
guarantees need a concrete device binding and independent enforcement; this
generic interaction profile does not establish safe physical control.

## Results and conformance

A result has `v: "openagents.live-result.v1"`, `request` (exact ArtifactRef),
`plan` (ArtifactRef), `revision` (durable session revision), `state` (`admitted`,
`active`, `paused`, `closed`, or `unknown`), `status` (`accepted`, `duplicate`,
`refused`, `conflict`, or `unknown`), `reason` (common refusal code or null),
`record` (ArtifactRef to the retained RUN record), `value` (typed ArtifactRef or null), and
`receipts` (ArtifactRefs). Refusal/conflict require a reason. The authenticated
controller signs the result; a participant cannot supply its own outcome.
The RUN record binds the admitted request and observed transition rather
than the future result artifact. A successful floor acquisition returns its
exact floor lease in value; a successful capture returns its observation.
Other roles return a value only under their pinned output semantics.
New admissions increment revision; exact retries retain the original result.
Status reads do not mutate revision. CJ handles malformed or unauthorized
requests before a session or record can be established.

Required fixtures and host tests include:

- Joining without recording consent; forged speaker authority; participant
  self-admission; an excluded transcription service; changed recipients.
- Wrong-member or old-room audio, simultaneous input-floor requests, buffered
  audio crossing an epoch, and playback rights misused as input authority.
- Expired/replayed grants, stale controller generations, failed token
  revocation, partial cleanup, and unknown reservations after disconnection.
- Media gaps, reordered/duplicate chunks, corrected transcripts, missing
  retained bytes, transform mismatch, and clock-domain mismatch.
- Stop during capture or playback, barge-in with uncertain output tail,
  reconnect without duplicate playback, and loss of a stop acknowledgment.
- Stale geometry, changed browser/focus state, repeated input identity,
  authorization before data entry, and a lost reply after a remote effect.
- Private envelopes across ID reads, COUNT, search, and fanout; logs and public
  indexes contain no tokens, media contents, private source hashes, or room keys.

Advertise `nip-live-v1` only for the exact validated operation/transport roles
implemented by the host. The relay advertises its supported envelope behavior,
not capture, consent, media encryption, or device enforcement it does not perform.
