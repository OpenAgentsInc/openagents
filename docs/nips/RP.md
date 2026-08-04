> Status: proposed draft from the All Work NIP program ([`PROPOSED.md`](PROPOSED.md)).
> Layer: 1 — planning.

NIP-RP
======

Release Planning
----------------

`draft` `optional`

This NIP defines release **planning** records: Release Planning Records,
Release Pipelines, Release Stages, and Release Scope Links.

The boundary is the entire point of this NIP:

> Nothing in this NIP is a Release. A pipeline flagged production, a stage
> named `published`, a scope list, a progress row, or a target commit is
> planning data. It cannot create a Release Candidate, pass a Deployment
> Gate, authorize publication or distribution, or satisfy release evidence.
> Actual releases live with the release authority and its gates, and public
> capability claims live with the Product Promise registry (future NIP-PP).

Planning data is still worth signing: it lets every client and agent see
what is intended to ship, when, and containing which Work — portable,
attributable, and auditable — without ever confusing intention with
authorization.

## Kinds

This NIP reserves:

| Kind  | Type        | Description |
| ----- | ----------- | ----------- |
| 32240 | Addressable | Release Planning Record |
| 32241 | Addressable | Release Pipeline |
| 32242 | Addressable | Release Stage definition |
| 32243 | Addressable (unique `d`) | Release Scope Link |
| 32244-32249 | — | Reserved for future NIP-RP use |

All are authority-signed, mutated through NIP-WI intents.

## 1. Release Planning Record (`kind:32240`)

The planning projection for one intended product version. Address:

```text
32240:<authority_pubkey>:<release_planning_ref>
```

### 1.1 Required tags

- `d`: stable `release_planning_ref`
- `org`
- `name`: version or release name
- `a` with marker `pipeline`: the owning Release Pipeline
- `stage`: the current Release Stage ref
- `revision`, `published_at`

### 1.2 Recommended tags

- `target`: target date timestamp
- `commit`: target commit or artifact ref — an identifier of intent, not
  proof the artifact exists or passed anything
- `a` with markers `document`, `note`: linked NIP-DD Documents and notes
- `progress`: scoped-Work completion projection `"<done>/<total>"`
- `t`: discovery topics

`content` MAY carry bounded public-safe release notes text or a NIP-44
payload for the Organization audience.

## 2. Release Pipeline (`kind:32241`)

An ordered planning workflow for Release Planning Records. Address:
`32241:<authority_pubkey>:<pipeline_ref>`.

Required tags: `d`, `org`, `name`, ordered `a` refs with marker `stage`,
`revision`, `published_at`. Recommended: `team` refs, and
`production_presentation`: `"true"` when the pipeline presents itself as
production-facing.

`production_presentation` is exactly what its name says — presentation. It
changes styling and warning affordances in clients. It grants nothing,
gates nothing, and MUST NOT be read by any automation as deployment or
release eligibility.

## 3. Release Stage (`kind:32242`)

One coarse planning state within a Pipeline. Address:
`32242:<authority_pubkey>:<pipeline_ref>:<stage_ref>`.

Required tags: `d` (`<pipeline_ref>:<stage_ref>`), `org`, `name`,
`category` (`planned`, `started`, `completed`, `canceled`), `position`,
`revision`.

A stage's display name is unconstrained — teams will name stages
`candidate`, `verification`, `published`. The `category` is the only
machine-meaningful value, and it is a planning category. A stage named
`published` with `category=completed` states that planning considers the
record done; whether anything was actually published is answerable only by
the release authority's own records and NIP-EV evidence.

## 4. Release Scope Link (`kind:32243`)

A typed planning relation between Work and a Release Planning Record.
Address: `32243:<authority_pubkey>:<scope_link_ref>`.

### 4.1 Required tags

- `d`: unique `scope_link_ref`
- `org`
- `a` with marker `release`: the Release Planning Record
- `a` with marker `work`: the scoped Work (or its Issue projection)
- `inclusion`: `intended`, `committed`, `deferred`, or `removed`
- `p` with marker `actor`
- `published_at`

### 4.2 Rules

- A scope link means **intended inclusion only**. The live release
  contract, its gates, and the artifact identity decide what actually
  ships; a `committed` link that never shipped is a planning miss, not a
  contradiction in canonical state.
- Scope changes are append-visible: moving Work out of a release
  republishes the link with `inclusion=deferred` or `removed` rather than
  deleting it, so scope history survives.
- The NIP-PI Issue projection surfaces its release refs with marker
  `release`, so trackers can render release columns without new queries.

## Relationship to actual release records

When a real release occurs under the release authority, its evidence chain
lives in NIP-EV (evidence and verification of the artifact and gates) and
its public claim, if any, in the Product Promise registry (future NIP-PP).
A Release Planning Record MAY then gain an `a` ref with marker `outcome`
pointing at that evidence — the planning record referencing reality, never
substituting for it.

## Security considerations

- **The word "release".** Every field in this NIP is one adjective away
  from an overclaim. Clients MUST label these surfaces as planning, and
  automations MUST NOT branch on stage names, pipeline flags, or scope
  inclusion to perform deploy, publish, tag, announce, or settle actions.
- **Target-commit trust.** `commit` refs are unverified intent. Supply
  chains resolve artifacts through their own signed provenance, never
  through a planning tag.
- **Scope leakage.** Release scope reveals unshipped intent. Private
  Organizations keep planning records on restricted relays; public
  roadcasting of scope is a deliberate disclosure decision.

## References

- NIP-01, NIP-44
- NIP-WK, NIP-WI, NIP-EV (layer 0)
- NIP-PI, NIP-PG, NIP-DD (this layer)
- NIP-PP (layer 4) — the public Product Promise registry
- `docs/omega/GLOSSARY.md` — Release Planning Record, Release Pipeline,
  Release Stage, Release Scope Link boundary terms

## Changelog

**v0 (2026-08-03)**

- Initial proposed draft: Release Planning Record, Pipeline, Stage, and
  Scope Link with the planning-never-authorizes-release boundary.
