# Live media controller

`live-host` implements the NIP-LIVE session controller. It covers participant
and recipient consent, bounded plans, transport credentials and media epochs,
input and speaking floors, captures with media anchors, transcripts, and
device input admitted against a fresh observation. Media, capture, and device
access go through explicit adapters (`Broker`, `Source`, `Device`). A room, a
recognized voice, a transcript, or a capture transport grants nothing on its
own.

```sh
live-host init --state DIR --key-file KEY --policy POLICY.json
live-host submit --state DIR --key-file KEY --input BUNDLE
live-host status --state DIR --key-file KEY --session ID
```

The controller key signs every result. POLICY names authorized owners and the
tasks that exist. BUNDLE carries one signed private `3188` event, its exact
artifacts, and further signed declarations, such as a participant's consent.
The binary mounts the synthetic adapters. A production host mounts its own
broker, capture source, and device behind the same roles.

## Rules the controller enforces

- **Plans.** The owner signs the plan, and it must name this controller and a
  task that already exists. The conversation scope must match its participants
  (`owner_private`, `direct`, or `group`). Bounds must be finite for wall time,
  storage, and concurrency. A joining client cannot choose the controller
  generation or the media epoch. A replacement names the exact previous plan
  and revision, advances the epoch by exactly one, revokes every old
  credential first, and requires fresh participant admissions. A replacement
  whose revocations cannot be confirmed is refused and leaves the session
  `unknown`.
- **Participants.** The owner admits participants; no one can admit
  themselves. A participant may withdraw itself. Each right must fit a track's
  direction: publish, record, and transcribe need capture tracks; playback
  needs a playback track. The policy's consent-requiring rights need the
  participant's own signed consent for the exact plan, rights, and tracks; an
  owner-signed claim does not count.
- **Credentials.** The broker mints a short-lived credential bound to the
  session, epoch, participant, tracks, and rights. Delivery is local and
  happens once. The controller keeps only the credential's digest, and tests
  check that no stored file contains the token. A credential cannot be redeemed
  by another principal or for another use.
- **Lifecycle.** The states run `admitted`, `active`, `paused`, `active`, and
  end in `closed`. Start and resume recheck consent and expiry. Stop revokes
  everything, and a revocation that cannot be confirmed leaves the session
  `unknown` with the unresolved items listed. An exact retry returns the
  original result; changed bytes refuse. Status reads do not change the
  revision. On disconnect, the controller follows the plan's pinned policy:
  pause or stop.
- **Floors.** An input floor needs the source participant's publish right on
  the capture track, an admitted destination, and admitted processing. Only
  one is active per destination and processing pair, so playback rights never
  become input authority. The input gate checks lease, participant, track, and
  epoch before each forward. Reconnection starts a new epoch and drops every
  floor, so audio from the old room and old leases is refused. A speaking
  floor needs a playback right, and only one exists per track. Barge-in
  revokes it and records the confirmed position and the uncertain tail.
  Playback never replays after reconnection.
- **Captures and transcripts.** Captures come only from admitted capture
  tracks. Recording needs bounded retention and recording consent from every
  participant on the track. Observations carry image geometry or a media
  interval and a signed freshness record. Bytes are retained only when the
  track's retention allows it. Transcripts need an admitted transform, the
  observation's clock domain, and segments inside its interval. Corrections
  are new revisions that point to their predecessor.
- **Device input.** A proposal is inert. The admission must pin the proposal
  through its POL action and carry approvals. The observation must still match
  the device's generation and focus version within its age limit. Dispatch is
  journaled before the input is applied. A lost reply stays `unknown` and is
  never re-applied. A changed admission for the same proposal is refused until
  the original is reconciled.
  The actual input call receives the frozen observation, binding, all input
  preconditions, and an absolute deadline. `Device::apply_checked` refuses by
  default; it never falls back to unchecked input. Adapters check generation,
  focus, and every supported condition at the write boundary. A known refusal
  records that no input was sent. Capture clocks start before capture and cannot
  be renewed by saving state, a later request, or restart; a new controller
  requires a new observation before device input.

All fixtures use synthetic devices, throwaway keys, and local state; no media
leaves the process and no money moves. Production media transports, platform
capture permission, and physical device bindings are not part of this crate.
