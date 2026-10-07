# Verse networking, the Agent Studio, and NIPs

Status reconciled October 6, 2026. This guide distinguishes implemented
networking from the original October 4 convergence plan and future NIP mirrors.
The [current capability table](status.md) owns runtime status and measured
acceptance; the generated [runtime contract](runtime-contract.json) owns current
versions and Cargo feature declarations.

## The answer

Nostr cannot carry live, authoritative play by itself, and this repository
already decided so in several places. Nostr is the right layer for identity,
discovery, presence, permissions, durable records, asynchronous messages,
and wake-ups. Live simulation needs a direct authenticated channel to one
authority. That channel is not another outside system: the repository has
two implemented adapters: NIP-REACH channels and the chamber's TLS transport.
They carry the same chamber wire and command boundary. The independent host
uses TLS; OpenAgents integrates REACH and NIP-HOST admission.

Why not relays for live play:

- **Rate.** The relay admits 60 events a minute per key and 120 per
  address by default (`crates/nostr-relay/src/gateway/config.rs`). A presence
  session caps its durable states at 54 a minute
  (`crates/verse/src/session.rs`); its pose frames go on the relay's
  per-second pose lane (12 a second per key) instead.
- **Cost per message.** Ephemeral events still pass through a Postgres
  transaction and `NOTIFY` fan-out (`crates/nostr-relay/src/store/mod.rs`), a database round trip per
  pose.
- **Authority.** NIP-MV: "A signature proves who published a pose, not that
  the pose is honest. Worlds that need authoritative positions … need an
  authority that validates movement; this NIP does not define one." The
  engine architecture: "Nostr identity/presence and Tailscale routes do not
  substitute for world command authorization."

## Implemented networking

| Stack | Carries | Identity and admission | Transport |
| --- | --- | --- | --- |
| NIP-MV presence | Plaza poses, entities, gestures, world chat | Nostr keys; signatures prove publication only | Relay WebSockets |
| NIP-HOST and NIP-REACH | Coder tasks, terminals, the Agent Studio (`studio.*`) | Host-signed device grants with rights and revocation epochs | Same-machine control socket; REACH over iroh, TCP, or WebSocket; relay for CJ |
| Chamber service (`verse-world::service`) | 30 Hz combat or hosted social authority, replicated poses, progression, account and realm services | Content-bound Schnorr challenge and enrolled character role; REACH also requires the NIP-HOST `world` right and current grant epoch/generation | TLS TCP or REACH TCP/WebSocket; bounded correlated duplex requests, movement confirmations, and acknowledged spatial deltas |

The chamber's REACH adapter uses TCP or WebSocket; the host stack's iroh route
does not imply chamber iroh support. Browser clients use REACH WebSocket;
native clients can use TLS or REACH. The
[platform guide](platform-clients.md) records configuration and tested scope.
Realm listeners support independent instances in one bounded local realm.
TLS clients explicitly configure certificate trust and addresses. REACH binds
the channel to the host key and rechecks grant rights and revocation epochs;
the application can publish instance `worlds` in the existing host directory.
Joining still needs an enrolled world role. A directory entry or channel grant
does not assign a character or expose Studio panels.

## Convergence status and remaining proposals

Chamber and social presentation consume shared engine contracts and intent-only
commands. The original convergence decisions now have the following status:

| Decision | Implemented path | Remaining proposal or limit |
| --- | --- | --- |
| Host-authorized world access | NIP-HOST `world` grants, `service::reach`, per-request and timed epoch checks; configured TLS enrollment remains supported. | Static enrollment and REACH are explicit deployment choices, not interchangeable credentials. |
| Direct channel reuse | Same chamber frames on TLS and REACH TCP/WebSocket; shared client/runtime/session on native and browser. | Chamber iroh support and automatic route selection are not established. |
| Instance discovery | REACH directory entries carry `worlds`; the OpenAgents host can add its instance to an existing owner directory entry. | Public NIP-MV `33300` instance advertisements and a unified public world-selection flow remain proposed. Browser routes still use explicit configuration. |
| Content identity | Login binds the admitted content digest. Hosted Everglade uses its pinned pack digest; closed authored releases seal document, pack, scene, textures, and mips with compatibility admission. | Everglade VTP and chamber JSON packs remain distinct formats; a universal signed zone distribution format is not claimed. |
| Agent seat placement | `social::studio::StudioHost` owns shared seat motion from the co-located host snapshot; replicas draw public seat poses. | Remote Studio observation under an `observe` grant needs its own adapter; seat keys and public presence remain later work. |
| Social rules | `play::social` hosts bounded plaza/Everglade profiles without combat requirements, through shared realm, save, command, and replication paths. | General arbitrary worlds and larger social populations require separate admission and acceptance. |
| Panel disclosure | World walking rights remain separate from Studio `observe`, `operate`, and `review` checks. | Proposed NIP mirrors must preserve those disclosure boundaries. |

## Layering

| Layer | Carries | Mechanism |
| --- | --- | --- |
| Identity | People, devices, agents (and seats, once they have keys) | Nostr keys; Block NIP-OA for agent attestation |
| Discovery | Hosts, directory-bound world instances, the studio's host | NIP-REACH directory implemented; public NIP-MV `33300` instance advertisements proposed |
| Admission | Who may watch, act, merge, or join a world | NIP-HOST grants with rights and revocation epochs |
| Presence | Who is on the plaza and in which instance | NIP-MV `33301` and pose frames; an instance's own snapshot wins inside it |
| Live authority | World simulation; studio intents and updates | The chamber wire over a REACH channel; NIP-HOST `studio.*` over the same channel |
| Durable records and mirrors | Plans, reviews, landings, knowledge, XP | Private `3188` artifacts and public NIPs per the table below, never authoritative |
| Wake-ups | A decision waiting on a person | NIP-WS activity summaries and Block NIP-PL pushes |

## NIPs for the Agent Studio

NIP-HOST and the host's own task and studio state stay the only authority.
WORK and COORD say this directly: a board "is a projection, not a second
authority". Other NIPs are adopted as vocabulary or as mirrors. Codes:
**adopt** (on the wire now), **mirror** (record or publish beside the host
state), **later**, **no**.

| Studio concept | NIP | Decision |
| --- | --- | --- |
| Device rights: watch, act, merge | HOST `observe` / `operate` / `review` | Adopt (in place) |
| Live view | HOST `studio.snapshot` / `studio.update`; WS projection semantics | Adopt; align cursors, resume, and staleness with NIP-WS so a later WS row schema is mechanical |
| A decision waiting on the person | WS `activity-summary.v1` attention, Block PL wake | Adopt now: a studio decision raises its task's summary; the headline comes from host state (seat, task title), never engine text |
| Messages to a seat | SESS steering capability and acknowledgments | Adopt the semantics now: record each delivery's native mode and whether the engine consumed it ("an accepted steer is not a consumed one") |
| Questions and approvals | SESS `session-interaction.v1`; POL `approval-request` / `approval-decision` | Mirror; and close the gap below |
| Goal, plan, disposition | WORK `work-item` (project), evidence `plan`, disposition `accepted` / `revise` / `rejected` | Mirror |
| Task graph, claims, worktrees | COORD dependencies and integration enum; WS `worktree-binding.v1` | Mirror; per-task worktrees are COORD's isolated-workspace fallback |
| What a seat was shown | CTX selection receipt | Mirror |
| Repository conventions | KB entries (`3188` private, `3190`/`30190` public); seat-written entries stay `candidate` until reviewed | Mirror |
| Checks | Shared verification enum in RUN `resolved` and WORK `verification` evidence | Mirror; a green check alone never means accepted |
| Spend | POL `route-usage.v1` aggregated from ATIF metrics; Block AM `44200` per turn | Mirror; unknown cost is not zero |
| Effects and recovery | RUN journal; ATIF drives presentation only | Mirror |
| Seat identity | Block AP `30175` personas, OA attestation, GS-signed commits | Later, when seats get keys; the approved landing stays signed by the person |
| Seats on the plaza | NIP-MV `33301` `role: agent` | Later, after a disclosure decision |
| Studio work earning XP | NIP-XP | Later: needs a new rule (owner as referee, an accepted landing as evidence) |
| Model judgments at the oracle | DEC | Adopt where Jev checks a plan; not for human decisions |
| Paid labor | MKT, LAB, X402 | No (a v1 non-goal) |
| Raw agent telemetry | Block AO `24200` | No: it carries tool arguments and output the studio never discloses |
| Studio content in world chat | NIP-MV C7 | No: scope is a display rule, not privacy |

Two gaps, both closed (#10551):

- **Approval authority.** NIP-HOST says no HOST right approves a POL action,
  yet `studio.decision.answer` answered approvals under `operate`. The host
  now binds the answering device (key, grant, and epoch) as the approver of
  that exact step, bound to the task revision, turn, and run, and consumes
  the binding once when the task's command journal accepts the answer
  ([`studio_approvals.rs`](../../crates/coder/src/task/studio_approvals.rs)).
- **Kind `39005`.** Block NIP-CW uses `39005` for thread summaries, and
  upstream NIP-29 uses it for pinned events. Both meanings stay; the shape
  and the delivery path tell them apart, and the studio pins only through
  NIP-29
  ([the decision](../protocol/nip-expansion.md#kind-39005-pinned-events-and-thread-summaries)).

Step 1 also adopts the wake and steering rows above. A studio task's
question or approval raises its NIP-WS summary with a headline of its seat
and plan title; a goal's own decision raises a summary under a subject
derived from the goal, closed by a superseding summary once it is answered.
The summary sealed to a device is the NIP-PL wake, because the phone's push
lease matches the device's `3188` artifacts. Each message to a seat records
its native mode (`mid_turn` through the steer path, `turn_boundary` through
the next briefing) and whether the engine read it, and the seat panel shows
both. A steered message whose task ended unread returns to the seat's next
briefing.

### The Block lane as a mirror

Following "the relay is the workspace", the studio can also appear as
relay-native conversation: a private NIP-29 group per studio, a subgroup per
goal, a thread per task (seat and person messages as replies), the
repository as NIP-34 `30617` grouped by Block MP `30621`, patches and
statuses as NIP-34, review comments as NIP-22, and per-thread read state
with Block RS. These events mirror host state for people and other clients.
They never dispatch work: WORK says "a workroom mention MUST NOT dispatch
execution".

## Original convergence plan and implementation record

This October 4 plan retains the issue-level implementation record. The table
above describes current status; mirrors and later identity features remain
proposals unless their entry names an implemented path.

1. **Studio, now.** WS summaries and PL wakes for studio decisions; SESS
   steering acknowledgments for messages; the POL approver binding; the
   `39005` decision.
2. **One direct channel.** Run the chamber wire over a REACH channel,
   admitted by NIP-HOST grants; advertise instances in REACH directories;
   drop the out-of-band certificate. Browsers then connect over the WebSocket
   fallback. Implemented (#10552): `verse_world::service::reach` carries the
   unchanged chamber frames over a REACH channel (TCP or WebSocket), admitted
   by the new NIP-HOST `world` right (`coder_host::authority::WorldGrants`)
   and rechecked before every request and on a timer, and directory entries
   carry `worlds`. `openagents chamber host` serves it when its
   configuration says `"transport": {"type": "reach"}`: it opens the Coder
   host's access store and key the way `openagents host serve` does
   (`--state`, `--keys`, or `--keychain`), names the instance as the
   channel's generation, and, when its keys include the owner key, adds the
   instance to the host's existing directory entry. `openagents chamber
   --reach HOST` and the `verse_remote` example join as this device with the
   grant from the computers store. TLS stays the default, `verse_host`
   serves TLS only, and NIP-MV `33300` instance advertisements remain proposed.
3. **Shared Everglade.** A social rules profile in `verse-world`; the shared
   content digest; authority-owned seat actors fed by the studio snapshot;
   panel access checked against studio rights. Implemented in rules
   (#10553): `verse_world::social` holds the shared controller, navigation,
   heightfield, and solids, and `social::world` hosts a zone with no combat
   requirement; `Everglade::social_profile` carries the pinned pack digest
   (`everglade_pack::content_digest`) as the instance's content identity;
   `social::studio::StudioHost` walks seats from a snapshot source and
   publishes `SeatPose`s, which `Studio::follow_authority` draws; `world`
   alone opens no panel, and panels need `observe`, `operate`, and
   `review`. Hosted (#10553): `openagents chamber host` with
   `"profile": "everglade"` serves Everglade under the chamber's social
   rules (`social::hosted::everglade_profile`: the heightfield as one
   bounded mesh and one seat object per studio slot) and binds the pinned
   pack digest into the login challenge's content identity. Its
   `StudioFeed` reads the co-located Coder host's studio over the control
   socket (`studio.snapshot`, or `--studio-socket`), walks the seats on the
   authority's tick, and publishes them as the wire's public seat poses
   (`State::social.studio`), so every viewer sees the same seat at the same
   place. Desktop Verse joins with `--join FILE` (built with
   `remote-chamber`), records its grant's rights as the studio grant, and
   draws the host's avatars and seats. Reading the studio under an
   `observe` grant over NIP-HOST, rather than the control socket, is not
   implemented.
4. **Mirrors.** WORK, COORD, CTX, KB, RUN, and POL records beside host state;
   the Block-lane group mirror.
5. **Later.** Seat keys (AP, OA, GS), seats on the plaza, studio XP.

Acknowledged spatial deltas, movement prediction, and realm instance management
are implemented. Their [acceptance profiles](status.md) establish bounded
workloads; the combat battle campaign does not qualify a larger social population
or the proposed public discovery and mirror flows.


## Hosted social profile implementation

The optional `host::Config::social_profile` selects the closed v1 Plaza or
Everglade profile implemented by `verse_world::play::social`. These hosted
variants use the authenticated gateway's movement, seat/switch interactions,
scoped snapshots, and realm transfer. Profile geometry is included in content
identity and shared with native presentation. Local-only profiles keep their
existing rules; unsupported hosted profile names are refused.

`verse::hosted::Client` projects admitted snapshots into `WorldRuntime` after
explicit instance/profile admission. During this mode, `Session::tick_world`
retains discovery and suppresses NIP-MV crowd geometry. The runtime refuses local
placement, zone changes, and Studio operations. Public Studio poses come only
from the separately authorized host through the realm's local control channel.
Existing apps remain on their local flows until they explicitly attach this SDK;
this does not add join screens or convert imported artwork.

The [V07 audit update](../audits/2026-10-04-verse-engine-audit.md#v07-hosted-social-authority-is-implemented)
and [acceptance receipt](../../bench/verse/2026-10-04/social-authority/run.json)
record the supported bounds and remaining platform/performance work.


Chamber transport admission now partitions pending connections from admitted
capacity and retains principal request credit across reconnects. Scoped/full
projections and commands use separate bounded work buckets. Excess work returns
`rate_limited` before authority dispatch; callers must back off and retain their
operation identity. Standalone and realm listener statistics expose aggregate
refusal and connection outcome counters. [V08](../audits/2026-10-04-verse-engine-audit.md#v08-admission-and-request-work-have-bounded-policies)
records the policy, scratch acceptance evidence, and its throughput limits.


Realm wire version 25 separates account authentication from character selection.
A known account can observe the destination before selecting its owned dormant
character. `Client::select_character` uses an authority-chosen entry point;
`Client::logout` commits the current character and closes its session. Account
recovery requires the local operator control channel and current leases, preserves
character IDs and inventory, and refuses the retired credential. Unrelated
sessions remain live through admission, transfer, logout, and recovery.
The [V09 audit](../audits/2026-10-04-verse-engine-audit.md#v09-persistent-accounts-and-characters-have-a-recovery-contract)
records lifecycle, participation, recovery, and remaining operating limits.

The SDK worker backs off snapshot, inventory, and event reads independently for
100 ms after `storage_busy` or `rate_limited`, and stops after ten seconds of
continuous refusal. It never replays a command. Optional `worker::Observer`
telemetry retains at most 256 response observations and counts omissions without
blocking authority updates. Request turnaround includes queues, TLS, server work,
and validation; snapshot freshness starts at local verification. Neither is
isolated network RTT or one-way network age. The
[V10 audit](../audits/2026-10-04-verse-engine-audit.md#v10-frame-costs-have-separate-measurement-contracts)
records the CPU, GPU, capture, and transport measurements and their limits.
