> Status: proposed draft from the All Work NIP program ([`PROPOSED.md`](PROPOSED.md)).
> Layer: 4 — hosts, outcomes, and public trust.

NIP-HP
======

Hosts and Placement
-------------------

`draft` `optional`

This NIP defines where work runs: **Host Records** for every placement
class, **Capacity Statements**, and admitted **Dispatch Decisions**.

It extends the NIP-TRN node-record pattern (`kind:39501`) from training
networks to general Work, and sits in the NIP-SA `39xxx` neighborhood
because hosts, capacity, and dispatch are agent-economics records shared
with the open-market lane.

The boundaries, stated once and enforced everywhere:

> Host reachability is not an execution grant. A capacity advertisement
> is not a Dispatch Decision. A Dispatch Decision authorizes one
> placement under one policy revision — it is not proof the work ran,
> succeeded, or was accepted.

## Kinds

This NIP reserves:

| Kind  | Type        | Description |
| ----- | ----------- | ----------- |
| 39560 | Addressable | Host Record |
| 39561 | Addressable | Capacity Statement |
| 39562 | Addressable (unique `d`) | Dispatch Decision |
| 39563-39579 | — | Reserved for future NIP-HP use |

Host Records and Capacity Statements are signed by the **host owner's**
key (the party operating the placement); Dispatch Decisions are signed by
the All Work authority that admitted the dispatch.

## 1. Host Record (`kind:39560`)

One identified placement. Address:

```text
39560:<owner_pubkey>:<host_ref>
```

### 1.1 Required tags

- `d`: stable `host_ref`
- `kind_label`: placement class — `local_machine`, `remote_host`,
  `sandbox`, `pylon`, `agent_computer`, `ci_worker`,
  `production_target`, or deployment-declared
- `generation`: monotonic placement generation (reprovisioning bumps it)
- `status`: `online`, `degraded`, `offline`, or `revoked`
- `observed_at`: when the status was last verified
- `revision`, `published_at`

### 1.2 Recommended tags

- `p` with marker `owner`: the operating principal (defaults to the
  signer)
- `cap`: repeated capability declarations
  (`["cap", "backend", "cuda"]`, `["cap", "containment", "sandbox_v2"]`,
  `["cap", "network", "egress_restricted"]`)
- `a` with marker `grant`: grant refs admitted for this host
- `org`: Organizations this host serves
- `relay`: coordination relay hints
- `env`: environment-identity ref (image, profile revision) for managed
  placements

A Host Record from an unknown owner key is discoverable inventory, not a
usable placement: use requires the Organization's policy to admit the
host owner and the NIP-AD grant to name the host.

### 1.3 Rules

- **Generation fences placements.** A Session bound to generation `n` is
  fenced when the host reprovisions to `n+1`; late results from the old
  generation cannot mutate current state (the NIP-AS/WI generation rules
  apply transitively).
- **Health is observed, not assumed.** `status` carries `observed_at`;
  a stale `online` renders as unknown. Cached reachability is never
  current health.
- **Production is explicit.** `production_target` hosts are an authority
  change the product must show before Work is created there; policies
  SHOULD require distinct grants for them.

## 2. Capacity Statement (`kind:39561`)

Typed availability for a host or fleet. Address:

```text
39561:<owner_pubkey>:<host_ref>:cap
```

Required tags: `d`, `a` with marker `host`, `class` (the execution/
workload class the capacity applies to), counted slots as
`["slots", "<available>", "<ready>", "<busy>", "<queued>"]`,
`observed_at`, `published_at`.

Recommended: `window` (validity window), `flex` (flexibility class —
`fixed`, `deferrable`, `interruptible`, `opportunistic`, `preemptible` —
with checkpoint/resume policy refs when interruptible), and `cost` class
refs.

A Capacity Statement is routing evidence with freshness. It cannot
reserve, promise, or grant anything, and dispatch against expired
capacity refuses rather than assumes.

## 3. Dispatch Decision (`kind:39562`)

The admitted choice to start, defer, refuse, interrupt, or place Work.
Address:

```text
39562:<authority_pubkey>:<dispatch_ref>
```

### 3.1 Required tags

- `d`: unique `dispatch_ref`
- `org`, `work`
- `decision`: `place`, `defer`, `refuse`, `interrupt`, or `migrate`
- `a` with marker `host`: the selected Host Record (for `place` /
  `migrate`)
- `policy`: the exact dispatch-policy revision applied
- `e`/`a` with marker `input`: the inputs the decision consumed —
  capacity statements, health observations, budget state, workload-class
  refs
- `p` with marker `actor`: the requesting principal
- `decided_at`, `published_at`

### 3.2 Recommended tags

- `a` with marker `session`: the NIP-AS Session the placement produced
- `reason`: typed reason on `defer` / `refuse` / `interrupt`
- `a` with marker `predecessor`: the prior decision on `migrate`

### 3.3 Rules

- A recommendation, schedule, price signal, or capacity row is not a
  Dispatch Decision; only this authority-signed record is.
- Interruption and curtailment produce a typed outcome — defer,
  checkpoint, interrupted session, migration, or failure — never silent
  loss or false completion.
- Energy- or cost-aware inputs are named `input` refs like any other;
  a decision citing measured energy data names its provenance, and a
  modeled input is labeled modeled.

## Security considerations

- **Reachability-as-authority.** The recurring local-compute failure this
  NIP fences: possessing a port, a tunnel, or an enrolled daemon is not
  authority. Every execution still requires the grant naming the host and
  admission naming the generation.
- **Capacity spoofing.** Capacity is owner-signed advertising; dispatch
  policies weigh a host owner's measured delivery history, not its
  self-reported slots.
- **Host fingerprinting.** Capability tags reveal infrastructure. Private
  fleets publish Host Records to restricted relays; public market
  participation is a deliberate disclosure.
- **Production blast radius.** `production_target` records plus distinct
  grants make the dangerous placements enumerable and auditable — the
  inventory itself is a safety surface.

## References

- NIP-01
- NIP-WK, NIP-WI (layer 0); NIP-AD, NIP-AS (layer 2)
- NIP-TRN `kind:39501` — the node-record pattern this generalizes
- NIP-SA — sovereign-agent runners; NIP-AC — funding spendful placement
- `docs/omega/GLOSSARY.md` — Host, Capacity, Placement, Dispatch
  Policy, Dispatch Decision, Flexibility Class

## Changelog

**v0 (2026-08-03)**

- Initial proposed draft: owner-signed Host Records and Capacity
  Statements, authority-signed Dispatch Decisions, generation fencing,
  and the reachability/capacity/decision boundaries.
