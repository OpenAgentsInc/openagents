# Sensitive data vault

Status: **design, 2026-10-10. Nothing in this document is implemented.**
Wire formats are in the draft [NIP-VAULT](../../nips/openagents/NIP-VAULT.md);
attested workloads and their public release log are in the draft
[NIP-ATT](../../nips/openagents/NIP-ATT.md). The model step, where most of
the real leak risk lives, is designed in
[private inference](private-inference.md). Implementation is tracked in the
issue linked from the [security index](README.md).

## The ask

The owner, 2026-10-10:

> not have it stealable or seeable, ideally by ANYONE, worst-case only by
> the system owners, but if it's possible to have some setup where there's
> some system decryption key unrecoverable by me, then only the system could
> decrypt the data when the user requests it

The first data is a person's financial statements: PDFs and spreadsheets
they upload to a chat or a project (the `finances` project), and files we
read from their Google Drive through Connections
([#11238](https://github.com/OpenAgentsInc/openagents/issues/11238)).

This document sets out three tiers, says plainly who can read the data in
each, and recommends what to ship first.

## Contents

1. [Summary](#summary)
2. [What exists today](#what-exists-today)
3. [Threat model](#threat-model)
4. [Key hierarchy](#key-hierarchy)
5. [Tier A: only you](#tier-a-only-you)
6. [Tier B: only sealed hardware, with your share](#tier-b-only-sealed-hardware-with-your-share)
7. [Tier C: we can read it](#tier-c-we-can-read-it)
8. [The model step](#the-model-step)
9. [Agent runs: borrowing access and handing it back](#agent-runs-borrowing-access-and-handing-it-back)
10. [Deletion is crypto-shredding](#deletion-is-crypto-shredding)
11. [Logging](#logging)
12. [Connections and chat attachments](#connections-and-chat-attachments)
13. [What the person sees](#what-the-person-sees)
14. [Recommendation and milestones](#recommendation-and-milestones)
15. [Sources](#sources)

## Summary

| Tier | Who can decrypt the stored data | Who sees the plaintext while a model reads it | What breaks it |
| --- | --- | --- | --- |
| **A, only you** | Only the person's devices: passkey, device key, Nostr key, or recovery code. Our servers hold ciphertext only. | Whatever the person sends it to. Their own device, a sealed-hardware model ([private inference](private-inference.md)), or a named provider they pick. | Malicious client code that we serve (the web page is ours), or a compromised device. If every key and the recovery code are lost, the data is gone. |
| **B, sealed hardware plus your share** | Only an attested OpenAgents vault workload, an exact public image on Intel TDX in Confidential Space, and only while the person's client supplies its share for that request. No person holds a decrypt grant, the owner included. | With **Private** processing, the same sealed hardware. With **Fast** processing, also Google's Gemini on Vertex AI, under Google Cloud's terms. | A malicious image that the person's client accepts. Mitigations: public digest log, notice delay, optional per-person approval. Also Google and Intel as hardware roots of trust, side channels, and loss of every user key, as in Tier A. |
| **C, we can read it** | The OpenAgents service, so anyone with production access, the owner included, and Google as the cloud provider. Every key use is audit-logged. | Whichever model provider handles the chat, as listed on the privacy page. | Anyone with production access. The protection is the log, not cryptography. |

**Recommendation.** Ship Tier B first for financial data, with the person's
share required on every request and **Private** processing on sealed
hardware as the default. Offer **Fast** (Gemini) as a choice the person
makes, labelled for what it is. Ship Tier A as an opt-in "Only me" mode.
Tier C stays the plain truth for ordinary chats, and the copy keeps saying
"Our team can read them". The milestones and test gates are at the
[end](#recommendation-and-milestones).

## What exists today

The vault starts from this. A survey of `origin/main` on 2026-10-10:

- **Chats and attachments are plaintext JSON in Google Cloud Storage.** The
  production bucket is `openagentsgemini-web-chats-prod` (versioned, 7-day
  soft delete), written by `crates/openagents-web/src/chat_store.rs`.
  Attachments (`crates/openagents-web/src/chat_files.rs`, #11174) sit beside
  the chat under `files/{chat}/{file}`. Google's default encryption at rest
  is the only encryption: no CMEK, no app-layer sealing. The privacy page
  already says it plainly: "Our team can read them."
- **Server-held secrets use `oa-seal`.** `crates/oa-seal` is AES-256-GCM
  under a named keyring with rotation, binding associated data. It seals the
  BYO model credentials (`crates/openagents-web/src/cloud/byo.rs`, custody
  in `cloud/custody.rs`) and gateway BYOK keys. The keyring is a Secret
  Manager secret (`openagents-web-production-byo-keys`) injected as an
  environment variable on Cloud Run. Anyone who can read that secret can
  decrypt everything sealed under it. There is no per-user key.
  `crates/inference/src/seal.rs` and `crates/oa-auth/src/repos.rs` are two
  more single-key AES-GCM sealers.
- **There is no Cloud KMS, no TEE, no passkey and no account deletion.**
  `docs/decision-models/service/confidential-inference.md` defers
  confidential inference. `docs/auth/README.md` plans passkeys for P3.
- **`secret-screen`** refuses or redacts credential shapes (private keys,
  `sk-ant-`, `nsec1`, JWTs and more) in chat files, traces and Coder output.
  It gates content, not logs. Logging is mostly `eprintln!`. INVARIANTS
  forbid message text in the chat worker's logs, and secrets in logs.
- **Keys.** The phone keeps a device key in the Keychain (this device only).
  The desktop and Coder host keep owner, host and iroh keys in the OS
  keychain. Web visitors have no key of their own: the server derives a
  signing key per visitor from a salt, so the server can sign as any web
  user.
- **Training.** The privacy page says chats may be used to train our models.
  `crates/tenancy/src/training.rs` requires a grant reference for every
  training item, but there is no opt-out in code.
- **Connections (#11238)** are not built. No Google OAuth client, Drive
  client or Google sign-in exists yet.

The NIPs give us most of the building blocks:

- NIP-44 v2 encryption.
- NIP-59 seals and gift wraps.
- NIP-17 kind-15 file messages: a per-file AES-GCM key inside an encrypted
  rumor.
- NIP-49 password-encrypted keys (scrypt).
- NIP-46 remote `nip44_encrypt`/`nip44_decrypt`.
- NIP-07 `window.nostr.nip44`.
- The private artifact envelope `3188` in [contracts](../../nips/openagents/contracts.md).
- Custody operations in [NIP-SOV](../../nips/openagents/NIP-SOV.md). Each
  decryption request fixes the ciphertext, who receives the plaintext and
  the scope, and is consumed once. "Protecting keys does not protect
  plaintext after an authorized runtime decrypts it."
- POL disclosure policies and single-use approvals, and EXT's rule that a
  new version needs explicit adoption.

None of the NIPs covers WebAuthn PRF, a key hierarchy, TEE attestation or
attestation-bound key release. The two new drafts add those.

## Threat model

**Protected:** the content of a person's vault objects: uploaded files,
imported Drive files, extracted text, and the derived artifacts listed
below.

**Derived artifacts** carry the source's tier: extracted text, embeddings,
summaries, model answers about vault data, and the chat turns that quote
them. An answer about your bank statement is as sensitive as the statement.
A vault project's chat is stored as vault objects, never as plain chat JSON.

**Adversaries, by tier:**

| Adversary | A | B | C |
| --- | --- | --- | --- |
| Outsider with a stolen bucket, backup or disk | Protected | Protected | Protected (KMS key needed) |
| Our engineer or the owner reading storage directly | Protected | Protected | **Not protected** |
| The owner with full IAM power over the project and org | Protected | Protected unless the person's client accepts a malicious image ([limits](#honest-limits-of-tier-b)) | Not protected |
| Google as operator (not as a hardware root) | Protected | Protected by TDX memory encryption and attestation, assuming Google's attestation service is honest | Not protected |
| A model provider the person chose for Fast processing | Sees what is sent to it | Sees what is sent to it | Sees what is sent to it |
| Malicious JavaScript served by openagents.com | **Not protected** on the web | **Not protected** on the web | Not protected |

**Not protected in any tier:** metadata. That includes that a vault
exists, object count and sizes, upload and access times, which project, and
which model route was used. Also not protected: a compromised device, or a
person who shares their own data.

**The web-client caveat is real.** On the web, the code that does the
encryption is code we serve. A malicious page could exfiltrate keys. The
apps and Coder are signed binaries and are better here. For the web, we
publish the vault script's digest in the same release log and pin it with
Subresource Integrity, and we keep the script small and separately
reviewable. This narrows the risk but does not close it. The honest
position: the strongest form of each tier is in the apps.

## Key hierarchy

Every object has its own key. Every person has a vault master key. Tier B
adds a system key that only attested hardware can use.

```text
VMK  vault master key, 32 random bytes, generated on the person's device
 │     never leaves the person's devices, except wrapped in a key slot
 ├─ key slots: VMK wrapped under each way the person can unlock (see below)
 ├─ K_user_wrap = HKDF(VMK, info "openagents.vault.v1/user-wrap")      Tier A
 └─ S_user      = HKDF(VMK, info "openagents.vault.v1/user-share")     Tier B

SK   system key pair (X25519), one per person per key epoch            Tier B
       private half sealed by Cloud KMS; decrypt allowed only to the
       attested vault image (attestation-bound IAM, below)

DEK  data key, 32 random bytes, one per object (file, chat, index)
       content: AES-256-GCM in 64 KiB chunks, chunk nonce from counter
       Tier A: DEK wrapped with AES-256-GCM under K_user_wrap
       Tier B: DEK wrapped with HPKE (RFC 9180) mode_psk to SK.pub,
               psk = S_user. Unwrapping needs SK.priv AND S_user.
       Tier C: DEK wrapped by Cloud KMS (ordinary key, service decrypts)
```

Why HPKE `mode_psk` for Tier B: the client can wrap a new file's key
without contacting the enclave, using the person's system public key and
its own share. Unwrapping needs both the system private key and the share.
HPKE is a published standard with test vectors, so independent clients can
implement it. Exact parameters, associated data and test vectors are in
[NIP-VAULT](../../nips/openagents/NIP-VAULT.md).

**Key slots** (each holds `VMK` wrapped under one unlock method):

| Slot | How the wrapping key is made | Where it works | Notes |
| --- | --- | --- | --- |
| Passkey | WebAuthn PRF output (salt fixed per slot) through HKDF | Chrome/Edge 116+, Firefox 139+ desktop; Safari 18+ on Apple platforms with known gaps | Synced passkeys (iCloud Keychain, Google Password Manager) carry the slot to the person's other devices. PRF on sign-in (`get()`) is not reported as supported for Safari in MDN's data, while third-party reports say iCloud Keychain PRF works from Safari 18 with bugs before iOS 18.4 and fails over a cross-device QR scan. **Treat PRF as one slot among several, never the only one.** |
| Device key | A random key in the phone's Keychain or Android Keystore (this device only), or the OS keychain on desktop | iOS, Android, Mac app, Coder host | The apps already keep device keys this way. |
| Nostr key | NIP-44 v2 to self: conversation key of the person's own key pair, as NIP-51 private lists do | Any NIP-07 extension, NIP-46 bunker, or the app's own key | Lets Coder and Nostr clients unlock without a browser. Through a bunker, the bunker sees the VMK when it decrypts. |
| Recovery code | 24 words (BIP-39 wordlist, 256 bits), stretched with scrypt using NIP-49's parameters (`log_n` 20 by default) | Anywhere | Shown once at setup and never stored by us. It is the only way back if every device is lost. |

Slots are stored server-side, as ciphertext only. Adding a device means
unlocking on an existing device and wrapping the VMK to the new device's
slot. A device-pairing QR, as NIP-HOST's enrollment does, carries the new
device's public key one way and the wrapped VMK back. Removing a device
deletes its slot. A removed device that copied the VMK still has it, so
removing a device you suspect is compromised triggers a **VMK rotation**:
a new VMK, new slots, and every object key rewrapped. In Tier B the
objects need no re-encryption: the client rewraps the DEKs through the
enclave in one batch.

## Tier A: only you

**Guarantee.** The service stores only ciphertext and key slots. No key that
can open them exists outside the person's devices and their recovery code.
We cannot read the data, and neither can the owner, Google, or anyone who
steals our storage. This holds on the condition that the client code is
honest (see the [web caveat](#threat-model)).

**How a model reads it.** Only by the person's client decrypting it for that
turn and sending the plaintext somewhere:

- **Web.** The vault script decrypts in the browser and attaches the
  plaintext to the turn. It is sealed to an attested private-inference
  endpoint ([private inference](private-inference.md)), so our gateway never
  sees it. Or it goes to Fast processing, where the person accepts that our
  chat service and Google see that turn. The answer comes back to the
  browser and is stored as a vault object under the person's own key.
- **Phone and Mac apps.** The same, with the device-key slot. The apps can
  also run a small local model on the device. The data then never leaves.
- **Coder.** Coder on the person's computer unlocks with the device or
  Nostr slot, writes plaintext only into the task's working directory for
  the run, and deletes it after (see [agent runs](#agent-runs-borrowing-access-and-handing-it-back)).
  The model Coder calls (Codex, Claude, a local model) sees what the task
  reads. That is the person's choice of engine, and the run card says so.

**Cost.** We can't index, search or run agents over Tier A data unless one
of the person's devices is online and unlocked. Scheduled jobs (a monthly
spending summary) need a device to wake and do the work, or a Tier B
standing lease. Losing every slot and the recovery code loses the data.
We cannot help, and the copy says so before the person turns it on.

## Tier B: only sealed hardware, with your share

**The idea.** The data can be decrypted only inside a specific, publicly
logged program running on Intel TDX hardware in Google Cloud Confidential
Space. Cloud KMS releases the system key only to that program. Even that
program can only decrypt when the person's client hands it the person's
share for that request. No human, including the owner, holds a grant to
decrypt.

### The platform: Intel TDX in Confidential Space

Google offers three CPU technologies for Confidential VM:

| | AMD SEV | AMD SEV-SNP | Intel TDX |
| --- | --- | --- | --- |
| Memory encryption | Yes | Yes, plus integrity against a malicious hypervisor | Yes, isolated trust domain |
| Attestation | Through the Shielded VM vTPM at boot. The vTPM is managed by the host's hypervisor, so Google calls it software-attested. | Launch measurement signed by the AMD Secure Processor (VCEK). Reports can be requested at any time. | Measurements held in the TDX module; quotes signed by the TD Quoting Enclave. Hardware-attested. |
| Live migration | Yes, on N2D and C3D | No | No |
| Machine series | C2D, C3D, C4D, G4, N2D | N2D (Milan) only | C3, C4, A3 High (with H100) |
| **Confidential Space** | Yes | **Not listed** | Yes, and TDX with NVIDIA H100 |

**Choice: Intel TDX on `c3-standard` for the key-release workload, with a
condition requiring `hwmodel == GCP_INTEL_TDX`.** The reasons:

1. **Hardware-rooted evidence.** TDX evidence comes from the CPU's TDX
   module, not from a vTPM that the host's hypervisor manages. SEV's
   boot-time vTPM attestation trusts the very party Tier B is meant to
   exclude, Google's host stack, more than we want for the component that
   receives the person's share.
2. **SEV-SNP would also qualify on the hardware, but Confidential Space does
   not list it.** Confidential Space is the product that gives us a locked
   production image, a Google-run attestation verifier issuing OIDC tokens,
   and KMS integration through workload identity federation. SNP is
   available on plain N2D Confidential VMs, and on Confidential GKE Nodes,
   with our own verifier. That is a fallback, not the first build.
3. **No live migration** is a feature here. The VM does not move hosts
   while holding keys.
4. **The model path.** TDX is also the CPU pairing for confidential H100
   GPUs in Confidential Space, so the private model lane can use the same
   attestation chain ([the model step](#the-model-step)).

Intel Trust Authority is available as a second, independent attestation
verifier for TDX. Using it as well as Google Cloud Attestation reduces our
reliance on Google as the only verifier, at extra cost and integration
work. This is a later option.

### Attestation-bound key release

Set up in a dedicated GCP project, `openagents-vault` (name to be decided),
separate from `openagentsgemini`:

1. **The KMS key.** A key ring `vault` holds one asymmetric-decrypt or
   symmetric key per key epoch. It seals each person's `SK.priv`, so a KMS
   ciphertext per person is stored beside their vault. Protection level
   HSM (FIPS 140-2 Level 3), so the key cannot be exported.
2. **Workload identity pool** `vault-attested`, with a provider that trusts
   `https://confidentialcomputing.googleapis.com`. Its attribute condition
   follows Google's documented form:

   ```text
   assertion.swname == 'CONFIDENTIAL_SPACE'
   && 'STABLE' in assertion.submods.confidential_space.support_attributes
   && assertion.hwmodel == 'GCP_INTEL_TDX'
   && assertion.submods.container.image_digest in [<the admitted digests>]
   && 'vault-workload@openagents-vault.iam.gserviceaccount.com' in assertion.google_service_accounts
   ```

   `STABLE` excludes the debug image, where the operator has SSH and root.
3. **The only decrypter.** `roles/cloudkms.cryptoKeyDecrypter` on the key
   is granted to
   `principalSet://iam.googleapis.com/projects/<n>/locations/global/workloadIdentityPools/vault-attested/attribute.image_digest/<digest>`
   and to nothing else. No user, group or service account has decrypt.
4. **Deny guardrails.** An IAM deny policy on the project denies
   `cloudkms.googleapis.com/cryptoKeyVersions.useToDecrypt`,
   `cryptoKeys.setIamPolicy`, `keyRings.setIamPolicy`,
   `workloadIdentityPools.setIamPolicy` and
   `cloudresourcemanager.googleapis.com/projects.setIamPolicy` to every
   principal except the attested principal set and a named release
   identity. Changing a deny policy needs `roles/iam.denyAdmin` at the
   organization.
5. **Public audit.** Admin Activity audit logs are always written and
   cannot be disabled. We export the vault project's Admin Activity log to
   a public, read-only sink, and the release process mirrors every
   policy-relevant change into the public release log (below). Data Access
   logs for KMS `Decrypt` are enabled and exported as well. They record
   that a decryption happened, never the plaintext.

**What the owner can still do, said plainly.** The owner controls the
organization. They can remove the deny policy, grant themselves decrypt,
and call KMS. Every step leaves an Admin Activity record that cannot be
switched off and that we publish. **Even then they get only the system
private keys, which are useless without each person's share.** The share
never reaches a server that is not running an admitted, attested image. So
the cryptographic guarantee does not rest on IAM. It rests on the person's
client refusing to hand its share to anything but an admitted digest. IAM
and deny policies are a second, logged fence.

### The person's share, per request

Every Tier B decryption is a request from the person's client:

1. The client asks the vault endpoint for a fresh **session**. The vault
   workload generates an ephemeral X25519 key and requests a Confidential
   Space attestation token, with a custom audience and a nonce binding
   `SHA-256(ephemeral public key ‖ client nonce)`. Up to six nonces of
   10–74 bytes are allowed, and TLS exported keying material can also be
   bound. The token is a Google-signed OIDC JWT, or a PKI token verifiable
   against Google's attestation root.
2. The client verifies the token: the signature and issuer, `swname`,
   `STABLE`, `hwmodel == GCP_INTEL_TDX`, the nonce, and that `image_digest`
   is in the **admitted set**. The admitted set is the current
   [NIP-ATT](../../nips/openagents/NIP-ATT.md) release head, past its
   notice delay, and, if the person turned on approvals, approved by them.
3. The client sends an HPKE-sealed request to the ephemeral key:
   `S_user`, the object IDs, the purpose, the model route (Private or Fast),
   an expiry, and a run ID. Our Cloud Run front door only relays
   ciphertext.
4. Inside the enclave: unwrap the person's `SK.priv` through KMS (allowed
   only because of the attested identity), HPKE-open each object's DEK with
   `SK.priv` and `psk = S_user`, decrypt, process, and seal the result back
   to the client's ephemeral key. New derived objects are wrapped the same
   way.
5. The enclave zeroes `S_user`, the DEKs and the plaintext when the request
   or lease ends ([agent runs](#agent-runs-borrowing-access-and-handing-it-back)).

**Approving new versions (the person's choice):**

- **Default.** A new image digest is published in the release log at least
  **7 days** before it is admitted. It carries a source commit, a
  reproducible-build recipe and a Rekor entry, and every vault user gets a
  notice ("The sealed vault program will update on 17 Oct. What changed.").
  The person can download their data or turn the vault off before then.
- **"Ask me before any update"** (opt-in). The client admits only digests
  the person approved. Until they approve, Tier B requests to a newer image
  are refused. With **device copy** on (below), the person can still open
  every file on their own device in the meantime.
- **Device copy** (on by default, can be turned off). Each DEK is also
  wrapped under `K_user_wrap`, as in Tier A, so the person's own devices
  can open their files without the enclave. This does not let us read
  anything. What it changes is that a stolen, unlocked device alone is
  enough to open the files, rather than a device plus our enclave. People
  who want the enclave to be required every time turn it off.
- **Export is always available.** "Download my data" runs in the enclave
  and returns plaintext to the client, sealed to its key. A person can
  leave at any time.

**Emergency fixes.** A security fix may shorten the delay to 24 hours. It
is announced in the log with the reason, and people who require approval
still must approve. There is no path that skips the person's client.

**Break-glass: none.** There is no recovery key, support override,
law-enforcement path, or escrow. If we receive a lawful demand, we can
produce ciphertext and metadata only, and the transparency copy says so.

### Recovery and multiple devices in Tier B

Tier B needs the share, and the share comes from the VMK. So Tier B has the
same recovery story as Tier A: synced passkeys, more devices, and the
recovery code. The difference is convenience. Agents and scheduled jobs can
work while the person is away, under a standing lease (below), and nothing
heavy runs on the device.

A weaker variant, **B-recoverable**, was considered and rejected for
financial data. In it the enclave alone decrypts when an authenticated
account asks. Account authentication is run by us, so the owner could forge
the request. It would be "only the system can decrypt, and we decide when".

### Honest limits of Tier B

- **Trust roots.** Intel, for the TDX module and quoting enclave
  correctness, and Google, for the Confidential Space image, the attestation
  verifier and KMS honouring IAM. A compromised or compelled Google
  attestation service could vouch for a false environment. Adding Intel
  Trust Authority as a second verifier reduces this.
- **Side channels.** TEEs have a record of microarchitectural and physical
  attacks. Confidential VMs do not claim protection against every one of
  them. Assume a determined, well-resourced attacker with physical access
  may extract secrets.
- **The image is ours.** The owner decides which image is deployed. A
  malicious image could leak shares and plaintext. The mitigations are
  reproducible builds (Google points to Bazel and provides no
  reproducible-build service), a public digest log with Rekor entries, the
  7-day notice delay, and per-person approval. Together they make a
  malicious update public before it can read anything. They do not make it
  impossible.
- **The web page is ours** (see the [threat model](#threat-model)).
- **Metadata** is visible to us and Google.
- **Plaintext leaves the enclave if the person picks Fast processing.**
- **Availability.** If the attestation service, KMS or capacity in our zone
  is down, Tier B fails closed. The person can still open their data on
  their own device if device copy is on.
- **Not live-migratable.** Host maintenance restarts the workload, and
  in-flight leases end.

## Tier C: we can read it

**Guarantee.** Protection against storage theft and casual access, plus a
record of every key use. It is **not** protection from us.

- Objects are encrypted with a per-object DEK wrapped by a Cloud KMS key
  (`roles/cloudkms.cryptoKeyEncrypterDecrypter` held by the web service
  account). The bucket also gets CMEK with the same key ring, so restoring
  a raw object or backup needs KMS.
- KMS Data Access logs on `Decrypt` record which principal used the key,
  and when. Access Transparency covers access by Google staff, not by us.
- This replaces `oa-seal`'s Secret Manager keyring for new per-user data,
  because KMS logs each use. The keyring has no per-use record.
- The copy says it plainly: **"We can read these. Every time our systems
  open one, it's logged."**

Tier C is the right tier for ordinary chats today. It is the honest floor
for anything a person has not put in a vault.

## The model step

Encryption at rest means little if the plaintext then goes to a model
provider. Every vault request names its **route**, and the person picks the
route per project, with a per-turn override:

| Route | Who sees the plaintext | Available in |
| --- | --- | --- |
| **Private** (default for vault projects) | Only sealed hardware: a model running inside an attested TEE with a confidential GPU, our own engine or an open model. See [private inference](private-inference.md). | Tier A, B |
| **Fast** | Our vault enclave (Tier B) or the person's browser (Tier A), then **Google's Gemini on Vertex AI**. Google does not train on it. It caches prompts in memory for up to 24 hours unless we turn caching off, and may log prompts for abuse monitoring unless granted an exception. | Tier A, B, C |
| **On this device** | Only the person's device (a local model in the apps or Coder). | Tier A, B (apps, Coder) |
| **Chat default** | Whatever the ordinary chat uses, per the privacy page (OpenRouter models, Vercel AI Gateway, TypeSafe). | Tier C only. **Never used for vault data.** |

For Fast, our Vertex project must: disable the prompt cache
(`cacheConfig.disableCache`); request the abuse-monitoring exception, and
until it is granted say "Google may keep it for abuse checks"; keep
request-response logging off; and use no Search or Maps grounding, which
Google retains and cannot be turned off. Vertex is not a TEE. Fast is never
described as private.

**Confidential GPU options on Google Cloud**, as listed in the
supported-configurations page:

- **A3 High (`a3-highgpu-1g`), NVIDIA H100, with Intel TDX.** Zones:
  europe-west4-c, us-central1-a, us-east5-a. This is the only GPU that
  Confidential Space supports. Google's blog calls Confidential Space with
  H100 a preview. Its deployment guide requires spot or flex-start
  provisioning for GPU workloads, and A3 High confidential VMs do not
  support reservations.
- **G4 (`g4-standard-48`), NVIDIA RTX PRO 6000 Blackwell, with AMD SEV.**
  These are offered widely as Confidential VMs, but not in Confidential
  Space. SEV gives only boot-time vTPM attestation.

So the first private model lane is TDX + H100 in Confidential Space, with
the same release log and the same client checks as the vault. The model is
our own engine (Clef or Psionic, #11225) or a pinned open model. The data
is decrypted, analysed and re-encrypted without leaving attested hardware.
The cost is capacity: spot or flex-start only, three zones, one GPU per VM.
So Private may queue where Fast answers at once, and the UI says so.

## Agent runs: borrowing access and handing it back

An agent run (a chat turn, a scheduled summary, a Coder task) gets a
**vault lease** from the person's client:

```text
lease = {
  run, objects[] or project, purpose,
  route: private | fast | device,
  effects: read-only by default; "write derived objects" listed explicitly,
  expires_at (default 15 min for a turn; up to 24 h for a standing lease),
  max_uses, issued_by (device slot id), signature
}
```

- The lease travels with `S_user` inside the HPKE request (Tier B), or is
  enforced by the client itself (Tier A). It is a POL-style single-use
  approval for the run, not a bearer token. The enclave checks it against
  the run ID it is serving.
- **Giving it up.** The run ends, the lease expires or the person revokes
  it. The enclave then drops `S_user` and every DEK and zeroes buffers, and
  it records `released` in the run's private record. A standing lease
  (weekly summary) shows in Settings → Vault → Active access with a
  **Stop** button. Stopping sends a revocation, and the enclave refuses
  further use.
- **Coder on the person's computer** gets plaintext files in the task's
  working directory, mode 0600, under a per-run directory. It deletes them
  when the task ends, even on failure (a cleanup guard). The task record
  lists which vault objects the run read. Plaintext is never in the trace,
  checkpoints or images. This reuses the rule BYO credentials follow today:
  released fresh per boot, never written into checkpoints.
- **Cloud computers** (Coder Cloud) are Tier C machines. A vault lease to a
  cloud computer means Fast-like exposure, and the lease card says "This
  run's computer can read these files while it runs."
- **Sub-agents** inherit at most the parent's lease (attenuation, as in
  [contracts](../../nips/openagents/contracts.md)). They never widen it.

## Deletion is crypto-shredding

Today, deleting a chat removes the object and its versions, but GCS soft
delete keeps a recoverable copy for 7 days. In the vault, deletion destroys
keys, so copies anywhere become unreadable:

- **One file.** The object's DEK exists only as a wrapped value inside the
  person's **key index**, an object sealed under the person's current
  index key. Deleting a file rewrites the index without that DEK, under a
  new index key version. The old version is then destroyed. Ciphertext
  copies, in soft delete, backups or a caller's cache, can no longer be
  unwrapped.
  - Tier B: the index key is a per-person KMS key version. Its scheduled
    destruction is set to the minimum, 24 hours. KMS keeps a version
    scheduled for destruction restorable for that window. After it,
    destruction cannot be reversed. An org policy can enforce a floor.
  - Tier A: the index key is derived from the VMK and an index epoch. The
    old epoch's wrapped index is deleted. The VMK itself lives on the
    person's devices.
- **The whole vault or account.** Destroy the person's KMS key versions
  (Tier B system key and index key), delete every slot, and delete the
  objects. After the destruction window, nothing anywhere can be decrypted.
- **What the copy promises.** "Deleted files can't be opened by anyone
  after 24 hours, including copies our storage provider keeps." In Tier A:
  "immediately", for us. A device the person still holds may keep its own
  copy until they remove it.
- KMS key versions cost a monthly fee each, so per-person keys have a
  real, if small, cost. That is the price of per-person shredding. A
  person-level key, not a file-level key, is the KMS unit. Files shred
  through the index rewrite.

## Logging

- **Never logged, anywhere** (servers, enclave, Cloud Logging, traces,
  analytics, error reports): plaintext; extracted text; DEKs, `S_user`,
  VMKs or slot material; file names; or the plaintext digest. A digest of a
  known statement template is guessable.
- **Logged:** account ID (or its digest in public logs), object ID,
  ciphertext size, tier, route, lease ID and run ID, key version, image
  digest, the decision (`granted`, `refused: <reason>`), and timings.
- **The enclave.** The Confidential Space production image does not stream
  the container's output unless the launch policy allows log redirection.
  We allow it only for structured, pre-declared event lines. A test scans
  for a canary (below).
- **KMS Data Access logs** show every system-key use, by principal.
  Exported publicly with account IDs hashed, they let anyone count
  decryptions and spot a decrypt by a human principal.
- `secret-screen` stays a gate on what enters plaintext surfaces. It is
  not the guarantee. The vault's guarantee is that plaintext never reaches
  a logging path. This is enforced by types: vault plaintext lives in a
  `Zeroizing` buffer type without `Display` or `Debug`.

## Connections and chat attachments

- **Chat attachments** in a project with the vault on are encrypted on the
  client before upload. The same 10 MiB / 4-per-message / 32-per-chat
  limits apply to the plaintext. `chat_files.rs` stores the ciphertext and
  the NIP-VAULT header under `files/{chat}/{file}`. Byte-sniffing, credential
  screening and PDF text extraction move to where plaintext exists: the
  client (Tier A) or the enclave (Tier B). Outside a vault project nothing
  changes. Attachments stay Tier C, and the privacy page keeps saying so.
- **Google Drive (#11238).** Google can read Drive files; nothing we do
  changes that, and the copy says it. What we control is our copy and our
  path:
  - The Drive OAuth refresh token is a vault object. In Tier B it is wrapped
    to the system key with the person's share, so only the enclave can use
    it during a lease. In Tier C it is sealed like other BYO credentials.
  - `drive.read` for a vault project runs inside the enclave. Files are
    fetched over TLS from Google into the enclave, analysed there, and any
    copy we keep is stored as a vault object. Our ordinary servers never
    see Drive file contents.
  - "Import into vault" copies files once. "Live" re-reads them per turn
    and keeps nothing but derived objects the person saves.
  - The V1 slice in #11238 (Gemini on Vertex answering from Sheets) is
    exactly the **Fast** route, and is labelled that way. A finances
    project defaults to Private when the private lane is available.
- **Training.** Vault data and its derived objects are never training
  data. In Tiers A and B this is enforced by cryptography: the training
  pipeline cannot decrypt. In Tier C it is enforced by classification:
  `tenancy` training refuses items whose provenance is a vault object.

## What the person sees

Plain words, no system words. Settings → **Vault**, and a lock on any
project that uses it:

- **Turn on the vault for this project**
  - "Files you add here are locked with a key made on your device. We store
    them locked."
- **Who can open them**, the tier choice:
  - **Sealed** (recommended): "Only a sealed program can open your files,
    and only when you ask. We can't, and neither can our staff. The program
    is published, and you'll hear 7 days before it changes."
  - **Only me**: "Only your devices can open your files. If you lose all
    your devices and your recovery code, your files are gone. We can't get
    them back."
  - (Ordinary chats, shown on the privacy page, not as a choice): "We can
    read these. Every time our systems open one, it's logged."
- **How answers are made**, the route:
  - **Private**: "Processed only inside sealed hardware. Slower,
    sometimes queued."
  - **Fast**: "Processed by Google Gemini. Google doesn't train on it, but
    can see it while answering."
  - **On this device** (apps): "Your phone or computer does the work."
- **Recovery code**: "Write these 24 words down. They're the only way back
  in if you lose your devices."
- **Active access**: each running or standing lease, with the run name,
  the files and **Stop**.
- **Updates**: "The sealed program updates on 17 Oct. See what changed ·
  Ask me before updates."
- **Delete vault**: "Your files will be unreadable by anyone within 24
  hours."

Every claim in that copy must be true when it ships. A claim that is not
yet true does not appear.

## Recommendation and milestones

**Ship first:** Tier B with the person's share, for financial data, on TDX
Confidential Space, with Private processing where capacity allows and
Fast clearly labelled. Tier A "Only me" follows as an opt-in, because most
of its machinery (slots, client crypto, export) is built for B anyway.
Ordinary chats stay Tier C, made honest and logged.

Each milestone ends with a gate that must pass on the deployed build.

| # | Milestone | Gate (test that must pass) |
| --- | --- | --- |
| V0 | **Honest Tier C.** CMEK on the chat bucket. A KMS envelope for new attachments. KMS Data Access logs on. Vault and finances data excluded from training. Privacy copy unchanged except to add the logging line. | Restore a soft-deleted object with the KMS key disabled: it fails. Training refuses an item with vault provenance. |
| V1 | **Client vault core.** VMK, the four slot types, chunked AES-GCM, the NIP-VAULT header, and HPKE mode_psk wrapping in a shared Rust crate (`oa-vault`, compiled to WASM for the web) plus the apps. NIP-VAULT test vectors. | Vectors pass in Rust and WASM. A **server-blindness test**: run the full upload path with a canary file. No server process, log, bucket object or trace contains the canary bytes or their digest. |
| V2 | **Sealed key release.** The `openagents-vault` project, the HSM key ring, the attested WIP with a TDX + `STABLE` + digest condition, deny policies, public audit export, and the vault workload on `c3-standard` TDX. | **Owner denial test**: as `chris@openagents.com` with Owner, `Decrypt` on the vault key returns `PERMISSION_DENIED`. A debug-image token is refused. A token from a non-admitted digest is refused. Only the admitted production digest decrypts. A **policy-drift test**: any change to the key IAM, WIP or deny policy appears in the public log within 10 minutes. |
| V3 | **Release log and client checks.** NIP-ATT release events, Rekor entries, a reproducible build of the vault image from a pinned commit (two independent builders get the same digest), the 7-day delay and notices, and opt-in approval. | The client refuses a digest that is not in the head, still inside its delay, or (with approval on) unapproved. A rebuild by a second builder matches the published digest. |
| V4 | **Tier B in the product.** Finances project vault, attachments, Fast route through Vertex with caching off, leases, Active access, export, and crypto-shredding. | A lease expires and the next use is refused. Revoke stops a standing lease. **Shred test**: delete a file, wait out the destroy window, restore every soft-deleted and versioned object, and confirm nothing decrypts. Export round-trips. |
| V5 | **Private route.** A model in TDX + H100 Confidential Space ([private inference](private-inference.md)). | A client-to-enclave sealed request whose plaintext never appears outside the enclave (canary scan across gateway, relay and logs). The receipt carries the measurement. |
| V6 | **Tier A "Only me"** and the apps (Keychain and Keystore slots), and Coder unlock and run cleanup. | Server holds no key able to open a Tier A object (static check: no code path receives VMK, K_user_wrap or S_user). Coder leaves no plaintext after a failed run. |
| V7 | **Connections into the vault**: Drive token as a vault object, and `drive.read` in the enclave. | Drive file contents never reach `openagents-web` (canary scan). |

Owner-only steps (creating the GCP project and org-level deny policy,
Vertex abuse-monitoring exception request) are recorded in the workspace
`NEEDS_OWNER.md` when the milestone that needs them starts, not before.

## Sources

Google Cloud (checked 2026-10-10):

- Confidential VM overview: https://docs.cloud.google.com/confidential-computing/confidential-vm/docs/confidential-vm-overview
- Attestation overview (SEV vTPM vs SNP VCEK vs TDX module): https://docs.cloud.google.com/confidential-computing/confidential-vm/docs/attestation-overview
- Supported configurations (machine series, technologies, zones, live migration, GPUs): https://docs.cloud.google.com/confidential-computing/confidential-vm/docs/supported-configurations
- Confidential VM pricing: https://cloud.google.com/confidential-computing/confidential-vm/pricing
- Confidential Space overview: https://docs.cloud.google.com/confidential-computing/confidential-space/docs/confidential-space-overview
- Confidential Space images (production vs debug, support attributes): https://docs.cloud.google.com/confidential-computing/confidential-space/docs/confidential-space-images
- Deploy workloads (SEV or TDX; H100 only; spot/flex-start for GPUs): https://docs.cloud.google.com/confidential-computing/confidential-space/docs/deploy-workloads
- Grant access to confidential resources (attribute conditions, `principalSet` decrypter): https://docs.cloud.google.com/confidential-computing/confidential-space/docs/create-grant-access-confidential-resources
- Token claims (`hwmodel`, `swname`, `image_digest`, `support_attributes`): https://docs.cloud.google.com/confidential-computing/confidential-space/docs/reference/token-claims
- Custom attestation tokens (audience, nonces, PKI tokens): https://docs.cloud.google.com/confidential-computing/confidential-space/docs/connect-external-resources
- Signed workload images: https://docs.cloud.google.com/confidential-computing/confidential-space/docs/create-customize-workloads
- Confidential GKE Nodes: https://docs.cloud.google.com/kubernetes-engine/docs/how-to/confidential-gke-nodes
- Confidential Space with H100 preview: https://cloud.google.com/blog/products/identity-security/from-clicks-to-clusters-confidential-computing-expands-with-intel-tdx
- IAM deny policies: https://docs.cloud.google.com/iam/docs/deny-overview, https://docs.cloud.google.com/iam/docs/deny-permissions-support
- Audit logs: https://docs.cloud.google.com/logging/docs/audit
- Key states and destruction: https://docs.cloud.google.com/kms/docs/key-states, https://docs.cloud.google.com/kms/docs/destroy-restore
- Cloud HSM: https://docs.cloud.google.com/kms/docs/hsm; HSM key attestation: https://docs.cloud.google.com/kms/docs/attest-key
- Cloud EKM: https://docs.cloud.google.com/kms/docs/ekm
- Key Access Justifications (Assured Workloads only): https://docs.cloud.google.com/assured-workloads/key-access-justifications/docs/overview
- Access Transparency: https://docs.cloud.google.com/assured-workloads/access-transparency/docs/overview
- Vertex AI zero data retention: https://docs.cloud.google.com/vertex-ai/generative-ai/docs/vertex-ai-zero-data-retention

Standards and other sources:

- WebAuthn Level 3, PRF extension: https://www.w3.org/TR/webauthn-3/
- Browser PRF support: MDN browser-compat-data, `api/CredentialsContainer.json` (https://github.com/mdn/browser-compat-data); third-party Safari reports: https://www.corbado.com/blog/passkeys-prf-webauthn, https://developer.apple.com/forums/thread/774112
- HPKE: RFC 9180, https://www.rfc-editor.org/rfc/rfc9180
- Sigstore Rekor: https://docs.sigstore.dev/logging/overview/
- NIP-44, NIP-49, NIP-51, NIP-59, NIP-17: `nips/official/`

### Claims we checked and corrected

- SEV-SNP is **not** listed for Confidential Space. The docs list SEV and
  TDX, and `hwmodel` has no SNP value. Our design uses TDX.
- Confidential Space GPUs are H100 only, and spot or flex-start only. G4
  RTX PRO 6000 confidential VMs pair with AMD SEV and are outside
  Confidential Space.
- Key Access Justifications need Assured Workloads, and Access
  Transparency covers Google staff, not us. Neither stops our own owner;
  the person's share does.
- Cloud HSM is FIPS 140-2 Level 3 (no 140-3 claim found).
- Google states no rule that a rebuild changes the digest. Byte-identical
  rebuilds are our job (reproducible builds).
