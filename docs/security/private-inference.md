# Private inference

Status: **design, 2026-10-10. The first sealed path is live** (#11241): one
Intel TDX machine in Google Confidential Space answers sealed Clef decisions,
and <https://openagents.com/att> runs a verified round in the browser. See
[Live today](#live-today). Everything else below is still design.
Wire formats are in the draft [NIP-ATT](../../nips/openagents/NIP-ATT.md).
Companion to the [sensitive data vault](sensitive-data-vault.md): the vault
keeps data sealed at rest, and this keeps it sealed while a model reads it.
It supersedes the "deferred" decision in
[confidential hosted inference](../decision-models/service/confidential-inference.md)
by giving it a design. The threat model there still holds.

## Live today

Recorded 2026-10-10 (#11241). This is milestone P2's shape on a CPU: no
GPU, Clef instead of a chat model, and the browser page as the client.

**What runs.** One Confidential VM, `oa-att-tdx-1` (`c3-standard-8`, Intel
TDX, `us-central1-a`, project `openagentsgemini`), boots Google's production
Confidential Space image (`confidential-space-images/confidential-space`,
not the debug image) with the workload service account
`oa-att-workload@` (roles: `confidentialcomputing.workloadUser`,
`logging.logWriter`, `artifactregistry.reader`). The launcher pulls
`us-central1-docker.pkg.dev/openagentsgemini/openagents/att-provider` by
digest and runs `pylon serve --attested` (`crates/pylon/src/attested.rs`):

1. The endpoint key is made in the workload's memory and never written.
2. The Clef-Flash Q4_K_M weights are fetched from Hugging Face at a pinned
   revision and refused unless their SHA-256 is
   `fd3e9060…638c`, which is compiled into the image.
3. **Psionic (OpenAgents)** `psionic-openai-server` serves Clef on the CPU
   on loopback (`POST /v1/systemone`); the pylon refuses to serve unless
   Psionic reports the same artifact digest. No third-party inference
   server is in the image.
4. The pylon asks the launcher (`/run/container_launcher/teeserver.sock`)
   for a PKI token with audience `openagents.att.v1` and the NIP-ATT
   binding of its key and the release as the nonce, and publishes a
   `30203` endpoint carrying it, signed by that key, every 30 minutes.
5. It answers only sealed `25910` decisions that require
   `openagents.attested.v1` and name its endpoint and release. Its answers
   carry `response.attested` (endpoint, release, level, measurement,
   request ciphertext digest, model and weights digest), covered by the
   receipt's seal. It logs no prompt or answer.

**The client** is `crates/oa-att`, the same Rust in the gateway and, as
WebAssembly, in the page. It verifies the release, head and endpoint
signatures; Google's token up to the Confidential Space root, which it pins
by bytes and SHA-256 (`148b2938…6c39`); `hwmodel GCP_INTEL_TDX`,
`swname CONFIDENTIAL_SPACE`, `dbgstat disabled-since-boot` and support
`STABLE`; the image digest against the release; the head's admission and
notice delay; and the binding in `eat_nonce`. Only then does it seal the
question with NIP-44 to the endpoint key. The page's "tamper" choices
change the logged measurement or swap in an unbound key before those
checks, and the round is refused with nothing sent.

**What a browser still trusts.** In a browser, the page and its
WebAssembly are fetched from openagents.com on every visit, so in principle
they could be swapped for one person and read the text before it is sealed.
`/att/release.json` publishes the page's files by SHA-256 (and SRI form),
from the reproducible `scripts/build-att-web.sh` build. The apps (coming,
#11246) and the command line run the same check-and-seal from a signed,
versioned program instead, so they don't have this exposure:

```sh
cargo install --git https://github.com/OpenAgentsInc/openagents oa-att --features net
oa-att round --publisher 77fabebbeb49a7b9b384422ee6ef5662cf4db7da70acc94981378c0017ecc56e \
    --state "The weather in Lisbon is sunny today." --question "Is this about the weather?"
```

Every path still trusts Google and Intel for the hardware evidence. The
page's legend says who sees what: the browser sees the question and the
answer; the OpenAgents relay and gateway see sizes, timing and sealed
bytes; Google and Intel see that a sealed machine ran, not the data; the
sealed program sees the question, inside the TEE only.

**The gateway** is the website's `/att/api/*` (`crates/openagents-web/src/pages/att.rs`).
It runs the same checks and refuses to forward to an endpoint that fails
them, publishes the sealed event unchanged to `wss://relay.openagents.com`,
and returns the endpoint's sealed answers. It sees kinds, keys, sizes and
timing, never the text. Rounds are limited to 3 a minute and 20 an hour per
visitor address, and 240 an hour in all.

**The release.** The publisher key `77fabebb…c56e` (secret: Secret Manager
`att-publisher-key`, and `~/work/.secrets/att-publisher.key` on the owner's
Mac) signs the `3202` release and the `30202` head for the workload
`clef-decisions`. The release lists the image digest, the SHA-256 of
`psionic-openai-server` and `pylon` inside it, the weights digest, the
source commit and the recipe (`deploy/att/Dockerfile`). The demo's notice
delay is 600 seconds; production releases should use days.

**Measured on production (2026-10-10).** Release
`4295efa9…2e6e` admits image `sha256:3bd3098d…825d`, built from
`df789ae8c5` (`psionic-openai-server` `sha256:dd411551…57fc`, `pylon`
`sha256:54b8a621…c71a`). A second Cloud Build of the same commit
(`att-provider:df789ae8c5-rebuild`) produced the identical image digest and
binaries. It is the same build service, not an independent builder, so
P1's gate is not met yet. In the browser, a round takes about 27 s: the
checks take under 300 ms, and Clef on 8 TDX vCPUs about 22 s for the short
question. The demo question answered "yes" at 97.3 %. Both tamper choices
are refused before anything is encrypted. `crates/oa-att/tests/live_fixture.rs`
replays the captured records: the valid chain, a changed measurement, an
unbound key, expired evidence, a head rollback, another publisher, and a
changed claim (Google's signature then fails).

**Run, stop and cost it.** From a checkout, as the automation account:

```sh
cargo build -p oa-att --features net            # the publisher's CLI
scripts/deploy/att-provider.sh build origin/main  # prints the image digest
scripts/deploy/att-provider.sh release DIGEST origin/main   # 3202 + 30202
# wait for the notice delay (600 s), then:
scripts/deploy/att-provider.sh start RELEASE_ID DIGEST
scripts/deploy/att-provider.sh status | logs
scripts/deploy/att-provider.sh stop      # keeps the disk; resume to restart
scripts/deploy/att-provider.sh delete
target/debug/oa-att round --publisher 77fabebbeb49a7b9b384422ee6ef5662cf4db7da70acc94981378c0017ecc56e \
    --state "The weather in Lisbon is sunny today." --question "Is this about the weather?"
```

A new instance makes a new key; clients find it through the head, so a
restart needs nothing else. A new image needs a new release, and the page
admits it only after the notice delay.

Cost: `c3-standard-8` on demand in us-central1 is about $0.40 an hour
(8 vCPU, 32 GB) before the Confidential VM surcharge, so roughly $300 a
month left running, plus a 40 GB balanced disk (about $4 a month). Stopped,
only the disk is billed. `ATT_MACHINE=c3-standard-4` halves the machine
price at the cost of slower answers.

### The sealed GPU lane (H100 in confidential-computing mode)

Recorded 2026-10-10 (#11241). The same attested pylon, with Psionic's Clef
lane on CUDA on one NVIDIA H100 in confidential-computing (CC) mode, in an
`a3-highgpu-1g` (Intel TDX + 1 H100) Confidential Space VM,
`oa-att-h100-1` in `us-east5-a` (us-central1-a was out of Spot capacity).
The VM is Spot, stops (keeping its disk) on preemption and after a 1 hour
maximum run, and is woken on demand: the gateway calls Compute
`instances.start` (the web runtime account may start and stop only that
instance), and the workload stops its own VM after 15 minutes without a
decision (`pylon serve --attested --idle-stop 900`; the workload account
may stop only that instance).

- **Image.** `deploy/att/Dockerfile.gpu`, built by
  `deploy/att/cloudbuild-gpu.yaml` into `.../openagents/att-provider-gpu`:
  NVIDIA's CUDA 13.0.2 devel and runtime images and the Rust 1.97.1 image,
  all pinned by digest. The builder compiles Psionic's kernels with nvcc
  for `sm_90` and refuses to finish unless `psionic-openai-server` links
  `libcudart.so.13` and carries `sm_90` code (no stub kernels). Confidential
  Space installs NVIDIA's CC driver at boot (`tee-install-gpu-driver=true`)
  and mounts it at `/usr/local/nvidia/lib64`, which is on the image's
  `LD_LIBRARY_PATH`.
- **Refusal.** The entrypoint runs `pylon serve --attested --workload
  clef-decisions-gpu --decision-device cuda --decision-chunk 2048
  --require-gpu-cc`: after the first token the pylon refuses to start unless
  `submods.nvidia_gpu.cc_mode` is `ON` and every `gpus[].hwmodel` is
  `GCP_NVIDIA_H100`, and it stops if a refreshed token says otherwise.
- **Release.** Workload `clef-decisions-gpu` has its own head. Its releases
  carry `gpu: {vendor: nvidia, mode: cc, models: [H100]}`, and `oa-att`
  refuses an endpoint whose token does not show that GPU in CC mode
  (`--tamper gpu` shows the refusal).
- **The token** (Google verifies the GPU's NVIDIA evidence and signs it
  into the same token whose `eat_nonce` binds the endpoint key): `hwmodel
  GCP_INTEL_TDX`, `swname CONFIDENTIAL_SPACE`, `dbgstat
  disabled-since-boot`, support `LATEST STABLE USABLE`, and
  `submods.nvidia_gpu` = `cc_mode ON`, `cc_feature SPT`, `gpus: [{hwmodel
  GCP_NVIDIA_H100, driver_version 595.58.03, vbios_version
  96.00.D9.00.01, ueid, l4_serial_number}]`. The driver version is per GPU.
- **Measured.** A wake (`instances.start` to the `30203` on the relay)
  takes about 4 minutes: about 2 for boot and the CC driver install, then
  about 80 s to fetch and check the 6.5 GB weights, load Clef on the GPU and
  get the token. A first create took about 3 m 45 s. A verified `oa-att
  round` takes 355 to 460 ms end to end (six runs), of which Psionic spends
  54 to 56 ms on the H100 for a 186-token decision; the CPU lane takes about
  22 s.
- **Cost.** Spot `a3-highgpu-1g` in us-east5 is about $6.40 an hour (H100
  $5.71, 26 vCPU $0.39, 234 GB $0.30), plus the Confidential Computing
  surcharge for an A3 on Spot, $0.44 an hour: about $6.83 an hour while it
  runs. us-central1 is about $6.68. Each wake costs boot plus the 15 minute
  idle window (about 20 minutes), about $2.30. Stopped, only the 60 GB disk is billed.

```sh
scripts/deploy/att-provider.sh gpu-build origin/main
scripts/deploy/att-provider.sh gpu-release DIGEST origin/main
ATT_ZONE=us-east5-a scripts/deploy/att-provider.sh gpu-retarget RELEASE_ID DIGEST   # the stopped VM's next image
ATT_ZONE=us-east5-a scripts/deploy/att-provider.sh gpu-resume | gpu-stop | gpu-status | gpu-logs
target/debug/oa-att round --publisher 77fabebbeb49a7b9b384422ee6ef5662cf4db7da70acc94981378c0017ecc56e \
    --workload clef-decisions-gpu --state "..." --question "..."
```

### On this device

The same flow works with no TEE when Psionic runs on the person's own
machine, because then the job never leaves the device:

1. Start Psionic on loopback: `psionic-openai-server -m Clef-Flash-Q4_K_M.gguf
   --host 127.0.0.1 --port 18096` (the CUDA or Metal lane where there is
   one).
2. Start a decision pylon on the same machine with a key that stays in its
   home directory: `pylon serve --decide http://127.0.0.1:18096
   --decisions-only --allow <the app's own key> --relay ws://127.0.0.1:PORT`,
   with a relay on loopback (`crates/nostr-relay`, or `pylon`'s in-process
   fixture relay for tests).
3. The app seals each `25910` decision with NIP-44 to that pylon's key, as
   above, and reads the sealed `26910` answer. Nothing crosses the network
   interface, so no attestation is needed: the level is the person's own
   device, which the app shows as "On this device".

The checks that matter here are local: the app compares the pylon's served
identity (`clef-flash@sha256:fd3e9060…`) with the weights it expects, and
the receipt names that digest. The page at `/att` shows only the sealed
cloud path today.

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
