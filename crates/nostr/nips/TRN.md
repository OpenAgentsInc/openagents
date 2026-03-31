NIP-TRN
=======

Model Training Coordination
---------------------------

`draft` `optional`

This NIP defines the coordination and publishing layer for model training on
Nostr.

Nostr already has several pieces that can help with training coordination.
NIP-89 can announce services and handlers. NIP-90 can carry small one-off
jobs like "check this result", "rerun this work", "show proof", or "score
this model". NIP-32 can publish fraud markers and reputation signals. NIP-44
and NIP-59 can wrap private coordination messages. NIP-66 can help describe
relay quality. What Nostr does not yet have is one shared training format
that ties those pieces together. This NIP introduces that format.

The goal is to make model-training coordination easy to understand across
nodes, validators, operators, and supporting services so they can refer to the
same network, the same window, the same assignments, the same verdicts, and the
same accepted checkpoint or result without falling back to private
spreadsheets, operator-only host lists, lane-specific dashboards, ad hoc JSON
receipts, or unverified reward and result claims.

In practical terms, this NIP is not about putting training itself on Nostr. It
is about making the coordination layer around training work across tools and
easy to check. That means network identity, node capability publication,
training-window identity, assignment and transition receipts, validator
results, pointers to checkpoints or proof files, and final contribution
outcomes all get one shared protocol surface, while trainer execution, collective synchronization,
checkpoint or weight transfer, rollout payload transport, gradient exchange,
and other fast-moving runtime work remain outside this NIP.

TRN should support more than one training shape. A network may be public and
easy to discover, or private and based on an approved list of nodes. A
validator may need a small replay job through NIP-90, while an operator may
need wrapped private assignment messages through NIP-44 and NIP-59. Another
network may only want public proof and settlement publication. This NIP
therefore defines one small core for the shared training objects and leaves discovery, privacy, challenge,
and reputation as optional profiles layered alongside that core.

This NIP is designed to fit alongside:

- NIP-01: events, tags, subscriptions
- NIP-32: labels and reputation signals
- NIP-44 / NIP-59: wrapped private coordination messages
- NIP-66: relay discovery and health signals
- NIP-89: handler and service announcements
- NIP-90: small check, rerun, and proof jobs
- NIP-94: file and bundle metadata when public artifact references are needed

It also fits alongside our in-repo drafts:

- NIP-DS for dataset identity and delivery
- NIP-SKL for reusable skill identity and trust
- NIP-AC for credit, settlement, and limited spend flows where training
  incentives or challenge payments later need an interoperable payment layer

TRN core covers network, node, window, receipt, verdict, artifact pointer,
and final outcome records. The optional profiles are TRN-Discovery via
NIP-89 and NIP-66, TRN-Private via NIP-44 and NIP-59, TRN-Challenge via
NIP-90, and TRN-Reputation via NIP-32.

## Abstract

The training system needs one public answer to the questions that sit above the
runtime: what network is this node participating in, what role is it claiming,
what training window is active, what work was assigned, what result did
validators produce, which checkpoint or file was accepted, and what reward,
hold, quarantine, or refusal followed. TRN defines lightweight, signed
control records for those answers.

The design rule is simple:

> TRN carries names, links, checksums, version ids, pointers, and receipts.
> Big training files stay off Nostr.

## Rationale

NIP-90 is a good fit for small jobs like "check this result", "rerun this
work", "show proof", or "score this model", but it is not, by itself, a good
fit for questions like "what training network is this node in?", "what role is
this node claiming?", or "which checkpoint became the accepted one?" Likewise,
NIP-89 is useful for service and handler discovery, but it does not define
shared network records, window identity, validator results, or checkpoint
pointer records for training.

Model training is also broader than one service request. A training cycle may
span a network contract, multiple node records, one or more windows, many
assignments, validator-owned results, accepted or held checkpoint state, and a
reward or no-reward final outcome. These objects need to stay linked even when
different parties publish them.

TRN introduces a training coordination layer that can be used with
public discovery through node records and handler announcements, private or
permissioned coordination through wrapped receipts and pointers, small
challenge or rerun jobs through NIP-90, and public or partly public
reputation and fraud signaling through NIP-32. It is therefore not trying to
make Nostr do everything. It is trying to standardize the narrow layer that
benefits most from signatures, copying events across relays, shared discovery,
public or partly public checking, and a permanent record of receipts.

## Terms

- `network`: one named training group
- `node`: one published participant record that says who a machine is and what
  it can do
- `window`: one limited chunk of training time, assignment time, or validation
  time
- `assignment`: one piece of work given to one node in one window
- `verdict`: one validator result about a submission, assignment, or claimed
  result
- `artifact locator`: a signed pointer to a checkpoint, update bundle, proof
  file, or other large object without embedding that large object in the event
- `closeout`: one final published outcome for a contribution or window, such as
  rewarded, held, quarantined, or refused

## Scope And Layering

### TRN Core

TRN core is normative and defines:

- one shared network record
- one shared node record format
- one shared training window record
- lightweight assignment and state-change receipts
- validator result publication
- signed artifact pointer metadata
- final outcome linkage

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

- training code internals
- optimizer state shared between machines
- how machines talk to each other during training
- how often machines exchange live training data
- moving checkpoint pieces between machines
- moving model weights between machines
- dataset transfer
- rollout data transfer
- sandbox runtime behavior
- exact reward math

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
| 5000-5999 / 6000-6999 / 7000 | NIP-90 | optional check, rerun, proof, or eval jobs |

## Common Tags

TRN uses these common tags:

- `["d", "<identifier>"]` — id for an addressable event
- `["network", "<network_id>"]` — shared training network id
- `["window", "<window_id>"]` — shared training window id
- `["assignment", "<assignment_id>"]` — shared assignment id
- `["artifact", "<artifact_id>"]` — shared artifact id
- `["checkpoint", "<checkpoint_id>"]` — shared checkpoint id
- `["policy", "<policy_revision_id>"]` — policy revision bound to the event
- `["manifest", "<manifest_digest>"]` — artifact or checkpoint manifest digest
- `["status", "<value>"]` — machine-readable state
- `["role", "<role_name>"]` — node or receipt role
- `["cap", "<capability_name>", "<value>"]` — capability declaration
- `["class", "<execution_class>"]` — allowed execution class
- `["k", "<kind>"]` — optional supported kind or handler signal where relevant
- `["e", "<event_id>", "<relay>", "<marker>"]` — referenced events
- `["a", "<coordinate>", "<relay>", "<marker>"]` — referenced addressable events
- `["x", "<sha256_digest>"]` — file or bundle digest
- `["url", "<location_hint>"]` — public or semi-public location hint
- `["reason", "<reason_code>"]` — machine-readable reason

Implementations MAY add additional tags, but parsers MUST NOT treat unknown
tags as invalid by default.

## 1. Training Network Contract (`kind:39500`)

The training network contract is the main shared record for one training
network.

Its address is:

```text
39500:<publisher_pubkey>:<network_id>
```

It SHOULD define:

- network id
- current governance revision
- model family or workload family
- window timing
- allowed roles
- software or manifest compatibility rules
- reward rules
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

The node record publishes one node's identity and allowed capabilities inside a
training network.

Its address is:

```text
39501:<node_pubkey>:<network_id>
```

Core responsibilities:

- bind a node to one network
- declare allowed roles
- declare allowed execution classes
- publish build or software digest
- publish capabilities
- publish coordination endpoints or relay hints
- publish whether the node was replaced or revoked

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

The training window is the main shared record for one training round or
validation round.

Its address is:

```text
39510:<publisher_pubkey>:<window_id>
```

It SHOULD name:

- `network_id`
- `window_id`
- active policy revision
- workload family
- assignment seed used to choose work
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

The training receipt is the append-only record for assignments and window state
changes.

This is intentionally one generic receipt kind so implementations can evolve
without exploding the kind space.

Its job is to publish lightweight coordination updates such as:

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

The validator verdict is the simple machine-readable result for one
contribution, artifact, assignment, or claimed outcome.

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

`content` SHOULD contain a compact JSON verdict summary. Large proof files
SHOULD be stored off-Nostr and referenced through `kind:39520` artifact
pointers or optional NIP-90 result flows.

## 6. Training Artifact Locator (`kind:39520`)

The artifact locator publishes metadata for a large training file without
embedding the file itself.

Its address is:

```text
39520:<publisher_pubkey>:<artifact_id>
```

Valid artifact classes include:

- checkpoints
- update bundles
- eval result bundles
- proof files
- score files

Required tags:

- `["d", "<artifact_id>"]`
- `["network", "<network_id>"]`
- `["status", "staged|stored|accepted|revoked"]`

Recommended tags:

- `["artifact", "<artifact_id>"]`
- `["checkpoint", "<checkpoint_id>"]`
- `["manifest", "<manifest_digest>"]`
- `["x", "<file_digest>"]`
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

It requires only that the pointer metadata be signed and easy for software to
read.

## 7. Training Contribution Closeout (`kind:39530`)

The contribution closeout is the final published payment or operational outcome
for a contribution or sealed window.

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
  artifact index services
- NIP-66 relay-discovery events to publish preferred coordination relays and
  relay health

### TRN-Private

Implementations MAY use NIP-44 and NIP-59 for:

- private assignment details
- private challenge details
- private operator instructions
- private locator hints

TRN-Private SHOULD be used only for occasional coordination messages, not for
fast runtime traffic.

### TRN-Challenge

Implementations MAY use NIP-90 for:

- validator check jobs
- replay requests
- requests to generate proof files
- small benchmark or eval jobs

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

Checkpoints, model weights, rollout payloads, gradients, and optimizer state
SHOULD NOT be embedded in TRN events.

TRN is for coordination metadata, not heavy artifact transfer.

### Distinguish pointers from proofs

A signed pointer shows who published a reference and what digest they claimed.

It does not, by itself, prove the underlying artifact is valid. Validator
verdicts and external proof flows remain necessary.

### Distinguish training relays from Nostr relays

Some training systems use the word `relay` for runtime transport or overlay
roles. TRN uses Nostr relays only for event distribution and coordination
publication.

These roles SHOULD NOT be mixed up.

### Private coordination is still metadata-sensitive

Even when NIP-44 and NIP-59 are used, implementations SHOULD minimize
sensitive metadata leakage through relay choice, timing, and tag structure.

### Revocation and replacement

For addressable events, the latest version is the current one for a network
contract, node record, window state, or artifact locator. Historical receipts
and verdicts are append-only and SHOULD NOT be rewritten.

## Example Flow

One training cycle can look like this:

1. A coordinator publishes a `kind:39500` training network contract.
2. Participants publish `kind:39501` node records advertising roles and
   capabilities.
3. A coordinator publishes a `kind:39510` window.
4. Assignment receipts are emitted as `kind:39511` events.
5. Heavy artifacts move outside Nostr.
6. Validators publish `kind:39512` verdicts.
7. Checkpoint or proof metadata is published through `kind:39520`.
8. Final reward, hold, or quarantine status is published through `kind:39530`.

This preserves one shared public coordination trail without pretending the
training itself runs on relays.

## Changelog

- v0: initial umbrella draft for model-training coordination on Nostr with one
  core document and optional discovery, privacy, challenge, and reputation
  profiles
