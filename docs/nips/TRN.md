> Status: draft living spec restored for interoperability.
> Last shipped in: `f5919c766^:crates/nostr/nips/TRN.md`.
> Market stream: training.

NIP-TRN
=======

AI Model Training Coordination
------------------------------

`draft` `optional`

This NIP defines the coordination and publishing layer for training AI models
such as LLMs, multimodal models, embedding models, and related neural-network
models.

Large multi-party training runs should be recoverable and forkable. If a
coordinator disappears or a group wants to continue on different terms,
another operator should be able to read the relay history, find the accepted
checkpoint or weight pointers, see the active window and policy, and continue
or fork from that state without depending on one private database or
dashboard.

TRN standardizes network identity, node capability publication, window
records, receipts, validator results, and pointers to checkpoints, weights,
and proof files. It does not move model bytes, gradients, checkpoints, or
runtime traffic.

TRN builds on:

- NIP-01 for base events, tags, and subscriptions
- NIP-32 for fraud and reputation labels
- NIP-44 and NIP-59 for private coordination
- NIP-66 for relay health and discovery
- NIP-89 for service discovery
- NIP-90 for small check and replay jobs
- NIP-94 for public file metadata
- our in-repo drafts NIP-DS, NIP-SKL, and NIP-AC for datasets, skills, and
  settlement-related flows

TRN core covers:

- network records
- node records
- window records
- receipts
- verdicts
- artifact pointers
- local-update artifact metadata
- round-level aggregation metadata
- canonical post-aggregation checkpoint linkage
- closeouts

Optional profiles cover:

- discovery
- DiLoCo-style local-update rounds
- private coordination
- challenge
- reputation

## Abstract

The AI training system needs one public answer to the questions that sit above the
runtime: what network is this node participating in, what role is it claiming,
what training window is active, what work was assigned, what result did
validators produce, which checkpoint or file was accepted, what earlier run or
artifact this run builds on, and what reward, hold, quarantine, or refusal
followed. TRN defines lightweight, signed control records for those answers.

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

Training AI models is also broader than one service request. A training cycle
may span a network contract, multiple node records, one or more windows, many
assignments, validator-owned results, accepted or held checkpoint state, and a
reward or no-reward final outcome. These objects need to stay linked even when
different parties publish them.

Large training runs also need reusable state. If one operator disappears,
another should be able to recover the last known state from relays instead of
asking for a private database export. If part of the network wants to continue
on different terms, it should be able to publish a new run that points back to
the earlier network, window, checkpoint, weight files, and proofs it started
from. TRN makes that recovery and fork trail part of the protocol instead of
an after-the-fact spreadsheet.

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
- `artifact locator`: a signed pointer to a checkpoint, model-weight file,
  local-update bundle, aggregate bundle, proof file, or other large object
  without embedding that large object in the event
- `fork`: one new training run or window that continues from earlier published
  state
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
- recovery and fork links to earlier runs, windows, or artifacts
- final outcome linkage

### Optional Profiles

TRN core intentionally does not force one privacy rail, one payment rail, or
one challenge mechanism.

Optional profiles are:

- TRN-Discovery via NIP-89 and NIP-66
- TRN-DiLoCo for local-update / infrequent-sync training rounds
- TRN-Private via NIP-44 and NIP-59
- TRN-Challenge via NIP-90
- TRN-Reputation via NIP-32

### Out Of Scope

This NIP does not standardize:

- training code internals
- optimizer state shared between machines
- how machines talk to each other during training
- how often machines exchange live training data
- how local updates are numerically aggregated
- optimizer internals during local training
- moving checkpoint pieces between machines
- moving model weights between machines
- transport of update tensors or optimizer tensors
- dataset transfer
- rollout data transfer
- sandbox runtime behavior
- exact all-reduce, RPC, or parameter-server behavior
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
- `["aggregate", "<aggregate_id>"]` — aggregate artifact or aggregate outcome id
- `["checkpoint", "<checkpoint_id>"]` — shared checkpoint id
- `["sync_profile", "<profile_name>"]` — training synchronization profile such
  as `diloco`
- `["round", "<round_index_or_round_id>"]` — round number or equivalent local
  synchronization index
- `["base_checkpoint", "<checkpoint_id>"]` — checkpoint from which local work
  in this round begins
- `["policy", "<policy_revision_id>"]` — policy revision bound to the event
- `["manifest", "<manifest_digest>"]` — artifact or checkpoint manifest digest
- `["status", "<value>"]` — machine-readable state
- `["planned_local_steps", "<count>"]` — intended local optimizer steps for one
  round
- `["local_steps", "<count>"]` — realized local optimizer steps in one
  contribution
- `["tokens", "<count>"]` — token count attributed to one contribution or round
- `["examples", "<count>"]` — example count attributed to one contribution or
  round
- `["weight", "<value>"]` — aggregation weight value claimed for one submitted
  contribution
- `["aggregation_rule", "<rule_name>"]` — machine-readable aggregation rule
- `["aggregation_weight", "<weight_basis>"]` — declared weighting basis
- `["role", "<role_name>"]` — node or receipt role
- `["group", "<group_id>"]` — optional island, shard group, or cohort id
- `["promotion", "candidate|accepted|superseded"]` — aggregate or checkpoint
  promotion state
- `["cap", "<capability_name>", "<value>"]` — capability declaration
- `["class", "<value>"]` — execution class, expected artifact class, or stored
  artifact class depending on the event
- `["k", "<kind>"]` — optional supported kind or handler signal where relevant
- `["p", "<pubkey>", "<relay>", "<marker>"]` — referenced actor such as the
  affected contributor, validator, or coordinator
- `["e", "<event_id>", "<relay>", "<marker>"]` — referenced events such as
  source windows, verdicts, or receipts
- `["a", "<coordinate>", "<relay>", "<marker>"]` — referenced addressable
  events such as source networks, source windows, checkpoint pointers, or
  weight pointers
- `["x", "<sha256_digest>"]` — file or bundle digest
- `["url", "<location_hint>"]` — public or semi-public location hint
- `["reason", "<reason_code>"]` — machine-readable reason

Implementations MAY add additional tags, but parsers MUST NOT treat unknown
tags as invalid by default.

Markers such as `source`, `resume`, `fork`, `bootstrap`, and `weights` are
RECOMMENDED when they make recovery or fork lineage clearer.

For receipts, verdicts, closeouts, and labels that affect a particular actor,
implementations SHOULD use `p` tags with explicit markers such as `subject`,
`validator`, or `coordinator`.

Implementations SHOULD use the same `base_checkpoint` value across receipts,
verdicts, and artifact locators that belong to one DiLoCo-style round.

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
- synchronization profile and round semantics when one profile governs the
  entire run
- allowed roles
- software or manifest compatibility rules
- source network or source artifacts when the run resumes or forks earlier work
- reward rules
- supported optional profiles

Recommended tags:

- `["d", "<network_id>"]`
- `["status", "active|paused|retired"]`
- `["model_family", "<family>"]`
- `["window_cadence", "<seconds>"]`
- `["sync_profile", "diloco"]`
- `["aggregation_rule", "weighted_avg|uniform_avg|custom"]`
- `["aggregation_weight", "tokens|examples|steps|uniform|custom"]`
- `["role", "trainer"]`
- `["role", "validator"]`
- `["role", "checkpoint_authority"]`
- `["role", "aggregator"]`
- `["role", "miner"]`
- `["profile", "trn-discovery"]`
- `["profile", "trn-private"]`
- `["profile", "trn-challenge"]`
- `["profile", "trn-reputation"]`
- `["a", "39500:<pubkey>:<network_id>", "<relay>", "source"]`
- `["a", "39520:<pubkey>:<artifact_id>", "<relay>", "bootstrap"]`
- `["a", "39520:<pubkey>:<artifact_id>", "<relay>", "weights"]`

A training network that continues earlier work SHOULD reference the source
network contract and the source artifact locators it starts from, especially
the accepted checkpoint or model-weight files needed to continue or fork the
run.

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
- base checkpoint id for the round
- round index
- expected local-work target for participants
- aggregation deadline
- minimum participation or minimum admitted weight needed to reconcile
- accepted aggregate or promoted checkpoint once the round closes
- source window or recovery point, if the window resumes earlier work
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
- `["sync_profile", "diloco"]`
- `["round", "<round_index>"]`
- `["assignment_seed", "<seed_digest>"]`
- `["workload", "<workload_family>"]`
- `["base_checkpoint", "<checkpoint_id>"]`
- `["planned_local_steps", "<count>"]`
- `["aggregation_deadline", "<unix_seconds>"]`
- `["min_participants", "<count>"]`
- `["min_weight_fraction", "<decimal_string>"]`
- `["aggregation_rule", "weighted_avg|uniform_avg|custom"]`
- `["aggregation_weight", "tokens|examples|steps|uniform|custom"]`
- `["aggregate", "<aggregate_id>"]`
- `["checkpoint", "<promoted_checkpoint_id>"]`
- `["a", "39510:<pubkey>:<window_id>", "<relay>", "resume"]`
- `["a", "39520:<pubkey>:<artifact_id>", "<relay>", "bootstrap"]`

For TRN-DiLoCo, one window normally represents one local-training round that
starts from one `base_checkpoint` and may end with one accepted aggregate or
one promoted checkpoint for the next round. The window SHOULD make the
intended local-work target and aggregation rule explicit so another operator
can resume or replay the coordination state without private coordinator data.

Example window for one DiLoCo round:

```jsonc
{
  "kind": 39510,
  "content": "{\"schema\":1,\"round\":172,\"aggregation_deadline_unix\":1786352400}",
  "tags": [
    ["d", "psion-r172"],
    ["network", "psion-trainnet-a"],
    ["status", "active"],
    ["sync_profile", "diloco"],
    ["round", "172"],
    ["policy", "g-4"],
    ["workload", "psion.pretrain.v1"],
    ["base_checkpoint", "ckpt-171"],
    ["planned_local_steps", "500"],
    ["aggregation_rule", "weighted_avg"],
    ["aggregation_weight", "tokens"],
    ["min_participants", "8"],
    ["min_weight_fraction", "0.80"]
  ]
}
```

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
- local update submitted
- local update admitted
- aggregate candidate published
- aggregate accepted
- checkpoint promoted
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
- `["p", "<subject_pubkey>", "<relay>", "subject"]`
- `["p", "<coordinator_pubkey>", "<relay>", "coordinator"]`
- `["reason", "<reason_code>"]`
- `["class", "<expected_artifact_class>"]`
- `["checkpoint", "<checkpoint_id>"]`
- `["base_checkpoint", "<checkpoint_id>"]`
- `["round", "<round_index>"]`
- `["local_steps", "<count>"]`
- `["tokens", "<count>"]`
- `["examples", "<count>"]`
- `["weight", "<value>"]`
- `["group", "<group_id>"]`
- `["a", "39520:<pubkey>:<artifact_id>", "<relay>", "source"]`
- `["e", "<referenced_event_id>", "<relay>", "window"]`
- `["a", "39510:<pubkey>:<window_id>", "<relay>", "window"]`

Example statuses:

- `assignment_published`
- `assignment_accepted`
- `assignment_expired`
- `artifact_uploaded`
- `update_submitted`
- `update_admitted`
- `update_rejected`
- `aggregate_candidate_published`
- `aggregate_accepted`
- `checkpoint_promoted`
- `window_sealed`
- `window_reconciled`
- `replay_required`

When `status` is assignment-related or local-update-related, `content` SHOULD
be a compact JSON object that includes:

- `subject_pubkey`
- `assignment_deadline_unix` when an assignment deadline exists
- `expected_artifact_class`
- either `sample_pool_digest` or `shard_digest` when data selection is part of
  the policy
- either `source_checkpoint_id` or `source_checkpoint_coordinate`
- `base_checkpoint_id` when the work belongs to one DiLoCo-style round
- `local_step_count` when local work is step-bounded
- `consumed_token_count` when token-weighted aggregation is used
- `consumed_example_count` when example-weighted aggregation is used
- `aggregation_weight_value` when the contribution publishes a concrete
  weighting value
- `group_id` when the run uses islands, cohorts, or subgroup coordination

That profile gives validators and replacement coordinators enough information
to reason about local-update submissions and round closure without adding new
receipt kinds or standardizing runtime execution.

Example receipt for a submitted local update:

```jsonc
{
  "kind": 39511,
  "content": "{\"schema\":1,\"subject_pubkey\":\"<trainer>\",\"base_checkpoint_id\":\"ckpt-171\",\"local_step_count\":500,\"consumed_token_count\":182340992,\"aggregation_weight_value\":\"182340992\"}",
  "tags": [
    ["network", "psion-trainnet-a"],
    ["window", "psion-r172"],
    ["status", "update_submitted"],
    ["assignment", "asg-172-044"],
    ["role", "trainer"],
    ["artifact", "upd-r172-n44"],
    ["class", "local_update"],
    ["base_checkpoint", "ckpt-171"],
    ["round", "172"],
    ["local_steps", "500"],
    ["tokens", "182340992"],
    ["weight", "182340992"],
    ["p", "<trainer>", "", "subject"],
    ["a", "39520:<pubkey>:upd-r172-n44", "", "source"],
    ["a", "39510:<pubkey>:psion-r172", "", "window"]
  ]
}
```

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
- `["base_checkpoint", "<checkpoint_id>"]`
- `["round", "<round_index>"]`
- `["aggregate", "<aggregate_id>"]`
- `["policy", "<policy_revision_id>"]`
- `["p", "<subject_pubkey>", "<relay>", "subject"]`
- `["p", "<validator_pubkey>", "<relay>", "validator"]`
- `["promotion", "candidate|accepted|superseded"]`
- `["reason", "<reason_code>"]`
- `["validator_policy", "<validator_policy_id>"]`
- `["x", "<artifact_or_bundle_digest>"]`
- `["e", "<challenge_or_request_event_id>", "<relay>", "challenge"]`

`content` SHOULD contain a compact JSON verdict summary. Large proof files
SHOULD be stored off-Nostr and referenced through `kind:39520` artifact
pointers or optional NIP-90 result flows.

For TRN-DiLoCo, verdict summaries SHOULD make clear whether the verdict applies
to one local update, one aggregate candidate, or one promoted checkpoint. When
a verdict covers one aggregate candidate, implementations SHOULD reference both
the candidate aggregate artifact and the base checkpoint from which the round
started.

Recommended compact JSON fields for TRN-DiLoCo verdicts are:

- `subject_type` with values such as `local_update`, `aggregate_candidate`, or
  `promoted_checkpoint`
- `base_checkpoint_id`
- `round_index`
- `loss_delta` when available
- `eval_metric` when available
- `drift_metric` when available
- `nan_detected`
- `divergence_flag`

This keeps acceptance logic inspectable without forcing one proof system.

## 6. Training Artifact Locator (`kind:39520`)

The artifact locator publishes metadata for a large training file without
embedding the file itself.

Its address is:

```text
39520:<publisher_pubkey>:<artifact_id>
```

Implementations SHOULD treat `artifact_id` as a resolver-backed logical id.
They SHOULD NOT derive it by munging a host-local path, download-cache path,
or cloud-object relative path into a pseudo-identifier.

Valid artifact classes include:

- checkpoints
- model weight files
- optimizer snapshots
- config bundles
- local-update bundles
- aggregate bundles
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
- `["class", "checkpoint|weights|optimizer|config|local_update|aggregate|eval|proof|score"]`
- `["window", "<window_id>"]`
- `["policy", "<policy_revision_id>"]`
- `["reason", "<reason_code>"]`
- `["base_checkpoint", "<checkpoint_id>"]`
- `["round", "<round_index>"]`
- `["local_steps", "<count>"]`
- `["weight", "<value>"]`
- `["aggregate", "<aggregate_id>"]`
- `["promotion", "candidate|accepted|superseded"]`
- `["a", "39520:<pubkey>:<artifact_id>", "<relay>", "source"]`

Location hints MAY reference:

- object storage
- authenticated HTTP endpoints
- content-addressed stores
- future peer fetch mechanisms

TRN does not require one storage backend.

It requires only that the pointer metadata be signed and easy for software to
read.

Artifact locators are also the main reuse, recovery, and fork surface in TRN.
When a locator is meant to support resume or fork, its manifest SHOULD identify
the files another operator needs to continue the run, such as checkpoint files,
model weights, optimizer snapshots, config bundles, or proof files.

When TRN-DiLoCo is used, implementations SHOULD distinguish clearly between:

- one `local_update` artifact published by a participant for one round
- one `aggregate` artifact published as the candidate or accepted merged result
- one `checkpoint` artifact published as the promoted starting point for the
  next round

A `local_update` locator SHOULD identify:

- the `base_checkpoint` from which the local work started
- the round or window it belongs to
- the claimed local step count when relevant
- the claimed aggregation weight basis and value when relevant

An `aggregate` locator SHOULD identify:

- the `base_checkpoint` for the round it aggregates
- the set or manifest of admitted local updates
- whether it is a candidate or accepted aggregate
- the promoted checkpoint id when it directly materializes the next round's
  starting state

For `class=local_update`, `content` SHOULD be a compact JSON object containing:

- `schema`
- `artifact_role` with value `local_update`
- `base_checkpoint_id`
- `round_index`
- `local_step_count` when relevant
- `aggregation_weight_basis`
- `aggregation_weight_value`
- `format` or bundle format id
- `manifest_digest`

For `class=aggregate`, `content` SHOULD be a compact JSON object containing:

- `schema`
- `artifact_role` with value `aggregate`
- `base_checkpoint_id`
- `round_index`
- `aggregation_rule`
- `aggregation_weight_basis`
- `admitted_update_count`
- `admitted_weight_total`
- `promotion_state` with value `candidate` or `accepted`
- `promoted_checkpoint_id` when applicable
- `manifest_digest`

Example local update locator:

```jsonc
{
  "kind": 39520,
  "content": "{\"schema\":1,\"artifact_role\":\"local_update\",\"base_checkpoint_id\":\"ckpt-171\",\"round_index\":172,\"local_step_count\":500,\"aggregation_weight_basis\":\"tokens\",\"aggregation_weight_value\":\"182340992\",\"format\":\"upd.bundle.v1\",\"manifest_digest\":\"abc123\"}",
  "tags": [
    ["d", "upd-r172-n44"],
    ["network", "psion-trainnet-a"],
    ["window", "psion-r172"],
    ["status", "stored"],
    ["class", "local_update"],
    ["artifact", "upd-r172-n44"],
    ["base_checkpoint", "ckpt-171"],
    ["round", "172"],
    ["local_steps", "500"],
    ["weight", "182340992"],
    ["manifest", "abc123"],
    ["x", "<bundle_digest>"],
    ["url", "https://store.example/upd-r172-n44"]
  ]
}
```

Example accepted aggregate locator:

```jsonc
{
  "kind": 39520,
  "content": "{\"schema\":1,\"artifact_role\":\"aggregate\",\"base_checkpoint_id\":\"ckpt-171\",\"round_index\":172,\"aggregation_rule\":\"weighted_avg\",\"aggregation_weight_basis\":\"tokens\",\"admitted_update_count\":9,\"admitted_weight_total\":\"1468123456\",\"promotion_state\":\"accepted\",\"promoted_checkpoint_id\":\"ckpt-172\",\"manifest_digest\":\"def456\"}",
  "tags": [
    ["d", "agg-r172"],
    ["network", "psion-trainnet-a"],
    ["window", "psion-r172"],
    ["status", "accepted"],
    ["class", "aggregate"],
    ["artifact", "agg-r172"],
    ["aggregate", "agg-r172"],
    ["base_checkpoint", "ckpt-171"],
    ["checkpoint", "ckpt-172"],
    ["promotion", "accepted"],
    ["round", "172"],
    ["manifest", "def456"],
    ["x", "<aggregate_digest>"],
    ["url", "https://store.example/agg-r172"]
  ]
}
```

When `class=score`, the locator SHOULD reference a signed score snapshot file.
That file SHOULD identify:

- whether the snapshot scope is the whole network or one window
- the scoring or scheduling policy revision
- the snapshot time
- the columns included in the file

Each score row SHOULD include at least:

- the subject pubkey
- the validator pubkey when a validator-specific score exists
- accepted-count or accepted-weight facts
- replay-required count
- slashed count
- current scheduling weight or equivalent assignment priority

The score snapshot file stays off Nostr. The locator and its manifest make the
snapshot discoverable, signed, and replayable from relay history.

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
- `["p", "<subject_pubkey>", "<relay>", "subject"]`
- `["p", "<validator_pubkey>", "<relay>", "validator"]`
- `["p", "<coordinator_pubkey>", "<relay>", "coordinator"]`
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

## Recovery And Forking

TRN is designed so large AI training runs can survive partial failure. If a
coordinator, validator service, or operator disappears, another operator can
rebuild the last known public state from the network contract, the latest
window records, receipts, verdicts, and artifact locators on relays.

TRN is also designed so runs can fork cleanly. A new network contract, window,
or artifact locator SHOULD link back to the source network, the source window,
and the accepted checkpoint or weight locators it builds on. That makes it
possible for other participants to see exactly what was reused, what changed,
and where the new run started.

For TRN-DiLoCo, a replacement coordinator or forking operator SHOULD determine
the canonical starting point for the next round by locating the latest window
whose round state is closed and whose accepted aggregate or promoted checkpoint
is linked through signed receipts, validator verdicts, and artifact locators.
When both an accepted aggregate and a promoted checkpoint are published, the
promoted checkpoint SHOULD be treated as the canonical bootstrap artifact for
the next round.

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

### TRN-DiLoCo

TRN-DiLoCo is an optional profile for training systems that perform multiple
local optimizer steps before a less frequent synchronization or aggregation
step.

This profile is useful for DiLoCo-style, local-SGD-style, or other
periodically synchronized training runs where the coordination layer needs to
publish:

- the base checkpoint for one round
- the expected number of local steps or equivalent local-work target
- one or more signed local-update artifact pointers
- the accepted aggregate or promoted checkpoint for the next round
- replay or rejection reasons when a submitted local update is not admitted

TRN-DiLoCo does not define runtime transport, tensor formats, or optimizer
math.

It defines only the metadata needed to make these rounds discoverable,
auditable, recoverable, and forkable from relay history.

Recommended tags for TRN-DiLoCo-enabled networks or windows are:

- `["sync_profile", "diloco"]`
- `["aggregation_rule", "weighted_avg|uniform_avg|custom"]`
- `["aggregation_weight", "tokens|examples|steps|uniform|custom"]`
- `["base_checkpoint", "<checkpoint_id>"]`
- `["round", "<round_index>"]`
- `["planned_local_steps", "<count>"]`
- `["min_participants", "<count>"]`
- `["min_weight_fraction", "<decimal_string>"]`
- `["max_staleness_windows", "<count>"]`

Implementations MAY add additional profile-specific tags, but they SHOULD make
the base checkpoint, round identity, and aggregation weighting rule explicit.

Implementations MAY also publish `inner_optimizer` and `outer_optimizer` in
tags or compact JSON content when those fields are needed to interpret one
round or one network policy, but TRN-DiLoCo does not require one optimizer
pair.

### TRN-Reputation

Implementations MAY use NIP-32 labels for:

- fraud markers
- validator-quality markers
- build-revoked markers
- checkpoint-warning markers
- reputation or trust annotations

Implementations that publish TRN reputation labels SHOULD use one of these
namespaces:

- `trn/contributor`
- `trn/validator`
- `trn/build`
- `trn/checkpoint`

Recommended label values are:

- `trn/contributor`: `good`, `poor`, `quarantined`, `fraud`
- `trn/validator`: `good`, `poor`, `inconsistent`
- `trn/build`: `admitted`, `stale`, `revoked`
- `trn/checkpoint`: `warning`, `revoked`

When a label affects one concrete actor, implementations SHOULD target both:

- the actor pubkey
- the relevant TRN receipt, verdict, artifact locator, or closeout event

That keeps reputation trails queryable by actor and auditable by source event.

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
4a. Trainers perform local work outside Nostr starting from one shared base
checkpoint for the round.
4b. Each trainer publishes a `kind:39511` receipt with
`status=update_submitted` and references one `kind:39520` locator with
`class=local_update`.
4c. Validators or aggregators publish verdicts over submitted local updates.
4d. An aggregator publishes one `kind:39520` locator with `class=aggregate`
for the candidate or accepted merged result.
4e. A coordinator, checkpoint authority, or equivalent publisher emits
`aggregate_accepted` or `checkpoint_promoted` receipts and links the promoted
checkpoint for the next round.
5. Heavy artifacts move outside Nostr.
6. Validators publish `kind:39512` verdicts.
7. Checkpoint, model-weight, proof, or score-snapshot metadata is published
   through `kind:39520`.
8. Final reward, hold, quarantine, or slash status is published through
   `kind:39530`.
9. NIP-32 labels attach public reputation, fraud, validator-quality, build, or
   artifact annotations to the affected actors and source events.
10. If the run needs to resume or fork, a new network or window points back to
   the accepted artifacts and keeps going.

This preserves one shared public coordination trail without pretending the
training itself runs on relays.

## Changelog

- v0: initial umbrella draft for model-training coordination on Nostr with one
  core document and optional discovery, privacy, challenge, and reputation
  profiles
- v0.1: standardized actor attribution with `p` tags, added assignment receipt
  profile guidance, standardized score snapshot publication through artifact
  locators, and defined recommended TRN reputation namespaces and labels
- v0.2: added TRN-DiLoCo profile guidance for local-update rounds, aggregation
  metadata, aggregate finalization, and checkpoint-promotion recovery
