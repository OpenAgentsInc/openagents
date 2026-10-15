# Opaque Systems and confidential AI: what we should adapt

Research note, 2026-10-14. The owner asked us to research Opaque after the
news that Anthropic is licensing its technology, and to list anything we
should adapt from its approach where we want the same encryption and
privacy guarantees.

## Summary

- **Who they are.** Opaque Systems is a San Francisco company that runs AI
  jobs inside hardware-sealed machines (trusted execution environments,
  TEEs) and hands the customer signed evidence of what ran, where it ran and
  which rules held. It came out of UC Berkeley's RISELab. On October 13,
  2026 Anthropic agreed to license its technology and hire part of its team,
  including CEO Aaron Fulkerson. It is a license plus a hire, not a purchase,
  and the terms are undisclosed.
- **Their technology is not magic.** It is the same stack anyone can rent:
  Intel TDX or AMD SEV-SNP confidential VMs, NVIDIA H100 GPUs in
  confidential-computing mode, the vendors' attestation services, a
  Kubernetes control plane, and a verifier that releases keys only to
  attested workloads. Their real product is the layer on top: the
  workload's whole configuration is locked and hashed at registration, the
  policy travels with the agent, and every run leaves a signed record
  ("receipt") that a third party can check.
- **We already have the core.** Since 2026-10-10 we run a sealed Clef lane
  in Google Confidential Space on Intel TDX, with and without an H100 in
  confidential-computing mode, a client that verifies the hardware evidence
  before sealing a request to the workload's key, and a signed public
  release list with a notice delay
  ([private inference](../../security/private-inference.md),
  [NIP-ATT](../../../nips/openagents/NIP-ATT.md)). The vault design
  already has attestation-bound Cloud KMS key release
  ([sensitive data vault](../../security/sensitive-data-vault.md)).
- **What Opaque does that we don't, yet.** (1) Its evidence says which
  *rules* held, not just which image ran. (2) It covers agent runs that use
  tools and data, not only a model call. (3) It covers every workload that
  touches customer data, not one demo lane. Our cloud computers, our chat
  worker and our BYOK Claude token path all run outside sealed hardware
  today, and on cloud computers the token is briefly written to `/tmp` in
  plain text.
- **Top three to adopt, smallest first:** put the rules into the evidence
  (a policy digest in every release and receipt); move cloud computers to
  Confidential VMs and release BYOK tokens only to an attested machine, in
  memory; publish every image that touches customer data in an append-only
  log with an independent rebuild, in the style of Apple's Private Cloud
  Compute.

## Who Opaque is, and what we verified

"Verified" means a primary source or two independent reports agree.
"Inferred" means our reading, or a single secondary source.

| Claim from the owner's paste | Status | Notes and sources |
| --- | --- | --- |
| Anthropic is licensing Opaque's technology and hiring some engineers | **Verified** | Announced 2026-10-13. A license plus a partial hire, not a full acquisition; terms not disclosed; Fulkerson posted about the agreement. [Constellation Research](https://www.constellationr.com/insights/news/anthropic-licenses-releases-opaque-acquihire), [Crypto Briefing](https://cryptobriefing.com/anthropic-licenses-opaque-systems-confidential-ai/), [MarketScreener](https://www.marketscreener.com/news/anthropic-signs-deal-with-ai-security-startup-opaque-systems-ce785dd2dd8af324). We found no announcement on Anthropic's own site. |
| CEO Aaron Fulkerson joins Anthropic | **Verified** (reported by several outlets) | Fulkerson is CEO since October 2023 and is not a founder ([opaque.co/about](https://www.opaque.co/about)). He was earlier a ServiceNow executive ([Constellation](https://www.constellationr.com/blog-news/insights/opaque-systems-names-servicenow-alum-fulkerson-ceo)). |
| CTO Rishabh Poddar joins Anthropic | **Not confirmed** | Poddar is a co-founder and CTO ([opaque.co/about](https://www.opaque.co/about)). His move is reported only by an aggregator post on X; no primary source names him. Some employees are reported to be looking for other roles. |
| Began as a UC Berkeley RISELab project (Opaque, MC², Secure XGBoost) about 10 years ago | **Verified, with a date fix** | The original Opaque system was built at RISELab in 2017 (about nine years ago); the company was founded in 2021. Co-founders: Ion Stoica, Raluca Ada Popa, Rishabh Poddar, Chester Leung, Wenting Zheng ([about](https://www.opaque.co/about)). |
| $24M Series B in February at about $300M, $55.5M total | **Verified** | Announced 2026-02-12, led by Walden Catalyst, with Intel Capital, Race Capital, Storm Ventures, Thomvest and ATRC; earlier rounds $9.5M (2021) and $22M (2022) ([SiliconANGLE](https://siliconangle.com/2026/02/12/opaque-raises-24m-300m-valuation-confidential-ai-platform/), [Crowdfund Insider](https://www.crowdfundinsider.com/2026/02/262175-opaque-raises-24m-series-b-to-expand-confidential-ai-platform-for-enterprises/)). One outlet says $31.6M total; we take $55.5M, which matches the round sum. $300M is the February valuation, not the deal price. |
| Anthropic was already a customer | **Verified (vendor-sourced)** | Opaque's June 2026 Confidential Computing Summit slides list Anthropic, ServiceNow and Accenture as design partners ([slides](https://hosted-files.sched.co/ccsummit2026/0e/opaque-rishabh-ccsummit.pdf)); Crypto Briefing calls Anthropic a prior customer. |
| ServiceNow and Accenture are customers | **Verified (vendor-sourced)** | Case studies on [opaque.co/about](https://www.opaque.co/about) and the slides above. |
| Anthropic published a confidential inference design with Pattern Labs in June 2025, noting some AI chips lack full confidential-computing support | **Verified** | [Confidential Inference via Trusted Virtual Machines](https://www.anthropic.com/research/confidential-inference-trusted-vms), 2025-06-18, with the whitepaper [Confidential Inference Systems](https://assets.anthropic.com/m/c52125297b85a42/original/Confidential_Inference_Paper.pdf). It says not all accelerators fully support confidential computing yet. Pattern Labs has since renamed itself Irregular ([Irregular](https://www.irregular.com/research/confidential-inference-systems)). |

Open after the deal (inferred from the reporting): whether Opaque keeps
operating and serving its customers, and when or how the technology shows
up in Anthropic products. Nobody has said.

## How their technology works, simply

**The research roots (2017 to 2022).** All three projects are Apache-2.0
on GitHub and have not been updated since 2022 to 2023.

- **Opaque** (NSDI 2017, Zheng, Dave, Beekman, Popa, Gonzalez, Stoica):
  Spark SQL whose operators run inside Intel SGX enclaves, so the cloud
  sees only encrypted data. An optional "oblivious" mode also hides memory
  access patterns, at 1.6x to 46x extra cost
  ([paper](https://usenix.org/conference/nsdi17/technical-sessions/presentation/zheng)).
  The code lives on as [mc2-project/opaque-sql](https://github.com/mc2-project/opaque-sql).
- **Secure XGBoost** (Law, Leung, Poddar, Popa and others, 2020): several
  parties train one XGBoost model on their pooled data inside enclaves,
  with data-oblivious algorithms against side channels
  ([arXiv 2010.02524](https://arxiv.org/abs/2010.02524),
  [repo](https://github.com/mc2-project/secure-xgboost)).
- **MC²** ([repo](https://github.com/mc2-project/mc2)): the umbrella
  platform for secure analytics and ML that bundled the two.

These were built on SGX process enclaves. The industry has since moved to
whole confidential VMs (TDX, SEV-SNP) because they run unmodified software,
and Intel dropped SGX from its client CPUs.

**The company platform (2025 to 2026).** From their summit slides and
press coverage ([SiliconANGLE, Oct 2025](https://siliconangle.com/2025/10/29/opaque-introduces-confidential-ai-platform-end-end-privacy-compliance-guarantees/),
[Converge Digest, Jun 2026](https://convergedigest.com/opaque-launches-open-framework-for-verifiable-ai-agents-and-secure-mcp/)):

1. **Sealed machines.** The customer-hosted data plane is Kubernetes on
   Intel TDX, AMD SEV-SNP and NVIDIA H100 confidential computing. Memory is
   encrypted by the CPU, so the cloud operator, the hypervisor and the
   customer's own admins cannot read a running job.
2. **Attestation.** Before a job runs, the hardware signs a report of what
   booted. Their verifier ("OAS") checks CPU evidence against Intel or AMD
   roots (on Azure, through Microsoft Azure Attestation) and GPU evidence
   through NVIDIA's Remote Attestation Service (NRAS).
3. **A locked configuration.** At registration they hash the container
   image digest, environment variables, command-line arguments, volume
   mounts and network policy into one value. Any change fails attestation.
   This is what lets them say which *rules* were in force, not only which
   code.
4. **Attested TLS.** Services talk to each other over TLS whose keys are
   bound into the attestation, so a service only sends data to a verified
   peer. Their slides show this to model providers too (Anthropic, Azure
   OpenAI) and to data sources (Snowflake, Postgres).
5. **Keys tied to attestation.** Signing and encryption keys are made
   inside the TEE and are useless outside it; they advertise post-quantum
   algorithms (ML-KEM, ML-DSA).
6. **Agent identity and policy.** "Agent Manifest" gives each agent a
   signed identity, and "Confidential MCP" gates tool calls; both extend
   Microsoft's open-source Agent Governance Toolkit. A PII sanitizer runs
   in the data plane.
7. **Evidence.** Each run produces a signed record: the attestation
   document, the workload identity, the locked configuration, the policy
   result, and a "workload graph" of what touched what. An auditor can check
   it without trusting Opaque.

The claims are mostly vendor-sourced; there is little independent review.
General availability of "OPAQUE 3.0" was scheduled for July 2026.

## The Anthropic deal and what it signals

- **What Anthropic designed already.** The June 2025 design keeps data
  encrypted except at the moment of processing. A small trusted loader in a
  separate VM decrypts and feeds the accelerator, runs only programs signed
  by Anthropic's reviewed CI, and receives keys from a keyserver only after
  a TPM-rooted attestation shows encrypted memory, debugging off and the
  right code. Most of the serving stack stays outside the trusted part. It
  admits that sharing encrypted memory with accelerators is not well
  established and that Anthropic then relied on its cloud providers for
  physical and hypervisor security.
- **What the deal adds** (inferred): people who have shipped
  attestation, policy and evidence to regulated enterprises, and a product
  shape for "Claude on your confidential data, with proof". Coverage ties
  it to Anthropic's push into finance, healthcare, sovereign and on-premise
  deployments.
- **What it means for us** (inferred):
  - Buyers of agent work will expect *proof* that a run kept to its rules,
    not a policy page. Opaque's slides point at DORA, the EU AI Act and
    HIPAA asking for runtime evidence; that is their framing, but the
    direction is clear.
  - A Claude endpoint with attestation may appear. Our private-inference
    levels should be able to admit a model provider's attested endpoint as
    evidence, the same way we admit our own.
  - None of this changes our rules. We build on our own GCP with Google's
    confidential computing, our own code and our own protocol, as we did
    for the sealed Clef lane.

## Our side today

| Surface | Where data is plain text today | Where |
| --- | --- | --- |
| Sealed Clef decisions | Only inside a TDX (and TDX + H100) Confidential Space VM | `crates/pylon/src/attested.rs`, `crates/oa-att`, `deploy/att/`, [private inference](../../security/private-inference.md) |
| BYOK Claude credentials at rest | Sealed with AES-256-GCM through `oa_seal`, keyring in Secret Manager | `crates/openagents-web/src/cloud/byo.rs`, `cloud/custody.rs`; [audit](../../audits/2026-10-14-computers-for-chats-audit.md) |
| BYOK plan token on a cloud computer | Decrypted per run, **written to `/tmp/oa-coder-{job}.env` in plain text** on the VM, then held in the process environment. The in-memory per-turn `release_turn` exists but has no callers | `crates/coder-cloud/src/boat_backend.rs`, `byo.rs` `release_turn`; same audit, rows on plan tokens |
| Cloud computers | Plain GCE VMs (`e2-standard-*`, `n2d-standard-*`) made by `oa-boat`, driven over SSH. Per-turn env values stay in `oa-computers` memory and go over SSH stdin | `crates/oa-boat/src/sizes.rs`, `crates/oa-computers`, [computers](../../cloud/computers.md) |
| Chat worker | NIP-44 sealed from the phone to the worker, plain text in the worker and at Vertex Gemini | [chat worker](../../deployment/chat-worker.md) |
| Model gateway | Vertex first; Vertex is not a TEE | `crates/gateway`, [private inference](../../security/private-inference.md) "Vertex Gemini is not a TEE" |
| Project files and Drive/Gmail | Vault Tier A (client keys) is built; Tier B (sealed hardware plus the person's share) is design. Connection secrets are sealed refs resolved at call time; tools are gated by allow/approve/block policy | [vault](../../security/sensitive-data-vault.md), `crates/oa-connections/src/lib.rs` |
| Pylon contributors | Plain text on the contributor's machine. Honesty comes from self-checks and sampled recompute after answers arrive | `crates/pylon/src/verify.rs`, `items.rs`; [reax.md](../decisionmodels/reax.md), [calibration.md](../decisionmodels/calibration.md) |
| Receipts | Digests, not content; "a receipt is a claim, not attestation" | `crates/receipts/src/lib.rs`; sealed answers already carry `response.attested` |
| Payments | Bitcoin only (Lightning, MDK payout). Unchanged by anything here | [payments](../../payments/README.md) |

## What we should adapt

Effort: **S** about a day, **M** a few days to a week, **L** several weeks.
Costs are Google list prices from our own measurements in
[private inference](../../security/private-inference.md) unless noted.

### 1. Put the rules into the evidence

**Opaque's idea.** Lock and hash the whole run configuration, not just the
image: arguments, environment names, mounts, network egress, tool policy.
Then a receipt can say which rules held.

**Ours.** Add a `policy` digest to the NIP-ATT `3202` release and to the
sealed answer's `response.attested` block: the entrypoint and flags (for
example `--require-gpu-cc`, `--idle-stop`), the allowed egress hosts, the
log policy ("no prompt or answer"), and for agent runs the
`oa-connections` policy set in force. The client already compares the
image digest with the release; it compares this digest the same way.
Confidential Space already enforces most of this: the launcher refuses
environment and argument overrides the image does not allow, and the token
carries the image digest and arguments.

- Cost: none to run.
- Maturity: high. It is our own format over Confidential Space claims we
  already verify.
- Effort: **S** for the decision lane; **M** once agent runs carry their
  tool policy.

### 2. Cloud computers and BYOK tokens on attested machines

**Opaque's idea.** Sensitive work runs only on machines whose evidence was
checked, and secrets are released only to that evidence.

**Ours, in two steps.**

1. **Stop the plain-text file** (no TEE needed): wire the existing
   `release_turn` so the plan token goes in memory, per turn, to the
   process only, as `/v1/work` hosts already do. This closes the worst gap
   the [computers audit](../../audits/2026-10-14-computers-for-chats-audit.md)
   found.
2. **Confidential VMs for computers.** Make `oa-boat` create computers as
   Confidential VMs: `n2d-standard-*` with AMD SEV or SEV-SNP for the
   small sizes (today's `e2` machines cannot be confidential), `c3-standard-*`
   with TDX for larger ones. At boot the computer gets a Google Cloud
   Attestation token from its vTPM (`go-tpm-tools`, Apache-2.0) naming the
   boot image and secure boot state. `oa-computers` records it, and
   `release_turn` releases a BYOK credential only to a computer whose token
   is fresh and names an image in our log.

**Be honest about what step 2 buys.** A computer is a general-purpose VM
we drive over SSH. A Confidential VM protects its memory from Google's
host, hypervisor and other tenants, and from cold disks and snapshots
outside the VM. It does **not** protect it from us, because we can still
SSH in. Protection from us needs Confidential Space (no SSH in the
production image, a single container, the launcher measures it), and that
does not fit an interactive computer. So step 2 is "sealed against the
cloud", not "sealed against OpenAgents", and the UI must say so.

- Cost: the confidential surcharge is small; the bigger change is `e2` to
  `n2d` (inferred: roughly 10 to 30 % more per vCPU, check the pricing
  page). SEV on N2D keeps live migration; SEV-SNP and TDX do not.
- Maturity: high for SEV and TDX VMs (GA on GCP); medium for SEV-SNP
  attestation (Google's SNP reports moved to v4 in October 2025 and broke
  older parsers).
- Effort: step 1 **S**; step 2 **M**.

### 3. Signed run evidence tied to our receipts

**Opaque's idea.** Every run leaves a record a third party can check: what
ran, on which machine, under which rules, touching what.

**Ours.** A new versioned receipt type in `crates/receipts` (a run
receipt, next to `execution`), signed by a key that lives only in the
attested workload, with:

- the image digest and the release id,
- the digest of the attestation token (and for GPU work, the CC mode and
  GPU model claims we already check),
- the policy digest from item 1,
- digests of the inputs and outputs (never content), the data objects and
  connections used, and each approval decision from `oa-connections`,
- the payer as metadata (Bitcoin receipts are unchanged).

It upgrades "a receipt is a claim, not attestation" to "a claim bound to
hardware evidence" where the run was sealed, and stays a plain claim
everywhere else, labelled as such. The sealed decision lane's
`response.attested` is the first instance; this generalises it to agent
runs and sealed chat.

- Cost: none to run.
- Maturity: our format; Opaque's "workload graph" shows the shape is
  useful to auditors.
- Effort: **M**.

### 4. A transparency log of every image that touches customer data

**Apple's idea** ([Private Cloud Compute](https://security.apple.com/blog/private-cloud-compute/)):
measurements of all production software go into an append-only,
tamper-evident log; devices send data only to nodes whose attested
measurement is in the log; images are published for researchers.
Opaque does the per-customer version (a locked configuration per
workload); Tinfoil does it with Sigstore and reproducible builds.

**Ours.** NIP-ATT already gives signed releases, a head, rollback checks
and a notice delay over our relays. What is missing (NIP-ATT
"Implementation status"):

- **Rekor entries** for each release, and **an independent rebuild** (a
  second builder that is not our Cloud Build; today's rebuild matched, but
  on the same service). Rekor is Apache-2.0; we can run our own instance on
  GCP and have it witnessed, or also post to the public Sigstore log
  (owner decision: that is a third party, though a public log rather than
  a hosted service holding data).
- **Coverage.** Log every image that sees customer plain text: the chat
  worker, the `oa-coder-host` computer image, the vault workload, and the
  att-provider images. A computer's attestation token (item 2) then names
  an image anyone can look up.
- **Published images.** Make the logged images pullable so outsiders can
  inspect them, as Apple does.

- Cost: a small always-on service if we self-host Rekor; storage is tiny.
- Maturity: Rekor and Sigstore are mature; our NIP-ATT log is live.
- Effort: **M**.

### 5. Confidential GPUs for model serving where they exist

**Ours.** Done for Clef: H100 in CC mode inside TDX Confidential Space,
355 to 460 ms per verified decision, about $6.83 an hour on Spot in
us-east5. Next is a sealed *chat* lane: an open-weights chat model on the
same node type for projects marked Private, with the chat worker passing
sealed jobs through (private-inference P3). Gemini on Vertex stays the
**Fast** choice, labelled as Google processing it.

Honest limits:

- On GCP the confidential GPU options are `a3-highgpu-1g` (TDX, one H100,
  three zones, no reservations, no multi-node) and `g4-standard-48` (AMD
  SEV with one RTX PRO 6000 Blackwell, GA 2026-08-07, but SEV attests at
  boot only and Confidential Space does not support it). No confidential
  B200 on GCP that we found. So large models that need several GPUs cannot
  be sealed on GCP today.
- Spot capacity in three zones means private answers can queue; they fail
  closed.
- Google's own "Private AI Compute" (Gemini on TPUs in enclaves) is for
  Google's products; we found no sign it is offered to Vertex customers
  (inferred, secondary sources only).

- Cost: about $6.83 an hour per running H100 node on Spot; about $2.30
  per wake with the 15 minute idle stop.
- Maturity: medium (H100 CC GA on GCP since 2025-07-31; one GPU per VM).
- Effort: **L** for a sealed chat lane.

### 6. Attestation for Pylon contributors, as a complement to re-runs

**Ours.** Sampled recompute (`pylon check decide`) checks that answers are
*right*. Attestation would check that the job stayed *private*. They answer
different questions, so attestation complements re-runs and never
replaces them. The trust levels in
[private inference](../../security/private-inference.md) already fit:

- **`tee`**: a contributor with a TDX or SEV-SNP server and an H100, H200,
  Blackwell data-center GPU or RTX PRO 6000 in CC mode. Evidence: the CPU
  quote plus an NRAS token for the GPU (NVIDIA's `nvtrust`, Apache-2.0).
  Few contributors own this hardware. The owner of the box can still try
  physical attacks (TEE.fail), so `tee` is for data a person chooses to
  send, not vault data.
- **`hardened`**: a Mac with a Secure Enclave-bound key and App Attest.
  This proves the key belongs to our signed app on real Apple hardware. It
  does **not** stop the Mac's owner, with root, from reading memory. Never
  called private.
- **`open`**: everyone else, including every consumer NVIDIA card (RTX
  4080, 4090, 5090: no confidential-computing mode) and consumer CPUs
  (SEV-SNP is EPYC-only, TDX is Xeon-only; inferred from vendor support
  lists).

So for nearly all contributors, attestation adds identity (this is our
binary, these weights) and nothing for privacy. Private work stays on our
`tee-cloud` lane, and the Verse rule holds: pooled compute only gets work a
stranger could read.

- Cost: none to us; hardware is the contributor's.
- Maturity: high for data-center hardware, none for consumer GPUs.
- Effort: **M** (raw TDX and SNP quote and NRAS verification in `oa-att`,
  private-inference P4).

### 7. Customer-held keys

**Opaque's idea.** Keys live in the customer's control and are released
only to attested workloads.

**Ours.**

- **People:** vault Tier A (only your devices) is built; Tier B (the system
  key in Cloud KMS, released only to the attested vault image, plus the
  person's share on every request) is designed. Shipping Tier B is the
  customer-held key for individuals.
- **Teams and companies:** let a workspace bring its own key-encryption
  key, either in its own Cloud KMS project (we get a decrypt grant
  conditioned on our workload identity and attestation) or through Cloud
  External Key Manager so the key never leaves their own key manager. They
  can cut us off by revoking it, and their key logs show every use.

- Cost: KMS operations are cents per ten thousand; EKM needs the customer
  to run or buy an external key manager.
- Maturity: high (Cloud KMS, workload identity federation, EKM are GA).
- Effort: Tier B **L** (already scoped in the vault doc); team keys **M**
  after Tier B.

## What not to adopt, and why

- **Opaque's product itself.** It is a hosted, Azure-leaning enterprise
  platform (Auth0, Azure Attestation, Kubernetes) whose future after the
  Anthropic deal is unclear, and our rules say our own GCP, no hosted third
  parties. We take the ideas, not the service.
- **SGX enclaves and oblivious algorithms** from Opaque SQL and Secure
  XGBoost. SGX needs rewritten software and is gone from client CPUs;
  oblivious execution costs up to 46x. Whole confidential VMs are the
  practical unit now.
- **A Kubernetes confidential control plane** (Confidential GKE, Contrast,
  Confidential Containers). Useful at scale, but our sealed workloads are a
  handful of single VMs, and Confidential Space already gives us a
  measured launcher. Edgeless's Constellation, an earlier option, was
  archived on GitHub in January 2026.
- **"Sealed against us" claims for interactive computers.** Not true while
  we hold SSH. We say "sealed against the cloud" until a computer can run
  without our access.
- **Calling Gemini on Vertex private.** It is not a TEE for us. It stays
  labelled Fast.
- **Calling consumer hardware private.** Macs and GeForce cards get
  `hardened` or `open`, never private.
- **Post-quantum keys now.** Opaque advertises ML-KEM and ML-DSA. Our
  NIP-44 and secp256k1 stack would need a protocol change; we note it and
  wait for the Nostr ecosystem.
- **Compliance marketing** ("math to your auditor"). We show the receipt
  and say plainly what it does and does not prove: attestation shows which
  code ran, not that the answer is right.

## Build order, smallest first

| # | Step | Size | Gate |
| --- | --- | --- | --- |
| 1 | Wire `release_turn`: the BYOK plan token goes to a computer per turn, in memory, never to `/tmp` | S | A canary token never appears on the VM's disk or in logs |
| 2 | Policy digest in NIP-ATT releases and sealed answers; `oa-att` compares it | S | A changed flag or egress list is refused, like a changed image |
| 3 | `oa-boat` computers on Confidential VMs (`n2d` SEV for small sizes, `c3` TDX for large); record the boot attestation token | M | A computer's record shows a verified token; the UI says "sealed against the cloud" |
| 4 | `release_turn` releases only to a computer with a fresh token naming a logged image | S after 3 | A computer booted from an unlogged image gets no credential |
| 5 | Rekor entries, an independent rebuilder, and the log covering the chat worker, computer image and vault images | M | A second builder outside Cloud Build matches the digest (private-inference P1) |
| 6 | Run receipts in `crates/receipts`, signed inside the attested workload, with policy, inputs, connections and approvals by digest | M | A third party verifies a receipt with only public keys and the log |
| 7 | Vault Tier B: attestation-bound KMS key release with the person's share | L | The vault doc's milestone gates |
| 8 | Sealed chat lane on A3 H100 for Private projects | L | Private-inference P3: a canary prompt never appears in the gateway, relay or Cloud Logging |
| 9 | `tee` and `hardened` Pylon levels from raw TDX, SNP and NRAS evidence | M | Each provider class shows the right label; re-runs still run for all |
| 10 | Team-held keys through customer Cloud KMS or EKM | M | Revoking the customer's key stops all decryption at once |

## Open questions for the owner

1. **Computers:** is "sealed against the cloud, not against us" worth the
   switch from `e2` to `n2d`/`c3` machines for every computer, or only for
   projects marked Private?
2. **Public log:** self-host Rekor on our GCP only, or also post entries to
   the public Sigstore log (a third party, but one that holds no data)?
3. **Sealed chat:** which open-weights model for a Private chat lane, given
   one H100 per sealed VM on GCP?
4. **Anthropic:** should we ask Anthropic whether an attested Claude
   endpoint is coming, and whether that changes their position on stored
   plan tokens if the token is only released to attested machines?
5. **Team keys:** is there a customer asking for bring-your-own-key now,
   or does it wait for one?

## Sources

- Deal: [Constellation Research](https://www.constellationr.com/insights/news/anthropic-licenses-releases-opaque-acquihire),
  [Crypto Briefing](https://cryptobriefing.com/anthropic-licenses-opaque-systems-confidential-ai/),
  [MarketScreener](https://www.marketscreener.com/news/anthropic-signs-deal-with-ai-security-startup-opaque-systems-ce785dd2dd8af324),
  [Explainx analysis](https://explainx.ai/blog/anthropic-opaque-confidential-ai-license-acquihire-300m-valuation-2026),
  [Rohan Paul on X](https://x.com/rohanpaul_ai/status/2110467312549810266) (the only source naming Poddar).
- Company: [opaque.co/about](https://www.opaque.co/about);
  [SiliconANGLE, Series B](https://siliconangle.com/2026/02/12/opaque-raises-24m-300m-valuation-confidential-ai-platform/);
  [SiliconANGLE, platform](https://siliconangle.com/2025/10/29/opaque-introduces-confidential-ai-platform-end-end-privacy-compliance-guarantees/);
  [Converge Digest, OPAQUE 3.0](https://convergedigest.com/opaque-launches-open-framework-for-verifiable-ai-agents-and-secure-mcp/);
  [CC Summit 2026 slides](https://hosted-files.sched.co/ccsummit2026/0e/opaque-rishabh-ccsummit.pdf).
- Research: [Opaque, NSDI 2017](https://usenix.org/conference/nsdi17/technical-sessions/presentation/zheng);
  [Secure XGBoost](https://arxiv.org/abs/2010.02524);
  repos [mc2](https://github.com/mc2-project/mc2),
  [opaque-sql](https://github.com/mc2-project/opaque-sql),
  [secure-xgboost](https://github.com/mc2-project/secure-xgboost) (all Apache-2.0).
- Anthropic: [Confidential Inference via Trusted VMs](https://www.anthropic.com/research/confidential-inference-trusted-vms);
  [whitepaper PDF](https://assets.anthropic.com/m/c52125297b85a42/original/Confidential_Inference_Paper.pdf);
  [Irregular (formerly Pattern Labs)](https://www.irregular.com/research/confidential-inference-systems).
- Google Cloud: [supported configurations](https://docs.cloud.google.com/confidential-computing/confidential-vm/docs/supported-configurations);
  [release notes](https://docs.cloud.google.com/confidential-computing/confidential-vm/docs/release-notes);
  [Confidential Space overview](https://docs.cloud.google.com/confidential-computing/confidential-space/docs/confidential-space-overview);
  [Private AI Compute coverage](https://thehackernews.com/2025/11/google-launches-private-ai-compute.html).
- NVIDIA: [supported platforms for confidential containers](https://nvidia.github.io/cloud-native-docs/review/pr-499/confidential-containers/latest/supported-platforms.html);
  [Blackwell confidential computing](https://developer.nvidia.com/blog/hardware-rooted-ai-security-that-wont-slow-you-down/);
  [nvtrust](https://github.com/NVIDIA/nvtrust) (Apache-2.0).
- Open source: [Confidential Containers](https://github.com/confidential-containers/confidential-containers) (Apache-2.0);
  [Gramine](https://github.com/gramineproject/gramine) (LGPL-3.0, SGX library OS);
  [Open Enclave](https://github.com/openenclave/openenclave) (MIT);
  [Edgeless Contrast](https://github.com/edgelesssys/contrast) (active),
  [Constellation](https://github.com/edgelesssys/constellation) (archived);
  [Continuum docs](https://continuum-docs.edgeless.systems);
  [Apple Private Cloud Compute](https://security.apple.com/blog/private-cloud-compute/),
  [apple/security-pcc](https://github.com/apple/security-pcc);
  [Rekor](https://github.com/sigstore/rekor) (Apache-2.0);
  [go-tpm-tools](https://github.com/google/go-tpm-tools) (Apache-2.0).
- Ours: [private inference](../../security/private-inference.md),
  [sensitive data vault](../../security/sensitive-data-vault.md),
  [NIP-ATT](../../../nips/openagents/NIP-ATT.md),
  [computers](../../cloud/computers.md),
  [computers-for-chats audit](../../audits/2026-10-14-computers-for-chats-audit.md),
  [BYOK](../../byok/README.md), [Claude plan](../../byok/claude-plan.md),
  [chat worker](../../deployment/chat-worker.md),
  [REAX](../decisionmodels/reax.md), [calibration](../decisionmodels/calibration.md),
  `crates/receipts/src/lib.rs`, `crates/oa-connections/src/lib.rs`,
  `crates/oa-boat/src/sizes.rs`.
