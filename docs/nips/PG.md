> Status: proposed draft from the All Work NIP program ([`PROPOSED.md`](PROPOSED.md)).
> Layer: 1 — planning.

NIP-PG
======

Planning Graph
--------------

`draft` `optional`

This NIP defines the portfolio layer above Work: **Initiatives**,
**Roadmaps**, **Projects** with configured **Project Statuses**, **Project
Milestones**, **Cycles**, and authored **Updates**.

Two rules bound the whole layer:

1. **Every layer is optional per Work Domain.** Incidents, research,
   operations, service, and data Work can use the same portfolio views
   without pretending to be product development, and none of them is
   required to use any layer.
2. **Planning is context, not authority.** A Project date, Cycle
   membership, Roadmap position, or health color supplies priority
   context. It does not grant implementation, release, spend, or
   external-action authority, and progress percentages are projections.

## Kinds

This NIP reserves:

| Kind  | Type        | Description |
| ----- | ----------- | ----------- |
| 32220 | Addressable | Initiative |
| 32221 | Addressable | Roadmap |
| 32222 | Addressable | Project |
| 32223 | Addressable | Project Status definition |
| 32224 | Addressable | Project Milestone |
| 32225 | Addressable | Cycle |
| 32226 | Addressable (unique `d`) | Project Update |
| 32227 | Addressable (unique `d`) | Initiative Update |
| 32228-32239 | — | Reserved for future NIP-PG use |

All are authority-signed. Address form is
`<kind>:<authority_pubkey>:<record_ref>` throughout. Mutations flow through
NIP-WI intents (`project.create`, `project.update`, and analogous
operations extend the NIP-WI vocabulary).

## 1. Initiative (`kind:32220`)

A strategic collection of Projects and outcome metrics across applicable
Work Domains.

Required tags: `d`, `org`, `name`, `status` (`planned`, `active`,
`completed`, `canceled`), `revision`, `published_at`.

Recommended tags: `p` with markers `owner` and `lead`; `a` with markers
`project` (member Projects), `parent` (parent Initiative for
sub-initiatives), `document`; `metric` refs naming Outcome Metric
definitions (definition, source, window — a metric without those facts is
display only); `target`: horizon timestamp; `health`: `on_track`,
`at_risk`, `off_track` — a display echo of the latest Initiative Update,
never independent truth.

## 2. Roadmap (`kind:32221`)

An ordered statement of intended outcomes and sequencing.

Required tags: `d`, `org`, `name`, `revision`, `published_at`.
Recommended: `p` with marker `owner`; repeated `a` with marker `entry`
carrying Initiative or Project addresses, where tag order is the roadmap
order.

A Roadmap supplies priority context only. Nothing on a Roadmap is admitted
Work, and roadmap position cannot be cited as authority for any action.

## 3. Project (`kind:32222`)

A bounded collection of Work toward an outcome.

### 3.1 Required tags

- `d`, `org`, `name`
- `status`: a Project Status ref (`kind:32223`)
- `revision`, `published_at`

### 3.2 Recommended tags

- `p` with markers `owner`, `lead`, `member`
- `team`: repeated contributing Team refs
- `a` with markers `initiative`, `milestone`, `document`, `need`,
  `workroom` (NIP-OT Workroom Binding), `update` (latest Project Update)
- `start` and `target`: date timestamps
- `progress`: completed/total Work counts as `"<done>/<total>"` — a
  projection with the freshness of its inputs
- `t`: discovery topics

Work joins a Project through its own refs: the NIP-WK Work Record and
NIP-PI Issue carry `a`-refs marked `project`. The Project record does not
enumerate every Work ref at scale; clients query
`{"kinds":[32200],"#a":["32222:<authority>:<project_ref>"]}`-style filters
instead. A Project is not the universal Work object and not an IDE project
graph.

## 4. Project Status (`kind:32223`)

Organization-scoped configured lifecycle states for Projects, mirroring
the NIP-WS pattern: required `d`, `org`, `name`, `category` (`backlog`,
`planned`, `started`, `paused`, `completed`, `canceled`), `position`,
`revision`. A Project Status is distinct from a Project Update: one is
configured lifecycle state, the other is an authored report.

## 5. Project Milestone (`kind:32224`)

A named checkpoint within a Project. Required tags: `d`, `org`, `a` with
marker `project`, `name`, `revision`, `published_at`. Recommended:
`target` date, `status` (`planned`, `reached`, `missed`, `canceled`),
`progress`.

Use the exact term Project Milestone: it is a planning checkpoint, and it
is not a release, billing, or verification milestone. `reached` is a
planning statement; outcome truth stays with the member Work's NIP-EV
records.

## 6. Cycle (`kind:32225`)

An optional time-boxed planning interval for a Team. Required tags: `d`,
`org`, `team`, `number` (dense per-Team counter), `start`, `end`,
`revision`, `published_at`. Recommended: `name`, `state` (`upcoming`,
`active`, `completed`), `progress`, and `a` with marker `carryover`
naming the successor Cycle that unfinished Work rolled into.

Work joins a Cycle by its own `cycle` ref, the same pattern as Projects.
Cycle rollover is an explicit NIP-WI batch operation; nothing rolls over
silently.

## 7. Project Update (`kind:32226`) and Initiative Update (`kind:32227`)

Dated authored health reports — the human voice of the planning layer.
Address: `<kind>:<authority_pubkey>:<subject_ref>:upd:<n>`.

### 7.1 Required tags

- `d`: `<subject_ref>:upd:<n>` (unique, append-only)
- `org`
- `a` with marker `subject`: the Project or Initiative
- `p` with marker `author`: the reporting principal
- `health`: `on_track`, `at_risk`, or `off_track`
- `published_at`

### 7.2 Recommended tags

- `x`: digest of the exact body bytes
- `e`/`a` with marker `evidence`: NIP-EV refs the report cites
- `metric`: metric observations the report cites

`content` carries the report body (public-safe or NIP-44 encrypted to the
Organization audience). An Update is an authored claim by its author. It
can cite evidence; it is not canonical Work state, verification, or
acceptance, and a green health value is a statement of belief, not a
record of proof.

## Security considerations

- **Health theater.** `health` and `progress` values are the most
  overclaim-prone fields in this NIP. Clients MUST render them as authored
  or projected values with source and freshness, never merge them into
  evidence displays.
- **Planning-as-pressure injection.** Target dates and roadmap positions
  feed agent context. They are data; an agent citing a deadline still has
  no authority beyond its grants.
- **Cross-team visibility.** Projects spanning Teams expose each Team's
  Work refs to the Project audience; authorities publish cross-team
  Projects only to relays every member Team's audience may read.

## References

- NIP-01, NIP-44
- NIP-WK, NIP-WI, NIP-OT (layer 0)
- NIP-PI, NIP-WS, NIP-RP (this layer)
- `docs/omega/2026-08-03-canonical-all-work-planning-authority.md`

## Changelog

**v0 (2026-08-03)**

- Initial proposed draft: Initiative, Roadmap, Project, Project Status,
  Project Milestone, Cycle, and authored Updates with the
  planning-is-not-authority boundary.
