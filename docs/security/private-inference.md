# Private inference

Status: **design, 2026-10-10. Nothing in this document is implemented.**
Wire formats are in the draft [NIP-ATT](../../nips/openagents/NIP-ATT.md).
Companion to the [sensitive data vault](sensitive-data-vault.md): the vault
keeps data sealed at rest, and this keeps it sealed while a model reads it.
It supersedes the "deferred" decision in
[confidential hosted inference](../decision-models/service/confidential-inference.md)
by giving it a design. The threat model there still holds.

## The ask

The owner asked whether a Nostr protocol could carry inference securely, in
the manner of Darkbloom (Layr-Labs/d-inference) and Tinfoil, but built our
own way. That means our own protocol and code, with ideas only from those
projects. Darkbloom is under a proprietary licence (the "DARKBLOOM LICENSE
AGREEMENT", Eigen Labs), so we copy nothing from it.

## Summary

A person's client verifies that a Pylon is an exact, publicly logged program
running in a hardware TEE, then encrypts the job straight to a key that only
that program holds. Our gateway and relays carry ciphertext only. Billing
works from metadata and signed receipts. Each provider is shown to the
person at a **trust level** computed from evidence. Data is routed by how
sensitive it is: financial data goes only to `tee-cloud` endpoints, while
ordinary chat can go to any Pylon.

| Level | Example provider | Can the provider operator read prompts? | Use for |
| --- | --- | --- | --- |
| `open` | Any Pylon today | Yes | Ordinary chat, public work |
| `hardened` | A Mac Pylon with App Attest and a Secure Enclave-bound key | Yes, with root or physical access | Ordinary chat, with less casual exposure. **Never called private.** |
| `tee` | A provider's own TDX or SEV-SNP machine with a confidential GPU | Not without physical or side-channel attacks on their own box | Personal data a person chooses to send |
| `tee-cloud` | Our, or anyone's, TDX + H100 node in Google Confidential Space | No; the hardware roots are Google, Intel and NVIDIA | **Vault and financial data** |

## What we learned from the two references

**Darkbloom** (read from the docs in a shallow clone; no code taken):

- **Providers.** Apple Silicon Macs run MLX inside a hardened provider
  process. The process denies debugger attach, uses Hardened Runtime,
  checks SIP, disables core dumps and scrubs dynamic-loader variables.
- **Provider keys.** Each provider holds an X25519 key per process.
  Registration binds it to a Secure Enclave P-256 key, checked every five
  minutes. Higher trust comes from an MDM cross-check, Apple Managed Device
  Attestation, APNs delivery that proves code identity, and App Attest.
- **Encryption.** Every request is sealed to the provider with a
  per-request NaCl box.
- **The coordinator sees everything.** Darkbloom's coordinator is a Go
  service on a GCP Confidential VM using AMD SEV, not SNP. It decrypts every
  prompt to route, template, fetch media and bill, and its own privacy doc
  says the claim "the coordinator never sees plaintext" is false. No
  attestation of the coordinator is offered to consumers.
- **Root can read the key.** The threat model admits that an operator with
  root can read the provider key from memory, and that the weight-hash check
  fails open.
- **Lesson.** Process hardening on a Mac the provider controls is not a
  TEE. A routing middle that reads plaintext undoes the end-to-end claim.

**Tinfoil** (docs at docs.tinfoil.sh; open-source verifiers such as
`tinfoil-rs`, Apache-2.0):

- **Hardware.** Inference runs in confidential VMs (AMD SEV-SNP or Intel
  TDX) with NVIDIA H100, H200 or B200 in confidential-computing mode. The
  VM refuses to start if the GPU fails attestation at boot.
- **Client checks.** The client fetches the expected measurement from a
  Sigstore bundle produced by CI for a reproducible build, with Rekor
  inclusion. It then checks the enclave's report against the vendor chain,
  compares the measurement registers, and pins TLS (and an HPKE key) to the
  key bound in the report data.
- **Weights** are bound by dm-verity commitments.
- **Limits they admit.** Physical attacks such as TEE.fail, side channels,
  access-pattern metadata, denial of service, and that release freshness is
  beta.
- **Lesson.** Hardware attestation, plus a public reproducible-build log,
  plus a key bound into the report, gives client-to-enclave encryption with
  no plaintext middle. That is the shape we want, carried over Nostr.

## Design

### Parties

- **Client**: web (a WASM verifier and sealer), the phone and Mac apps,
  and Coder.
- **Gateway**: `/v1/systemone`, the chat worker, and the inference gateway.
  It routes events and pays. **It never decrypts a sealed job.**
- **Relays**: carry CJ events and NIP-ATT records.
- **Pylons**: providers. A private-capable Pylon runs an admitted
  [NIP-ATT](../../nips/openagents/NIP-ATT.md) release inside a TEE.
- **Publisher**: OpenAgents signs releases of the private-inference image
  (Psionic serving Clef or a pinned open model). Others may publish their
  own; the client trusts a publisher only by policy.

### Flow

1. **Release.** We build the inference image reproducibly from a pinned
   commit, with the weights digest compiled in. We log it in Rekor and
   publish a NIP-ATT `3202` release. After the notice delay it enters the
   `30202` head.
2. **Endpoint.** A Pylon boots the image on TDX + H100 in Confidential
   Space. The workload generates its secp256k1 endpoint key and an X25519
   HPKE key inside the TEE, and asks the launcher for an attestation token.
   The token's nonce is the NIP-ATT binding of those keys and the release.
   The workload checks the GPU is in CC mode, or refuses to start. It then
   publishes a `30203` endpoint, signed by the endpoint key, every hour. Its
   NIP-PYLON beacon lists the service, the same as today.
3. **Choice.** The client reads beacons and endpoints, verifies each
   endpoint itself (NIP-ATT [client verification](../../nips/openagents/NIP-ATT.md#client-verification)),
   and computes its level. The data's policy says the minimum level:
   `tee-cloud` for vault data, anything for ordinary chat.
4. **Sealed job.** The client sends a CJ (chat) or DEC (decision) request
   with `requires: ["openagents.attested.v1"]`, NIP-44 v2 encrypted to the
   endpoint key. It is optionally gift-wrapped, so the relay doesn't learn
   who is asking. Streamed output comes back sealed per chunk to the
   client's key.
5. **Through the gateway.** A web client that does not speak to relays
   directly posts the sealed event to the gateway. The gateway adds payment
   (x402 or account billing) as metadata, forwards the event unchanged, and
   relays the sealed reply back. It can't read either.
6. **Receipt.** The provider's signed result carries `units`. The buyer's,
   or the gateway's, NIP-PYLON `3201` receipt uses ciphertext digests and
   names the endpoint, release and measurement. That is what billing and
   the public pool totals use.

### What changes in existing code

- **`crates/nostr`**: NIP-ATT types and validators, a verifier for
  Confidential Space PKI tokens, and later TDX, SEV-SNP and NVIDIA evidence.
  The `openagents.attested.v1` feature for CJ and DEC, with the receipt
  fields.
- **`crates/pylon`**: a `--attested` serve mode that only runs inside the
  release image. It refuses caller-paid keys and outside model calls, and
  logs no content.
- **`crates/psionic`** (Clef and open models): serving inside the image,
  with weights checked against the release digest, failing closed.
- **Gateway `/v1/systemone` and the chat worker** (#11225): pick an
  endpoint by level, and pass sealed events through without opening them.
  A request that needs a private level never falls back to Vertex or a
  lower level. It queues or refuses, and says so.
- **Web**: a small, separately pinned WASM module for verifying and sealing,
  shared with the vault.

### Where our private nodes run

Google Cloud options, from the supported-configurations page (checked
2026-10-10):

| Option | CPU TEE | GPU | Confidential Space | Zones | Notes |
| --- | --- | --- | --- | --- | --- |
| `a3-highgpu-1g` | Intel TDX | 1 × H100 80 GB | Yes (Google's blog calls it a preview) | europe-west4-c, us-central1-a, us-east5-a | Spot or flex-start only for GPU workloads in Confidential Space. No reservations. **Our `tee-cloud` lane.** |
| `g4-standard-48` | AMD SEV | RTX PRO 6000 Blackwell | No | many | SEV attests through the vTPM at boot only. Usable as a plain Confidential VM with our own evidence handling. That would be level `tee` at best; it is not used for vault data. |
| `c3-standard-*` / `c4-standard-*` | Intel TDX | none | Yes | many | CPU-only small models, the vault key-release workload, and the attested router if we ever need one. |
| Confidential GKE Nodes | SEV, SEV-SNP, TDX | as the node type allows | n/a | n/a | An option for scale later. The attestation story is ours to build. |

Pricing: Confidential VM adds a per-vCPU and per-GB surcharge on top of the
machine price, which differs by technology. The GPU and A3 machine price
dominates. Check https://cloud.google.com/confidential-computing/confidential-vm/pricing
and the Compute Engine GPU pricing before sizing. A single spot A3 High
node is the first step, not a fleet.

### How this fits the rest

- **Clef and Psionic (#11225).** Decisions already route to Pylons through
  `/v1/systemone`. A private decision is the same job with the
  `openagents.attested.v1` feature, sent to an attested endpoint. Clef runs
  inside the image; because it's open weights, the release pins its digest.
- **Vertex Gemini** is not a TEE. It is never used for anything labelled
  private. In the vault it is the **Fast** route, chosen by the person and
  labelled "processed by Google Gemini".
- **The vault.** The vault workload opens a person's files (with their
  share) and, for the Private route, sends a sealed job to an attested
  inference endpoint. It verifies that endpoint itself, just as a client
  would. Data moves enclave to enclave and is never plaintext in between.
  The vault and the model may also share one image on one A3 node, which
  removes the hop.
- **Ordinary Pylons** are unchanged and stay `open`. Their beacons need
  nothing new. The Verse compute rule ("OpenAgents' own agents send pooled
  compute only work a stranger could read") still holds for `open` and
  `hardened`.

## Honest limits

- **Hardware roots.** `tee-cloud` trusts Google's attestation service and
  data-center physical security, Intel's TDX module, and NVIDIA's GPU
  firmware and attestation. A compromise of any of them breaks the
  guarantee.
- **Side channels and physical attacks.** Published memory-bus interposer
  attacks (TEE.fail) extract secrets from TDX and SEV-SNP machines with
  physical access. For a provider's own box (`tee`), that attacker is the
  provider. This is why vault data needs `tee-cloud`.
- **The image is ours.** We publish the image that runs. A malicious release
  could leak prompts. The mitigations are the same as the vault's:
  reproducible builds, Rekor, a notice delay, independent rebuilds, and
  per-person approval. They make a bad release public before it is
  admitted; they do not make one impossible.
- **Client code on the web** is served by us (see the vault's threat
  model).
- **Metadata leaks** at every level: who asks (unless gift-wrapped, and
  even then the gateway knows its paying account), which endpoint, sizes in
  NIP-44's padding bands, timing, and the model.
- **`hardened` is not private.** A Mac provider with root can read the
  process memory; Darkbloom's own threat model says so. We show `hardened`
  as "harder to snoop on", never as private.
- **No proof of correct computation.** Attestation says which code ran, not
  that the answer is right.
- **Capacity.** Spot or flex-start A3 capacity in three zones means private
  answers can queue or fail. They fail closed, never falling back to a
  lower level without the person's choice.

## What the person sees

- On a project or chat: **Private** "Processed only inside sealed hardware."
  · **Fast** "Processed by Google Gemini." · **Any helper** "Answered by
  community computers that can read your messages."
- On an answer: a small lock with "Sealed hardware · OpenAgents release
  2026-10-17 · verified on your device". It links to the release's public
  record.
- When Private is busy: "Sealed hardware is busy. Wait, or use Fast for this
  answer?" It never switches without asking.

## Milestones

| # | Milestone | Gate |
| --- | --- | --- |
| P0 | NIP-ATT types and validators in `crates/nostr`, and a Confidential Space PKI token verifier (Rust and WASM). | Fixtures: valid, debug image, wrong digest, binding mismatch, expired, head rollback. |
| P1 | Reproducible inference image (Psionic plus one open model), Rekor entry, `3202`/`30202` publishing, and two independent builders matching. | A second builder reproduces the digest. |
| P2 | One spot `a3-highgpu-1g` Confidential Space node publishing `30203` and serving `openagents.attested.v1` decisions and chat. | **Canary test**: a sealed prompt's canary never appears in the gateway, relay or Cloud Logging. A debug-image node is refused by the client. |
| P3 | Gateway pass-through, level-based routing, ciphertext-digest receipts, and web and app verification. | A vault Private turn end to end on production, with the receipt naming the measurement. |
| P4 | Third-party `tee` Pylons (raw TDX, SEV-SNP and NVIDIA evidence), and a `hardened` level for Macs. | Level computed only from evidence. Each provider class shows the right label. |

## Sources

- Darkbloom (Layr-Labs/d-inference) docs, read locally:
  `README.md`, `docs/architecture/security/{encryption,attestation,provider-trust,identity-binding,enrollment}.md`,
  `docs/consumer/privacy-expectations.md`, `docs/threat-model.yaml`,
  `docs/reports/2026-07-17-eigencloud-to-gcp-migration.md`. Proprietary
  licence; ideas only.
- Tinfoil docs: https://docs.tinfoil.sh/llms.txt (verification, the secure
  enclave primer, attestation architecture, EHBP); Rust SDK
  https://github.com/tinfoilsh/tinfoil-rs.
- Google Cloud: supported configurations
  https://docs.cloud.google.com/confidential-computing/confidential-vm/docs/supported-configurations;
  attestation overview
  https://docs.cloud.google.com/confidential-computing/confidential-vm/docs/attestation-overview;
  Confidential VM overview
  https://docs.cloud.google.com/confidential-computing/confidential-vm/docs/confidential-vm-overview;
  pricing https://cloud.google.com/confidential-computing/confidential-vm/pricing;
  Confidential Space deploy (GPU, spot/flex-start)
  https://docs.cloud.google.com/confidential-computing/confidential-space/docs/deploy-workloads;
  custom attestation tokens
  https://docs.cloud.google.com/confidential-computing/confidential-space/docs/connect-external-resources;
  H100 preview
  https://cloud.google.com/blog/products/identity-security/from-clicks-to-clusters-confidential-computing-expands-with-intel-tdx;
  Vertex AI data retention
  https://docs.cloud.google.com/vertex-ai/generative-ai/docs/vertex-ai-zero-data-retention.
- TEE.fail (physical memory-bus attacks on TDX and SEV-SNP), as cited by
  Tinfoil's secure-enclave primer.
