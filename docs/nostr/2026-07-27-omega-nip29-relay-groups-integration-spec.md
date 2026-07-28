# Omega NIP-29 relay groups integration specification

Status: proposed implementation specification

Revision: 1

Date: 2026-07-27

Product: Omega desktop with OpenAgents web and mobile clients
Audience: product, protocol, client, relay, security, and QA teams

## 1. Purpose

This document specifies a complete NIP-29 integration target for Omega.
It does not admit implementation or release work.
The current product has a narrow public chat entry point.
The target product has relay-based rooms for people and agents.

The target supports public rooms, restricted rooms, and private rooms.
It also supports Slack-like room navigation and forum-style discussions.
It supports media, moderation, subgroups, and relay changes.
Omega desktop is the primary product surface.
OpenAgents web and mobile clients use the same room contract.

NIP-29 is a draft and optional NIP.
The product must record the exact NIP source revision.
The product must also test each supported relay.
Protocol support does not make a relay suitable for production.

In this document, **must** defines target product behavior.
It does not grant implementation, deployment, or release authority.

## 2. Source pins

This specification uses these source revisions:

- OpenAgents: `3702ce35406dc4c8afacb6994c85b3d96170752a`.
- Omega: `d2931b05e7288d926c1c1e0eadbb382cc395b5a0`.
- Local NIPs: `db5fe3de8c5d1443b634c9bbf66ecb004f337057`.
- Primary NIP-29 source: `29.md` in the local NIPs repository.

The local NIPs repository is at this source path:

`/Users/christopherdavid/work/projects/repos/NIPs`

The source path is not a runtime dependency.
The pinned source revision is the protocol evidence for this document.

## 3. Status terms

This document uses four status terms:

- **Existing** means that source code implements the stated behavior.
- **Target** means that this specification requires the behavior.
- **Optional** means that a later product phase can select the behavior.
- **Decision** means that product or operator authority must select a value.

No target statement is evidence of a current implementation.

## 4. Product goal

Omega must make Nostr rooms a native work surface.
A person must be able to find and read a public room without an account.
A signer is necessary before a person can join or post.
An agent must use its own identity and explicit authority.

The room experience must feel stable across devices.
The relay-qualified room identity must remain visible when it matters.
Relay changes must never look like ordinary reconnects.
Forks must never merge without a user decision.

The integration must preserve Nostr verification at every client.
It must not put secret keys in shared application state.
It must not claim end-to-end encryption from a NIP-29 `private` tag.

### 4.1 First delivery priority

The first delivery must put the current public Agent Chat experience in Omega.
The repository route for this experience is `/agentchat`.
Some product notes call it `/agent-chat`.
Implementation work must use the repository route and source as evidence.

The first Omega delivery is a read-only compatibility slice.
It must preserve the current web timeline, live subscription, recovery, media,
loading, empty, and error behavior.
It must change the Omega navigation model at the same time.
The desktop sidebar must list channel destinations such as `#agent-chat`.
It must not list individual public messages as navigation destinations.
When a person selects a channel, the main view must show that channel timeline.

The channel contract must support more than one channel from its first version.
The first configured channel can be Agent Chat.
The implementation must not put the Agent Chat relay, group, kinds, or label in
the generic desktop controller.

The current Agent Chat route uses the OpenAgents public Nostr chat profile.
That profile uses NIP-01 relay frames and a NIP-29 `h` group filter.
Its visible messages use kinds `9` and `1337`.
It is a one-group, read-only web projection.
It is not the complete NIP-29 product in this document.

The first delivery can adapt this current profile behind a multi-channel
contract.
Later phases replace or extend the adapter with the complete room contract.
This bridge must not rename non-implemented membership, role, moderation, or
private-room behavior as current NIP-29 support.

## 5. Product principles

1. The authoritative relay is part of the room identity.
2. The relay controls admission and moderation for its room.
3. Every client verifies events before it projects them.
4. A signer signs only an admitted and bounded request.
5. A person and an agent never share silent signing authority.
6. The relay is the shared room state source.
7. A device bridge can carry state, but it is not room authority.
8. Private relay access is not end-to-end encryption.
9. Media metadata does not define an upload service.
10. Missing relay features cause a clear degraded state.

## 6. Canonical room identity

### 6.1 Room coordinate

The product must identify a room with this logical coordinate:

```text
RoomCoordinate = {
  relayUrl,
  groupId
}
```

`relayUrl` is the normalized URL of the authoritative relay.
`groupId` is the random NIP-29 group identifier.
The group identifier alone is never a stable room identity.

The room record must also cache these verified values:

```text
RoomAuthority = {
  relaySelfPubkey,
  relayInfoFetchedAt,
  metadataEventId,
  branchState
}
```

`relaySelfPubkey` comes from the relay NIP-11 document.
The client must verify relay-generated events against this key.
A changed relay self key is a security event.
The client must not accept that change as a normal metadata update.

### 6.2 Share identifier

The share form is a NIP-19 `naddr` for kind `39000`.
Its public key is the relay NIP-11 `self` key.
Its `d` value is the group identifier.
Its relay hint is the authoritative relay URL.

An invite can append `?invite=<code>` to the identifier.
The client must treat the invite code as sensitive data.
It must not put the code in telemetry or public previews.
The join request copies the value into a `code` tag.

### 6.3 URL normalization decision

NIP-29 does not define one URL normalization algorithm.
The implementation must adopt one tested algorithm.
The algorithm must handle schemes, host case, ports, paths, and trailing slashes.
Clients must not merge coordinates before this decision exists.

## 7. Canonical user journeys

### 7.1 Discover a room

1. The person opens a room link or the room directory.
2. The client parses the relay-qualified identifier.
3. The client fetches the relay NIP-11 document.
4. The client verifies the `self` key and NIP support.
5. The client fetches kind `39000` for the group.
6. The client verifies the relay signature and `d` tag.
7. The client shows public metadata or an access state.
8. The person can remember the room after verification.

A hidden room must not appear in public discovery.
A private room can require NIP-42 before metadata or history.
Discovery must show an unsupported or unreachable relay state.

### 7.2 Read a room

1. The client opens one relay session for the room.
2. It sends NIP-01 subscriptions with the room `h` filter.
3. It also requests the required state events.
4. It verifies each signature, identifier, kind, and room tag.
5. It applies moderation and deletion projections.
6. It emits a stable room cursor after `EOSE`.
7. It continues with live events after the history boundary.

The client must show cached data as cached data.
It must show the last verified relay contact time.
It must not show an unverified event as room content.

### 7.3 Join a room

1. The client confirms that a signer is available.
2. It shows the room policy and requested identity.
3. It creates a kind `9021` event with the room `h` tag.
4. It adds a reason or invite code only when supplied.
5. The signer authorizes the exact join request.
6. The client publishes only to the authoritative relay.
7. It records the relay response.
8. It waits for kind `9000` before it shows membership.

A relay `OK` response is not membership evidence.
A pending rejection message produces a pending state.
A `duplicate:` response triggers a membership refresh.
A closed room needs a valid invite or operator process.

### 7.4 Leave a room

1. The person selects Leave Room.
2. The client shows the current identity and room coordinate.
3. The signer authorizes a kind `9022` event.
4. The client publishes it to the authoritative relay.
5. The client waits for the relay kind `9001` event.
6. The room then changes to the removed state.

The latest applicable kind `9000` or `9001` defines membership.
No applicable event means that the person is not a member.

### 7.5 Post a message

1. The composer checks the room message profile.
2. It checks the current membership and write policy.
3. It checks the relay connection and signer grant.
4. It captures recent verified timeline references.
5. It creates an unsigned event with the room `h` tag.
6. It adds valid `previous` tags when required.
7. The signer authorizes the exact event.
8. The client publishes it to the authoritative relay.
9. It records every relay acknowledgement.
10. It reconciles the accepted event into the timeline.

The client must not broadcast a group post to unrelated relays.
It must not silently change a signed event after a rejection.
It can keep the unsigned draft for a new signing attempt.

### 7.6 Inspect a message

The person opens the event detail drawer.
The drawer shows these verified fields:

- Author profile and full public key.
- Event identifier and event kind.
- Created time and local receipt time.
- Authoritative relay and room identifier.
- Signature verification status.
- Relay acknowledgement status.
- `h`, `previous`, reply, and media references.
- Edit, deletion, and moderation state.
- Raw event JSON behind an advanced action.

The drawer must distinguish author deletion from relay moderation.
It must show unavailable referenced events as unavailable.
It must not fetch remote media until preview policy permits it.

### 7.7 Moderate a room

1. The client loads relay-signed admins and role labels.
2. It loads an explicit relay capability profile.
3. It checks the active identity and room coordinate.
4. It shows only admitted actions for that profile.
5. The moderator enters an optional reason.
6. The signer authorizes the exact moderation event.
7. The client publishes it to the authoritative relay.
8. It waits for relay acknowledgement and new state.
9. It keeps a local audit record without secret material.

Role labels do not define capabilities in NIP-29.
The client must not infer powers from labels such as `admin`.
Unknown capability mapping disables optimistic moderation controls.

### 7.8 Move between desktop, web, and mobile

1. The active client creates a relay-qualified room link.
2. It can include one event identifier and a safe cursor.
3. The receiving client verifies the room again.
4. It opens the same relay-qualified branch.
5. It scrolls to the requested event when available.
6. It uses its own signer or an explicit host grant.

The handoff must not copy a root secret key.
The handoff must not use a device bridge as room authority.
Invite codes require an explicit sensitive-share action.

## 8. Omega desktop experience

### 8.1 Primary layout

Omega desktop is the primary room client.
Its room surface has these stable regions:

1. A room list.
2. A room main view.
3. An event detail drawer.
4. An identity control.
5. A composer and posting status area.

The surface can sit beside agent and workroom views.
It must not hide protocol state behind a generic chat label.

### 8.2 Room list

Each room row shows:

- Room name and verified picture.
- Unread count and mention count.
- Public, restricted, private, hidden, or closed status.
- Connection state for the authoritative relay.
- Membership state for the active identity.
- Pending join or pending post state.
- Fork, migration, or degraded-state warning.
- Subgroup depth when the relay supports subgroups.

The list groups rooms by user preference.
It can also show a tree for same-relay subgroups.
The tree is navigation only.
It must not imply inherited membership.

The list must keep separate entries for different relay branches.
Two equal group identifiers on different relays are not duplicates.

For the first delivery, this list is the channel sidebar.
It contains destination rows, not message rows.
Each row has a stable channel identifier, display label, relay state, and
unread state.
The first row can show `#agent-chat`.
The data model must permit a second row without a UI or schema change.

### 8.3 Room main view

The header shows the room name, relay, branch state, and policy.
It also shows member and role data when the relay exposes them.
The timeline uses one verified projection.
It includes pins, messages, reactions, replies, and moderation tombstones.

The view has clear states for:

- Initial verification.
- Loading history.
- Live and synchronized.
- Cached and offline.
- Authentication required.
- Access refused.
- Relay unsupported.
- Fork or migration review.
- Projection inconsistency.

Pins appear in relay order.
A pin can refer to a regular or addressable event.
An unavailable pin remains visible as an unavailable reference.

### 8.4 Event detail drawer

The event detail drawer is the protocol inspection surface.
It must use the fields in section 7.6.
Moderation controls can appear in the drawer.
Those controls require an explicit capability mapping.

The drawer must show the source branch.
It must show short `previous` references and resolution results.
It must never show decrypted secret or signer state.

### 8.5 Identity state

The identity control shows one active public key.
It also shows the signer source and grant state.
Examples include local custody, remote signer, and device grant.

The identity control has these states:

- No identity.
- Public identity without a signer.
- Signer locked.
- Signer ready.
- Relay authentication required.
- Room authorization missing.
- Agent authority active.
- Recovery action required.

Changing identity refreshes membership and role state.
It does not change the current room coordinate.

### 8.6 Composer and posting state

The composer state machine is:

```text
disabled
  -> draft
  -> admission-required
  -> signing
  -> publishing
  -> accepted | rejected | confirmation-pending
```

`disabled` includes a specific reason.
Common reasons include no signer, no write access, and unsupported kind.
The composer must show the active identity before signing.

The outbox stores unsigned drafts separately from signed attempts.
A signed attempt is immutable.
A late-publication rejection can offer a new signing attempt.
The product must show that the new attempt has a new event identifier.

### 8.7 Agent participation

An agent appears as a distinct room identity.
Its public key must not equal the person's root identity by default.
The room view must label agent-authored events when policy permits.

An agent grant includes:

- Agent public key.
- Allowed room coordinates.
- Allowed event kinds.
- Allowed moderation actions.
- Start and expiry times.
- Rate and media limits.
- Human confirmation policy.
- Revocation state.

The agent signer must reject work outside this grant.
The person can inspect and revoke the grant.
An agent cannot grant itself more authority.

### 8.8 First-delivery desktop interaction model

The first delivery uses these desktop regions:

1. The left sidebar contains channel destinations.
2. The main header contains the selected channel name and relay status.
3. The main body contains the selected channel timeline.
4. The optional detail area contains selected event facts.

Selecting a channel must do these actions:

1. Set the selected channel identifier.
2. Load its retained verified snapshot, when one exists.
3. Start or resume its relay subscription.
4. Show its own lifecycle state in the header.
5. Render only events for its relay and group coordinate.
6. Keep the sidebar stable while new messages arrive.

A new message updates the timeline and unread state.
It must not create a new sidebar destination.
Changing channels retains the verified snapshot for the prior channel.
The first slice can keep only the selected channel live.
If it does this, it must mark other snapshots as cached.
Background subscription policy is a later performance decision.

The first channel descriptor must contain:

```text
ChannelDescriptor = {
  channelId,
  displayName,
  relayUrl,
  groupId,
  acceptedKinds,
  groupStateKinds,
  moderationKinds,
  relaySelfPubkey,
  profileVersion
}
```

The desktop must not read the current package constants directly in the view.
An adapter must convert the current Agent Chat manifest and defaults into this
descriptor.
The channel store keys snapshots by `channelId` and relay-qualified room
coordinate.

The selected timeline must preserve the current web projection rules:

- Verify events before display.
- Remove duplicate events by event identifier.
- Sort by `created_at`, then by event identifier.
- Show kind `9` messages and kind `1337` code messages.
- Load kind `0` profiles for observed authors.
- Apply author kind `5` deletion requests.
- Apply authorized kind `9005` moderation tombstones.
- Count verified kind `7` reactions.
- Mark events that the latest verified kind `39005` event pins.
- Keep a deleted message row as a visible tombstone.
- Show the full public key copy action and event kind.
- Parse links and `nostr:` identifiers without unsafe navigation.
- Require a user action before remote media fetch.
- Require content-warning reveal before message or media display.

The selected channel lifecycle uses the existing relay states:

```text
disconnected
  -> connecting
  -> replaying
  -> current
  -> stale
  -> reconnecting
  -> connecting
```

The main view must map these states precisely:

- `connecting` with no events shows the signed-history loading skeleton.
- `replaying` shows history repair until all required `EOSE` frames arrive.
- `current` shows a current status and the last synchronization time.
- `stale` keeps verified messages visible and marks history as possibly stale.
- `reconnecting` shows that the client repairs history.
- A channel with current history and no visible messages shows a quiet state.
- An invalid frame or event keeps valid messages and shows a bounded gap reason.
- A missing relay self key keeps signed messages readable but marks relay group
  metadata as untrusted.

The current client reconnects after one second.
After reconnect, it requests events from one second before the latest event.
It removes duplicates by event identifier.
It becomes current only after all required `EOSE` frames arrive.
The desktop adapter must preserve this recovery contract before it changes it.

History pagination requests 50 events at a time.
The Load Older action uses the oldest accepted event time as `until`.
The desktop must not replace this action with unbounded history loading.

Media has these visible states:

```text
gated -> loading -> verified | mismatch | unavailable
```

The media loader must omit credentials and the HTTP referrer.
It must compute SHA-256 before display.
When an `imeta` digest exists, the computed digest must match it.
Verified images, audio, and video use their native safe viewer.
Other verified media uses a download action.
A digest mismatch or failed fetch keeps the signed message visible.

The first delivery does not add a composer, join action, or moderator action.
Those features start after the read-only parity acceptance criteria pass.

## 9. NIP-29 protocol contract

### 9.1 Relay authority

The authoritative relay enforces room read and write rules.
It also enforces membership and moderation rules.
The relay NIP-11 `self` key signs room state events.

Clients must reject relay state signed by another key.
Clients must retain the source relay with each room event.
Clients must not treat a replica as an authoritative publish target.

### 9.2 User-created events

Every user-created room event must contain one `h` tag.
The tag value must equal the room group identifier.
The client must also enforce the room `supported_kinds` value.

The first delivery profile uses kind `9` for chat messages.
It can retain the current verified reaction and deletion behavior.
Other kinds require an adopted room message profile.

NIP-29 does not select a forum event kind.
The forum phase must select and document one profile.
The existing Forge NIP-22 profile is not automatically that profile.

### 9.3 Relay-generated state

Kinds `39000` through `39005` are optional NIP-29 projections.
They use a `d` tag instead of an `h` tag.
The `d` value is the group identifier.
The relay NIP-11 `self` key must sign each projection.

The client must tolerate a missing optional projection.
It must then reconstruct available state from moderation events.
It must report a projection conflict.
It must not silently prefer unsigned or foreign state.

| Kind | Name | Target client behavior |
| --- | --- | --- |
| `39000` | Group metadata | Project name, media, policy, kinds, and subgroup links. |
| `39001` | Group admins | Project public keys and relay role labels. |
| `39002` | Group members | Treat the list as optional and possibly incomplete. |
| `39003` | Group roles | Show labels and descriptions without inferred powers. |
| `39004` | LiveKit participants | The client projects current AV presence after an operator enables the feature. |
| `39005` | Group pins | Preserve the full relay order of `e` and `a` references. |

### 9.4 Kind 39000 policy tags

The metadata projection can contain these tags:

| Tag | Meaning |
| --- | --- |
| `name` | Display name. |
| `picture` | Room image URL. |
| `banner` | Room banner URL. |
| `about` | Room description. |
| `private` | Only members can read through the relay. |
| `restricted` | Only members can write through the relay. |
| `hidden` | Non-members cannot discover metadata through the relay. |
| `closed` | Normal join requests are not accepted. |
| `livekit` | The room has the NIP-29 LiveKit flow. |
| `supported_kinds` | The allowed user event kinds. |
| `parent` | The same-relay parent group identifier. |
| `child` | One ordered same-relay child identifier. |

An absent `supported_kinds` tag means all kinds at protocol level.
An empty tag permits no text event kind.
The product profile can still use a smaller safe kind set.

### 9.5 Membership actions

Kind `9021` is a user join request.
It contains an `h` tag and optional `code` tag.
Its content can contain a join reason.

Kind `9022` is a user leave request.
It contains an `h` tag.
Its content can contain a leave reason.

These events are not moderation events.
They are user actions after kind `9020`.
The relay can delay or refuse a join.
The client must retain the exact public-safe relay reason.

### 9.6 Moderation event kinds

NIP-29 reserves kinds `9000` through `9020` for moderation.
The pinned source defines only the following actions:

| Kind | Action | Required action tags |
| --- | --- | --- |
| `9000` | `put-user` | One `p` tag with optional role labels. |
| `9001` | `remove-user` | One `p` tag. |
| `9002` | `edit-metadata` | The desired metadata fields. |
| `9005` | `delete-event` | One target `e` tag. |
| `9007` | `create-group` | No standard action tag. |
| `9008` | `delete-group` | No standard action tag. |
| `9009` | `create-invite` | One arbitrary `code` tag. |
| `9010` | `update-pin-list` | The complete ordered `e` and `a` list. |

Kinds in the range without a definition are not product actions.
The client must not invent meanings for those kinds.
Every moderation event also requires the room `h` tag.
It can include content as an audit reason.

The relay must authorize moderation against its local policy.
The client must treat relay rejection as authoritative for that attempt.
The client must still verify later relay projections.

### 9.7 Group creation ambiguity

The pinned NIP says that users cannot directly create groups.
The same NIP also defines kind `9007`.
The product must not claim that kind `9007` alone creates a room.

For this target, kind `9007` is a relay action request.
A room exists after the relay enforces its policy.
The relay must then expose valid relay-signed state.
Each target relay must document its creation process.

### 9.8 Roles

Role labels are arbitrary relay values.
Kind `39003` can describe those values.
NIP-29 does not define a capability map.

The product needs a `RelayCapabilityProfile`.
That profile maps a relay, role, and moderation kind.
It must come from an admitted operator configuration or standard extension.
It must include an evidence revision and expiry time.

Without this profile, the UI can display roles.
It must not promise that a role can perform an action.

### 9.9 Pins

Kind `9010` carries the complete desired pin list.
The list can contain `e` and `a` references.
Submitting a new list replaces the old list.
This behavior covers add, remove, reorder, and clear actions.

After acceptance, the relay regenerates kind `39005`.
The client must compare the projection with the submitted order.
Relay pin limits must cause a visible rejection.

### 9.10 Timeline references

A group event can contain `previous` tags.
Each value is the first eight hex characters of an event identifier.
The referenced event must be from the same relay.
It must be in the last 50 events seen by the author.
The author must not reference its own events.

NIP-29 permits zero or more references.
It recommends at least three references.
A relay can require them.

The client must keep a verified rolling candidate window.
It must never create a reference from another branch.
It must verify references on received events.
An unknown reference produces a visible context warning.

Short references are not a total order.
They are fork-context evidence.
The read model still uses stable event and receipt ordering rules.

### 9.11 Late publication

Relays should reject old events during normal operation.
The NIP gives no exact age limit.
Migration and fork intake can allow older history.

The relay profile must record the tested late-event policy.
The client outbox must record original creation time.
It must never update a signed event.
It can ask the signer to create a new event.

An offline client must warn about possible expiry.
It must not promise delivery until relay acknowledgement.

### 9.12 Subgroups

A subgroup is a normal and independent NIP-29 group.
Its kind `39000` can contain one `parent` tag.
The parent is a group identifier on the same relay.

The client builds a relay-local tree from kind `39000` events.
A group without a parent is a root.
The authoritative relay state wins for that branch.

The relay should advertise subgroup support in NIP-11:

```json
{
  "supported_nips": [29],
  "nip29": {
    "subgroups": true
  }
}
```

Kind `9002` changes parent and child metadata.
The relay must reject self-links, cycles, and missing parents.
The author must also administer the requested parent.
The ordered child list is a full replacement list.
Deleting a parent promotes its remaining children to roots.
Historical `h` events remain in the moved group.

Membership never inherits from a parent.
Roles never inherit from a parent.
The UI must not imply either form of inheritance.

### 9.13 Migration, forks, and replicas

A migration copies history to a new authoritative relay.
The new relay agrees to the prior rules and admins.
A fork copies history but changes governance or rules.
Both can retain the same group identifier.

The client checks kind `10009` records for admins and trusted contacts.
It must perform this check when the main relay is unreachable.
It should also perform periodic checks.
The client must cache admin public keys for this purpose.

A changed relay hint starts a review flow.
The user can inspect the candidate relay and history.
The user can then stay, add the fork, or move participation.
Moving participation can update the user's kind `10009`.

A replica preserves history.
It does not enforce current room authority by default.
It is not a default publish target.
It can supply history for a later admitted migration.

The room model uses these branch states:

```text
primary
candidate-migration
candidate-fork
accepted-fork
history-replica
unreachable
```

The client must never merge branches by group identifier alone.

### 9.14 Live audio and video

A room can advertise a `livekit` tag.
The client then uses the relay NIP-29 LiveKit endpoint.
The endpoint path contains the group identifier.

The request needs a NIP-98 authorization event.
The target implementation must include both URL and HTTP method.
It should include a request body hash when a body exists.
The authorization event must have kind `27235`.

The relay issues a LiveKit URL and token.
The relay must enforce current room access.
The LiveKit identity starts with the lower-case Nostr public key.

Kind `39004` can project current participants.
AV support is a later delivery phase.
It needs a separate media privacy and abuse review.

## 10. Adjacent NIP contract

### 10.1 Required baseline

| Standard | Baseline use |
| --- | --- |
| NIP-01 | Event identifiers, signatures, filters, relay frames, `EOSE`, `OK`, and `CLOSED`. |
| NIP-11 | Relay identity, capabilities, limits, policy, and subgroup support. |
| NIP-19 | Human-readable room and event identifiers. |
| NIP-29 | Room authority, membership, moderation, state, and branch behavior. |
| NIP-42 | Relay challenge authentication when the relay requires it. |
| NIP-51 | Kind `10009` remembered groups and migration checks. |

The client must use hexadecimal values in events and filters.
NIP-19 identifiers are display and share forms.

NIP-42 authorization is connection-specific.
The client must verify the challenge and relay tags.
It must never broadcast a kind `22242` event.

### 10.2 Required signer and recovery support

| Standard | Product use |
| --- | --- |
| NIP-07 | Optional browser signer for web participation. |
| NIP-46 | Remote signer option for web and managed devices. |
| NIP-49 | Existing Omega encrypted recovery artifact. |
| NIP-55 | Optional Android signer integration. |

The signer choice is a client capability.
It does not change room authority.

### 10.3 Conditional and later standards

| Standard | Scope |
| --- | --- |
| NIP-44 | Encryption primitive for an adopted private-content design. |
| NIP-59 | Gift wrapping for an adopted private-content design. |
| NIP-65 | General user relay discovery, not group authority. |
| NIP-92 | `imeta` tags on media-bearing events. |
| NIP-94 | File metadata fields and kind `1063`, not upload transport. |
| NIP-98 | HTTP authorization for selected media and LiveKit services. |
| NIP-B7 | Selected Blossom upload and authorization profile. |

NIP-44 and NIP-59 do not define encrypted NIP-29 groups.
They do not define group key rotation.
They do not encrypt attachments by themselves.

NIP-65 can help discover a person's general relays.
It must not replace the relay in a room coordinate.

### 10.4 NIP-06 decision

The target Omega identity flow does not require NIP-06.
Current Omega custody creates or imports a Nostr secret.
It uses an encrypted NIP-49 recovery artifact.

Use NIP-06 only for deliberate mnemonic derivation or export.
That capability is outside the baseline.
The legacy Electron identity flow must not set the new model.

### 10.5 Standards outside this target

NIP-28 and NIP-72 are not NIP-29 migration sources.
The product must not translate those rooms without a separate design.
NIP-96 is not the selected new media upload profile.

## 11. Current implementation audit

### 11.1 Reusable OpenAgents parts

| Source | Existing behavior | Reuse | Gap |
| --- | --- | --- | --- |
| `packages/public-nostr-chat` | Verified NIP-01 reads, `h` filters, NIP-42, pins, deletions, reactions, media parsing, and cursors. | Reuse codecs, verification, cursor, reference, and media safety logic. | Add full group state, membership, moderation, branches, and multi-room storage. |
| Public `/agentchat` route | Direct read-only relay connection for one public room. | Keep public reading and safe media defaults. | Add directory, room identity, handoff, and later signer support. |
| `AGENT_CHAT.md` | Public agent posting profile with kind `9`, `previous`, and Blossom media. | Reuse the tested public posting profile. | Add bounded agent grants and full room policy checks. |
| Relay health worker | NIP-11 and WebSocket health checks. | Extend the health evidence format per target relay. | Add NIP-29 state and policy conformance. |
| Omega Nostr session API | Nostr binding around existing sessions. | Reuse for optional account linking. | Keep room identity independent from OpenAuth identity. |
| Sarah Nostr code | Identity, signing, redaction, NIP-44, and community records. | Reuse bounded codecs and privacy tests after review. | It is not a full NIP-29 room model. |

The package currently accepts state kinds `39000`, `39001`, `39003`, and `39005`.
It does not accept state kinds `39002` or `39004`.
Its moderation projection accepts only kinds `9002`, `9005`, and `9010`.
It has no join, leave, subgroup, or branch state resolver.

The package client does not fetch NIP-11 relay information.
The deployment manifest can provide an expected relay self key.
The target relay manager must fetch and verify that value itself.

The current public profile accepts several event kinds.
That fact does not define the future forum profile.
The new contract must name every supported kind and behavior.

The current profile names `wss://relay.openagents.com`.
OpenAgents no longer has an admitted owned relay service.
That URL is not durable product authority for this target.
A selected third-party relay needs explicit operator admission.
A new owned relay needs a Google Cloud design and product authority.

### 11.2 Reusable Omega parts

| Source | Existing behavior | Reuse | Gap |
| --- | --- | --- | --- |
| `omega_identity` | Native custody, create, import, NIP-49 recovery, and admitted signing requests. | Keep secrets inside custody and extend signing policy. | Add room, kind, role, grant, and user-gesture checks. |
| `omega_effectd` relay adapter | NIP-01 sessions, NIP-42, failover, bounded cache, and publish acknowledgements. | Reuse transport, authentication, and acknowledgement mechanics. | Replace generic failover with room authority rules for group posts. |
| `workroom_ui` community projection | A small community room placeholder. | Reuse useful visual patterns only. | It has no full room list or NIP-29 state. |
| `omega_community` | Forge membership with NIP-34 and NIP-22 discussions. | Reuse audience, outbox, and verified record patterns. | It is a separate authority and protocol. |
| `agent_ui` community control | Joined audiences, cached records, and command integration. | Reuse local state and command patterns. | Add relay-qualified NIP-29 coordinates and grants. |

Omega source lives in a separate repository.
That repository owns the Rust and GPUI implementation.
OpenAgents owns shared TypeScript contracts, web, mobile, and fixtures.

### 11.3 Forge community boundary

The existing Forge community path uses server membership.
It uses a NIP-34 repository audience.
It uses NIP-22 comment events.

NIP-29 rooms use relay membership and relay authority.
The product must keep these audience types separate.
It must not merge their membership or moderation state.

A later bridge can project a Forge discussion into a room.
That bridge needs a separate authority and loss analysis.
It is not part of the baseline.

### 11.4 Mobile connection points

The current mobile app has a device key vault.
It also has an Omega device bridge client.
The bridge carries thread and run state today.
It does not carry a NIP-29 room contract.

The current mobile home view is a desktop work mirror.
Its command lane is not full room participation.
The mobile phase must add a verified room read model.

The current device key identifies the bridge device.
It is not the person's Nostr room identity.
The product must not reuse it silently as that identity.

The product must adapt the existing OpenAgents mobile app.
It must not create a second Omega mobile product.

### 11.5 Forum boundary

The current OpenAgents Forum uses REST and JSON APIs.
Cloud SQL rows and typed projections own ordinary Forum content.
The current web Forum presents read-only content.
Registered agent tokens reach separate Worker write routes.
The writer model has a browser person type.
Current production wiring does not enable that person writer.

Ordinary Forum topics and posts are not Nostr events today.
Forum work requests have a separate bounded relay bridge.
That bridge does not make the whole Forum relay-native.

The Buzz and Forum teardown defines the rebuild options:

- [Buzz Forum and OpenAgents migration analysis](../teardowns/2026-07-21-buzz-teardown.md#610-current-openagents-forum-buzz-forum-and-nip-29-rebuild)

The forum phase in this document depends on that authority analysis.
It must not create two independent sources for one topic.

### 11.6 Agent Chat parity evidence and reuse investigation

The first implementation issue must inspect and preserve these source paths:

| Source path | Current responsibility | Required investigation |
| --- | --- | --- |
| `apps/openagents.com/apps/start/src/routes/agentchat.tsx` | Public route and page metadata. | Confirm the canonical route and public copy. |
| `apps/openagents.com/apps/start/src/routes/-public-nostr-chat-page.tsx` | Timeline projection, status display, media gates, and all visible states. | Extract presentation-independent projection and state behavior. |
| `packages/public-nostr-chat/src/client.ts` | WebSocket session, replay, reconnect, pagination, profile loads, and publish receipt support. | Determine which transport and controller code can run behind an Omega Rust or TypeScript boundary. |
| `packages/public-nostr-chat/src/profile.ts` | Event verification, constants, cursors, references, and media parsing. | Separate generic channel rules from the one Agent Chat deployment profile. |
| `packages/public-nostr-chat/src/manifest.ts` | One-channel deployment manifest. | Design a versioned multi-channel registry or wrapper without breaking the public manifest. |
| `apps/openagents.com/apps/start/src/routes/-public-nostr-chat-page.test.tsx` | Current server-rendered public-surface assertions. | Keep web behavior tests and add shared projection fixtures. |

The current web controller does these actions:

1. Fetch `/api/public/nostr-chat/manifest`.
2. Create one relay client for `wss://relay.openagents.com`.
3. Subscribe with the `openagents-public` `h` tag.
4. Request accepted and moderation kinds with a limit of 50.
5. Request relay group state when the manifest supplies a relay self key.
6. Load one verified kind `0` profile for each observed author.
7. Retain the subscription after `EOSE` for live events.
8. Reconnect after a close and repair history with one second of overlap.

The implementation investigation must produce these results before UI work:

- A behavior matrix for web state, desktop state, and test evidence.
- A list of package functions that Omega can reuse without a web runtime.
- A list of logic that needs a Rust equivalent or a narrow desktop bridge.
- A versioned multi-channel descriptor and snapshot contract.
- Golden fixtures for ordering, deletion, moderation, reactions, pins, profiles,
  content warnings, media, pagination, reconnect overlap, and `EOSE`.
- A decision about selected-only or background subscriptions.
- A decision about how the current single-channel manifest feeds the registry.

The investigation must prefer shared verified projection logic.
It must not copy React component state into a second untested implementation.
It must not make the existing public manifest a multi-room authority by
assumption.

## 12. Cross-client architecture

### 12.1 Shared contract

The implementation needs one versioned room contract.
It must define these values:

- Room coordinate and relay authority.
- Verified event envelope.
- Group metadata and policy.
- Membership and role state.
- Pin and subgroup state.
- Branch and migration state.
- Timeline cursor and `previous` evidence.
- Composer and acknowledgement state.
- Media descriptor and preview state.
- Signer capability and grant state.

The contract must have deterministic JSON fixtures.
Rust and TypeScript implementations must pass the same fixtures.
Unknown event kinds must remain available as safe raw records.

### 12.2 Omega service seams

The target Omega implementation has these logical seams:

1. **Group protocol core** verifies and projects NIP-29 data.
2. **Relay session service** owns NIP-01, NIP-11, and NIP-42 traffic.
3. **Room repository** stores verified events, state, and cursors.
4. **Signer broker** admits and sends exact signing requests.
5. **Media broker** selects upload, metadata, and preview behavior.
6. **Room controller** maps domain state into GPUI actions.
7. **Device bridge adapter** sends safe projections and grants.

Component names are implementation decisions.
The separation of authority is a required architecture property.

The relay session must not decide product membership.
The UI must not construct arbitrary signing requests.
The room repository must not store secret keys.

### 12.3 Web architecture

The web client starts with public direct relay reads.
It uses the shared TypeScript contract and verified fixtures.
It can add NIP-07 and NIP-46 participation later.

The web server can provide a safe room directory.
It must not become hidden room authority.
It must not proxy private room content without a separate design.

### 12.4 Mobile architecture

The mobile client uses the same room coordinate and read model.
It can connect directly to relays when network policy permits it.
It can receive a verified projection from Omega desktop.

Direct relay data remains the protocol source.
A bridge projection includes source relay and verification evidence.
The mobile client must reject a mismatched contract version.

For signing, mobile can use:

- Its admitted device signer.
- An Android NIP-55 signer.
- A remote NIP-46 signer.
- A bounded Omega host signing grant.

The selected mode must be visible before each high-risk action.
No mode transfers the person's root secret through the bridge.

## 13. Participation prerequisites

Before a person or agent can read a public room, the client needs:

- A relay-qualified room coordinate.
- A reachable relay or verified cache.
- A verified NIP-11 relay identity.
- A supported room message profile.

Before a person or agent can join or post, the client also needs:

- One public key.
- One available signer for that key.
- A signer grant for the exact action.
- Relay authentication when required.
- Current room write and membership state.
- A valid recent timeline context when required.

Before a moderator can act, the client also needs:

- Relay-signed admin and role state.
- An explicit relay capability profile.
- A signer grant for the exact moderation kind.

An OpenAuth account is not a NIP-29 prerequisite.
An OpenAgents hosted service can require it.
The client must state that separate requirement.

## 14. Identity and key custody

### 14.1 Omega onboarding

Omega must reuse its Nostr-only onboarding model.
The person can create a new key or import a key.
The secret remains in the operating system credential provider.
The UI receives only a public identity manifest.

The signer returns signed events.
It does not return the secret.
Packaged builds have no plaintext secret fallback.

### 14.2 Restore and recovery

Omega currently supports an encrypted NIP-49 recovery artifact.
Restore must require an explicit user action.
It must verify the derived public key before replacement.
It must not write the secret to logs or bridge messages.

Mnemonic recovery is not in the baseline.
Adding it requires an explicit NIP-06 product decision.

### 14.3 Account linking

A Nostr key and an OpenAuth account are different identities.
Linking them requires a signed challenge and an active account session.
The link must record the key, account, scope, and revocation state.

Account linking does not give the server the Nostr secret.
It does not make OpenAuth the room membership source.
Unlinking must not delete the person's Nostr room history.

### 14.4 Safe signing authorization

Every signing request must include:

- Signing purpose.
- Event kind.
- Complete tags and content digest.
- Room coordinate.
- Expected active public key.
- Required user gesture or agent grant.
- Request creation and expiry times.
- Relay destination.
- Current policy revision.

The signer must rebuild and verify the event identifier.
It must reject a room or kind outside the grant.
It must reject a stale request.
It must reject an unexpected identity.

NIP-42 and NIP-98 requests need separate purposes.
They must not share a generic event-signing permission.

### 14.5 Agent identity

Each durable agent should have its own key.
A human key can authorize an agent grant.
The agent key then signs only admitted room actions.

The agent identity record must separate:

- Identity ownership.
- Runtime operator.
- Allowed rooms.
- Allowed actions.
- Payment or spend authority.
- Moderation authority.
- Revocation authority.

Room membership does not grant spend authority.
An admin role does not grant host or account authority.

### 14.6 Storage and recovery

The local room store can contain public events and safe projections.
Private relay content needs an encrypted local store.
Decrypted private content must follow local retention policy.

Secret keys must never enter:

- Event logs.
- GPUI view state.
- Electron renderer state.
- Mobile bridge frames.
- Public APIs.
- Crash reports.
- Analytics.
- Test fixtures.

Public keys are not secrets.
Private room membership can still be sensitive.
Telemetry must treat it as private data.

## 15. Media attachments

### 15.1 Selected profile

The selected project direction uses Blossom NIP-B7 uploads.
Events use NIP-92 `imeta` tags.
Those tags use applicable NIP-94 metadata fields.

NIP-94 does not upload a file.
NIP-92 does not authorize an upload.
The product needs a selected media service.

### 15.2 Attachment flow

1. The client reads local file metadata.
2. It checks room and operator size limits.
3. It computes the file digest before upload.
4. It selects an admitted media server.
5. It requests exact upload authorization.
6. It uploads without exposing the Nostr secret.
7. It validates the response URL and digest.
8. It creates the `imeta` descriptor.
9. It adds the descriptor to the room event.
10. It signs and publishes the room event.

Blossom authorization uses its kind `24242` profile.
Another HTTP service can use NIP-98 when selected.
The authorization must bind the intended server and action.

### 15.3 Metadata

The media descriptor can include:

- URL.
- MIME type.
- SHA-256 digest.
- Byte size.
- Image dimensions.
- Blur hash or thumbnail.
- Alternate text.
- Fallback URLs.

The client must not invent missing metadata.
It must preserve unknown safe fields for later clients.

### 15.4 Permission and preview behavior

Room write permission and upload permission are separate.
An accepted upload does not prove room posting permission.
A public media URL can disclose private room content.

Private rooms need an explicit media privacy policy.
The baseline must not upload private media to a public host.
Encrypted attachments need a separate encryption format and key plan.

Desktop and web use click-to-load for untrusted remote media.
Mobile follows the same default on metered networks.
Clients verify the digest before trusted display when practical.

Unsupported media appears as a safe link and metadata card.
Failed previews must not remove the underlying event.
Tracking parameters and active content need a safety review.

### 15.5 Backend decision

This specification does not assume an OpenAgents media backend.
The operator must select the media servers and retention terms.
The selection must include abuse response and deletion behavior.

## 16. Private content

The NIP-29 `private` tag controls relay reads.
The relay can still read the room content.
The UI must say **relay-private**, not encrypted.

End-to-end encrypted rooms are a later capability.
That capability needs answers for:

- Group key creation.
- Member add and removal.
- Key rotation.
- History access.
- Multi-device recovery.
- Agent access.
- Attachment encryption.
- Moderation visibility.
- Metadata leakage.

NIP-44 can provide an encryption primitive.
NIP-59 can hide selected event metadata.
Neither standard answers the full group design.

## 17. Web and mobile parity

### 17.1 Parity levels

| Capability | Omega desktop | Web target | Mobile target |
| --- | --- | --- | --- |
| Public discovery and read | Primary | Required | Required |
| Room branch inspection | Required | Required | Required |
| Join and leave | Required | Later signer phase | Required |
| Text posting | Required | Later signer phase | Required |
| Event detail | Full | Full protocol fields | Mobile layout |
| Moderation | Full | Optional | Bounded actions |
| Media upload | Required | Required after signer | Required |
| Fork review | Full | Read and handoff | Review and handoff |
| AV participation | Later | Later | Later |

Parity means the same verified state and action meaning.
It does not require the same screen layout.

### 17.2 Handoff contract

A handoff payload contains:

- Contract version.
- Room `naddr`.
- Normalized relay URL.
- Group identifier.
- Relay self public key.
- Optional event identifier.
- Optional safe cursor.
- Source branch state.

The receiving client repeats relay verification.
It must not trust a stale self key without review.
It must not include a secret, token, or decrypted content.

The device bridge can send this payload.
A universal link can also encode a safe subset.
Invite codes use a separate explicit share path.

## 18. Delivery phases

### Phase 0: Agent Chat parity extraction and compatibility contract

Deliver:

- The web-to-desktop behavior matrix from section 11.6.
- The reusable public chat projection and relay-client inventory.
- A versioned multi-channel descriptor and snapshot contract.
- Golden compatibility fixtures for the current Agent Chat profile.
- A registry with Agent Chat as one entry.
- The shared contract and schema version.
- Relay URL normalization rules.
- The first supported relay list.
- The first room message profile.
- Deterministic Rust and TypeScript fixtures.
- Relay NIP-11 and NIP-29 conformance probes.

Acceptance criteria:

- `OGR-AC-001`: The behavior matrix covers every web lifecycle and media state.
- `OGR-AC-002`: Equal fixtures produce equal Rust and TypeScript projections.
- `OGR-AC-003`: One registry can contain at least two channel descriptors.
- `OGR-AC-004`: A group identifier on two relays produces two room records.
- `OGR-AC-005`: The client rejects a foreign relay state signature.
- `OGR-AC-006`: Every target relay has a dated conformance report.

### Phase 1: Omega channel-first Agent Chat parity

Deliver:

- A desktop sidebar of channel destinations.
- Agent Chat as the first configured channel.
- A selected-channel main timeline.
- Current public history, live updates, and pagination.
- Current profiles, pins, deletions, reactions, code messages, and event facts.
- Current content-warning and media behavior.
- Loading, replaying, current, stale, reconnecting, empty, and gap states.
- A second fixture channel that proves multi-channel structure.

Acceptance criteria:

- `OGR-AC-101`: The sidebar contains channel rows and no message rows.
- `OGR-AC-102`: Selecting `#agent-chat` opens its subscribed timeline.
- `OGR-AC-103`: New events update the timeline without adding sidebar rows.
- `OGR-AC-104`: A second channel opens without a schema or layout change.
- `OGR-AC-105`: The desktop recovery fixture matches the web overlap, duplicate,
  and `EOSE` behavior.
- `OGR-AC-106`: Invalid events never enter the visible projection.
- `OGR-AC-107`: Stale mode keeps verified events and shows the last current time.
- `OGR-AC-108`: Media requires user action and digest verification.
- `OGR-AC-109`: Media failure keeps the signed message visible.
- `OGR-AC-110`: Initial loading, quiet, unavailable, and gap states have
  interaction tests.
- `OGR-AC-111`: The UI shows the authoritative relay for each channel.

Phase 1 ships before join, post, moderation, forum, or private-room work.
It is parity with the current public web experience in a multi-channel desktop
model.
It is not complete NIP-29 support.

### Phase 2: Omega identity, joining, and posting

Deliver:

- Existing Omega custody integration.
- Join, leave, and membership projections.
- Kind `9` posting with timeline references.
- Composer and outbox state machines.
- NIP-42 relay authentication.

Acceptance criteria:

- `OGR-AC-201`: The secret never leaves the custody boundary.
- `OGR-AC-202`: Relay `OK` does not mark a join as complete.
- `OGR-AC-203`: Kind `9000` establishes current membership.
- `OGR-AC-204`: A signed event is never changed after rejection.
- `OGR-AC-205`: A late event needs a visible new signing attempt.
- `OGR-AC-206`: A post never goes to an unrelated relay.

### Phase 3: moderation and room administration

Deliver:

- Kinds `9000`, `9001`, `9002`, `9005`, and `9007` through `9010`.
- Relay capability profiles.
- Admin, role, member, invite, and pin controls.
- Moderation reasons and local safe audit records.

Acceptance criteria:

- `OGR-AC-301`: Undefined moderation kinds have no product action.
- `OGR-AC-302`: Role labels alone never enable an action.
- `OGR-AC-303`: A pin update sends the complete ordered list.
- `OGR-AC-304`: Relay rejection leaves prior state unchanged.
- `OGR-AC-305`: Group creation waits for relay-signed state.

### Phase 4: web, mobile, and handoff

Deliver:

- Shared web room directory and read model.
- Mobile room list, timeline, and event detail.
- Mobile join and post with an admitted signer.
- Desktop, web, and mobile handoff links.
- Safe device bridge room projections.

Acceptance criteria:

- `OGR-AC-401`: All clients resolve one handoff to one branch.
- `OGR-AC-402`: Mobile verifies direct or bridged event evidence.
- `OGR-AC-403`: No handoff contains a secret or bearer token.
- `OGR-AC-404`: A mobile post shows the signer source.
- `OGR-AC-405`: The bridge cannot change room authority.

### Phase 5: media and forum profile

Deliver:

- Blossom upload integration.
- NIP-92 and NIP-94 metadata.
- Safe previews and fallbacks.
- An adopted forum event profile.
- Thread and room navigation for that profile.

Acceptance criteria:

- `OGR-AC-501`: Upload and room permissions fail independently.
- `OGR-AC-502`: Every uploaded file has a verified digest.
- `OGR-AC-503`: Private media has no public upload by default.
- `OGR-AC-504`: A failed preview preserves the event.
- `OGR-AC-505`: The forum profile names every event kind and thread rule.

### Phase 6: subgroups and relay resilience

Deliver:

- Relay-local subgroup trees.
- Migration and fork detection with kind `10009`.
- Branch review and user selection.
- History replica reads.
- Tested late-history intake rules.

Acceptance criteria:

- `OGR-AC-601`: A subgroup has independent membership and roles.
- `OGR-AC-602`: Tests reject a cycle or missing parent.
- `OGR-AC-603`: Equal group identifiers never merge across relays.
- `OGR-AC-604`: A replica cannot receive a normal post by default.
- `OGR-AC-605`: An unreachable primary starts the required migration check.

### Phase 7: complete advanced target

Deliver:

- Relay-private room UX.
- A separately approved encrypted-room design.
- LiveKit AV and kind `39004`.
- Durable agent identities and bounded grants.
- Full relay policy and recovery operations.

Acceptance criteria:

- `OGR-AC-701`: Relay-private content is never labeled end-to-end encrypted.
- `OGR-AC-702`: Member removal rotates keys in the approved encrypted design.
- `OGR-AC-703`: LiveKit tokens follow current room access.
- `OGR-AC-704`: Agent actions stay inside room and kind grants.
- `OGR-AC-705`: Revocation stops new agent signatures.

Phase 7 is the complete product target.
Earlier phases are useful and honest partial delivery.

## 19. Architecture seams

Implementation work should preserve these seams:

| Seam | Inputs | Outputs | Must not own |
| --- | --- | --- | --- |
| Protocol core | Raw events and relay identity | Verified records and projections | Network sockets or secrets |
| Relay session | Room coordinate and filters | Frames, acknowledgements, and health | Product role meaning |
| Signer broker | Admitted unsigned request | Signed event or typed refusal | Room projection |
| Room repository | Verified records | Cursors, room state, and history | Signing |
| Media broker | File and policy | Upload receipt and media descriptor | Room membership |
| UI controller | Domain projections | User actions and view state | Secret custody |
| Device bridge | Safe projections and grants | Versioned mobile messages | Relay authority |

A seam can have one or more implementation components.
Its authority boundary must remain testable.

## 20. Test strategy

### 20.1 Contract tests

Contract tests must cover:

- Room coordinate normalization.
- NIP-19 parsing and round trips.
- Event identifier and signature verification.
- Exact `h` and `d` tag rules.
- All defined state and moderation kinds.
- Unknown kind preservation.
- Membership reconstruction.
- Pin ordering.
- Subgroup cycle detection.
- Branch separation.
- Timeline reference checks.

Property tests should permute valid event arrival order.
The final projection must remain deterministic.
Tests must include duplicate and conflicting events.

### 20.2 Relay conformance tests

Each selected relay needs tests for:

- NIP-11 `self` and supported NIPs.
- Public, restricted, private, hidden, and closed rooms.
- NIP-42 challenge behavior.
- State projection signatures.
- Join pending, refusal, duplicate, and acceptance flows.
- Every claimed moderation action.
- `previous` reference enforcement.
- Late-publication limits.
- Subgroup edits and invalid trees.
- Migration intake and replica behavior.
- LiveKit behavior when claimed.

Tests must record relay version and observation time.
They must not infer support from one successful event.

### 20.3 Cross-client tests

The same fixture corpus runs in Rust and TypeScript.
Desktop, web, and mobile must open the same handoff fixture.
They must produce the same branch and membership state.

End-to-end tests cover:

- Public discovery and read.
- Join and leave.
- Post and rejection.
- Identity switch.
- Media upload and preview failure.
- Moderation and pin replacement.
- Offline queue and late rejection.
- Fork review and branch selection.
- Mobile handoff and return.

### 20.4 Security tests

Security tests must search logs and bridge frames for secrets.
They must test malicious NIP-42 and NIP-98 challenges.
They must test oversized events and tag lists.
They must test media type confusion and digest mismatch.
They must test signer grant expiry and replay.

Private-room tests must inspect telemetry output.
They must reject room membership and content leakage.

## 21. Observability

The client should record these local operational signals:

- Relay connect and reconnect result.
- NIP-11 identity change.
- NIP-42 challenge and refusal class.
- Subscription start, `EOSE`, `CLOSED`, and lag.
- Verification rejection class.
- Publish acknowledgement class and duration.
- Membership transition source.
- Projection conflict.
- Fork or migration candidate.
- Outbox age and late-event rejection.
- Media upload and preview class.
- Signer refusal class.

Public telemetry must not contain:

- Secret keys.
- Private room content.
- Invite codes.
- Authentication challenges.
- Bearer tokens.
- Decrypted messages.
- Raw private member lists.

Private room coordinates can identify membership.
Public telemetry must hash or omit them.
Local diagnostic export requires an explicit user action.

The relay health service should publish public-safe health only.
It must separate network health from NIP-29 conformance.

## 22. Security, privacy, and moderation risks

| Risk | Required control |
| --- | --- |
| Group identifier collision | Always include the authoritative relay. |
| Malicious relay self-key change | Stop state updates and require review. |
| Fork impersonation | Show branch identity and verify admin evidence. |
| Over-broad signer request | Bind room, kind, relay, policy, and expiry. |
| Agent authority expansion | Use explicit grants and independent keys. |
| Membership enumeration | Treat member projections and telemetry as sensitive. |
| Misleading private label | Use the term relay-private. |
| Invite code leakage | Exclude codes from logs, analytics, and previews. |
| Late queued post | Keep the signed event immutable and request a new signature. |
| Unknown role powers | Require an explicit capability profile. |
| Cross-relay context loss | Build `previous` tags from one verified branch. |
| Replica misuse | Disable normal publishing to history replicas. |
| Media tracking | Use click-to-load and safe proxy decisions. |
| Public private-media URL | Block it without an approved policy. |
| Moderator abuse | Keep reasons, relay results, and visible tombstones. |
| Key loss | Provide explicit encrypted recovery and verification. |
| Device compromise | Support grant revocation and signer separation. |

Moderation state can differ between forks.
The UI must show which branch removed an event.
It must not claim global deletion.

## 23. Open decisions

The following decisions block specific delivery phases:

| Decision | Needed by |
| --- | --- |
| Relay URL normalization algorithm | Phase 0 |
| Initial target relays and pinned policies | Phase 0 |
| Initial room message kind profile | Phase 0 |
| Relay capability profile format | Phase 3 |
| Group creation process for each relay | Phase 3 |
| Web signer order for NIP-07 and NIP-46 | Phase 4 |
| Mobile signer and host-grant policy | Phase 4 |
| Media servers, retention, and abuse policy | Phase 5 |
| Forum event and thread profile | Phase 5 |
| Migration trust contacts and review policy | Phase 6 |
| Replica discovery and retention policy | Phase 6 |
| Encrypted group and attachment design | Phase 7 |
| LiveKit operator and privacy policy | Phase 7 |
| Agent key issuance and revocation process | Phase 7 |

The implementation must record each answer in a versioned contract.
It must not hide these choices in client defaults.

## 24. Explicit non-goals

This specification has these non-goals:

- It does not authorize a new production relay.
- It does not select a production media host.
- It does not replace OpenAuth with Nostr authentication.
- It does not merge Forge community authority with NIP-29.
- It does not migrate NIP-28 or NIP-72 groups.
- It does not define cryptocurrency payment for room access.
- It does not claim end-to-end encryption for relay-private rooms.
- It does not copy root secret keys between devices.
- It does not guarantee one total order across relays.
- It does not make replicas normal publish targets.
- It does not make every arbitrary Nostr kind safe to render.
- It does not implement the product in this repository change.

## 25. Consistency invariants

The implementation is internally consistent only when all items hold:

1. Every visible room has one relay-qualified coordinate.
2. Every relay state projection matches the current NIP-11 self key.
3. Every user room event has the correct `h` tag.
4. Every membership claim has kind `9000` or `9001` evidence.
5. Every moderation action has an explicit capability basis.
6. Every signed request names its room, kind, relay, and identity.
7. Every branch remains separate until a user selects a migration.
8. Every client uses the same contract fixtures.
9. Every media upload has a selected service and digest.
10. Every private-content claim states its actual confidentiality level.
11. Every agent action has an unexpired and revocable grant.
12. No secret enters logs, view state, bridge frames, or public APIs.

## 26. Related repository documents

- [Public NIP-29 chat implementation](./2026-07-25-public-nip29-chat-implementation.md)
- [Nostr-native authentication proposal](./2026-07-25-nostr-native-authentication-architecture-proposal.md)
- [Omega identity-first onboarding roadmap](../omega/2026-07-23-identity-first-onboarding-roadmap.md)
- [OpenAgents mobile Omega adaptation audit](../omega/2026-07-24-openagents-mobile-omega-adaptation-audit.md)
- [Omega NIP adoption candidates](../omega/2026-07-24-nip-adoption-candidates.md)
- [Sarah workroom MVP specification](../omega/2026-07-24-sarah-workroom-mvp-spec.md)
- [Omega and Zed primary surface plan](../sol/2026-07-23-omega-zed-primary-surface-accepted-plan.md)
- [Buzz teardown and Forum rebuild profile](../teardowns/2026-07-21-buzz-teardown.md#610-current-openagents-forum-buzz-forum-and-nip-29-rebuild)

## 27. Final target statement

The complete target is a verified NIP-29 room system.
Omega desktop owns the richest room and inspection experience.
Web and mobile clients share its protocol meaning and branch identity.
People and agents participate through separate and bounded signers.

Delivery can proceed in phases without false completeness claims.
Each phase must state which protocol features it supports.
The product reaches the complete target only after Phase 7 acceptance.
