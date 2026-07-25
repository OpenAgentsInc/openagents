# Nostr-native authentication for OpenAgents

- Date: 2026-07-25
- Class: source-grounded architecture proposal
- Status: proposal only
- Target: OpenAgents web, Omega desktop, and OpenAgents mobile
- OpenAgents source revision: `1f6f0f4fc282c39c2e46372d513ab463e36285a0`
- NIPs source revision: `db5fe3de8c5d1443b634c9bbf66ecb004f337057`
- Buzz teardown revision: `c664db99efd96de49d226c08a2f0e3ae11105103`
- Audience: human
- STE profile: base descriptive

## 1. Executive recommendation

Add Nostr as an authentication method for one canonical OpenAgents user.
Keep OpenAuth as the only application session issuer in the first release.
Keep GitHub and email sign-in for users who prefer conventional authentication.
Do not make a user create a Nostr identity to use OpenAgents.

Use a Nostr signature to prove control of a Nostr public key.
After this proof, issue the same OpenAuth session that the product uses now.
For web login, use a strict OpenAgents profile of NIP-98.
For relay connections, use NIP-42 only for relay authentication.
Do not use NIP-42 as a web login protocol.

Use NIP-07 first when a browser signer is present.
Use NIP-46 when the user selects a remote signer.
Use NIP-55 on Android when a compatible signer is present.
Keep native keys behind a narrow signer interface.
Do not expose a mnemonic, an `nsec`, or raw private-key bytes to a view layer.

Do not copy a root identity key through a QR flow.
This is the main lesson from the updated Buzz teardown.
Adapt its ephemeral exchange, short code, and two-device confirmation.
Change the transferred object from a root secret to a device grant.
Each device must have its own revocable key and capability set.

Keep login, relay access, device authority, and wallet authority separate.
Nostr Wallet Connect is not a login protocol.
Its useful lesson is the use of a unique key for each connection.
OpenAgents must not use a wallet connection as proof of account ownership.

## 2. Decision labels

This document uses four labels.

- **[STANDARD]** identifies a fact from a selected NIP revision.
- **[EXISTS]** identifies current OpenAgents code or an accepted OpenAgents plan.
- **[PROPOSED]** identifies a design in this document.
- **[OPEN]** identifies a decision that still needs an owner or product decision.

A standard fact does not grant OpenAgents product authority.
Current code does not prove a released product journey.
A proposal does not admit implementation or release.

## 3. Source record and limits

### 3.1 OpenAgents sources

The source review used these current OpenAgents files:

- `apps/openagents.com/workers/api/src/index.ts`
- `apps/openagents.com/workers/api/src/auth/session.ts`
- `apps/openagents.com/workers/api/src/auth/mobile-session.ts`
- `apps/openagents.com/workers/api/src/identity-db.ts`
- `packages/khala-sync-server/migrations/0028_identity_auth_domain.sql`
- `packages/khala-sync-server/migrations/0042_identity_hard_cut.sql`
- `apps/openagents-desktop/src/desktop-session-pkce.ts`
- `apps/openagents-desktop/src/desktop-session-vault.ts`
- `apps/openagents-mobile/src/auth/native-session-pkce.ts`
- `apps/openagents-mobile/src/auth/native-session-vault.ts`
- `packages/environment-auth/src/dpop.ts`
- `packages/environment-auth/src/index.ts`
- `packages/sovereign-identity/src/machinery/signer.ts`
- `packages/sovereign-identity/src/machinery/web-signer-bridge.ts`
- `packages/sovereign-identity/src/machinery/mobile-custody.ts`
- `packages/sarah/src/issue31-nostr/records.ts`
- `docs/omega/2026-07-23-identity-first-onboarding-roadmap.md`
- `docs/omega/2026-07-24-openagents-mobile-omega-adaptation-audit.md`
- `docs/teardowns/2026-07-21-buzz-teardown.md`

The review also used the Nostr pivot analysis as strategy evidence.
It did not treat that analysis as current product authority.

### 3.2 NIP sources

The requested path `/Users/christopherdavid/work/projects/repos/nip` did not
exist. The available canonical clone was
`/Users/christopherdavid/work/projects/repos/nips`.
Its `master` revision matched the remote revision during this review.

The review read these files at that exact revision:

- `07.md`
- `42.md`
- `46.md`
- `47.md`
- `49.md`
- `55.md`
- `98.md`

The review also checked the canonical remote files.
The local and remote revisions matched at review time.

### 3.3 Buzz source limit

The updated OpenAgents Buzz teardown includes a full NIP inventory.
It also includes the NIP-AB QR device-pairing design.
The teardown uses Buzz revisions from 2026-07-21.
The current Buzz remote had later commits during this review.
This proposal uses only the teardown claims and its identified source limits.

NIP-AB is a Buzz draft.
It is not a standard NIP in the canonical NIPs repository.
OpenAgents can adapt a security pattern from it.
OpenAgents must not describe it as an ecosystem standard.

## 4. Current OpenAgents state

### 4.1 Hosted identity and OpenAuth

**[EXISTS]** The OpenAgents issuer uses OpenAuth.
It supports GitHub OAuth and an email one-time code.
The `UserSubject` schema permits only `github` and `email` today.
The issuer creates access and refresh tokens with one common session lifetime.

**[EXISTS]** Browser clients store the session in secure cookies.
The server verifies the cookie tokens and refreshes them when required.
The browser session boundary persists the verified user before route use.

**[EXISTS]** Desktop uses a public-client OpenAuth flow.
It uses GitHub, S256 PKCE, and an exact loopback callback.
The desktop host stores access and refresh tokens with Electron safe storage.
The renderer does not own these credentials.

**[EXISTS]** Mobile also uses a public-client OpenAuth flow.
It uses GitHub, S256 PKCE, and the `openagents://auth` redirect.
It stores the verified session in this-device-only native secure storage.
It supports access-token and refresh-token revocation.

**[EXISTS]** The canonical identity tables are `users` and `auth_identities`.
Cloud SQL is authoritative for these tables.
`auth_identities` already has a unique key on provider and provider subject.
This shape can support a Nostr identity without a second user table.

### 4.2 Current Nostr signer boundary

**[EXISTS]** The sovereign identity package has a narrow signer interface.
It permits these operations:

- get a public key
- sign an admitted event
- use NIP-44 encryption and decryption
- create a NIP-98 token
- read a public identity manifest

The normal interface has no raw-key export method.
Custody export is a separate interface.
The web bridge rejects raw-key-shaped fields.
It already declares a NIP-46 configuration seam.

**[EXISTS]** The current identity roadmap distinguishes three roles.
They are a device-local identity, a sovereign person identity, and a hosted
OpenAuth identity.
It states that an `npub` is not an OpenAuth owner ID.
It also states that a wallet key is not a person identity.

**[EXISTS]** The accepted Omega identity plan recommends a new Nostr-only
identity profile.
It rejects the legacy shared Nostr and Spark derivation profile for fresh use.
This proposal keeps that rule.

### 4.3 Current device grant patterns

**[EXISTS]** The issue 31 records already define a device grant lifecycle.
The lifecycle has a request, challenge, response, scoped grant, renewal, and
revocation.
It binds a device key, a host key, scopes, a generation, and an expiry.

**[EXISTS]** The environment auth package already defines DPoP-bound grants.
It uses a non-extractable P-256 key for proof of possession.
It binds each grant to an owner, environment, scope set, key thumbprint, and
expiry.
Scope exchange can only reduce authority.

These two designs are useful source material.
They do not yet form a general user authentication system.

## 5. Standards assessment

The canonical NIPs repository uses explicit status labels on many NIPs.
Most selected authentication NIPs are still marked `draft`.
The product must pin a revision and a local protocol profile.
It must not claim that these NIPs are final.

| Protocol | Current source status                 | Correct OpenAgents use                      | Incorrect use                         |
| -------- | ------------------------------------- | ------------------------------------------- | ------------------------------------- |
| NIP-07   | `draft`, `optional`                   | Browser signer discovery and signing        | Assume every browser has a signer     |
| NIP-42   | `draft`, `optional`, `relay`          | Authenticate one relay connection           | Create a web or API session           |
| NIP-46   | No `final` label in the selected file | Remote signing with a disposable client key | Copy the user key to the client       |
| NIP-49   | `draft`, `optional`                   | Explicit encrypted backup or import         | Default online custody or QR transfer |
| NIP-55   | `draft`, `optional`                   | Android external signer calls               | Cross-platform native signer standard |
| NIP-98   | `draft`, `optional`                   | Prove one exact HTTP request                | Long-lived bearer session             |
| NIP-47   | `draft`, `optional`                   | Wallet connection only                      | Login or owner identity proof         |

### 5.1 NIP-07

**[STANDARD]** NIP-07 defines the optional `window.nostr` object.
Its required methods get a public key and sign an event.
NIP-44 operations are optional.

**[PROPOSED]** OpenAgents must detect the object before use.
The user must select the Nostr sign-in action.
The application must show the public key and request purpose before signing.
The application must not send repeated hidden signing requests.

NIP-07 has enough interoperability for an optional first web path.
It does not define account linking, session issuance, or recovery.
OpenAgents must define and test those product rules.

### 5.2 NIP-42

**[STANDARD]** NIP-42 uses a relay challenge and a kind `22242` event.
The event binds the challenge and relay URL.
The relay session lasts for the connection.

**[PROPOSED]** Use NIP-42 only when a client connects to a relay.
The relay must still apply its membership and resource policy.
A valid NIP-42 event proves key control for that connection.
It does not prove OpenAgents account ownership or action authority.

NIP-42 is sufficiently clear for an optional authenticated relay profile.
The relay and client must pin URL normalization and freshness rules.
OpenAgents must not reuse it for HTTP login.

### 5.3 NIP-46

**[STANDARD]** NIP-46 separates three public keys.
They are the user key, the remote-signer key, and the client key.
The client key is disposable and should be deleted on logout.
Requests and responses use kind `24133` and NIP-44.
The connection can request permissions by method and event kind.

The selected NIP-46 file has an active changes section.
It has no `final` status label.
The product must pin its exact field and URI behavior.

**[PROPOSED]** Use NIP-46 for remote signing and hardware-signer bridges.
For login, request only one login-proof signature.
For continued workroom use, create a separate signer connection.
Give that connection only the required methods and event kinds.
The signer must show the site, device, permissions, and expiry.

NIP-46 has useful interoperability.
Its permission text is not a complete capability system.
OpenAgents must add server-side grants, expiry, generation, and revocation.

### 5.4 NIP-49

**[STANDARD]** NIP-49 encrypts a private key with a password.
It uses scrypt and XChaCha20-Poly1305.
The NIP warns against publication of encrypted private keys.

**[PROPOSED]** Do not use NIP-49 in normal login.
Permit it only in an explicit backup or import journey.
Mark its status as draft in the user interface.
Require an offline recovery warning and an owner confirmation.

NIP-49 is not a device enrollment protocol.
It does not give key rotation or device revocation.

### 5.5 NIP-55

**[STANDARD]** NIP-55 defines an Android signer application.
It supports Android intents, a content resolver, and a web callback.
It can request permissions by method and event kind.

**[PROPOSED]** Use NIP-55 only on Android.
Bind all callbacks to the expected package and request identifier.
Use a foreground approval for login and account-link actions.
Do not remember broad decryption or signing permission by default.

NIP-55 is sufficiently clear for an optional Android integration.
It is not an iOS standard.
It does not replace NIP-46 for cross-device signing.

### 5.6 NIP-98

**[STANDARD]** NIP-98 uses a kind `27235` event for HTTP authentication.
It binds an absolute URL and HTTP method.
It can bind the request-body hash in a `payload` tag.
It requires a short freshness window.

**[PROPOSED]** Require the `payload` tag for all OpenAgents Nostr auth posts.
The body must contain a server challenge and a single-use attempt reference.
The server must compare the exact external URL and method.
It must verify the signature, payload hash, challenge, expiry, and unused state.

NIP-98 is suitable for a bounded login proof.
It is still draft and optional.
The OpenAgents profile must define canonical body bytes and proxy URL handling.

### 5.7 Nostr Wallet Connect

**[STANDARD]** NIP-47 connects an application to a Lightning wallet service.
It recommends a unique client key for each connection.
It permits separate connections with separate limits.
It does not use the user's main key for the wallet connection.

**[PROPOSED]** Reuse only this connection-separation lesson.
Do not use NIP-47 to log in.
Do not infer person identity from a wallet connection.
Do not put wallet permission in an authentication device grant.

## 6. Product goals

The design has these goals:

1. A Nostr user can sign in without GitHub or email.
2. A conventional user can continue to use GitHub or email.
3. One user can link both methods to one canonical account.
4. A user can remove and revoke one device without rotating a root key.
5. Web, desktop, and mobile use one server identity model.
6. A signer can remain outside an OpenAgents client.
7. A malicious relay cannot create an OpenAgents session.
8. A signature cannot silently grant product authority.
9. The product can work with a customer-managed relay.
10. The product does not mix wallet custody with login custody.

## 7. Threat model

### 7.1 Protected assets

The design protects these assets:

- the canonical OpenAgents user ID
- linked authentication identities
- root Nostr signing keys
- device keys and DPoP keys
- OpenAuth access and refresh tokens
- account-link and recovery authority
- workroom capabilities
- private relay and signer metadata
- revocation and audit receipts

### 7.2 Adversaries

The design considers these adversaries:

- a malicious website that requests a misleading signature
- a compromised browser extension
- a malicious or compromised relay
- a stolen mobile or desktop device
- an attacker with a copied session token
- an attacker who replays a signed login request
- an attacker who changes a QR code
- an attacker who races an account-link flow
- an operator in a different tenant
- a compromised remote signer client key
- a recovery attacker with email or GitHub access
- an application bug that confuses a wallet key with an identity key

### 7.3 Required defenses

Every login proof must bind the exact origin, URL, method, body, challenge,
expiry, and attempt.
Every attempt must be single use.
Every account link must require two fresh proofs.
One proof comes from the current account.
The other proof comes from the new identity.

Every device grant must bind the user, device, scopes, audience, generation,
and expiry.
Every high-authority action must pass normal OpenAgents authorization.
A valid signature alone must never authorize the action.

The server must reject a Nostr identity that is linked to another active user.
It must not merge accounts from an email, a NIP-05 name, or a display name.
It must not use relay membership as an account-link proof.

### 7.4 Residual risks

A root-key compromise can let an attacker prove that identity.
OpenAgents cannot revoke the public Nostr key for the wider Nostr network.
It can revoke the link, sessions, devices, and OpenAgents grants.

A malicious signer can sign different bytes from the user request.
The client must show the returned event and compare its required fields.
A high-risk link flow should show a server receipt on both devices.

Revocation cannot erase data that a device already decrypted.
Relay operators can observe connection metadata, public tags, and timing.
NIP-44 does not hide all traffic metadata.

## 8. Canonical identity and account linking

### 8.1 One user, many login methods

**[PROPOSED]** Keep `users.id` as the canonical owner identifier.
Add `nostr` as an `auth_identities.provider` value.
Store the 64-character lowercase public key as `provider_subject`.
Do not use an `npub` as a database key.
Encode an `npub` only for display.

The user subject needs a new authentication-method field.
The public subject can include the linked Nostr public key.
It must not include a signer relay, client key, or secret.

The first release should keep OpenAuth as the token issuer.
A Nostr proof becomes one input to the issuer.
It must not create a second session format.

### 8.2 Signed-out Nostr login

**[PROPOSED]** A signed-out Nostr login has two outcomes:

- If the Nostr identity is linked, sign in to that canonical user.
- If it is not linked, create a new user after explicit confirmation.

The product must not create an account before signature verification.
It must show that there is no password-reset path for the Nostr identity.
It should invite the user to add a second recovery method.
This invitation must not block Nostr-only use.

### 8.3 Link Nostr to an OpenAuth account

**[PROPOSED]** The user starts from a fresh OpenAuth session.
The server creates a link attempt for that user.
The Nostr signer signs the exact link request.
The server verifies that the public key has no other active owner.
The server then shows a final confirmation with both identity names.

The final write must be one transaction.
It must create the identity link and its receipt together.
A repeated attempt with the same idempotency reference must return the same
result.

### 8.4 Link OpenAuth to a Nostr account

**[PROPOSED]** The reverse flow starts from a fresh Nostr session.
The user completes GitHub or email authentication.
The callback must return to the same link attempt.
The server must verify PKCE, state, and the current Nostr session.

The product must not merge accounts automatically.
If the conventional identity already belongs to another user, stop the flow.
Show a recovery path that requires proof for both accounts.

### 8.5 Unlink

**[PROPOSED]** Unlink requires recent authentication.
The user must keep at least one usable login method.
The system must revoke sessions that used only the removed method.
It must also revoke dependent signer connections and device grants.

An OpenAuth recovery must not assert control of a removed Nostr key.
A Nostr login must not restore a removed OpenAuth identity.
Each method keeps its own proof rules.

## 9. Login and session protocol

### 9.1 Attempt creation

**[PROPOSED]** Add a Nostr auth attempt endpoint.
It returns public data only:

- attempt reference
- random challenge
- exact completion URL
- HTTP method
- canonical body template
- expiry
- requested operation
- PKCE challenge for a public client
- server profile version

Store only a hash of the challenge when practical.
Bind the attempt to the current session for link operations.
Do not bind a signed-out attempt to an unverified public key.

### 9.2 NIP-98 completion

**[PROPOSED]** The client builds the exact JSON body.
It includes the attempt reference and challenge.
It then asks the signer for a kind `27235` NIP-98 event.
The event must bind the exact completion URL and `POST` method.
It must include the SHA-256 hash of the exact request body.

The server verifies these items in this order:

1. Decode the NIP-98 event.
2. Verify the event ID and Schnorr signature.
3. Verify kind `27235`.
4. Verify the exact external URL and method.
5. Verify the body hash.
6. Verify the short time window.
7. Load the attempt by reference.
8. Compare the challenge in constant time.
9. Verify the attempt operation and current session binding.
10. Atomically consume the attempt.

A reverse proxy must supply one trusted external origin.
Do not accept client-controlled forwarded-host fields.

### 9.3 Session issuance

**[PROPOSED]** After proof, the OpenAuth issuer creates a normal user subject.
Browser clients receive the current secure cookies.
Desktop and mobile receive the current access and refresh token form.
The clients keep their existing secure storage rules.

Record the authentication method in private session metadata.
Use one of these values:

- `openauth_github`
- `openauth_email`
- `nostr_nip07`
- `nostr_nip46`
- `nostr_nip55`
- `nostr_native`
- `nostr_device`

Do not put a raw signed event in the access token.
Keep the proof and receipt in private server evidence.

### 9.4 Proof of possession

**[PROPOSED]** Keep the normal browser cookie model.
Use origin and CSRF defenses for cookie requests.
Do not require a Nostr signature for every web request.

For native clients, bind high-value sessions or capabilities to a DPoP key.
Reuse the existing P-256 DPoP implementation.
Do not use the Nostr root key as the DPoP key.
The two keys have different rotation and privacy needs.

An access token proves a server session.
A DPoP proof proves possession of one session key.
A Nostr signature proves control of one Nostr key.
None of these facts alone grants an application action.

## 10. Platform flows

### 10.1 Web with NIP-07

**[PROPOSED]** The login page shows these equal choices:

- Continue with Nostr
- Continue with GitHub
- Continue with email

When the user selects Nostr, detect `window.nostr`.
If it exists, use this flow:

1. Create a server auth attempt.
2. Ask the extension for the public key.
3. Show the public key fingerprint and purpose.
4. Build the exact NIP-98 event.
5. Ask the extension to sign it.
6. Verify the returned event in the page.
7. Send it to the completion endpoint.
8. Receive the normal browser session.

The page must not ask for an `nsec`.
It must not offer a private-key paste field.
It must not store a root key in local storage or IndexedDB.

If NIP-07 is absent, show remote signer and conventional options.
Do not describe absence as an error.

### 10.2 Web with NIP-46

**[PROPOSED]** The browser creates a disposable NIP-46 client key.
It creates a `nostrconnect://` request for one login signature.
The QR and deep link can carry the client public key, relay set, secret, name,
site URL, and narrow permission.

The remote signer must show:

- `openagents.com`
- the browser label
- the requested method
- the event kind
- the one-time or persistent duration

After login, delete the client key for a one-time connection.
Do not silently retain it for workroom signing.
A persistent signer connection needs a separate consent and device record.

### 10.3 Omega desktop

**[PROPOSED]** Omega keeps local identity onboarding separate from hosted login.
A new user can create a Nostr-only local identity while offline.
This action must not create a wallet or a hosted OpenAgents account.

When the user links OpenAgents, Omega uses its host signer.
The GPUI view receives public identity data and progress only.
The host signs the NIP-98 request and stores the resulting session.
The view never receives the root key or OpenAuth tokens.

Omega also supports an external NIP-46 signer.
This path is the preferred hardware-signer path.
The local application stores only the disposable client key and connection
metadata.

The current OpenAuth loopback PKCE flow remains available.
A user can use Omega without a Nostr hosted-account link.
A user can also use Nostr login without creating a new Omega root key.

### 10.4 OpenAgents mobile

**[PROPOSED]** Mobile keeps the current OpenAuth PKCE flow.
Add three optional Nostr paths:

- a native signer with this-device-only custody
- NIP-55 on Android
- NIP-46 to a signer on another device

The default mobile enrollment must create a device key.
It must not copy the root person key from Omega.
The root signer can approve the device key and its scopes.

On iOS, store a native Nostr secret in protected Keychain storage.
On Android, use protected Keystore-backed storage.
Do not claim direct hardware-backed secp256k1 signing without device proof.
Some platform hardware APIs do not expose secp256k1 signing.
Use hardware-backed wrapping only where the platform proves it.

The React Native view receives signer operations and public data only.
It does not receive raw secret material.

### 10.5 Native and hardware signers

**[PROPOSED]** A native signer is a host service.
It is not a utility function in the view process.
It applies an allowlist for event kind, tag shape, content size, and audience.
It can require a user confirmation for high-risk events.

Use a hardware device through NIP-46 when possible.
If a vendor bridge does not use NIP-46, isolate it behind the same signer
interface.
Record the signer kind and assurance level.
Do not claim stronger custody than the integration can prove.

## 11. QR and cross-device enrollment

### 11.1 Buzz lesson

**[EXISTS]** The Buzz NIP-AB design uses a QR-initiated ephemeral exchange.
It uses throwaway keys, ECDH, HKDF, NIP-44, and a six-digit comparison code.
It sends the encrypted payload only after both devices confirm the code.

The Buzz design can transfer a root secret.
OpenAgents must not adapt that payload choice.

### 11.2 OpenAgents device enrollment

**[PROPOSED]** The new device creates these local keys:

- one Nostr device key for signed device records
- one P-256 DPoP key for server session proof
- one ephemeral enrollment key for the QR exchange

The existing device or root signer creates its own ephemeral enrollment key.
The QR carries public and single-use data only.
It must not carry a root secret, refresh token, or broad bearer token.

Both devices derive an enrollment secret.
They show the same short authentication string.
The user confirms the value on both devices.
The approving device then signs a device grant.

The grant binds:

- canonical user reference
- root Nostr public key
- device Nostr public key
- DPoP thumbprint
- platform and device label
- requested and granted scopes
- audience
- generation
- issue and expiry times
- enrollment attempt
- issuer and recipient confirmations

The new device receives the grant and a bounded bootstrap credential.
It exchanges the credential for a DPoP-bound session.
The root key never moves.

### 11.3 Confirmation and receipts

**[PROPOSED]** Both devices show the final result.
The receipt includes public-safe references and fingerprints.
It does not include a raw challenge or token.

The server stores the receipt before it reports success.
The new device must read back its device record.
The approving device must read back the same generation.

If either read fails, show an incomplete state.
Do not show a successful pairing from a relay acknowledgement alone.

## 12. Relay and HTTP authentication

### 12.1 Relay path

**[PROPOSED]** Use NIP-42 for an authenticated relay connection.
The client signs the relay challenge with the selected connection identity.
The relay verifies the challenge, relay URL, event time, and signature.

The relay then resolves its own policy.
It can check membership, tenant, device grant, or workroom role.
The authenticated public key is an input to this check.
It is not the complete authorization decision.

A relay acknowledgement proves only relay acceptance.
It does not prove canonical storage, command execution, or OpenAgents
acceptance.

### 12.2 HTTP path

**[PROPOSED]** Use NIP-98 at these boundaries:

- Nostr login completion
- Nostr account-link proof
- selected signer reauthentication
- Nostr-native HTTP resources that require the Nostr identity

Do not replace all OpenAuth API sessions with NIP-98.
Most product APIs should keep the current session and capability model.
This reduces signing prompts and replay risk.

### 12.3 NIP-98 and current Pylon rules

**[EXISTS]** Current Pylon routes reject NIP-98 as presence authority.
They require an OpenAgents agent bearer token.
This rule is correct.

**[PROPOSED]** A NIP-98 login can create a user session.
It must not create a Pylon registration or provider presence record.
Those actions still require their current agent and owner grants.

## 13. Device registry, revocation, and recovery

### 13.1 Device registry

**[PROPOSED]** Give users one device page on all three surfaces.
It shows:

- device label and platform
- first and last use
- authentication methods
- signer type
- granted scopes
- expiry
- current generation
- status
- public-key fingerprints

Do not show relay secrets or signer connection secrets.
Do not expose precise network history by default.

### 13.2 Revocation

**[PROPOSED]** Revocation must update these records in one durable operation:

- device status
- device grants
- signer connections
- refresh sessions
- server capabilities
- relay membership projection
- push registrations

Use a monotonically increasing device generation.
Reject a command or token from an older generation.
Publish a relay revocation projection only after the canonical write.

A relay can miss the projection.
The canonical server and host must still reject the device.

### 13.3 Recovery

**[PROPOSED]** Offer separate recovery paths:

- Nostr root recovery from the user's own backup
- OpenAuth recovery through GitHub or email
- recovery from another approved device
- account support for a link conflict

An OpenAuth recovery can restore the OpenAgents account.
It cannot recreate or rotate the user's Nostr root key.
A Nostr root recovery can prove the linked Nostr identity.
It cannot restore a removed GitHub or email identity.

The user can replace all device grants after recovery.
The replacement must increase an account device epoch.
Old device grants then fail even if a relay retains them.

### 13.4 Lost root key

**[OPEN]** The product needs an owner decision for a lost Nostr root key.
The recommended rule is conservative.
Keep the canonical OpenAgents user when another method proves it.
Mark the old Nostr identity as lost and disabled.
Do not claim cryptographic continuity to a replacement key.

The user can link a new Nostr identity after fresh recovery proof.
The audit record must show a discontinuity.

## 14. Privacy and user experience

### 14.1 Privacy

**[PROPOSED]** Use separate device keys to reduce cross-service correlation.
Do not use the root person key as a DPoP key or wallet connection key.
Do not publish device grants to public relays.

Keep signer relay lists private where possible.
Store only the relay metadata required for the connection.
Give the user a clear delete action for connection metadata.

Do not put email, GitHub login, device label, or account user ID in public
Nostr events.
Use opaque references in device records.
Keep account-link evidence private.

### 14.2 User language

The normal login page should use these terms:

- Nostr identity
- signing app
- device
- recovery method
- permission

Do not require the user to understand `npub`, `nsec`, NIP numbers, or relays.
Put protocol details in an advanced view.

The signing prompt must explain the exact action.
For example:

> Sign in to OpenAgents on this browser. This signature cannot spend money.

The product must not use the phrase “connect wallet” for login.
It must not show a payment icon for a signer.

### 14.3 Failure states

Use separate failure states for:

- signer not found
- user refused
- signature invalid
- challenge expired
- attempt already used
- identity linked elsewhere
- device revoked
- relay unavailable
- server session unavailable
- unsupported NIP revision

Do not replace these states with one “Nostr failed” message.

## 15. Customer-managed relay and workroom

### 15.1 Separate trust domains

**[PROPOSED]** A customer-managed relay controls its relay membership.
It does not control an OpenAgents cloud account.
An OpenAgents cloud issuer controls its own sessions.
An on-premises issuer controls its own local sessions.

The customer deployment must have an explicit trust configuration.
It names:

- deployment reference
- expected domains
- relay URLs
- relay signing public keys
- OpenAuth issuer, if used
- accepted Nostr auth profile
- local device grant authority
- cloud-link policy

Do not discover these authorities from the same relay stream that they verify.

### 15.2 Local-only mode

**[PROPOSED]** A local workroom can accept a Nostr identity without an
OpenAgents cloud account.
Its local issuer can create a deployment-scoped session.
That session cannot call OpenAgents cloud APIs.

If the user later links the deployment to OpenAgents cloud, require a separate
cloud account proof.
The link must identify both trust domains and its scopes.
Do not copy a cloud refresh token to the relay.

### 15.3 Customer OpenAuth

**[OPEN]** A customer can require conventional enterprise authentication.
The product must decide whether to support the customer's own OpenAuth issuer,
OIDC issuer, or both.

The Nostr design does not require this choice now.
The canonical local user can still have multiple auth identities.
Issuer federation needs a separate ProductSpec and assurance plan.

### 15.4 Relay failure

The client must keep a local session state separate from relay state.
A relay outage can stop Nostr transport.
It must not invalidate an otherwise valid local or cloud session.

A relay return does not restore a revoked device.
The client must compare the server or host device generation before use.

## 16. Proposed contracts and data model

### 16.1 Existing tables

**[PROPOSED]** Keep these existing tables:

- `users`
- `auth_identities`
- OpenAuth storage

Add `nostr` to the supported auth provider vocabulary.
Keep the unique provider and provider-subject constraint.
Do not put signer connections in `auth_identities`.

### 16.2 New records

Add these records in the canonical identity database:

| Record                     | Purpose                                                 |
| -------------------------- | ------------------------------------------------------- |
| `nostr_auth_attempts`      | Single-use NIP-98 login and link challenge              |
| `user_devices`             | Device identity, DPoP thumbprint, state, and generation |
| `device_capability_grants` | Scope, audience, expiry, lineage, and revocation        |
| `nostr_signer_connections` | NIP-46 or NIP-55 public connection metadata             |
| `auth_link_receipts`       | Durable result of a two-proof account link              |
| `auth_recovery_events`     | Recovery and identity-discontinuity audit               |

Secret tokens must use the existing secret storage pattern.
Normal rows store hashes, key references, and public keys only.

### 16.3 Attempt schema

The auth attempt must include:

- schema version
- attempt reference
- operation
- issuer
- external origin
- completion URL
- method
- challenge hash
- expected body hash rules
- current user reference for link flows
- PKCE challenge for public clients
- created and expiry times
- used time
- idempotency reference

The operation is one closed value:

- `login`
- `create_account`
- `link_nostr`
- `link_openauth`
- `reauthenticate`

### 16.4 Device schema

The device record must include:

- device reference
- user reference
- platform
- label
- Nostr device public key
- DPoP thumbprint
- signer type
- signer assurance
- generation
- status
- first, last, and revoked times
- enrollment receipt reference

The signer type is one closed value:

- `native`
- `nip07`
- `nip46`
- `nip55`
- `hardware_bridge`
- `none`

### 16.5 Capability schema

Reuse the narrowing rules from `packages/environment-auth`.
Do not create a second broad scope vocabulary.
Add identity and workroom scopes only through a versioned contract.

Each grant must include:

- grant reference
- user and device references
- source grant reference
- audience
- scopes
- DPoP thumbprint
- Nostr device public key, when required
- generation
- issue and expiry times
- state
- receipt reference

Raw token material must not enter the grant record.

### 16.6 API surface

The first server API should have these routes:

```text
POST   /api/auth/nostr/attempt
POST   /api/auth/nostr/complete
POST   /api/auth/nostr/link/attempt
POST   /api/auth/nostr/link/complete
GET    /api/auth/devices
POST   /api/auth/devices/enrollment/attempt
POST   /api/auth/devices/enrollment/complete
POST   /api/auth/devices/{deviceRef}/revoke
GET    /api/auth/receipts/{receiptRef}
```

The routes must use Effect Schema at every request and response boundary.
They must use one transaction for consume-and-issue operations.
They must return stable typed error reasons.

## 17. Migration and rollout

### Phase 0: Freeze the profile

Write a ProductSpec and AssuranceSpec before implementation.
Pin the selected NIP revision.
Freeze the canonical NIP-98 body and URL rules.
Freeze account-link, recovery, and lost-key rules.

Create conformance fixtures for each supported signer type.
Include negative fixtures for replay, wrong origin, wrong body, and wrong user.

### Phase 1: Nostr login on web

Add Nostr as an `auth_identities` provider.
Add the auth attempt and receipt tables.
Implement NIP-07 and one-time NIP-46 login.
Issue the existing OpenAuth browser session.

Keep the feature off by default.
Enable it first for test accounts.
Do not add account linking in the first internal test.

### Phase 2: Safe account linking

Add two-proof account linking and unlink.
Add link-conflict support.
Add device and session views.
Prove that no automatic account merge can occur.

Enable Nostr-only account creation after the link path is stable.
Encourage, but do not require, a second recovery method.

### Phase 3: Omega and mobile sessions

Connect Omega's Nostr-only local signer.
Keep the current OpenAuth PKCE path.
Add mobile NIP-46 and Android NIP-55.
Add DPoP binding for high-value native sessions.

Run physical-device tests on iOS and Android.
Do not use a simulator as hardware custody proof.

### Phase 4: Device enrollment

Implement the QR device-grant flow.
Use device-scoped keys and grants.
Add generation-fenced revocation.
Add two-device confirmation and receipts.

Do not transfer a root key in this phase.
Do not add NIP-49 to this flow.

### Phase 5: Relay and customer deployment

Add the NIP-42 relay profile.
Test a customer-managed relay with a separate trust configuration.
Test local-only workroom sessions.
Test optional cloud account linking.

Keep the relay below OpenAgents command and receipt authority.

## 18. Verification requirements

Implementation needs these proof groups:

### 18.1 Protocol proof

- canonical event and body fixtures
- NIP-07 extension compatibility
- NIP-46 remote signer compatibility
- NIP-55 Android compatibility
- NIP-98 wrong URL, method, time, and body tests
- NIP-42 wrong relay and challenge tests

### 18.2 Account proof

- first Nostr login
- repeat Nostr login
- Nostr-only account creation
- OpenAuth-to-Nostr link
- Nostr-to-OpenAuth link
- identity collision
- unlink with and without another method
- lost-key discontinuity

### 18.3 Device proof

- QR short-code match
- QR short-code mismatch
- replayed QR
- expired enrollment
- reduced scope grant
- device revocation
- old generation rejection
- lost device
- physical iOS and Android custody

### 18.4 Authority proof

- a signature cannot call an unauthorized product action
- NIP-42 cannot create an app session
- NIP-98 cannot create Pylon presence
- relay membership cannot create an account link
- a wallet connection cannot create a login
- a device grant cannot increase its parent scope

### 18.5 Privacy proof

- no raw key in logs, traces, receipts, or view state
- no session token in relay events
- no email or GitHub login in public Nostr events
- no root key in QR data
- no cross-tenant identity lookup
- deleted signer connection metadata is absent

## 19. Explicit non-goals

This proposal does not:

- replace OpenAuth
- require Nostr for all users
- make a relay the user database
- make NIP-42 an application login
- make NIP-98 a long-lived session
- copy a root key through QR
- create a wallet during login
- use Nostr Wallet Connect for authentication
- infer identity from NIP-05
- merge accounts from matching profile data
- make a Nostr signature prove permission or truth
- deploy a new OpenAgents relay
- revive the retired standalone relay or forge
- define payment, settlement, or wallet recovery
- admit implementation or release

## 20. Unresolved decisions

The owner and product team must decide these items:

1. Can a Nostr-only user create a hosted account in phase 1 or phase 2?
2. Which second recovery methods should the product recommend?
3. What happens when a user loses the root key but keeps OpenAuth access?
4. Which event kind and content profile should represent a device grant?
5. Does a customer deployment use OpenAuth, OIDC, or a local issuer?
6. Which native actions require a fresh root signature?
7. Which NIP-46 connections can stay active after login?
8. Which signer products form the first compatibility matrix?
9. What private evidence retention period applies to signed login events?
10. Which user-facing term should replace “root key” in normal views?

## 21. Final phased recommendation

Start with NIP-07 and one-time NIP-46 on the web.
Convert a verified NIP-98 request into the existing OpenAuth session.
Keep GitHub and email beside Nostr.
Do not change the normal API authorization model.

Next, add two-proof account linking and a device registry.
Then connect Omega and mobile to the same server contracts.
Use the current host-owned signer and secure session vault patterns.

Add QR enrollment only after revocation and generation fences exist.
The QR flow must enroll a new device key.
It must never transfer the root key.

Use NIP-42 only for relay connections.
Use NIP-98 only for exact HTTP proofs.
Use NIP-46 and NIP-55 only as signer transports.
Keep NIP-47 and all wallet authority outside authentication.

This sequence gives users sovereign Nostr login without removing familiar
authentication.
It also avoids a second session system and a second account authority.

## 22. References

### Canonical standards

- [NIP-07](https://github.com/nostr-protocol/nips/blob/db5fe3de8c5d1443b634c9bbf66ecb004f337057/07.md)
- [NIP-42](https://github.com/nostr-protocol/nips/blob/db5fe3de8c5d1443b634c9bbf66ecb004f337057/42.md)
- [NIP-46](https://github.com/nostr-protocol/nips/blob/db5fe3de8c5d1443b634c9bbf66ecb004f337057/46.md)
- [NIP-47](https://github.com/nostr-protocol/nips/blob/db5fe3de8c5d1443b634c9bbf66ecb004f337057/47.md)
- [NIP-49](https://github.com/nostr-protocol/nips/blob/db5fe3de8c5d1443b634c9bbf66ecb004f337057/49.md)
- [NIP-55](https://github.com/nostr-protocol/nips/blob/db5fe3de8c5d1443b634c9bbf66ecb004f337057/55.md)
- [NIP-98](https://github.com/nostr-protocol/nips/blob/db5fe3de8c5d1443b634c9bbf66ecb004f337057/98.md)

### OpenAgents evidence

- [Buzz teardown](../teardowns/2026-07-21-buzz-teardown.md)
- [Omega identity-first onboarding roadmap](../omega/2026-07-23-identity-first-onboarding-roadmap.md)
- [OpenAgents mobile Omega adaptation audit](../omega/2026-07-24-openagents-mobile-omega-adaptation-audit.md)
- [Nostr-native pivot analysis](../fable/2026-07-21-nostr-native-pivot-analysis.md)
- [Full Auto cross-app Nostr proposal](./2026-07-22-full-auto-cross-app-agent-delegation-over-nostr.md)
