# Marmot Secure-Group Rooms Plan

Status: draft
Owners: `apps/autopilot-desktop`, future secure-room adapter
Last updated: 2026-03-06

## Purpose

This document answers issue `#3035`: how OpenAgents should support end-to-end encrypted large-group chat as an optional future room type.

It builds on:

- `docs/plans/nostr-group-chat.md`
- `docs/kernel/nostr-managed-chat-contract.md`
- `/Users/christopherdavid/code/nips/EE.md`
- `/Users/christopherdavid/code/pylon/docs/nips/EE.md`
- `/Users/christopherdavid/code/wn-tui`

It is intentionally not an MVP implementation commitment. The MVP managed-chat path remains:

- `NIP-29` for server authority
- `NIP-28` for server channels
- `NIP-17` for DMs and small private rooms

Secure rooms stay optional and additive.

## Recommendation

OpenAgents should model large E2EE chat as a distinct `secure-group` room transport backed by Marmot or another MLS-based successor, not as a hidden encryption mode inside every managed server channel.

The key product decision is:

- use standalone secure rooms
- allow a managed server or channel to link to them
- do not make `NIP-29` membership itself the cryptographic source of truth for those rooms

Reasoning:

- strong E2EE wants device-level membership, epoch changes, and local key state that do not map cleanly to relay-enforced server channels
- server moderation and role policy are still useful for discovery and social structure, but they should not be confused with who can decrypt MLS traffic
- a separate room type preserves the clean room-model seam already reserved in `docs/kernel/nostr-managed-chat-contract.md`

## Why Not NIP-EE Directly

`NIP-EE` is still useful background material because it explains the MLS shape we need:

- per-device membership
- KeyPackage publication
- Welcome / Proposal / Commit / Application messages
- periodic signing-key rotation
- encrypted local state

But `NIP-EE` is explicitly marked unrecommended and points to Marmot as the successor path. OpenAgents should therefore treat it as a requirements document, not as the protocol we implement directly.

## Room Model

### Preferred model

`secure-group` rooms are standalone rooms with their own:

- room id
- device membership roster
- epoch history
- encrypted timeline
- invite / welcome flow

They may optionally be linked from:

- a managed server sidebar
- a channel message
- a pinned server announcement

But the server link is only discovery and policy context. It is not the room's encryption authority.

### Rejected model

Do not implement "secure channels inside a managed server" as the primary design.

That model looks convenient, but it creates constant ambiguity about:

- whether a removed server member should still decrypt cached room history
- whether server admins can add members without secure-room device consent
- whether unread/search/presence state is server truth or encrypted-room truth

If we later want a server-linked secure room, treat it as:

- a secure room attached to a server
- not a normal channel with an encryption toggle

## Identity And Device Keys

### Identity reuse

Reuse the existing Nostr identity keypair as the user's stable account identity for:

- invite addressing
- KeyPackage publication and verification
- device ownership labeling
- room membership display

Do not derive MLS group secrets or long-lived device ratchet state from the Bitcoin wallet seed or Spark wallet keys.

Reasoning:

- wallet keys have different blast radius and recovery expectations
- compromising message state must not imply wallet compromise
- compromising wallet state must not reveal room transcripts

### Device model

Each device is its own MLS leaf. That means:

- desktop A and desktop B join separately
- each device has its own signing material and ratchet state
- a user may appear multiple times in room member details, grouped under one identity

OpenAgents should generate device-local MLS signing and encryption keys on-device and store them encrypted at rest using:

- OS keychain / secure enclave when available
- a local encrypted app vault fallback when platform keystore access is unavailable

The room UI should show:

- room members
- per-member device count
- whether this device is healthy, stale, or missing key material

## Invite And Welcome Flow

The `wn-tui` model is directionally correct: the product needs a visible invite inbox, clear join states, and a dedicated group-detail management surface.

OpenAgents should use this flow:

1. Admin creates a secure room.
2. Admin adds members by Nostr identity, selecting one or more published device packages.
3. The adapter sends the room commit and an invite/welcome payload to the target user.
4. The recipient sees a pending secure-room invite row in a dedicated inbox.
5. Accepting the invite creates a local room shell in `pending-sync` state.
6. The room only becomes writable once the welcome is processed and the local device has current epoch state.

Required UI states:

- `pending invite`
- `processing welcome`
- `joined`
- `read-only / stale epoch`
- `rekey required`
- `cannot decrypt on this device`

If a user receives an invite for a device whose key material is unavailable locally, the UI must say that explicitly and leave the invite unresolved instead of pretending the room is usable.

## Rekeying

Rekeying must be routine, not exceptional.

OpenAgents should support:

- automatic rekey on member add
- automatic rekey on member removal
- automatic leaf rotation on a periodic schedule
- manual admin-triggered "rotate room keys"
- forced rekey after suspected device compromise

The UI should not expose MLS internals by default, but it must expose security-relevant state honestly:

- a small security banner when membership changed and a new epoch is being processed
- system timeline rows for member add/remove and key-rotation events
- a room-detail security section showing current epoch, pending commits, and failed devices

## Local Encrypted State Storage

The secure-room adapter needs a deterministic local store that separates:

- encrypted room state
- encrypted transcript cache
- decrypted render cache
- room membership snapshots
- pending invites / welcomes / proposals

Storage rules:

- encrypted room state is authoritative for local decrypt ability
- decrypted caches are disposable
- deleting the decrypted cache must not corrupt the room
- replaying canonical encrypted events must deterministically rebuild the same transcript and membership view

Recommended local model:

- `room_descriptor`
- `device_identity`
- `epoch_state`
- `encrypted_event_log`
- `projection_checkpoint`
- `decrypted_message_cache`

Only the first four are required for correctness.

## Recovery And Multi-Device Behavior

OpenAgents should take the stricter, more honest MLS posture:

- devices do not clone raw MLS state between each other by default
- a new device joins as a new leaf
- an existing healthy device must invite the new device
- if all devices are lost, past secure-room history is considered unrecoverable unless a future encrypted backup feature exists

That is a product constraint, but it is the least misleading one.

The UI should present:

- `Add this device to secure rooms` from account/security settings
- per-room `Invite another device`
- `This room is unavailable on this device until another trusted device invites it`

Future enhancement, not first version:

- encrypted backup/export of room state to user-controlled storage, protected by a passphrase and signed by the Nostr identity

## UI Plan For OpenAgents Desktop

`wn-tui` gives the right interaction skeleton:

- split view with chat rail, transcript, and composer
- group-detail screen for membership and admin actions
- invite inbox
- explicit status bar
- popup-driven add-member and confirmation flows

OpenAgents should keep those ideas, but render them in the existing desktop/WGPUI style rather than copying the terminal UI literally.

### Core secure-room surfaces

#### 1. Room rail row

Each secure room row should show:

- lock badge
- room name
- unread count
- last decrypted message preview or `Encrypted update`
- small sync-health indicator

If the room is attached to a managed server, the row may appear under that server's room group with a distinct `Secure` badge.

#### 2. Transcript pane

The transcript should render:

- decrypted application messages
- system rows for joins, removals, and key rotations
- explicit failure rows for decrypt errors or stale-device state
- composer-disabled state until this device is ready to send

A secure room must never silently drop undecryptable events. It should show:

- `Message unavailable on this device`
- `Room membership changed, catching up`
- `Invite accepted on another device only`

#### 3. Room detail drawer

Adapt the `wn-tui` group-detail pattern into a right-hand drawer or modal with:

- member list
- grouped devices per member
- admins / operators
- pending invites
- security state
- add/remove member actions
- rotate keys action
- leave room action

#### 4. Invite inbox

The product needs a single invite inbox for:

- DM requests
- server invites
- secure-room invites

Secure-room invites should expose:

- inviter identity
- linked server or channel context when available
- target device status
- accept / decline

#### 5. Security footer

Borrow the `wn-tui` status-bar idea and keep a compact footer line in the chat pane that reports:

- relay connectivity
- secure-room sync state
- current device readiness
- pending membership/security actions

### UX rules

- secure rooms should look different from normal channels immediately
- membership and decrypt failures must be visible in-line, not buried in logs
- do not overload the main server moderation surface with secure-room cryptographic actions
- a secure room can have replies and reactions only if the adapter can represent them deterministically
- search should default to local decrypted history only; do not imply server-side searchable ciphertext

## Integration Architecture

OpenAgents should keep the existing room-model boundary:

- `managed-channel`
- `dm`
- `secure-group`

For the secure-room implementation path, prefer a dedicated adapter boundary inside `apps/autopilot-desktop` that yields:

- room metadata snapshot
- member snapshot
- ordered timeline items
- send capability state
- invite inbox items
- deterministic local projection rebuild

Do not make the desktop call a CLI forever the way `wn-tui` does. That is a good spike pattern, not the target architecture.

Recommended delivery path:

1. Prototype against a local Marmot / WhiteNoise daemon or CLI to validate room lifecycle and UI copy.
2. Freeze the OpenAgents secure-room view model and adapter contract.
3. Replace the CLI bridge with an in-process or tightly scoped service integration once the lifecycle is understood.

## Delivery Phases

### Phase 0: protocol and UX spike

- stand up a local Marmot / WhiteNoise environment
- map actual invite, welcome, member-add, and rekey events to a desktop view model
- confirm whether replies, reactions, and edits are first-class or need OpenAgents-local constraints

### Phase 1: read-only secure-room shell

- discover secure-room invites
- accept invite
- show room rail row
- render transcript with decrypt and failure states
- show room detail membership

### Phase 2: writable secure rooms

- composer enablement once device state is healthy
- add/remove member flows
- leave room
- rotation status and admin action

### Phase 3: multi-device hardening

- add-device UX
- stale-device warnings
- encrypted local backup design if still needed

## Open Questions

These need real Marmot implementation validation before code lands:

- what exact event forms replace the `NIP-EE` KeyPackage / Welcome / Group Event assumptions
- whether reactions and threaded replies are native, emulated, or intentionally unsupported in secure rooms
- whether server-linked secure rooms should inherit any moderation affordances beyond simple discovery and membership suggestions

## Bottom Line

OpenAgents should support secure rooms later as a distinct transport:

- linked from servers when useful
- authenticated by existing Nostr identities
- keyed per device
- stored encrypted at rest
- explicit about non-recoverability and device limitations

That model fits the existing room abstraction, respects the MVP boundaries, and leaves managed chat simple instead of turning every server channel into a cryptographic special case.
