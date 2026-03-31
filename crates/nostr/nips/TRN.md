NIP-TRN
=======

Model Training Coordination
---------------------------

`draft` `optional`

This NIP defines the coordination and publication substrate for model training
on Nostr.

Nostr already has several pieces that can participate in training
coordination:

- NIP-32 for labels, fraud markers, and reputation signals
- NIP-44 and NIP-59 for private coordination envelopes
- NIP-66 for relay discovery and liveness posture
- NIP-89 for handler and service announcements
- NIP-90 for bounded challenge, replay, proof, or eval jobs
- NIP-94 for public file or bundle metadata when artifact references need to be
  published

What it does not yet have is a canonical training-coordination primitive.

This NIP introduces that primitive.

The goal is to make model-training coordination legible across nodes,
validators, operators, and supporting services so they can refer to the same
network, the same window, the same assignments, the same verdicts, and the
same promoted artifact state without relying on:

- private spreadsheets
- operator-only host lists
- lane-specific dashboards
- ad hoc JSON receipts
- unverifiable reward or verdict claims

In practical terms, this NIP is not about putting training itself on Nostr.

It is about making the coordination layer around training portable and
auditable. That includes:

- public or semi-public train networks
- permissioned registries of training nodes
- validator-owned challenge and verdict flows
- checkpoint and proof-bundle locator publication
- contribution closeout and settlement linkage

The point is to establish best practices for how training coordination should
move on Nostr:

- how a training network is identified
- how nodes declare capability and role posture
- how windows are identified and published
- how assignments and state transitions are receipted
- how validator outcomes are published
- how heavy training artifacts are referenced without embedding them
- how contribution outcomes become durable public or private closeouts

One protocol should support more than one training shape:

- sometimes a network is public and wants broad discovery
- sometimes it is permissioned and only wants signed registry records
- sometimes a validator needs a bounded challenge/replay job
- sometimes an operator needs private assignment envelopes
- sometimes a network only wants public proof and settlement publication

This NIP separates those concerns into a small core plus optional profiles.

It covers:

- training network identity
- node registry and capability publication
- training-window and assignment receipts
- validator verdict publication
- checkpoint and artifact locator metadata
- contribution closeout and settlement linkage

It does **not** define:

- trainer execution
- collective synchronization
- checkpoint or weight transfer
- rollout payload transport
- gradient or delta exchange
- runtime mesh membership or hot-path heartbeats

TRN exists to make model-training coordination legible across independently
operated nodes, operators, validators, and services without pretending Nostr is
the training runtime.

This NIP is designed to fit alongside:

- NIP-01: events, tags, subscriptions
- NIP-32: labels and reputation signals
- NIP-44 / NIP-59: private coordination envelopes
- NIP-66: relay discovery and liveness posture
- NIP-89: handler and service announcements
- NIP-90: bounded challenge, replay, and proof-request jobs
- NIP-94: file and bundle metadata when public artifact references are needed

It also fits alongside our in-repo drafts:

- NIP-DS for dataset identity and delivery
- NIP-SKL for reusable skill identity and trust
- NIP-AC for credit, settlement, and bounded spend flows where training
  incentives or challenge payments later need an interoperable payment layer

This NIP separates those concerns into:

- TRN core for network, node, window, receipt, verdict, locator, and closeout
  objects
- TRN-Discovery via NIP-89 and NIP-66
- TRN-Private via NIP-44 and NIP-59
- TRN-Challenge via NIP-90
- TRN-Reputation via NIP-32

## Abstract

The training system needs one public answer to these questions:

- what network is this node participating in?
- what role is this node claiming?
- what training window is currently active?
- what work was this node assigned?
- what verdict did validators produce?
- what checkpoint or artifact state was promoted?
- what reward, hold, quarantine, or refusal followed?

TRN defines lightweight, signed, replayable control objects for those answers.

The design rule is simple:

> TRN carries ids, refs, digests, policy revisions, locators, and receipts.
> Heavy training bytes stay off-Nostr.

## Rationale

NIP-90 is a good fit for bounded challenge, replay, proof, or evaluation jobs.

It is not, by itself, a good fit for "what training network is this node in?",
"what role is this node claiming?", or "what checkpoint state was actually
promoted?"

Likewise, NIP-89 is a good fit for service and handler discoverability, but it
does not define canonical network contracts, window identity, validator
verdicts, or artifact-locator semantics for training.

Model training is also broader than one service request. A training cycle may
span:

- a network contract
- multiple node records
- one or more windows
- many assignments
- validator-owned verdicts
- promoted or held artifact state
- reward or no-reward closeout

These objects need to stay linked even when different parties publish them.

TRN introduces a training-native coordination layer that can be used with
either model:

- public discovery through node records and handler announcements
- private or permissioned coordination through wrapped receipts and locators
- bounded challenge or replay through NIP-90
- public or semi-public reputation and fraud signaling through NIP-32

TRN is therefore not trying to make Nostr do everything.

It is trying to standardize the narrow layer that benefits from:

- signatures
- relay replication
- public or semi-public auditability
- interoperable discovery
- append-only receipt publication

## Terms

- `network`: one named model-training coordination domain
- `node`: one published participant identity and capability record
- `window`: one bounded training interval, assignment interval, or validation
  interval
- `assignment`: one declared work unit for a node in a specific window
- `verdict`: one validator-owned outcome over a submission, assignment, or
  claimed result
- `artifact locator`: metadata that points to a checkpoint, delta bundle, eval
  bundle, or other heavy object without embedding the heavy object itself
- `closeout`: one durable publication of reward, no-reward, hold, quarantine,
  or refusal outcome for a contribution or window

## Scope And Layering

### TRN Core

TRN core is normative and defines:

- canonical network identity
- canonical node registry records
- canonical training-window identity
- lightweight assignment and transition receipts
- validator verdict publication
- artifact locator metadata
- contribution closeout linkage

### Optional Profiles

TRN core intentionally does not force one privacy rail, one payment rail, or
one challenge mechanism.

Optional profiles are:

- TRN-Discovery via NIP-89 and NIP-66
- TRN-Private via NIP-44 and NIP-59
- TRN-Challenge via NIP-90
- TRN-Reputation via NIP-32

### Out Of Scope

This NIP does not standardize:

- live trainer internals
- distributed optimizer state
- dense-rank mesh behavior
- collective transport cadence
- checkpoint shard movement
- policy-weight broadcast bytes
- dataset page transport
- rollout payload delivery
- sandbox runtime behavior
- exact incentive math

Those belong in runtime and artifact systems outside Nostr.

## Kinds

This NIP introduces:

| Kind | Type | Description |
|------|------|-------------|
| 39500 | Addressable | Training Network Contract |
| 39501 | Addressable | Training Node Record |
| 39510 | Addressable | Training Window |
| 39511 | Regular | Training Receipt |
| 39512 | Regular | Validator Verdict |
| 39520 | Addressable | Training Artifact Locator |
| 39530 | Regular | Training Contribution Closeout |

This NIP reuses:

| Kind | NIP | Role |
|------|-----|------|
| 1985 | NIP-32 | fraud labels, trust labels, quality labels |
| 5 | NIP-09 | publisher-origin revocation |
| 30166 / 10166 | NIP-66 | coordination relay discovery and monitoring |
| 31989 / 31990 | NIP-89 | handler and service discovery |
| 5000-5999 / 6000-6999 / 7000 | NIP-90 | optional challenge, replay, proof, or eval jobs |

## Common Tags

TRN uses these common tags:

- `["d", "<identifier>"]` — addressable-event identifier
- `["network", "<network_id>"]` — canonical training network id
- `["window", "<window_id>"]` — canonical training window id
- `["assignment", "<assignment_id>"]` — canonical assignment id
- `["artifact", "<artifact_id>"]` — canonical artifact id
- `["checkpoint", "<checkpoint_id>"]` — canonical checkpoint id
- `["policy", "<policy_revision_id>"]` — policy revision bound to the event
- `["manifest", "<manifest_digest>"]` — artifact or checkpoint manifest digest
- `["status", "<value>"]` — machine-readable state
- `["role", "<role_name>"]` — node or receipt role
- `["cap", "<capability_name>", "<value>"]` — capability declaration
- `["class", "<execution_class>"]` — admitted execution class
- `["k", "<kind>"]` — optional supported kind or handler signal where relevant
- `["e", "<event_id>", "<relay>", "<marker>"]` — referenced events
- `["a", "<coordinate>", "<relay>", "<marker>"]` — referenced addressable events
- `["x", "<sha256_digest>"]` — payload or bundle digest
- `["url", "<location_hint>"]` — public or semi-public locator hint
- `["reason", "<reason_code>"]` — machine-readable reason

Implementations MAY add additional tags, but parsers MUST NOT treat unknown
tags as invalid by default.

## 1. Training Network Contract (`kind:39500`)

The training network contract is the root coordination object for one training
domain.

Its canonical coordinate is:

```text
39500:<publisher_pubkey>:<network_id>
```

It SHOULD define:

- canonical `network_id`
- current or admitted governance revision
- model family or workload family
- window cadence or epoch cadence
- admitted role vocabulary
- compatibility surface for node software or manifest revisions
- settlement posture
- supported optional profiles

Recommended tags:

- `["d", "<network_id>"]`
- `["status", "active|paused|retired"]`
- `["model_family", "<family>"]`
- `["window_cadence", "<seconds>"]`
- `["role", "trainer"]`
- `["role", "validator"]`
- `["role", "checkpoint_authority"]`
- `["role", "aggregator"]`
- `["role", "miner"]`
- `["profile", "trn-discovery"]`
- `["profile", "trn-private"]`
- `["profile", "trn-challenge"]`
- `["profile", "trn-reputation"]`

`content` SHOULD be a JSON object containing the network's human-readable and
machine-readable policy summary.

Minimal example:

```jsonc
{
  "kind": 39500,
  "content": "{\"schema\":1,\"name\":\"Psion Trainnet A\",\"governance_revision\":\"g-4\",\"policy_family\":\"psion.train.v1\"}",
  "tags": [
    ["d", "psion-trainnet-a"],
    ["status", "active"],
    ["model_family", "psion"],
    ["window_cadence", "3600"],
    ["role", "validator"],
    ["role", "checkpoint_authority"],
    ["role", "miner"],
    ["profile", "trn-discovery"],
    ["profile", "trn-challenge"]
  ]
}
```

## 2. Training Node Record (`kind:39501`)

The node record publishes one node's identity and admitted capability posture
inside a training network.

Its canonical coordinate is:

```text
39501:<node_pubkey>:<network_id>
```

Core responsibilities:

- bind a node to one network
- declare admitted roles
- declare admitted execution classes
- publish build or software digest
- publish capability posture
- publish coordination endpoints or relay hints
- publish revocation or replacement posture

Recommended tags:

- `["d", "<network_id>"]`
- `["network", "<network_id>"]`
- `["status", "online|degraded|offline|revoked"]`
- `["role", "<role_name>"]`
- `["class", "<execution_class>"]`
- `["build", "<software_digest>"]`
- `["cap", "backend", "cuda"]`
- `["cap", "accelerator", "8xh100"]`
- `["cap", "storage", "remote_authoritative"]`
- `["relay", "wss://coord.example.com"]`

`content` SHOULD summarize operator-facing metadata such as node label,
software version, or notes. Sensitive endpoint details MAY be omitted from
public publication and delivered privately.

## 3. Training Window (`kind:39510`)

The training window is the canonical coordination object for one bounded
training interval or validation interval.

Its canonical coordinate is:

```text
39510:<publisher_pubkey>:<window_id>
```

It SHOULD bind:

- `network_id`
- `window_id`
- policy revision in
- workload family
- assignment seed or selection seed
- current state

Recommended states:

- `planned`
- `active`
- `sealed`
- `scored`
- `reconciled`
- `canceled`

Recommended tags:

- `["d", "<window_id>"]`
- `["network", "<network_id>"]`
- `["policy", "<policy_revision_id>"]`
- `["status", "planned|active|sealed|scored|reconciled|canceled"]`
- `["assignment_seed", "<seed_digest>"]`
- `["workload", "<workload_family>"]`

## 4. Training Receipt (`kind:39511`)

The training receipt is the append-only event for assignments and window-level
state transitions.

This is intentionally one generic receipt kind so implementations can evolve
without exploding the kind space.

Its job is to publish lightweight control-plane outcomes such as:

- assignment published
- assignment accepted
- assignment expired
- upload acknowledged
- window sealed
- window reconciled
- replay requested

Required tags:

- `["network", "<network_id>"]`
- `["window", "<window_id>"]`
- `["status", "<receipt_status>"]`

Recommended tags:

- `["assignment", "<assignment_id>"]`
- `["policy", "<policy_revision_id>"]`
- `["role", "<role_name>"]`
- `["artifact", "<artifact_id>"]`
- `["reason", "<reason_code>"]`
- `["e", "<referenced_event_id>", "<relay>", "window"]`
- `["a", "39510:<pubkey>:<window_id>", "<relay>", "window"]`

Example statuses:

- `assignment_published`
- `assignment_accepted`
- `assignment_expired`
- `artifact_uploaded`
- `window_sealed`
- `window_reconciled`
- `replay_required`

## 5. Validator Verdict (`kind:39512`)

The validator verdict is the machine-readable result over one contribution,
artifact, assignment, or claimed outcome.

Required tags:

- `["network", "<network_id>"]`
- `["window", "<window_id>"]`
- `["status", "accepted|quarantined|rejected|replay_required"]`

Recommended tags:

- `["assignment", "<assignment_id>"]`
- `["artifact", "<artifact_id>"]`
- `["policy", "<policy_revision_id>"]`
- `["reason", "<reason_code>"]`
- `["validator_policy", "<validator_policy_id>"]`
- `["x", "<artifact_or_bundle_digest>"]`
- `["e", "<challenge_or_request_event_id>", "<relay>", "challenge"]`

`content` SHOULD contain a compact JSON verdict summary. Large proof bundles
SHOULD be stored off-Nostr and referenced through `kind:39520` artifact
locators or optional NIP-90 result flows.

## 6. Training Artifact Locator (`kind:39520`)

The artifact locator publishes metadata for a heavy training object without
embedding the heavy object itself.

Its canonical coordinate is:

```text
39520:<publisher_pubkey>:<artifact_id>
```

Valid artifact classes include:

- checkpoints
- delta bundles
- eval bundles
- proof bundles
- score bundles

Required tags:

- `["d", "<artifact_id>"]`
- `["network", "<network_id>"]`
- `["status", "staged|durable|promoted|revoked"]`

Recommended tags:

- `["artifact", "<artifact_id>"]`
- `["checkpoint", "<checkpoint_id>"]`
- `["manifest", "<manifest_digest>"]`
- `["x", "<payload_digest>"]`
- `["url", "<location_hint>"]`
- `["class", "checkpoint|delta|eval|proof|score"]`
- `["policy", "<policy_revision_id>"]`
- `["reason", "<reason_code>"]`

Location hints MAY reference:

- object storage
- authenticated HTTP endpoints
- content-addressed stores
- future peer fetch mechanisms

TRN does not require one storage backend.

It requires only that the locator metadata be signed and machine-legible.

## 7. Training Contribution Closeout (`kind:39530`)

The contribution closeout is the durable publication of the final economic or
operator-facing outcome for a contribution or sealed window.

This kind is intentionally broad enough to publish:

- rewarded
- no_reward
- held
- quarantined
- refused
- slashed

Required tags:

- `["network", "<network_id>"]`
- `["window", "<window_id>"]`
- `["status", "<closeout_status>"]`

Recommended tags:

- `["assignment", "<assignment_id>"]`
- `["artifact", "<artifact_id>"]`
- `["policy", "<policy_revision_id>"]`
- `["reason", "<reason_code>"]`
- `["amount", "<millisats>"]`
- `["e", "<verdict_event_id>", "<relay>", "verdict"]`
- `["e", "<settlement_event_id>", "<relay>", "settlement"]`

TRN does not define one payment rail.

Implementations MAY link this closeout to:

- Lightning or zap receipts
- NIP-AC settlement receipts
- internal accounting records
- later reward systems

## Optional Profiles

### TRN-Discovery

Implementations MAY use:

- NIP-89 `kind:31990` to advertise TRN handlers, registries, validators, or
  artifact-index services
- NIP-66 relay-discovery events to publish preferred coordination relays and
  relay-quality posture

### TRN-Private

Implementations MAY use NIP-44 and NIP-59 for:

- private assignment payloads
- private challenge details
- private operator instructions
- private locator hints

TRN-Private SHOULD be used only for low-frequency control payloads, not for
hot-path runtime coordination.

### TRN-Challenge

Implementations MAY use NIP-90 for:

- validator challenge jobs
- replay requests
- proof-generation requests
- bounded benchmark or eval jobs

TRN does not require NIP-90, but it composes well with it for narrow challenge
flows.

### TRN-Reputation

Implementations MAY use NIP-32 labels for:

- fraud markers
- validator-quality markers
- build-revoked markers
- checkpoint-warning markers
- reputation or trust annotations

## Security Considerations

### Do not move heavy bytes onto Nostr

Checkpoints, policy weights, rollout payloads, gradients, and optimizer state
SHOULD NOT be embedded in TRN events.

TRN is for coordination metadata, not heavy artifact transfer.

### Distinguish locators from proofs

A signed locator proves who published a reference and what digest they claimed.

It does not, by itself, prove the underlying artifact is valid. Validator
verdicts and external proof flows remain necessary.

### Distinguish training relays from Nostr relays

Some training systems use the word `relay` for runtime transport or overlay
roles. TRN uses Nostr relays only for event distribution and coordination
publication.

These roles SHOULD NOT be conflated.

### Private coordination is still metadata-sensitive

Even when NIP-44 and NIP-59 are used, implementations SHOULD minimize
sensitive metadata leakage through relay choice, timing, and tag structure.

### Revocation and replacement

Addressable events define the current head for a network contract, node record,
window state, or artifact locator. Historical receipts and verdicts remain
append-only and SHOULD NOT be rewritten.

## Example Flow

One bounded training cycle can look like this:

1. A coordinator publishes a `kind:39500` training network contract.
2. Participants publish `kind:39501` node records advertising roles and
   capability posture.
3. A coordinator publishes a `kind:39510` window.
4. Assignment receipts are emitted as `kind:39511` events.
5. Heavy artifacts move outside Nostr.
6. Validators publish `kind:39512` verdicts.
7. Checkpoint or proof metadata is published through `kind:39520`.
8. Final reward, hold, or quarantine status is published through `kind:39530`.

This preserves one shared public coordination trail without pretending the
training runtime itself lives on relays.

## Changelog

- v0: initial umbrella draft for model-training coordination on Nostr with one
  core document and optional discovery, privacy, challenge, and reputation
  profiles
