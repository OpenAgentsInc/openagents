# Omega Nostr authentication and onboarding target

- Date: 2026-07-30
- Class: source-grounded product and implementation proposal
- Status: proposal only
- Product: Omega
- Primary implementation: `OpenAgentsInc/omega`
- OpenAgents source pin: `5f84de903f27f9e79b9c4e4394f775a09994ce94`
- Omega source pin: `5cc5a29765fc60137a63e377091ab544d7dbb76c`
- Buzz identity and authentication pin: `5a3b8176aac5f4bced452ac8920477c5e059b828`
- Buzz desktop onboarding study pin: `acfbb1bb6af54cb29cb152496ff43b8285dcb8cf`
- Armada source pin: `5b99f88d309052abc1eeb4f0b2ef437de086e709`
- Audience: Omega product, desktop, identity, Nostr, mobile, and assurance teams

## 1. Purpose and decision

Omega already has a real Nostr identity implementation. It does not yet have a
complete Nostr account product.

Today a fresh Omega profile creates a key silently in the background before the
front door opens. That removes first-run friction and gives identity-consuming
features a signer. It does not let a person make an informed choice about
whether Omega should create a new identity, use an identity they already have,
or delegate signing to another signer. It also does not provide account
switching, remote signers, a durable identity dashboard, relay-auth visibility,
or a complete recovery journey.

The target should preserve the fast startup while adding an explicit
**Activate your Omega identity** journey before the identity performs a
meaningful external action. This is progressive onboarding, not a blocking
first-launch wizard:

1. Omega may provision a local candidate identity in the background.
2. The candidate remains usable for local exploration.
3. Before the first public post, community join, device grant, hosted-account
   link, agent attestation, or other durable identity-bearing action, Omega
   asks the person to keep the candidate, import an existing identity, or use
   an external signer.
4. Keeping a locally held identity requires an explicit recovery decision.
5. Once activated, the same account surface owns profile, relays, signer
   capabilities, recovery, devices, account switching, and logout.

This gives Omega the legibility of Armada and the local-first Rust custody of
Buzz without copying either product's weakest behavior. In particular:

- do not put a root `nsec` in a web renderer store as Armada does on Electron
  and web;
- do not make “backup failed” a normal skip path as Buzz does;
- do not copy one root key to mobile as Buzz's current NIP-AB flow does;
- do not treat NIP-42, a relay membership, or a valid signature as OpenAgents
  account or action authority; and
- do not replace the Rust identity service with a TypeScript signer.

This document proposes work. It does not admit implementation, change the
current startup behavior, or supersede an owner decision by itself.

## 2. Evidence labels

- **`[source]`** means behavior verified in the pinned source or a pinned
  source-grounded teardown.
- **`[existing plan]`** means an existing OpenAgents proposal or work-packet
  ledger, not necessarily current implementation.
- **`[proposed]`** means the target in this document.
- **`[decision]`** means an owner or product choice required before admission.
- **`[limit]`** means static source evidence does not prove installed behavior.

## 3. Existing documentation inventory

There are already Nostr identity and authentication documents. The gap is not
absence of design work; it is the absence of one current Omega-specific bridge
from landed behavior to a full account journey.

| Document | What it already owns | Why this document is still needed |
| --- | --- | --- |
| [Omega identity-first onboarding roadmap](./2026-07-23-identity-first-onboarding-roadmap.md) | Native custody, create/import/recovery states, secure input, failure handling, and the original blocking first-run journey | Its implementation narrative predates the 2026-07-29 owner direction that removed first-run identity onboarding and introduced background provisioning |
| [Nostr-native authentication for OpenAgents](../nostr/2026-07-25-nostr-native-authentication-architecture-proposal.md) | Canonical hosted user linking, NIP-07/42/46/49/55/98 roles, web login, device grants, revocation, and recovery | It spans web, desktop, and mobile and keeps OpenAuth as session issuer; it does not specify Omega's current local account UX or migration |
| [Omega NIP-29 relay groups integration specification](../nostr/2026-07-27-omega-nip29-relay-groups-integration-spec.md) | Room identity, NIP-29 participation, signer authorization, agent identity, cross-client architecture, and delivery phases | It assumes a complete identity experience but does not define how a person reaches or manages that state |
| [Omega application identity](https://github.com/OpenAgentsInc/omega/blob/5cc5a29765fc60137a63e377091ab544d7dbb76c/docs/src/development/omega-application-identity.md) | Omega release-channel isolation and the older native identity implementation narrative | Parts are stale against the pinned source: current `IdentityService::system` uses `FileSecretStore`, and startup no longer opens the identity-first journey |
| [Omega runtime credential storage](https://github.com/OpenAgentsInc/omega/blob/5cc5a29765fc60137a63e377091ab544d7dbb76c/docs/omega/runtime-credential-storage.md) | Current no-Keychain decision and exact local-file storage for identity, API keys, OAuth, and OpenAgents sessions | It accurately describes storage but does not define a user-facing identity or account journey |
| [Buzz teardown](../teardowns/2026-07-21-buzz-teardown.md) | Buzz identity custody, NIP-42/NIP-98 auth, desktop-to-mobile pairing, and security limits | It is a teardown, not an Omega product contract |
| [Armada teardown](../teardowns/2026-07-30-armada-teardown.md) | Armada's complete create/login/sync/account-switching flow and its host-specific custody | It provides the closest product-parity reference but not the Rust-first Omega adaptation |

The existing documents remain useful. This document should become the
implementation entry point for Omega human Nostr identity and onboarding, while
the cross-product authentication proposal remains the authority for linking a
Nostr identity to a canonical hosted OpenAgents user.

## 4. What Omega has now

### 4.1 Startup behavior

At the Omega source pin, `IdentityStartupCoordinator` starts one shared
background task per process. `IdentityService::provision_for_process_start`
does the following:

- `Ready`: reuse the existing identity;
- `Absent`: generate and persist a new Nostr key;
- `Unadopted`: adopt the identity already present in the selected store; and
- `Lost`, `Conflict`, `Incomplete`, `Locked`, `ResetFailed`, or
  `RelaunchRequired`: refuse with the named state.

Every launch path awaits the same task, so concurrent windows cannot create
different identities. A refusal is logged, but it does not block the front
door. Identity-dependent features later fail at their own boundary. [source]

The background transaction has the receipt
`omega-first-launch-background-keygen-v1`. Creation is journaled, serialized
across processes, read back, and matched to the expected public key before the
public manifest is committed. Interrupted or conflicting states are not
silently replaced. [source]

This is a strong custody transaction and a weak account introduction. The
person has not yet:

- seen which public identity Omega selected;
- chosen create versus import versus external signer;
- confirmed a recovery method;
- understood that the key is not a password and has no reset authority;
- chosen relays or community destinations; or
- granted particular signing capabilities.

### 4.2 Current custody

`omega_identity` is native Rust. Secret material is held in zeroizing wrappers,
and callers receive public identities or signed results rather than raw key
bytes in normal operation. Mutations use a process mutex and a channel-scoped
cross-process lock. Public manifests, transaction journals, reset markers, and
recovery-protection records are separate from the secret. [source]

The current system store is a local file:

```text
<Omega data root>/identity/identity.secret
```

It contains the raw 32-byte secret. The parent directory is mode `0700` and the
file is mode `0600` on Unix. Writes use `AtomicWriteFile`. The Windows path
does not set an equivalent ACL in this implementation. There is no at-rest
encryption supplied by the store; its boundary is the operating-system account,
application-data directory, and any full-disk protection. The
`KeyringLocator` remains as a logical compatibility name, but
`IdentityService::system` and `for_channel_data_root` instantiate
`FileSecretStore` at the pinned source. [source]

Omega's provider API keys, OAuth material, and hosted OpenAgents access and
refresh tokens are separately serialized under
`credentials/credentials.json`. That file is also atomic and mode `0600` on
Unix, but intentionally unencrypted at rest. Release-channel namespacing keeps
Dev, Nightly, RC, and Stable records separate. [source]

This corrects the older documentation that describes OS-keyring custody as the
current packaged path. The target may return to a platform keychain or use an
encrypted file vault, but the product and assurance copy must describe the
store that actually ships.

### 4.3 Recovery and raw-key exposure

Omega has two recovery mechanisms in source:

1. NIP-49 encrypted recovery artifacts with a bounded scrypt work factor,
   protected file writes, explicit selection, public-key preview, and
   identity-bound protection records.
2. A newer minimal backup surface that calls
   `export_nsec_for_backup`, renders the raw `nsec`, permits copy, and zeroizes
   the returned Rust string on drop.

The NIP-49 path is the stronger default. The raw-`nsec` surface is still a
real root-key exposure: once copied, the secret can remain in clipboard
history, accessibility tooling, screen capture, crash snapshots, or another
application. Zeroizing the Rust allocation does not recall those copies.
[source]

The backup nudge is deliberately quiet. It appears only after the identity
first accrues value through a public channel post, device grant, or Sarah
session; it is hidden on a fresh profile, dismissible forever, and fail-soft.
That honors the current no-prompt product direction, but it does not prove that
a valuable local identity is recoverable. [source]

### 4.4 Signing and network authentication

Callers submit `AdmittedSigningRequest` values to the service and receive a
signed event. The request binds a request reference, the expected identity,
purpose, timestamp, kind, tags, and content. The service verifies the active
identity before signing. It also exposes bounded NIP-44/NIP-59 private-message
operations and owner-to-agent attestation operations. [source]

Current public tester channels use the identity to sign kind `9` messages and
kind `1984` reports with an `h` group tag. They verify the returned event before
publication. The relay adapter reacts to a NIP-42 challenge, locally validates
the kind `22242` event, authenticates, then retries the exact already-signed
content event. [source]

The remaining weakness is authorization granularity. The generic admitted
event request validates shape and identity, but the long-term product needs a
policy decision that also binds:

- calling subsystem;
- room or tenant;
- allowed kinds;
- signer method;
- user-gesture requirement;
- expiry and replay key; and
- the OpenAgents grant or membership that authorizes the action.

### 4.5 Hosted OpenAgents session and binding

Omega already implements more than relay auth. `openagents_nostr_auth.rs`
builds a NIP-98 kind `27235` proof for the exact
`https://openagents.com/api/omega/auth/session` POST, including URL, method,
and SHA-256 payload tags. The local identity signs the proof. A successful
response returns a hosted access token and canonical OpenAgents user id.
Omega verifies that session through the existing hosted session endpoint,
stores the credentials through its channel-namespaced local credentials
provider, rotates returned access/refresh tokens, and exposes disconnect and
revocation behavior. [source]

`OpenAgentsBinding` uses the same proof to record a public-safe relation between
the Omega public key and an admitted OpenAgents account after an owner-scope
check. Tokens stay in the separate credential record rather than the public
binding record. Network failure, owner-scope refusal, and a valid bound state
are different results. Sarah voice can also obtain a Nostr-issued credential
when no verified bearer session is already present. [source]

This is current implementation, not merely a future seam. Static review does
not prove the endpoint is live for every identity or that the installed account
surface makes the lifecycle legible. The target should retain the exact
NIP-98-to-hosted-session boundary, surface it in the account product, and stop
describing a coarse `Connected` phase as though it also meant relay auth,
membership, or action admission. [limit] [proposed]

### 4.6 Present product gaps

Omega does not currently provide all of the following as one reachable account
product:

- create/import/external-signer choice before identity commitment;
- account list, add, switch, or per-account partitions;
- NIP-46 remote signer login;
- NIP-07 web handoff or NIP-55 Android signer integration;
- a profile editor that can optionally publish kind `0`;
- a relay list with NIP-42 state and exact relay identity;
- initial sync for profile, settings, groups, and encrypted room state;
- an invite resolver spanning Omega NIP-29, Buzz, and Armada profiles;
- device inventory and revocation;
- identity rotation or retirement;
- a complete logout/forget-device distinction;
- a repair surface automatically reached from every custody refusal; or
- consistent installed-product claims about storage and backup.

The retained editor-onboarding identity section can inspect and repair native
custody, but it is no longer the first-run route. It should be harvested into a
dedicated account surface rather than revived as the old editor-setup gate.

## 5. Buzz comparison

### 5.1 Account and login model

Buzz uses a Nostr keypair as the human account. There is no separate Buzz
password or durable server session. Possession of the signing key is local
login; NIP-42 proves key control to the relay; NIP-98 signs individual HTTP
requests. The relay then applies bans, allowlists, and NIP-43 community
membership. [source]

Buzz Desktop resolves identity in this order:

1. development `BUZZ_PRIVATE_KEY`;
2. the `identity` item in the `buzz-desktop` OS keyring;
3. an owner-only `identity.key` fallback when system-keyring support is absent;
4. generation of a new durable key on first launch.

Its machine-onboarding UI then asks the person to create/use the already
generated identity or import an `nsec`, shows the fresh key for backup, and
continues through agent runtime and default-configuration setup. Completion is
scoped to the public key. Lost, locked, reset-failed, and relaunch-required
states are distinct. Imports are written and read back before legacy material
is removed. [source]

Buzz therefore separates **key existence** from **human understanding** better
than current Omega: a key may already exist, but the machine-onboarding flow
still explains it and offers import. The main weakness is that a backup-load
failure exposes **Skip for now**, and the normal backup surface can reveal and
copy the raw `nsec`. [source]

### 5.2 Community onboarding

Buzz persists a restart-safe community-onboarding transaction after machine
identity. Its stages cover invite claiming, connecting, profile, team
introduction, finalizing, and entering the first channel. Completion is scoped
to both public key and relay URL. The relay is not just transport: it is the
community authority and membership boundary. [source]

Omega should adapt the restart-safe transaction and identity-scoped completion.
It should not import Buzz's assumption that one relay's event log becomes
OpenAgents product authority.

### 5.3 Mobile handoff

Buzz's implemented NIP-AB flow is an encrypted, two-device, SAS-confirmed key
copy. Desktop and mobile use ephemeral keys, NIP-44 v2, a strict timeout, and a
six-digit comparison code. After confirmation, Desktop sends the long-lived
root `nsec` to mobile. Both devices then become equal signers. [source]

The introduction and transcript-binding are useful. The payload is not.
Omega should enroll a new device key and transfer a revocable grant, never the
root identity secret.

### 5.4 What Omega should adapt and reject

Adapt:

- public-key-scoped completion;
- explicit lost/locked/conflict/relaunch states;
- import with public-key preview and read-back;
- restart-safe identity and community transactions;
- NIP-42 for relay connection auth;
- NIP-98 for bounded HTTP proofs where applicable;
- ephemeral pairing, SAS comparison, timeout, and replay bounds; and
- agents as separate Nostr identities attested by an owner.

Reject:

- raw root key in an environment variable for normal agent operation;
- raw root-key copy to mobile;
- backup failure as an ordinary completion path;
- one undifferentiated signer for person, device, and agent;
- relay membership as general OpenAgents authorization; and
- Buzz's React/Tauri/Flutter product implementation.

## 6. Armada comparison

### 6.1 Account creation

Armada's account is also a Nostr keypair, with no server registration,
username, email, phone, password, CAPTCHA, or password reset. Its explicit
four-stage account wizard is:

1. generate a key locally;
2. require a successful backup through a password manager or file export;
3. persist and select the local login;
4. optionally publish kind `0`, then create or join a community.

That order is the clearest reference for explaining what a Nostr account is.
The backup gate is stronger than Buzz and Omega because creation does not
complete until a backup path succeeds. The main security weakness is that
Electron and web persist a full local-signing login in renderer
`localStorage`. [source]

### 6.2 Existing-account login

Armada exposes one login dialog with several custody models:

| Method | What Armada stores |
| --- | --- |
| pasted or file-loaded `nsec` | the full root secret in the active login record |
| NIP-07 | public key and extension-backed login metadata |
| NIP-46 `bunker://` | local client secret, bunker public key, and relay set |
| NIP-46 `nostrconnect://` | pairing/client secret, user and bunker keys, and relay set |
| Android NIP-55 | signer package name and public key |

This is the product parity target for signer choice. It allows the user to keep
the root key outside the client. NIP-46 still leaves a live client capability
on the device, so its storage, expiry, and revocation remain security-critical.
[source]

### 6.3 Initial sync and signer consent

An existing Armada login runs a bounded recovery gate. It attempts to recover
settings, NIP-29 group lists, recent NIP-29 messages, Concord v1 state, and the
encrypted Concord v2 community list. The overall wait has a 30-second ceiling;
failures fall back to cache/defaults and runtime recovery continues. A brand-new
identity skips this remote sync. [source]

Armada also queues a demand-driven bulk-decryption consent. This prevents an
extension, remote signer, or Android signer from receiving a storm of prompts.
The choice is app-wide and local; declining leaves content behind explicit
decrypt actions. [source]

Omega should adapt both patterns: a bounded, best-effort identity hydration
gate and an explicit policy for repeated external-signer operations.

### 6.4 Accounts, platforms, and logout

Armada supports multiple logins, profile resolution, account switching,
per-login signer caches, and public-key-partitioned wallet state. Final logout
best-effort purges login records, stores, caches, drafts, read state, and
decrypted content before returning to the welcome surface. It correctly cannot
retract relay copies. [source]

The same React product runs on Electron, web/PWA, Android, and iOS, but custody
differs:

- Electron/web: renderer `localStorage`;
- Android/iOS: Capacitor secure storage;
- Android: Credential Manager backup and NIP-55;
- iOS: secure login storage but an apparent create-account backup blocker at
  the audited source pin.

Omega should seek behavioral parity through shared contracts and fixtures, not
by sharing a renderer or pretending every platform has the same signer APIs.

## 7. Comparison matrix

| Capability | Omega now | Buzz | Armada | Omega target |
| --- | --- | --- | --- | --- |
| Fresh key | Silent background Rust generation | Generated on first launch, then explained in machine onboarding | Explicit wizard action | Background candidate, explicit activation before durable use |
| Existing identity | Native import/recovery code exists, not first-run | `nsec` import | `nsec`, file, NIP-07, NIP-46, NIP-55 | `nsec`/NIP-49 plus NIP-46 desktop; platform adapters later |
| Local custody | Raw 32-byte local file; mode `0600` on Unix | OS keyring or owner-only fallback file | Electron/web `localStorage`; mobile secure storage | Rust vault backed by platform secure storage or encrypted file |
| Default backup | Quiet raw-`nsec` nudge after value; NIP-49 machinery exists | Raw `nsec`; failure may skip | Mandatory password-manager/file backup | Mandatory NIP-49 or verified platform backup before high-value use |
| Relay auth | NIP-42-capable transport on bounded surfaces | Mandatory NIP-42 | Signer-dependent relay auth | One observable NIP-42 state per relay connection |
| HTTP/hosted auth | NIP-98 exact-request proof already mints and verifies a hosted session; tokens persist in the local credential file | Per-request NIP-98 | Not the primary account model | Retain exact proof/session separation, add account lifecycle UI, and harden credential custody |
| Community entry | Limited tester channels; full NIP-29 spec proposed | Restart-safe relay/invite flow | Concord, Buzz, and NIP-29 resolver | Profile-aware resolver with explicit authority label |
| Initial sync | No unified identity hydration gate | Community-specific restore | Bounded multi-protocol SyncGate | Bounded Rust hydration plan with partial-result receipt |
| Multiple accounts | No | Desktop is principally one machine identity; mobile stores community records with copied keys | Yes | Yes, with strict per-account partitions |
| External signer | No reachable product path | No NIP-46 in pairing path | NIP-07/46/55 | NIP-46 first; NIP-07 web and NIP-55 Android adapters |
| Device pairing | Grant work exists; no complete account product | Copies root `nsec` over SAS flow | NIP-46 QR is signer pairing, not device grant | Ephemeral SAS enrollment of a revocable device key |
| Agent identity | Owner-attestation primitives exist | Separate agent key with NIP-OA/NIP-AA | Agents are not the central account flow | Separate agent key, bounded grant, owner attestation |
| Logout | No complete account-level semantics | Local state; copied mobile key remains valid | Remove one account or purge final account | Lock, sign out, forget device, and retire identity are distinct |
| Decrypted cache policy | Not unified with account UI | Product-specific encrypted records | Persistent plaintext decrypt cache | Account-partitioned, disclosed, expirable, purgeable |

## 8. Target identity and authority model

### 8.1 Four identities that must not collapse

Omega should distinguish:

1. **Person identity** — the portable Nostr public key the person recognizes.
2. **Device identity** — a per-installation key that can hold revocable grants.
3. **Agent identity** — a separate key for Omega Agent, Sarah, or another
   admitted agent, attested and constrained by the person or hosted authority.
4. **Hosted OpenAgents user** — the canonical application account and session
   subject, which may link to one or more verified Nostr identities.

A local-only person can use Omega without a hosted account. Linking the person
identity to OpenAuth is a separate, explicit action governed by the
cross-product authentication proposal.

### 8.2 Five meanings of “authenticated”

The UI and code must name which proof exists:

| State | Proof | What it does not prove |
| --- | --- | --- |
| Local signer ready | Omega can obtain a valid signature from the selected signer | Relay access, group membership, hosted account ownership, or action permission |
| Relay authenticated | A fresh NIP-42 AUTH event passed for one connection | Membership, moderation role, or hosted session |
| Group admitted | The selected room authority recognizes membership or an invite claim | OpenAgents account ownership or command authority |
| Hosted account linked | OpenAgents verified key control and linked it to a canonical user | Permission for every relay or local action |
| Action authorized | The relevant policy admitted this exact operation | Truth, quality, payment, or release acceptance |

These states should be separate types and separate UI phrases. “Connected”
must not stand in for all five.

### 8.3 Signer types

The Rust domain model should support:

```text
LocalNative
RemoteNip46
BrowserNip07
AndroidNip55
DeviceGrant
AgentGrant
```

Only supported methods render on a host. A signer record contains no raw root
secret. It names an account public key, signer type, capability set, storage
reference, creation time, last successful use, and recoverability state.

## 9. Canonical Omega journeys

### 9.1 Fresh desktop profile

1. Startup silently creates a **candidate local identity** as it does now.
2. Omega opens immediately. The account control shows a short public
   fingerprint and “Set up identity,” not an unexplained logged-in state.
3. Local exploration does not require a prompt.
4. The first durable identity-bearing action opens activation:
   - **Keep this new identity**
   - **Use an identity I already have**
   - **Use a signer on another device**
5. The keep path explains the public key, root secret, lack of password reset,
   and what signatures do and do not prove.
6. The person creates a NIP-49 artifact or completes a verified platform backup.
   Raw-`nsec` reveal lives under an advanced escape hatch with a fresh warning
   and clipboard-expiry attempt where the platform supports it.
7. Omega re-inspects custody, binds activation completion to the public key,
   and resumes the original action.
8. Profile and community setup are offered next but are skippable.

The original action must be held as a typed intent and resumed only if its
identity, account generation, destination, and authorization still match.

### 9.2 Existing background-created identity

Existing installs must not rotate or replace their key.

1. Detect a ready identity without an activation record.
2. Classify it as `CandidateExisting`.
3. Preserve all existing signatures, memberships, grants, and encrypted data.
4. Show the same activation choice before the next durable identity-bearing
   action.
5. If the person selects an imported or remote identity, present both public
   fingerprints and explain that switching changes authorship and may hide
   identity-encrypted history.
6. Commit the switch through a restart-safe transaction.
7. Keep the previous local identity disabled but recoverable until the person
   explicitly forgets it.

A migration must never infer that “no activation record” means “safe to
delete.” It means only that the newer product ceremony was not recorded.

### 9.3 Import or recover a local identity

Supported inputs should be:

- NIP-49 `ncryptsec` file and password, recommended;
- raw `nsec` paste or tightly bounded key file, advanced;
- an existing locally stored candidate selected by public fingerprint.

The view uses the existing `SecureInput` and opaque prepared-candidate model.
It derives and shows the public identity before mutation. A different public
key requires explicit selection. The service journals the chosen identity,
writes it, reads it back, and only then marks the account active.

The import surface must not place the secret in ordinary text widgets, undo
history, logs, telemetry, serialized workspace state, or TypeScript.

### 9.4 NIP-46 remote signer

1. The user pastes `bunker://` or starts a `nostrconnect://` QR/deep-link flow.
2. A Rust NIP-46 adapter creates a disposable client key and a bounded
   rendezvous relay set.
3. The UI shows the expected signer, requested methods, event kinds, relays,
   and expiry.
4. Omega verifies the acknowledgement, obtains the user public key, and stores
   only the encrypted client capability and public metadata in native custody.
5. The account becomes active only after a signed challenge verifies under the
   reported user key.
6. Explicit rejection fails immediately. Timeout, silence, wrong author,
   wrong request id, wrong relay, duplicate response, and malformed ciphertext
   are distinct failures.
7. Logout deletes the disposable NIP-46 client key. Revocation is visible even
   when the remote signer is unavailable.

The first profile should request only the methods and kinds Omega needs. Bulk
decrypt consent is separate from login consent.

### 9.5 Join a room or community

The destination resolver accepts an exact typed input:

- relay-qualified NIP-29 group address;
- NIP-29 relay URL;
- Buzz invite;
- Armada Concord v1 or v2 invite, when the corresponding profile is installed;
- an Omega/OpenAgents invite.

Before commitment it shows:

- protocol/profile;
- authoritative relay or service;
- room identifier;
- public/private visibility;
- requested signing operations;
- terms or age policy where applicable;
- recoverability implications; and
- whether the action is portable to Buzz, Armada, web, or mobile.

The join transaction persists before network mutation and records each
authority result independently. Adding a relay, authenticating with NIP-42,
claiming an invite, joining a NIP-29 group, and receiving an OpenAgents grant
are not one boolean.

### 9.6 Initial hydration

After an imported, recovered, switched, or remote-signer login, Omega runs a
bounded hydration plan:

1. kind `0` profile;
2. relay preferences and the selected NIP-29 group list;
3. recent membership and room metadata;
4. recent room pages within explicit limits;
5. OpenAgents device and account-link state, when linked;
6. Buzz or Armada profile-specific state, when those adapters are enabled.

The gate has an overall deadline and per-source deadlines. It returns a
structured partial result. Cached/local state may open after the deadline;
background recovery continues. A fresh unpublished candidate skips remote
hydration.

### 9.7 Add and switch accounts

The account switcher shows public fingerprint, optional profile, signer type,
recovery state, and last successful signer use. Switching:

- changes the active signer generation;
- partitions drafts, decrypted caches, wallets, relays, room lists, and signer
  objects;
- cancels or revalidates pending signing intents; and
- runs bounded hydration for the selected account.

No signer object or decrypted cache entry may be reused across public keys.

### 9.8 Lock, sign out, forget, and retire

These are different actions:

- **Lock** drops in-memory signer capability until local re-authentication.
- **Sign out** deselects an account and deletes disposable remote-signer
  sessions while retaining recoverable account metadata.
- **Forget this device** deletes local device grants, local signer material,
  account-partitioned plaintext, and caches after verified read-back.
- **Retire identity** is a separate protocol/policy action. It cannot retract
  events already held by relays or peers and must not be presented as “delete
  account.”

The final-account purge should follow Armada's breadth but report partial
failure instead of claiming success after a best-effort silent error.

### 9.9 Pair mobile or web

Use Buzz's ephemeral exchange shape with a different payload:

1. Omega and the target create ephemeral keys.
2. The QR contains only the ephemeral introduction, endpoint, version, expiry,
   and random secret.
3. Both sides derive and compare a short authentication string.
4. The target creates its own durable device key in platform custody.
5. Omega grants that device a narrow, expiring, generation-bound capability.
6. A server or relay records one-time redemption and revocation state where
   the capability requires hosted authority.
7. The root person `nsec` never crosses the pairing channel.

Web should prefer NIP-07 or NIP-46 and hold no local root key by default.
Android may add NIP-55. iOS should use NIP-46 or a future audited native signer
bridge; it must not claim NIP-55 parity.

## 10. Rust-first architecture

### 10.1 Native crates and services

The primary stack remains Rust and GPUI:

| Component | Responsibility |
| --- | --- |
| `omega_identity` | Local custody, manifests, import/recovery, signing, reset, public identity, and signer-independent contracts |
| `omega_signer_broker` | Select local/NIP-46/platform signer, enforce capability and gesture policy, correlate requests, verify returned signatures |
| `omega_account` | Account records, active generation, activation, switching, partitions, lock/logout/forget lifecycle |
| `omega_nostr_auth` | NIP-42 connection state, NIP-98 proof construction, freshness, relay normalization, and auth receipts |
| `omega_identity_sync` | Bounded profile/relay/group hydration and partial-result receipts |
| `omega_invites` | Typed invite parsing, preview, authority labeling, and restart-safe join transactions |
| GPUI account surface | Activation, account switcher, signer consent, backup, repair, relay status, devices, and logout |

Names for new crates are illustrative. Existing files should absorb work where
the logical boundary already exists.

`omega_identity` remains the only local root-secret holder. The signer broker
must work in terms of capabilities and signed results, not a common
`get_secret()` abstraction.

### 10.2 TypeScript boundary

TypeScript is appropriate for:

- web NIP-07 detection and calls;
- web deep-link and QR presentation;
- shared JSON Schema fixtures and generated bindings;
- interoperability test clients; and
- a future web/mobile UI that consumes public-safe account projections.

TypeScript must not become the desktop root-key custody layer. The GPUI view
receives public account state, signer prompts, and opaque operation handles.
It receives a raw secret only in the explicit advanced backup boundary, and the
preferred target removes even that from the ordinary path.

### 10.3 Account state machine

The durable state should be at least:

```text
Absent
  -> CandidateLocal
  -> Activating
  -> Active

Active
  -> Locked
  -> Switching
  -> SignedOut
  -> ForgetPending
  -> Forgotten

Any mutable state
  -> RepairRequired
  -> Conflict
```

Signer availability is orthogonal:

```text
Ready | UserApprovalRequired | Offline | Rejected | Revoked | Lost
```

Hydration is also orthogonal:

```text
NotStarted | Running | Partial | Complete | Offline
```

One flattened “logged in” boolean cannot represent these facts safely.

### 10.4 Signing authorization

Every signing request should bind:

```text
request_ref
account_ref
account_generation
signer_ref
calling_subsystem
purpose
event_kind
room_or_tenant
relay_or_http_origin
content_digest
capability_ref
user_gesture
issued_at
expires_at
```

The broker verifies the returned event id, signature, author, kind, tags,
content digest, request correlation, and active generation. A relay acceptance
cannot retroactively authorize a request the broker refused.

## 11. Security requirements

### 11.1 Custody

- Replace the current raw file store with an admitted platform secure-storage
  adapter or an encrypted file vault whose unlock and recovery policy is
  explicit.
- Preserve owner-only permissions and atomic write/read-back even when a
  platform keychain is used.
- Keep release-channel namespaces separate.
- Never fall back silently from secure storage to plaintext.
- Treat NIP-46 client keys and device-grant keys as secrets even though they
  are not the root person key.
- Redact secret-shaped fields, URIs, paths, clipboard values, and decrypted
  payloads from logs, telemetry, crash records, receipts, and debug output.

### 11.2 Backup and recovery

- Make NIP-49 or a verified platform recovery destination the default.
- Require backup confirmation before the first high-value durable action for a
  locally held root key.
- Permit postponement only while the identity has no durable external value.
- Re-prompt after value accrues if the person previously chose postponement;
  an indefinite dismissal is not sufficient protection for the target.
- Keep raw-`nsec` reveal advanced, time-bounded in the view, and excluded from
  screenshots where the platform permits.
- Explain that recovery artifacts protect identity access, not relay
  availability or locally cached plaintext.

### 11.3 External signers

- Pin NIP-46 protocol behavior and relay normalization.
- Store the minimum client capability.
- Ask separately for login proof, event signing, encryption, and bulk decrypt.
- Verify all returned signatures locally.
- Delete disposable client keys on logout.
- Expose signer availability and last successful use without treating
  availability as authorization.

### 11.4 Content and cache

- Partition ciphertext, plaintext, drafts, media, and signer caches by account.
- Make persistent plaintext caching visible in Settings.
- Give expiring/private messages a no-persistent-plaintext policy.
- Purge account plaintext on forget-device and verify the purge result.
- Do not imply that secure key custody encrypts every application cache.

### 11.5 Network and authority

- NIP-42 authenticates one relay connection only.
- NIP-98 proves one exact HTTP request only.
- NIP-29 membership is relay-scoped.
- Buzz and Armada adapters must label which relay/profile owns membership and
  moderation.
- Hosted OpenAgents sessions remain issued by the admitted hosted auth system.
- Signatures prove exact-key authorship, not truth, permission, payment, code
  quality, or release acceptance.

## 12. Delivery path

### OMEGA-AUTH-00: freeze current truth and target contracts

Deliver:

- source-pinned current-state record;
- account, signer, auth-state, activation, and signing-request schemas;
- storage and authority claims aligned with shipping source;
- negative fixtures for all ambiguous “authenticated” states.

Exit:

- documentation no longer claims Keychain custody while the system path is
  `FileSecretStore`;
- one glossary distinguishes local signer, relay auth, group admission,
  hosted link, and action authorization.

### OMEGA-AUTH-01: adopt the background identity

Deliver:

- `CandidateLocal` versus `Active` state;
- migration of every existing ready identity without key rotation;
- public fingerprint in the account control;
- typed interception of the first durable identity-bearing action.

Exit:

- a fresh profile still reaches the front door without a prompt;
- no public post, join, grant, attestation, or hosted link can use an
  unactivated candidate;
- an existing identity retains its public key and signed history.

### OMEGA-AUTH-02: activation and durable recovery

Deliver:

- keep/import/remote-signer activation surface in GPUI;
- NIP-49-first backup;
- recovery confirmation bound to the public identity;
- repair entry points for all custody refusal states;
- removal of indefinite backup dismissal after durable value.

Exit:

- a local identity cannot accrue new high-value external state without a
  recorded recovery decision;
- cancellation resumes nothing and leaks no secret;
- recovery/import failures remain restart-safe.

### OMEGA-AUTH-03: account home and multiple accounts

Deliver:

- identity dashboard;
- add, switch, lock, sign out, forget-device;
- per-account cache, relay, room, wallet, and signer partitions;
- final-account purge report.

Exit:

- switching accounts cannot reuse a signer or plaintext partition;
- pending operations from an old account generation are refused;
- forget-device reports and retries partial deletion.

### OMEGA-AUTH-04: NIP-46 desktop signer

Deliver:

- Rust `bunker://` and `nostrconnect://` adapter;
- permission preview;
- disposable client-key custody;
- response correlation and signature verification;
- explicit rejection, timeout, revoke, and logout behavior.

Exit:

- Omega can post through a remote signer without receiving the user `nsec`;
- only declared methods and kinds are requested;
- a stale response cannot complete a newer request.

### OMEGA-AUTH-05: relay and hosted authentication visibility

Deliver:

- per-relay NIP-42 state and receipts;
- account UI for the existing bounded NIP-98 proof and hosted session;
- custody hardening for hosted access and refresh tokens;
- Nostr-to-hosted-account binding governed by the cross-product proposal;
- UI copy that names each authority.

Exit:

- relay auth cannot mint a hosted session;
- a hosted link cannot silently join a group;
- every status can be falsified independently.

### OMEGA-AUTH-06: profile and bounded hydration

Deliver:

- optional kind `0` editor;
- relay and group-list settings;
- bounded identity hydration with cache/default fallback;
- external-signer bulk-decrypt consent.

Exit:

- fresh candidates skip pointless remote recovery;
- imported identities receive a structured complete/partial/offline result;
- decline of bulk decrypt leaves content locked without repeated prompt storms.

### OMEGA-AUTH-07: NIP-29, Buzz, and Armada entry

Deliver:

- typed invite resolver;
- restart-safe join transaction;
- standards-first NIP-29 profile;
- Buzz compatibility profile;
- Armada profile boundaries from the Armada integration plan;
- explicit authority and portability labels.

Exit:

- the same account can enter supported destinations without changing its key;
- unsupported profile data is preserved as opaque evidence, not misprojected;
- no adapter grants OpenAgents authority by inference.

### OMEGA-AUTH-08: device enrollment and mobile/web adapters

Deliver:

- ephemeral QR/deep-link exchange;
- two-screen SAS;
- per-device key and revocable grant;
- device inventory;
- web NIP-07/NIP-46 adapter;
- Android NIP-55 adapter when mobile scope is admitted.

Exit:

- no root `nsec` crosses the pairing channel;
- revoking one device does not rotate the person identity;
- web holds no root key by default;
- platform capability differences are visible and tested.

### OMEGA-AUTH-09: agent identity and assurance

Deliver:

- separate Omega Agent and Sarah keys;
- owner attestation and bounded agent grants;
- NIP-AA relay authentication where admitted;
- installed-candidate security matrix and secret tripwires.

Exit:

- no agent signs as the person by default;
- every agent-auth path names owner, agent, scope, expiry, and revocation;
- installed builds prove storage, recovery, switching, logout, and negative
  auth boundaries on each supported host.

## 13. Build order and dependencies

The recommended sequence is:

```text
AUTH-00
   |
AUTH-01 -> AUTH-02 -> AUTH-03
              |          |
              v          v
           AUTH-04 -> AUTH-05 -> AUTH-06
                                  |
                                  v
                               AUTH-07
                                  |
                                  v
                               AUTH-08
                                  |
                                  v
                               AUTH-09
```

The first valuable slice is `AUTH-00` through `AUTH-02`. It turns the silent
key into an understood, recoverable identity without restoring a blocking
startup ceremony. Multiple accounts and remote signers should follow before
the interoperability adapters, because Buzz/Armada parity without signer and
account partitions would hard-code the wrong product boundary.

## 14. Product decisions required

1. Is activation required before the first public post only, or before every
   durable external action including device grants and Sarah sessions?
2. Which secure store is admitted for macOS, Windows, and Linux?
3. Is NIP-49 sufficient as the first required recovery path, or must a
   platform-managed backup also exist?
4. May a raw `nsec` ever appear in ordinary product UI, or only behind an
   advanced recovery action?
5. Does Omega support multiple person identities in the first account release?
6. Which NIP-46 methods and event kinds form the first permission profile?
7. Which system owns the device registry in local-only mode?
8. Which Buzz and Armada invite/profile versions are compatibility targets?
9. What local plaintext is retained after sign out versus forget-device?
10. What is the retirement protocol for a compromised person key?

## 15. Non-goals

- Replacing OpenAuth with NIP-42
- Treating Nostr Wallet Connect as identity login
- Copying Armada's React/Electron client into Omega
- Copying Buzz's relay authority model
- Copying a root identity key to every device
- Requiring a hosted OpenAgents account for local Omega use
- Claiming social, threshold, or custodial recovery before it exists
- Making a kind `0` profile a registration record
- Making a valid signature sufficient authorization
- Reopening the removed first-launch identity wizard without a new owner
  decision

## 16. Verification matrix

At minimum, automated and installed tests must cover:

| Area | Required proof |
| --- | --- |
| Migration | An existing background-created identity keeps the same public key and can activate without rotation |
| Concurrency | Multiple windows and processes cannot create or activate different identities |
| Storage | Actual packaged secret location, permissions/encryption, namespace, lock behavior, and fallback policy |
| Recovery | NIP-49 export/import, wrong password, unsafe file, duplicate candidate, conflicting candidate, crash, and read-back |
| Secret handling | No secret in logs, telemetry, receipts, workspace serialization, ordinary inputs, snapshots, or TypeScript projections |
| Signing | Wrong account generation, kind, room, origin, content digest, request id, expiry, or signer response is refused |
| NIP-42 | Wrong relay, challenge, timestamp, signature, and replay are refused |
| NIP-46 | Wrong author, response id, relay, ciphertext, method, kind, expiry, duplicate, and explicit denial are distinct |
| Accounts | Switching partitions signer, drafts, cache, rooms, wallet, and pending operations |
| Logout | Lock, sign out, forget-device, and retire have different effects and honest failure reporting |
| Invites | NIP-29, Buzz, Armada, malformed, stale, banned, terms-required, and unsupported-profile cases |
| Hydration | Complete, partial, timeout, offline, corrupt cache, and external-signer denial |
| Pairing | Wrong SAS, leaked/expired QR, replay, peer substitution, crash after redemption, and revocation |
| Authority | Relay auth, group membership, hosted link, and action authorization cannot substitute for one another |

Static tests are necessary but insufficient. Each supported packaged host needs
an installed journey with the real secure store, clipboard, deep-link, signer,
and filesystem behavior.

## 17. Final recommendation

Do not choose between Omega's current zero-friction startup and a real Nostr
account product. Keep background generation as provisioning, then add explicit
activation at the first durable identity-bearing boundary.

The best target combines:

- Omega's Rust transaction, recovery, and admitted-signing foundation;
- Buzz's identity-scoped state, relay-auth rigor, and restart-safe onboarding;
- Armada's signer choice, mandatory backup, bounded sync, multi-account
  experience, and cross-community entry; and
- OpenAgents' stricter separation of signature, membership, session, device,
  agent, and action authority.

That path gets Omega closer to Buzz and Armada product parity without inheriting
their root-key exposure, renderer custody, shared-device-key, or relay-authority
assumptions.
