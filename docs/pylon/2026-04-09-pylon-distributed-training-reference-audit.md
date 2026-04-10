# 2026-04-09 Pylon Distributed Training Reference Audit

Status: openagents cross-repo analysis  
Scope date: 2026-04-09

## Scope

There is no standalone `pylon` repo.

Current active `Pylon` code lives in:

- `apps/pylon`
- `crates/openagents-provider-substrate`
- `crates/openagents-kernel-core`

This note compares those surfaces against the distributed training references in:

- `../competition/repos/prime-diloco`
- `../competition/repos/templar`

`Pylon` already has a training-shaped product label. The open question is
whether it already has the runtime, coordination, checkpointing, validation,
and artifact exchange machinery needed to run something comparable to those
systems.

## Short Answer

Not yet.

Current `Pylon` already has useful substrate for a training contributor:

- product identity for `adapter_training.contributor`
- capability matching over backend, adapter family, validator policy,
  checkpoint family, environment, and memory
- settlement hooks for accepted contributions and accepted sealed windows
- a kernel launch shape that already expects elastic, cluster-attached training
  with validator requirements

What it does not yet have is the actual operator plane required to run either
reference system.

To support **Prime DiLoCo-like runs**, `Pylon` needs a real distributed
training control plane around `psionic-train`: cluster admission, role
assignment, membership and heartbeats, launch supervision, checkpoint and live
recovery plumbing, object-store integration, and sealed run receipts.

To support **Templar-like runs**, `Pylon` needs all of the above plus a second
plane for validator work: deterministic windows, task assignment, shared
artifact exchange, gradient replay and scoring, contributor reputation, and
reward publication.

The practical conclusion is:

- `Psionic` should own the actual elastic training runtime
- `Pylon` should grow into the node operator, coordinator client, artifact
  courier, receipt publisher, and settlement shell
- `Nexus` or another authority service should likely own early admission and
  validation policy before trying to copy Bittensor's full chain design
- `TRN` should be the public Nostr coordination and reputation layer for the
  parts of Templar that currently live in chain state, validator publication,
  and public penalty or reward trails

For the MVP, that is enough. We do not need hostile-network or permissionless
training verification before running serious admitted-node training through our
own `Pylon` plus `Nexus` deployment.

## What Pylon Already Has

### 1. Product and capability vocabulary

`openagents-provider-substrate` already models an
`adapter_training_contributor` product with:

- `contributor_supported`
- `coordinator_match_supported`
- `authority_receipt_supported`
- execution backend labels
- adapter family and format lists
- validator policy refs
- checkpoint families
- environment refs
- memory requirements
- settlement trigger selection

That is materially better than having no training model at all. It means the
inventory language is already pointed in the right direction.

### 2. Match and settlement primitives

`openagents-provider-substrate` already has:

- `ProviderAdapterTrainingMatchRequest`
- `ProviderAdapterTrainingMatchVerdict`
- `ProviderAdapterTrainingSettlementHook`
- settlement triggers for `accepted_contribution` and
  `accepted_sealed_window`

This is important because both Prime-like and Templar-like systems need
authority decisions that can be turned into provider-visible receipts and later
settlement.

### 3. Kernel launch expectations already point at cluster training

`openagents-kernel-core` already validates the training contributor product as:

- backend family `PsionicTrain`
- execution kind `TrainingJob`
- topology kind `TrainingElastic`
- provisioning kind `ClusterAttached`
- proof posture `ChallengeEligible`
- validator requirements present

So the higher-level product model already expects something closer to a
clustered training lane than a local one-shot adapter job.

### 4. The current runtime path is still inert

The missing part is that current `Pylon` runtime detection still reports:

- `adapter_training_contributor: ProviderAdapterTrainingContributorAvailability::default()`

and inventory controls still default the training contributor lane off.

So today the training contributor product is mostly a declared capability
surface, not a live distributed training operator.

## What TRN Already Gives Us

`crates/nostr/nips/TRN.md` already defines most of the Nostr-native
coordination layer needed to replace Templar's chain-era public state.

As of the 2026-04-09 TRN draft update in `openagents` commit `cd9ccc613`,
the protocol text now also includes:

- explicit actor attribution through `p` tags on receipts, verdicts, closeouts,
  and related labels
- recommended TRN-specific NIP-32 namespaces and label values
- a concrete score-snapshot publication profile through `kind:39520`
  `class=score` artifact locators
- a tighter assignment-receipt profile for replayable validation work

TRN does not try to move heavy bytes or runtime traffic. That is correct. It
standardizes the signed metadata above the runtime:

- network records
- node records
- windows
- receipts
- verdicts
- artifact pointers
- closeouts
- optional challenge and reputation profiles

### 1. Nostr replacement for the public network record

TRN `kind:39500` Training Network Contract is the right replacement for the
public network or subnet record.

It already covers:

- network id
- governance revision
- model or workload family
- window cadence
- allowed roles
- bootstrap artifacts
- resume and fork lineage
- supported optional profiles

This is the right place to publish the public network contract that Templar
currently leans on chain state for.

### 2. Nostr replacement for miner and validator registration

TRN `kind:39501` Training Node Record is the right replacement for public
registration and capability advertisement.

It already covers:

- node identity
- network membership
- status
- roles
- execution class
- build digest
- capabilities
- relay or coordination hints

This maps cleanly to the facts `Pylon` should publish from its live provider
inventory.

### 3. Nostr replacement for block-driven window state

TRN `kind:39510` Training Window is the right replacement for the public window
state Templar derives from chain progression.

It already covers:

- network id
- window id
- policy revision
- assignment seed
- workload family
- resume and bootstrap links
- state transitions such as `planned`, `active`, `sealed`, `scored`,
  `reconciled`, and `canceled`

This is enough to publish deterministic training and validation windows without
depending on a chain-specific metagraph.

### 4. Nostr replacement for public receipts, validator results, and closeouts

TRN already splits the public trail into the right record types:

- `kind:39511` Training Receipt for assignment published or accepted, artifact
  uploaded, window sealed, window reconciled, or replay requested
- `kind:39512` Validator Verdict for `accepted`, `quarantined`, `rejected`, or
  `replay_required`
- `kind:39520` Training Artifact Locator for checkpoints, weights, deltas,
  proofs, eval bundles, and score files
- `kind:39530` Training Contribution Closeout for `rewarded`, `no_reward`,
  `held`, `quarantined`, `refused`, or `slashed`

That covers most of the public lifecycle Templar currently spreads across chain
state, object storage, and validator logic.

### 5. Nostr replacement for challenge, discovery, and reputation

TRN already defines the right optional profiles:

- TRN-Discovery via NIP-89 and NIP-66
- TRN-Private via NIP-44 and NIP-59
- TRN-Challenge via NIP-90
- TRN-Reputation via NIP-32

This matters because the Templar-style public plane does not need to become a
blockchain clone.

OpenAgents can use:

- NIP-89 to advertise coordinators, validators, artifact indexers, and
  training-capable providers
- NIP-66 to publish preferred coordination relays and relay health
- NIP-90 for narrow replay or proof jobs
- NIP-32 for fraud, trust, quality, and reputation labels

### 6. Existing OpenAgents Nostr code already points the right way

`crates/nostr/core/src/nip32.rs` already supports NIP-32 labels over
pubkeys, events, relays, topics, and addressable events.

`crates/nostr/core/src/nip_ac/reputation.rs` already maps
settlement and default events into NIP-32 reputation labels for NIP-AC.

That pattern should now be extended with TRN-specific helpers that match the
updated draft:

- verdict-to-label helpers
- closeout-to-label helpers
- build-revoked and checkpoint-warning label helpers

This is the Nostr-native replacement for Templar's ongoing public penalty and
reward annotations.

### 7. Direct mapping from Templar's public plane to TRN

For the public chain and off-chain coordination plane, the mapping is:

- Templar subnet or network contract -> TRN `kind:39500` Training Network
  Contract
- miner and validator registration -> TRN `kind:39501` Training Node Record
- block-driven active window -> TRN `kind:39510` Training Window
- assignment or upload acknowledgement -> TRN `kind:39511` Training Receipt
- validator judgment -> TRN `kind:39512` Validator Verdict
- checkpoint, proof, eval, or score pointer -> TRN `kind:39520` Training
  Artifact Locator
- rewarded, held, quarantined, refused, or slashed outcome -> TRN
  `kind:39530` Training Contribution Closeout
- public fraud or trust signal -> NIP-32 label
- rerun or challenge request -> NIP-90 job

That is the Nostr version of the public control and reputation plane. The
runtime, collectives, checkpoint bytes, and gradient bytes still belong outside
Nostr.

## What Prime DiLoCo Actually Has

From `../competition/repos/prime-diloco`, the important pieces are not just
"training" in the abstract. They are concrete runtime systems:

### 1. Elastic cluster membership

`zeroband/comms.py` provides an `ElasticDeviceMesh` with:

- leader-coordinated membership
- global TCP stores
- join queues
- world-size changes
- mesh versioning
- rank reassignment
- process-group recreation
- heartbeat intervals and timeouts
- monitored barriers

This is the core that lets nodes join or die without restarting the whole run.

### 2. Live checkpoint recovery

`zeroband/checkpoint.py` and `train.py` provide:

- structured checkpoint state
- local checkpoint saving
- remote checkpoint sync
- non-blocking checkpoint workers
- live recovery threads
- peer-to-peer checkpoint send/receive for joiners

This is what turns an unstable internet cluster into something that can
actually keep training.

### 3. Typed training-run configuration

`zeroband/config.py` gives a typed config for:

- model and optimizer
- DiLoCo outer loop
- data config
- checkpoint policy
- remote storage
- logging and metrics
- resume and recovery settings

That matters because distributed training is mostly operational state. If
`Pylon` cannot represent that state, it cannot supervise the run.

### 4. Runtime supervision assumptions

`train.py` assumes somebody can provide:

- environment variables for ranks and stores
- launch-time role identity
- checkpoint locations
- recovery source selection
- distributed startup and restart policy

Prime DiLoCo is therefore not just a model loop. It is a training runtime plus
an operator plane.

## What Pylon Must Add For Prime DiLoCo-Like Runs

### 1. A real training run manifest

`Pylon` needs a first-class run object, not just a generic contributor flag.

At minimum it should include:

- training run id
- role kind: coordinator, worker, validator, recovery source
- model and checkpoint refs
- dataset ref
- environment ref and environment digest
- cluster topology target
- world-size and per-node role counts
- admission policy and join policy
- checkpoint policy
- remote artifact and checkpoint storage refs
- authority and settlement policy refs

Today the substrate can say "I can contribute to adapter training." It cannot
yet describe the full run contract that Prime-like training needs.

### 2. Launch supervision for `psionic-train`

`Pylon` currently acts more like a provider shell than a distributed training
supervisor.

For Prime-like runs it needs to:

- launch `psionic-train` with run-specific env
- supervise lifecycle transitions
- restart or quarantine failed workers
- expose logs and health
- publish rank and membership state
- mark runs as joining, active, degraded, recovering, or sealed

This should stay a thin operator layer. The training math should still live in
`Psionic`.

### 3. Membership and liveness control

Prime's biggest missing piece in `Pylon` is elastic membership.

Something in the OpenAgents stack must provide:

- cluster admission
- rank assignment
- heartbeats
- dead-node detection
- rejoin handling
- process-group epoch tracking
- barrier and reinit signaling
- topology digests

This probably belongs mostly in `Psionic` runtime code, but `Pylon` needs to
configure it, surface it, and turn it into provider-visible status and
receipts.

### 4. Checkpoint and live-recovery plumbing

`Pylon` needs real checkpoint infrastructure, including:

- local checkpoint roots
- remote object-store targets
- content digests for checkpoint manifests
- checkpoint retention policy
- resume policy
- live recovery source and destination coordination
- peer checkpoint serving or brokered transfer

Without this, a long-running internet training job will be operationally
fragile even if the inner training code works.

### 5. Network preflight and capability checks

Prime optimizes around real cluster constraints. `Pylon` does not yet expose
enough for that.

Needed checks include:

- available accelerator memory
- storage capacity and fast temporary storage
- bandwidth and connectivity probes
- interface selection
- latency and failure posture
- whether the node can act as recovery source

The current training availability model is too small for this. It should be
expanded rather than replaced.

### 6. Run-level receipts

Prime-like distributed training needs more than a final completion receipt.

`Pylon` should be able to publish signed receipts for:

- run admission
- cluster join
- cluster reinit
- checkpoint seal
- recovery event
- sealed outer step
- accepted contribution or accepted sealed window
- run completion or abort

This fits naturally with the existing provider and settlement posture.

## What Templar Adds Beyond Prime

`templar` is not just elastic distributed training. It adds a validator and
incentive system.

The reference system includes:

- miners and validators as different roles
- synchronized windows tied to chain progression
- deterministic data assignment by UID and window
- gradient compression and upload to shared storage
- peer gather and aggregation
- validator replay and loss-based evaluation
- score updates, slashing, and weight setting
- bucket commitments and network-visible identity

That means Templar is not just "Prime plus object storage." It is a contribution
market with explicit verification and reward logic.

TRN is the right Nostr-native home for that public control plane. It already
standardizes the records Templar needs at the coordination layer without trying
to force runtime collectives or heavy artifacts onto relays.

## What Pylon Must Add For Templar-Like Runs

### 1. Separate validator role support

`adapter_training_contributor` is not enough if the goal is Templar-like
behavior.

`Pylon` needs explicit support for validator-class work:

- validator admission
- validator runtime launch
- validator capability inventory
- validator policy refs
- validator receipt formats
- validator payout or reward hooks

If this stays implicit, the system will never honestly match the Templar shape.

### 2. Deterministic window scheduling

Templar runs in windows. `Pylon` needs an authority-visible window scheduler
that can publish:

- window id
- run id
- start and end conditions
- contributor set revision
- deterministic task seed
- dataset or shard manifest
- checkpoint pointer for that window
- sealing decision

The good news is that existing OpenAgents compute types already talk in terms of
training windows and accepted outcomes. The missing work is turning that into a
live runtime flow.

### 3. Shared artifact exchange

Templar depends heavily on shared storage for gradients and aggregated results.

`Pylon` needs an artifact plane with:

- content-addressed uploads
- signed metadata
- per-window manifests
- compressed gradient payload descriptors
- presigned URL or delegated storage credentials
- retention and garbage collection
- replay-friendly artifact references

This can be object-store-backed without copying Templar's exact R2 commitment
scheme.

### 4. Validator replay and scoring lane

This is the most important Templar-specific addition.

Some authority or validator service needs to:

- download submitted artifacts
- reconstruct or replay the claimed update
- apply it against the correct checkpoint and environment
- measure loss delta or acceptance criteria
- record accepted, rejected, quarantined, or replay-required outcomes
- emit signed validator receipts

OpenAgents already has useful vocabulary here in the training window and
contribution outcome types. What is missing is the live executor path.

### 5. Reputation and assignment feedback

Templar uses weights and ongoing score updates. OpenAgents should make that
Nostr-native through TRN instead of cloning Bittensor's metagraph and weight
publication.

A practical version is:

- the coordinator publishes `kind:39500` network contracts and `kind:39510`
  windows
- `Pylon` nodes publish `kind:39501` node records from live capability state
- coordinators and aggregators publish `kind:39511` receipts for assignment,
  upload, seal, reconcile, and replay events
- validators publish `kind:39512` verdicts
- final accepted, rewarded, held, quarantined, refused, or slashed outcomes
  publish as `kind:39530` closeouts
- closeouts link to NIP-AC settlement receipts, Lightning receipts, or internal
  accounting receipts
- NIP-32 labels attach to contributor pubkeys, validator pubkeys, suspect
  builds, suspect artifacts, and repeated failure patterns
- assignment prefers nodes with accepted closeout history and clean label
  history

This gives OpenAgents a public reputation trail without pretending relays are
the training runtime.

### 6. Anti-cheat and duplicate-detection checks

Templar spends significant effort on overlap, missing gradients, and similarity
checks.

If OpenAgents wants similar public claims, `Pylon` needs support for:

- sample or shard digests
- duplicate artifact detection
- timing checks
- missing submission tracking
- repeated-failure penalties
- suspicious-similarity review flags

This is where the verification story becomes materially stronger than simple
"the node says it trained."

## What Can Be Added With Small Or Moderate Changes

These are realistic near-term extensions to the current code shape:

### 1. Turn training contributor availability into a live probe

Instead of defaulting it inert, `Pylon` should detect and report:

- `psionic-train` availability
- supported training backend families
- memory and accelerator facts
- local checkpoint capability
- object-store capability
- authority receipt capability
- whether the node can join elastic runs

### 2. Expand the match request rather than replacing it

The current training match request can be extended with fields such as:

- topology kind
- desired role
- minimum bandwidth
- storage class
- checkpoint recovery posture
- cluster size constraints
- validator-count or validator-pool requirements

That preserves the current substrate direction.

### 3. Reuse existing training window and settlement concepts

The existing accepted-contribution and accepted-sealed-window hooks already map
well onto:

- Prime-like sealed outer steps
- Templar-like validated windows

That means the settlement vocabulary does not need to be reinvented.

### 4. Keep early coordination off-chain

For the first serious version, use `Nexus` or another authority service for:

- admission
- scheduling
- window sealing
- validator policy
- reputation

Do not block on reproducing Bittensor's chain model before the training runtime
works.

## How OpenAgents Should Use TRN

TRN should be the public coordination layer for the Nostr version of the
Templar control plane.

The 2026-04-09 TRN draft update now covers the main protocol conventions this
audit previously called out as missing. What remains here is the implementation
plan for `Pylon`, `Nexus`, validators, and settlement code.

For the MVP, TRN should be implemented by our own `Pylon` and `Nexus`
deployment as the signed coordination and publication layer for the run. It
does not need to solve hostile-network consensus before it is useful.

### 1. Publish one network contract per trainnet

The coordinator or authority service should publish one `kind:39500` Training
Network Contract per training network.

That contract should carry:

- network id
- governance revision
- role set
- window cadence
- model family
- optional profiles in use
- bootstrap checkpoint or weight locators
- fork or resume lineage

This replaces the public network contract role that Templar currently gets from
chain state.

### 2. Have every Pylon node publish a live node record

Each `Pylon` node should publish `kind:39501` Training Node Records derived
from live runtime detection, not static config alone.

Those records should include:

- contributor or validator role claims
- execution classes
- build digest
- accelerator and memory capability
- checkpoint and storage posture
- relay hints
- current node status

This becomes the public registry for schedulers and operators.

### 3. Use TRN windows as the canonical assignment epoch

Use `kind:39510` windows to publish:

- active policy revision
- assignment seed
- source checkpoint pointer
- source or resume window
- current state

That gives validators and contributors one shared window identity without
depending on chain blocks.

### 4. Use receipts for the append-only operational trail

Use `kind:39511` receipts for:

- assignment published
- assignment accepted
- artifact uploaded
- replay requested
- window sealed
- window reconciled

This is the right place for the public lightweight status trail around actual
training work.

### 5. Keep heavy files off Nostr and publish signed locators

Use `kind:39520` artifact locators for:

- checkpoints
- deltas
- eval bundles
- proof bundles
- score snapshots

The actual bytes should stay in object storage or another artifact system.

### 6. Use NIP-90 for narrow challenge and replay jobs

When validators need a rerun, small proof, benchmark, or replay check:

- publish a NIP-90 challenge job
- run the check off-relay
- link the result from the TRN verdict or artifact locator

This is the right Nostr equivalent of Templar's spot-check and evaluation lane.

### 7. Use NIP-32 for public reputation and fraud signals

NIP-32 should carry the public reputation layer over TRN events.

OpenAgents should label:

- contributor pubkeys
- validator pubkeys
- verdict events
- closeout events
- node records carrying build digests
- artifact locators

The updated TRN draft now recommends these namespaces and labels:

- `trn/reputation`: `accepted_contribution`, `rewarded`, `held`, `slashed`
- `trn/fraud`: `replay_required`, `repeated_missing_submission`,
  `suspicious_similarity`, `duplicate_submission`
- `trn/validator`: `good`, `poor`, `inconsistent`
- `trn/build`: `revoked`, `mismatch`
- `trn/artifact`: `checkpoint_warning`, `digest_mismatch`, `proof_missing`

Those labels should target both the affected actor pubkey and the source TRN
event when possible.

### 8. Link economic outcomes through NIP-AC or Lightning

TRN `kind:39530` closeouts should link to:

- NIP-AC settlement receipts when using OpenAgents credit flows
- Lightning receipts when paying directly
- internal accounting receipts when needed

This keeps economics composable without putting settlement math inside TRN
itself.

### 9. Use NIP-89 and NIP-66 for discovery

Use:

- NIP-89 to advertise coordinators, validators, artifact indexers, and
  training-capable nodes
- NIP-66 to publish preferred coordination relays and relay health

This replaces the "find the active network participants" job that Templar gets
from its chain and bucket conventions.

## TRN Changes Now Applied

The TRN draft has now been updated to cover the protocol-level recommendations
this audit previously called out.

### 1. Reputation namespaces and labels are now standardized

The updated TRN draft now defines recommended NIP-32 namespaces and label
values for:

- contribution reputation
- fraud and replay problems
- validator quality
- build problems
- artifact warnings

That gives `Pylon`, validators, and schedulers one shared vocabulary for
public reputation and fraud signaling.

### 2. Actor attribution is now explicit on the key public events

The updated TRN draft now recommends `p` tags with markers such as:

- `subject`
- `validator`
- `coordinator`

for receipts, verdicts, closeouts, and related labels.

That makes relay-native attribution and reputation indexing materially easier.

### 3. Score snapshot publication is now described directly in TRN

The updated TRN draft now says `kind:39520` artifact locators with
`class=score` should reference signed score snapshot files and describes the
minimum row facts those files should carry:

- subject pubkey
- validator pubkey when relevant
- accepted facts
- replay-required count
- slashed count
- current scheduling weight

That is enough for `Pylon` to publish off-Nostr score files with on-Nostr
discoverability and digest integrity.

### 4. Assignment receipts now have a clearer replay profile

The updated TRN draft now says assignment-related `kind:39511` receipts should
carry a compact JSON profile with:

- subject pubkey
- assignment deadline
- expected artifact class
- shard or sample-pool digest
- source checkpoint id or coordinate

That gives validators and challenge jobs enough context to reconstruct what
work was actually assigned.

### 5. Remaining TRN work is now implementation, not protocol shape

After this update, the main remaining gaps are not "invent a better public
coordination NIP." The main remaining gaps are:

- helper code to emit and consume the new TRN conventions
- `Pylon` inventory publication
- `Nexus` scheduling and reconciliation logic
- validator replay and scoring executors
- settlement and reputation integration

## MVP Readiness For Pylon And Psionic-Only Contribution

If the MVP scope is:

- contributors only run work through our `Pylon` and `Psionic` binaries
- node admission is controlled by us or by a bounded authority service
- accepted work is limited to bounded training lanes we explicitly support

then the problem is materially easier than the open Templar-style public market.

This is closer to Prime DiLoCo's actual operating model than to Bittensor's
open validator-economy model.

### Readiness by layer

For that narrower MVP, current readiness is:

- protocol and evidence layer: strong
- runtime and operator layer: partial
- open-market adversarial trustlessness: weak

That is enough to justify a bounded MVP running through our own `Pylon` plus
`Nexus` setup. Hostile-network and permissionless training claims can be
postponed.

### Why we are more prepared in that narrower model

`Psionic` already has more training-governance and receipt vocabulary than a
generic PyTorch training script.

Relevant current evidence from the repo:

- `psionic/docs/PSION_TRUSTED_CLUSTER_RUN.md` already freezes one bounded
  trusted-cluster path with topology, replay, and checkpoint-recovery receipts
- `psionic/docs/PSION_DECENTRALIZED_CONTRIBUTION.md` already freezes one
  bounded `adapter_delta_window` contribution lane with contributor membership
  receipts, window plans, artifact receipts, provenance-security receipts,
  replay-aware window summaries, and sealed-window aggregation receipts
- `psionic/docs/TRAIN_SYSTEM.md` already documents validator-promotion
  contracts, validator challenge and scoring contracts, multi-validator
  consensus contracts, deterministic replay truth, and explicit security work
  for untrusted worker admission
- the updated TRN draft now gives us a public Nostr-native coordination and
  reputation layer above those receipts

That means the repo is already shaped for:

- admitted nodes
- bounded training windows
- explicit contributor receipts
- replay-aware validation
- accepted, held, quarantined, replay-required, and slashed outcomes
- promotion and rollback discipline above local contribution windows

That is a good fit for an MVP where we only trust work that flowed through our
own execution path.

### What is still not ready enough to overclaim

The biggest remaining weakness is not protocol vocabulary. It is live operator
closure.

Current weaknesses for the MVP are:

- `Pylon` still does not expose a live training contributor runtime path; the
  availability branch is still inert
- `Pylon` does not yet supervise `psionic-train` as a real long-running
  distributed job
- elastic membership, checkpoint transport, recovery, and run-state projection
  are still better represented in reference analysis and `Psionic` contracts
  than in a shipped `Pylon` operator flow
- some `Psionic` training surfaces are currently stronger as typed contracts,
  fixtures, and proof bundles than as one fully shipped end-to-end service

So the honest statement is:

- we are prepared to support a bounded admitted-node training MVP
- we do not need broad hostile-network training verification for that MVP
- hostile-network and permissionless trust assumptions can be deferred until
  after the operator plane is real

## Compare To Prime DiLoCo

Prime DiLoCo managed a real run without a blockchain-style public reputation
layer because it was not trying to solve the same public-market problem.

Its effective trust model was:

- admitted workers
- shared software stack
- operator-supplied environment variables and launch commands
- one coordinated global store
- heartbeat-based liveness
- asynchronous checkpoints
- peer-served live checkpoint recovery
- operator-visible metrics and logs

That is a strong runtime and operator model. It is not a public
cryptoeconomic-verification model.

Prime's version of "how did they do it without all this" was:

- keep the run permissioned enough that operator control and runtime discipline
  are sufficient
- solve elastic membership, checkpointing, recovery, and bandwidth first
- avoid pretending the run is trustless

That is why they could run a serious distributed training job without first
inventing a public reputation or settlement protocol.

That is also the right frame for our MVP. We can run through our own admitted
`Pylon` plus `Nexus` setup, use TRN for signed coordination records, and defer
hostile-network verification until later.

### What we should learn from Prime

The most important lessons are:

- do not block the first serious run on permissionless economics
- pin the node set, build, runtime env, and checkpoint roots
- make the cluster operator plane real before expanding the public story
- treat heartbeats, checkpoint freshness, and recovery as first-class
  correctness concerns
- publish honest receipts, but do not confuse receipts with trustlessness

Prime is still ahead of us on the runtime side:

- actual elastic process-group management
- actual live recovery path
- actual asynchronous checkpoint pipeline
- actual operator-ready training loop

We are ahead of Prime on the explicit receipt and governance shape:

- stronger typed vocabulary for accepted, held, quarantined, replay-required,
  and promotion outcomes
- a clearer path to public coordination and reputation publication through TRN
- stronger insistence on replay-safe, machine-legible authority and artifact
  lineage

The right takeaway is:

- copy Prime's bias toward getting the operator plane working
- keep our stronger receipt and authority discipline on top of that

## Extra Defensibility Additions Worth Making

For the bounded admitted-node MVP, the highest-value additions are in our code,
not in TRN.

### Code additions worth making before strong public claims

`Pylon` and `Psionic` should add:

- signed launch-admission receipts carrying run id, node pubkey, build digest,
  environment digest, dataset digest, role, and topology slot
- enforced per-run allowlists for build digests, environment manifests, and
  checkpoint families
- cluster join, heartbeat, reinit, and recovery receipts emitted as durable
  machine-readable records
- checkpoint seal and freshness receipts bound to checkpoint manifest digests
- sampled validator reruns as a required closeout gate, not just an optional
  operator action
- deterministic shard or sample-pool digests in every assignment receipt
- duplicate, stale, late, and oversized submission quarantine with typed reason
  codes
- explicit node replacement and revocation flows when a node changes build,
  machine, or operator posture mid-run

Those additions directly improve defensibility for "work only through our
software" claims.

### Optional stronger additions for later

Later, if we want harder assurances without jumping straight to a blockchain,
we should consider:

- threshold-signed window seals and accepted closeouts
- stronger machine attestation where the hardware path makes sense
- external anchoring for window seals or randomness
- multiple authority signatures over accepted score snapshots

Those are not required for the first honest MVP.

### Extra TRN changes

After the 2026-04-09 draft update, TRN is in decent shape for the MVP.

I do not think more core TRN changes are required before the first admitted
run. If we later want stronger public verifiability, the next TRN-level
improvements worth considering are:

- a standard way to express quorum or threshold-signature metadata on sealed
  windows and closeouts
- a standard admission or replacement receipt profile for node lifecycle
- a standard score-snapshot manifest profile if multiple implementations begin
  exchanging those files

For the MVP, code and operator work matters more than more NIP surface area.

## What Is Genuine New Work

These are not small surface changes. They are missing systems:

- elastic training membership control
- checkpoint and live recovery transport
- distributed training process supervision
- validator execution and scoring
- shared artifact exchange and retention
- contributor reputation and assignment feedback
- public receipts for cluster lifecycle events

Those systems are the real gap between current `Pylon` and the competitor
references.

## Recommended Build Order

### Phase 1: Prime-like private cluster runs

Ship a permissioned distributed training lane first:

- admitted nodes only
- `Pylon` launches and supervises `psionic-train`
- elastic membership and checkpoint recovery work
- authority seals cluster events and outer steps
- settlement is based on admitted participation and sealed outputs

This is the smallest honest path to "similar distributed training runs."

### Phase 2: Off-chain validated contribution windows

Then add a Templar-like contribution market without chain dependence:

- contributor windows
- deterministic assignments
- artifact uploads
- validator replay and scoring
- accepted contribution receipts
- reputation-informed future assignment

This is where verification becomes a first-class product primitive.

### Phase 3: Public economic publication

Only after the validator lane works cleanly should OpenAgents consider:

- chain publication
- stake-weighted validator sets
- public weight updates
- more automated slashing and economic policy

That ordering keeps the hard part in focus: runtime truth and validation.

## Bottom Line

Current `Pylon` already has the beginnings of the right product language for
distributed training, but not the operator plane needed to run Prime
DiLoCo-like or Templar-like systems.

The public Nostr coordination layer is now in better shape than before. The
updated TRN draft now covers the main protocol conventions needed for the
Templar-style public plane: actor attribution, reputation namespaces, score
snapshot publication, and replay-friendly assignment receipts.

The most important additions are:

- a real training run manifest
- `psionic-train` launch supervision
- elastic membership and recovery support
- checkpoint and artifact transport
- validator execution and scoring
- run and window receipts tied to settlement
- live TRN publication from `Pylon`, validators, and authority services

If those are added, `Pylon` can honestly become the node-facing shell for
distributed training runs. If they are not, then the current training
contributor surface remains mostly declarative.
