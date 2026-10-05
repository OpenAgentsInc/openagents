# Terminal and Workbench issue roadmap

Status: complete backlog published on October 5, 2026. These issues describe
planned work; filing them is not implementation or release evidence.

The public [OpenAgents Terminal and Workbench project](https://github.com/orgs/OpenAgentsInc/projects/20)
tracks the entire [workbench roadmap](workbench-roadmap.md) and its
[smart terminal specification](smart-terminal.md). All 90 issues also belong
to the required [OpenAgents board](https://github.com/orgs/OpenAgentsInc/projects/19).
The first three issues are the deployable Grid and standalone MVP. The
remaining 87 issues include 16 conditional research or later market profiles.

## First deployable release

Only these issues gate today's October 5 release target:

| Issue | Deliverable | Completion blockers |
| --- | --- | --- |
| [#10642](https://github.com/OpenAgentsInc/openagents/issues/10642) | Shared real shell, blocks, requests, context, and typed pending proposals | None; agree interfaces with #10643 |
| [#10643](https://github.com/OpenAgentsInc/openagents/issues/10643) | Shared Grid application and separately installable native window | None; extraction runs beside #10642 |
| [#10644](https://github.com/OpenAgentsInc/openagents/issues/10644) | Public supported macOS install and retained both-surface demonstration | #10642, #10643 |

The [Today view](https://github.com/orgs/OpenAgentsInc/projects/20/views/2)
contains exactly these three P0 issues. Later milestones do not gate this
release. The MVP preserves local PTYs while hidden; app exit ends them.
Durable attachment and moving between viewer processes arrive later.

## Delivery milestones

Dates after today are an order of delivery, not deadlines. Ready contracts
can advance in parallel with the first application work. Issues use native
GitHub blockers, mirrored in both projects' **Blocked by** fields. Related
integrations are linked in issue bodies without imposing completion blockers.

| Delivery | Issues | Milestone |
| --- | --- | --- |
| Today MVP | 3 | [Terminal 01: Grid and standalone MVP](https://github.com/OpenAgentsInc/openagents/milestone/6) |
| Next: Everglade | 8 | [Terminal 02: Everglade workshop](https://github.com/OpenAgentsInc/openagents/milestone/7) |
| Later: host sessions | 10 | [Terminal 03: durable host sessions](https://github.com/OpenAgentsInc/openagents/milestone/8) |
| Later: workbench | 18 | [Terminal 04: all-work panes and reuse](https://github.com/OpenAgentsInc/openagents/milestone/9) |
| Later: sharing | 6 | [Terminal 05: multiple hosts and sharing](https://github.com/OpenAgentsInc/openagents/milestone/10) |
| Later: paid cloud | 19 | [Terminal 06: paid cloud and credits](https://github.com/OpenAgentsInc/openagents/milestone/11) |
| Later: clients | 7 | [Terminal 07: device and platform clients](https://github.com/OpenAgentsInc/openagents/milestone/12) |
| Later: world | 3 | [Terminal 08: world screens and useful work](https://github.com/OpenAgentsInc/openagents/milestone/13) |
| Optional research | 16 | [Terminal 09: conditional enhancements](https://github.com/OpenAgentsInc/openagents/milestone/14) |

Project views also show [the next Everglade pass](https://github.com/orgs/OpenAgentsInc/projects/20/views/3),
[everything after the MVP](https://github.com/orgs/OpenAgentsInc/projects/20/views/4),
[paid cloud and credits](https://github.com/orgs/OpenAgentsInc/projects/20/views/5),
[conditional research](https://github.com/orgs/OpenAgentsInc/projects/20/views/6),
and the [status board](https://github.com/orgs/OpenAgentsInc/projects/20/views/7).
Each item has Delivery, Workstream, Priority, Size, Scope, and Status fields.
The repository milestone matches Delivery.

## Dependency decisions

- The next Everglade pass uses existing studio intents. Its later router
  adapter and the optional public HTTP API do not gate workshop integration.
- Shared resource identities precede pane adapters. Neither contract needs
  the entire durable-terminal implementation first.
- Host arbitration can be checked without a graphical viewer. Enrolled
  phone and browser baselines do not wait for live watch/drive sharing.
- Existing granted remote-task execution does not depend on the new
  terminal snapshot protocol. The router extends admission over that owner.
- Paid retail contracts, account binding, pricing, and funding can advance
  independently of app release. Provisioning requires an admitted quote and
  durable reservation; settlement preserves unknown liabilities.
- Each new route adapter receives its own qualification. One unavailable
  adapter does not postpone evaluation of another.
- Optional classifiers, compression, read-only auto-run, interop, partner
  API, composed execution, and market profiles do not gate the first release.

Paid cloud retains one account and financial owner across products. Credits
are a display over a versioned sats price book, separate from XP, wallet
funds, hosted model inference, and infrastructure-provider billing. The
first paid product supports one computer class and one task class. Later
workers, bids, escrow, training, and contributor payments have separate
contracts and qualification.

## Complete issue directory

The following tables record the initial scope and native prerequisites.
GitHub is authoritative for current issue state and dependencies. Each issue
contains its implementation boundary, acceptance, focused verification,
and links to related work. Existing delivered chat, studio, PTY, phone,
router, knowledge, plugin, Gym, XP, and payment features remain foundations.

### Today MVP

| Issue | Work | Native completion blockers |
| --- | --- | --- |
| [#10642](https://github.com/OpenAgentsInc/openagents/issues/10642) | Terminal MVP 1: shared shell blocks and requests for Grid and standalone | None |
| [#10643](https://github.com/OpenAgentsInc/openagents/issues/10643) | Terminal MVP 2: shared Grid application and standalone native install | None |
| [#10644](https://github.com/OpenAgentsInc/openagents/issues/10644) | Terminal MVP 3: publish the standalone build and retain the Grid demo | [#10642](https://github.com/OpenAgentsInc/openagents/issues/10642), [#10643](https://github.com/OpenAgentsInc/openagents/issues/10643) |

### Next: Everglade

| Issue | Work | Native completion blockers |
| --- | --- | --- |
| [#10645](https://github.com/OpenAgentsInc/openagents/issues/10645) | Workbench: define shared resource references and surface intents | [#10643](https://github.com/OpenAgentsInc/openagents/issues/10643) |
| [#10646](https://github.com/OpenAgentsInc/openagents/issues/10646) | Workbench: add typed product panes and resource navigation | [#10643](https://github.com/OpenAgentsInc/openagents/issues/10643), [#10645](https://github.com/OpenAgentsInc/openagents/issues/10645) |
| [#10647](https://github.com/OpenAgentsInc/openagents/issues/10647) | Everglade: open the shared workbench with workshop context | [#10642](https://github.com/OpenAgentsInc/openagents/issues/10642), [#10643](https://github.com/OpenAgentsInc/openagents/issues/10643), [#10646](https://github.com/OpenAgentsInc/openagents/issues/10646) |
| [#10648](https://github.com/OpenAgentsInc/openagents/issues/10648) | Workbench: project studio goals, tasks, seats, and memory | [#10647](https://github.com/OpenAgentsInc/openagents/issues/10647) |
| [#10649](https://github.com/OpenAgentsInc/openagents/issues/10649) | Workbench: mount studio approvals and exact-revision reviews | [#10647](https://github.com/OpenAgentsInc/openagents/issues/10647) |
| [#10650](https://github.com/OpenAgentsInc/openagents/issues/10650) | Everglade: retain the shared-workbench studio acceptance demo | [#10648](https://github.com/OpenAgentsInc/openagents/issues/10648), [#10649](https://github.com/OpenAgentsInc/openagents/issues/10649) |
| [#10678](https://github.com/OpenAgentsInc/openagents/issues/10678) | Terminal: add isolated bash shell integration | [#10642](https://github.com/OpenAgentsInc/openagents/issues/10642), [#10643](https://github.com/OpenAgentsInc/openagents/issues/10643) |
| [#10679](https://github.com/OpenAgentsInc/openagents/issues/10679) | Terminal: add isolated fish shell integration | [#10642](https://github.com/OpenAgentsInc/openagents/issues/10642), [#10643](https://github.com/OpenAgentsInc/openagents/issues/10643) |

### Later: host sessions

| Issue | Work | Native completion blockers |
| --- | --- | --- |
| [#10651](https://github.com/OpenAgentsInc/openagents/issues/10651) | NIP-TERM: specify authoritative snapshots, history, blocks, and session references | None |
| [#10652](https://github.com/OpenAgentsInc/openagents/issues/10652) | Host sessions: persist resource membership and default layouts | [#10645](https://github.com/OpenAgentsInc/openagents/issues/10645), [#10651](https://github.com/OpenAgentsInc/openagents/issues/10651) |
| [#10653](https://github.com/OpenAgentsInc/openagents/issues/10653) | Host terminals: own authoritative emulation and terminal side effects | [#10651](https://github.com/OpenAgentsInc/openagents/issues/10651) |
| [#10654](https://github.com/OpenAgentsInc/openagents/issues/10654) | coder-vt: serialize and restore screen-first snapshots with continuation | [#10651](https://github.com/OpenAgentsInc/openagents/issues/10651) |
| [#10655](https://github.com/OpenAgentsInc/openagents/issues/10655) | Host terminals: serve snapshot joins and bounded history pages | [#10653](https://github.com/OpenAgentsInc/openagents/issues/10653), [#10654](https://github.com/OpenAgentsInc/openagents/issues/10654) |
| [#10656](https://github.com/OpenAgentsInc/openagents/issues/10656) | Host terminals: journal command blocks and expose bounded reads | [#10642](https://github.com/OpenAgentsInc/openagents/issues/10642), [#10653](https://github.com/OpenAgentsInc/openagents/issues/10653), [#10651](https://github.com/OpenAgentsInc/openagents/issues/10651) |
| [#10657](https://github.com/OpenAgentsInc/openagents/issues/10657) | Terminal app: attach native window and Verse to durable host terminals | [#10644](https://github.com/OpenAgentsInc/openagents/issues/10644), [#10652](https://github.com/OpenAgentsInc/openagents/issues/10652), [#10655](https://github.com/OpenAgentsInc/openagents/issues/10655), [#10656](https://github.com/OpenAgentsInc/openagents/issues/10656) |
| [#10658](https://github.com/OpenAgentsInc/openagents/issues/10658) | Workbench: open an explicitly admitted shell for a studio task | [#10647](https://github.com/OpenAgentsInc/openagents/issues/10647), [#10657](https://github.com/OpenAgentsInc/openagents/issues/10657) |
| [#10669](https://github.com/OpenAgentsInc/openagents/issues/10669) | Router: bind workbench continuations to immutable admission | None |
| [#10699](https://github.com/OpenAgentsInc/openagents/issues/10699) | Router: admit work on an explicitly granted remote computer | [#10669](https://github.com/OpenAgentsInc/openagents/issues/10669) |

### Later: workbench

| Issue | Work | Native completion blockers |
| --- | --- | --- |
| [#10659](https://github.com/OpenAgentsInc/openagents/issues/10659) | Workbench: render shared chat threads as native panes | [#10642](https://github.com/OpenAgentsInc/openagents/issues/10642), [#10646](https://github.com/OpenAgentsInc/openagents/issues/10646) |
| [#10660](https://github.com/OpenAgentsInc/openagents/issues/10660) | Workbench: inspect managed runs and their child tasks | [#10646](https://github.com/OpenAgentsInc/openagents/issues/10646) |
| [#10661](https://github.com/OpenAgentsInc/openagents/issues/10661) | Workbench: add bounded file, diff, artifact, and preview panes | [#10646](https://github.com/OpenAgentsInc/openagents/issues/10646) |
| [#10662](https://github.com/OpenAgentsInc/openagents/issues/10662) | Workbench: inspect cited knowledge and studio memory | [#10646](https://github.com/OpenAgentsInc/openagents/issues/10646) |
| [#10663](https://github.com/OpenAgentsInc/openagents/issues/10663) | Workbench: inspect Gym studies, trials, and paired comparisons | [#10646](https://github.com/OpenAgentsInc/openagents/issues/10646) |
| [#10664](https://github.com/OpenAgentsInc/openagents/issues/10664) | Workbench: discover, inspect, test, and reuse admitted components | [#10646](https://github.com/OpenAgentsInc/openagents/issues/10646), [#10663](https://github.com/OpenAgentsInc/openagents/issues/10663) |
| [#10665](https://github.com/OpenAgentsInc/openagents/issues/10665) | Workbench: project the existing capability create/evaluate/publish flow | [#10664](https://github.com/OpenAgentsInc/openagents/issues/10664), [#10660](https://github.com/OpenAgentsInc/openagents/issues/10660), [#10661](https://github.com/OpenAgentsInc/openagents/issues/10661) |
| [#10666](https://github.com/OpenAgentsInc/openagents/issues/10666) | Workbench: turn retained runs into cited knowledge candidates | [#10662](https://github.com/OpenAgentsInc/openagents/issues/10662), [#10660](https://github.com/OpenAgentsInc/openagents/issues/10660), [#10663](https://github.com/OpenAgentsInc/openagents/issues/10663) |
| [#10667](https://github.com/OpenAgentsInc/openagents/issues/10667) | Knowledge: admit exact candidates from prospectively reviewed transfer evidence | [#10666](https://github.com/OpenAgentsInc/openagents/issues/10666) |
| [#10668](https://github.com/OpenAgentsInc/openagents/issues/10668) | Workbench: inspect and control existing background rules | [#10646](https://github.com/OpenAgentsInc/openagents/issues/10646) |
| [#10670](https://github.com/OpenAgentsInc/openagents/issues/10670) | Router: dispatch admitted capability and model routes from the workbench | [#10669](https://github.com/OpenAgentsInc/openagents/issues/10669), [#10645](https://github.com/OpenAgentsInc/openagents/issues/10645) |
| [#10671](https://github.com/OpenAgentsInc/openagents/issues/10671) | Workbench: retain a checked noncoding task across its panes | [#10659](https://github.com/OpenAgentsInc/openagents/issues/10659), [#10660](https://github.com/OpenAgentsInc/openagents/issues/10660), [#10661](https://github.com/OpenAgentsInc/openagents/issues/10661), [#10664](https://github.com/OpenAgentsInc/openagents/issues/10664), [#10670](https://github.com/OpenAgentsInc/openagents/issues/10670) |
| [#10672](https://github.com/OpenAgentsInc/openagents/issues/10672) | Workbench: connect reusable contributions to checks, adoption, and receipts | [#10665](https://github.com/OpenAgentsInc/openagents/issues/10665), [#10663](https://github.com/OpenAgentsInc/openagents/issues/10663) |
| [#10698](https://github.com/OpenAgentsInc/openagents/issues/10698) | Workbench: project shared routes, offers, and task lifecycle | [#10643](https://github.com/OpenAgentsInc/openagents/issues/10643), [#10645](https://github.com/OpenAgentsInc/openagents/issues/10645) |
| [#10700](https://github.com/OpenAgentsInc/openagents/issues/10700) | Router: add an admitted Agent Studio route adapter | [#10648](https://github.com/OpenAgentsInc/openagents/issues/10648), [#10669](https://github.com/OpenAgentsInc/openagents/issues/10669), [#10649](https://github.com/OpenAgentsInc/openagents/issues/10649) |
| [#10702](https://github.com/OpenAgentsInc/openagents/issues/10702) | Router: retain held-out evidence for workbench route extensions | [#10669](https://github.com/OpenAgentsInc/openagents/issues/10669) |
| [#10730](https://github.com/OpenAgentsInc/openagents/issues/10730) | Terminal: preview multiline prompt paste before sending it | [#10642](https://github.com/OpenAgentsInc/openagents/issues/10642), [#10643](https://github.com/OpenAgentsInc/openagents/issues/10643) |
| [#10731](https://github.com/OpenAgentsInc/openagents/issues/10731) | Terminal blocks: search commands and output by status and directory | [#10642](https://github.com/OpenAgentsInc/openagents/issues/10642), [#10643](https://github.com/OpenAgentsInc/openagents/issues/10643) |

### Later: sharing

| Issue | Work | Native completion blockers |
| --- | --- | --- |
| [#10675](https://github.com/OpenAgentsInc/openagents/issues/10675) | Host terminals: enforce one typist with explicit take and release | [#10651](https://github.com/OpenAgentsInc/openagents/issues/10651), [#10653](https://github.com/OpenAgentsInc/openagents/issues/10653) |
| [#10676](https://github.com/OpenAgentsInc/openagents/issues/10676) | NIP-TERM: issue and enforce terminal-scoped watch and drive shares | [#10675](https://github.com/OpenAgentsInc/openagents/issues/10675), [#10655](https://github.com/OpenAgentsInc/openagents/issues/10655), [#10656](https://github.com/OpenAgentsInc/openagents/issues/10656) |
| [#10680](https://github.com/OpenAgentsInc/openagents/issues/10680) | Workbench sessions: attach panes across admitted hosts | [#10657](https://github.com/OpenAgentsInc/openagents/issues/10657), [#10652](https://github.com/OpenAgentsInc/openagents/issues/10652) |
| [#10681](https://github.com/OpenAgentsInc/openagents/issues/10681) | Terminal app: show viewers and pause or revoke live sharing | [#10676](https://github.com/OpenAgentsInc/openagents/issues/10676), [#10657](https://github.com/OpenAgentsInc/openagents/issues/10657) |
| [#10682](https://github.com/OpenAgentsInc/openagents/issues/10682) | Host terminals: hand the typist role to an admitted agent | [#10675](https://github.com/OpenAgentsInc/openagents/issues/10675), [#10656](https://github.com/OpenAgentsInc/openagents/issues/10656), [#10657](https://github.com/OpenAgentsInc/openagents/issues/10657) |
| [#10697](https://github.com/OpenAgentsInc/openagents/issues/10697) | Terminal blocks: share a consented static excerpt | [#10642](https://github.com/OpenAgentsInc/openagents/issues/10642), [#10656](https://github.com/OpenAgentsInc/openagents/issues/10656), [#10645](https://github.com/OpenAgentsInc/openagents/issues/10645) |

### Later: paid cloud

| Issue | Work | Native completion blockers |
| --- | --- | --- |
| [#10704](https://github.com/OpenAgentsInc/openagents/issues/10704) | Cloud: freeze the first retail computer and task contract | None |
| [#10705](https://github.com/OpenAgentsInc/openagents/issues/10705) | Cloud: bind purchased compute to one shared customer account | [#10704](https://github.com/OpenAgentsInc/openagents/issues/10704) |
| [#10706](https://github.com/OpenAgentsInc/openagents/issues/10706) | Cloud: publish a versioned sats price book and credit display | [#10704](https://github.com/OpenAgentsInc/openagents/issues/10704) |
| [#10707](https://github.com/OpenAgentsInc/openagents/issues/10707) | Cloud: credit Lightning top-ups to the shared compute balance | [#10705](https://github.com/OpenAgentsInc/openagents/issues/10705), [#10706](https://github.com/OpenAgentsInc/openagents/issues/10706) |
| [#10708](https://github.com/OpenAgentsInc/openagents/issues/10708) | Cloud: enforce observation, execution, disclosure, and spending independently | [#10669](https://github.com/OpenAgentsInc/openagents/issues/10669), [#10704](https://github.com/OpenAgentsInc/openagents/issues/10704), [#10706](https://github.com/OpenAgentsInc/openagents/issues/10706), [#10705](https://github.com/OpenAgentsInc/openagents/issues/10705) |
| [#10709](https://github.com/OpenAgentsInc/openagents/issues/10709) | Cloud: quote exact rented-computer work through shared offers | [#10706](https://github.com/OpenAgentsInc/openagents/issues/10706), [#10708](https://github.com/OpenAgentsInc/openagents/issues/10708) |
| [#10710](https://github.com/OpenAgentsInc/openagents/issues/10710) | Cloud: reserve prepaid compute funds before provisioning | [#10705](https://github.com/OpenAgentsInc/openagents/issues/10705), [#10706](https://github.com/OpenAgentsInc/openagents/issues/10706), [#10709](https://github.com/OpenAgentsInc/openagents/issues/10709) |
| [#10711](https://github.com/OpenAgentsInc/openagents/issues/10711) | Cloud: provision one admitted retail computer from Boat or GCE | [#10704](https://github.com/OpenAgentsInc/openagents/issues/10704), [#10709](https://github.com/OpenAgentsInc/openagents/issues/10709), [#10710](https://github.com/OpenAgentsInc/openagents/issues/10710) |
| [#10712](https://github.com/OpenAgentsInc/openagents/issues/10712) | Cloud: deliver only admitted source and provider credentials | [#10708](https://github.com/OpenAgentsInc/openagents/issues/10708), [#10711](https://github.com/OpenAgentsInc/openagents/issues/10711) |
| [#10713](https://github.com/OpenAgentsInc/openagents/issues/10713) | Cloud: dispatch one funded task and stream its retained execution | [#10710](https://github.com/OpenAgentsInc/openagents/issues/10710), [#10712](https://github.com/OpenAgentsInc/openagents/issues/10712), [#10709](https://github.com/OpenAgentsInc/openagents/issues/10709) |
| [#10714](https://github.com/OpenAgentsInc/openagents/issues/10714) | Cloud: meter retail compute and provider usage against quoted scope | [#10706](https://github.com/OpenAgentsInc/openagents/issues/10706), [#10713](https://github.com/OpenAgentsInc/openagents/issues/10713) |
| [#10715](https://github.com/OpenAgentsInc/openagents/issues/10715) | Cloud: retain declared artifacts and stop orphaned retail computers | [#10713](https://github.com/OpenAgentsInc/openagents/issues/10713), [#10714](https://github.com/OpenAgentsInc/openagents/issues/10714) |
| [#10716](https://github.com/OpenAgentsInc/openagents/issues/10716) | Cloud: acknowledge cancellation and revocation with final charge state | [#10713](https://github.com/OpenAgentsInc/openagents/issues/10713), [#10715](https://github.com/OpenAgentsInc/openagents/issues/10715), [#10714](https://github.com/OpenAgentsInc/openagents/issues/10714), [#10708](https://github.com/OpenAgentsInc/openagents/issues/10708) |
| [#10717](https://github.com/OpenAgentsInc/openagents/issues/10717) | Cloud: reconcile funded execution after crashes and provider loss | [#10713](https://github.com/OpenAgentsInc/openagents/issues/10713), [#10714](https://github.com/OpenAgentsInc/openagents/issues/10714), [#10715](https://github.com/OpenAgentsInc/openagents/issues/10715), [#10716](https://github.com/OpenAgentsInc/openagents/issues/10716) |
| [#10718](https://github.com/OpenAgentsInc/openagents/issues/10718) | Cloud: settle measured charges and release unused balance holds | [#10710](https://github.com/OpenAgentsInc/openagents/issues/10710), [#10714](https://github.com/OpenAgentsInc/openagents/issues/10714), [#10717](https://github.com/OpenAgentsInc/openagents/issues/10717) |
| [#10719](https://github.com/OpenAgentsInc/openagents/issues/10719) | Workbench: show shared compute balance, quotes, usage, and receipts | [#10707](https://github.com/OpenAgentsInc/openagents/issues/10707), [#10709](https://github.com/OpenAgentsInc/openagents/issues/10709), [#10718](https://github.com/OpenAgentsInc/openagents/issues/10718), [#10698](https://github.com/OpenAgentsInc/openagents/issues/10698) |
| [#10722](https://github.com/OpenAgentsInc/openagents/issues/10722) | Cloud: prove retail funding and recovery with fake payments | [#10719](https://github.com/OpenAgentsInc/openagents/issues/10719), [#10716](https://github.com/OpenAgentsInc/openagents/issues/10716), [#10717](https://github.com/OpenAgentsInc/openagents/issues/10717), [#10718](https://github.com/OpenAgentsInc/openagents/issues/10718) |
| [#10723](https://github.com/OpenAgentsInc/openagents/issues/10723) | Cloud: build funded-qualification tooling and a bounded owner runbook | [#10722](https://github.com/OpenAgentsInc/openagents/issues/10722), [#10707](https://github.com/OpenAgentsInc/openagents/issues/10707) |
| [#10724](https://github.com/OpenAgentsInc/openagents/issues/10724) | Cloud: launch and operate the scoped retail compute service | [#10723](https://github.com/OpenAgentsInc/openagents/issues/10723) |

### Later: clients

| Issue | Work | Native completion blockers |
| --- | --- | --- |
| [#10683](https://github.com/OpenAgentsInc/openagents/issues/10683) | Phone workbench: attach saved sessions, blocks, threads, and pending proposals | [#10652](https://github.com/OpenAgentsInc/openagents/issues/10652), [#10655](https://github.com/OpenAgentsInc/openagents/issues/10655), [#10656](https://github.com/OpenAgentsInc/openagents/issues/10656), [#10675](https://github.com/OpenAgentsInc/openagents/issues/10675), [#10642](https://github.com/OpenAgentsInc/openagents/issues/10642) |
| [#10684](https://github.com/OpenAgentsInc/openagents/issues/10684) | Phone terminal: mount the shared glyph grid with native input | [#10683](https://github.com/OpenAgentsInc/openagents/issues/10683), [#10643](https://github.com/OpenAgentsInc/openagents/issues/10643) |
| [#10685](https://github.com/OpenAgentsInc/openagents/issues/10685) | Browser sessions: add a wasm-compatible admitted host transport | [#10651](https://github.com/OpenAgentsInc/openagents/issues/10651), [#10652](https://github.com/OpenAgentsInc/openagents/issues/10652), [#10655](https://github.com/OpenAgentsInc/openagents/issues/10655), [#10656](https://github.com/OpenAgentsInc/openagents/issues/10656) |
| [#10686](https://github.com/OpenAgentsInc/openagents/issues/10686) | Browser workbench: render host sessions and smart-terminal controls | [#10685](https://github.com/OpenAgentsInc/openagents/issues/10685), [#10675](https://github.com/OpenAgentsInc/openagents/issues/10675), [#10643](https://github.com/OpenAgentsInc/openagents/issues/10643), [#10642](https://github.com/OpenAgentsInc/openagents/issues/10642) |
| [#10687](https://github.com/OpenAgentsInc/openagents/issues/10687) | Plain TTY: add hook-only requests and inline pending proposals | [#10642](https://github.com/OpenAgentsInc/openagents/issues/10642), [#10678](https://github.com/OpenAgentsInc/openagents/issues/10678), [#10679](https://github.com/OpenAgentsInc/openagents/issues/10679), [#10656](https://github.com/OpenAgentsInc/openagents/issues/10656) |
| [#10689](https://github.com/OpenAgentsInc/openagents/issues/10689) | Standalone terminal: publish supported Linux window packages | [#10644](https://github.com/OpenAgentsInc/openagents/issues/10644) |
| [#10690](https://github.com/OpenAgentsInc/openagents/issues/10690) | Standalone terminal: publish a checked Windows ConPTY window build | [#10644](https://github.com/OpenAgentsInc/openagents/issues/10644) |

### Later: world

| Issue | Work | Native completion blockers |
| --- | --- | --- |
| [#10673](https://github.com/OpenAgentsInc/openagents/issues/10673) | Workbench: connect contribution evidence to the existing quest and XP ledger | [#10672](https://github.com/OpenAgentsInc/openagents/issues/10672) |
| [#10674](https://github.com/OpenAgentsInc/openagents/issues/10674) | Everglade: teach the first workbench and studio flow with fact-based quests | [#10650](https://github.com/OpenAgentsInc/openagents/issues/10650), [#10673](https://github.com/OpenAgentsInc/openagents/issues/10673) |
| [#10677](https://github.com/OpenAgentsInc/openagents/issues/10677) | Verse: mount admitted workbench views on optional world screens | [#10646](https://github.com/OpenAgentsInc/openagents/issues/10646), [#10657](https://github.com/OpenAgentsInc/openagents/issues/10657), [#10676](https://github.com/OpenAgentsInc/openagents/issues/10676) |

### Optional research

| Issue | Work | Native completion blockers |
| --- | --- | --- |
| [#10688](https://github.com/OpenAgentsInc/openagents/issues/10688) | Optional TTY multiplexer: attach and redraw durable host sessions | [#10657](https://github.com/OpenAgentsInc/openagents/issues/10657), [#10675](https://github.com/OpenAgentsInc/openagents/issues/10675), [#10687](https://github.com/OpenAgentsInc/openagents/issues/10687) |
| [#10691](https://github.com/OpenAgentsInc/openagents/issues/10691) | Optional terminal input: implement progressive Kitty keyboard encoding | [#10643](https://github.com/OpenAgentsInc/openagents/issues/10643) |
| [#10692](https://github.com/OpenAgentsInc/openagents/issues/10692) | Optional host memory: measure and add idle scrollback compression | [#10655](https://github.com/OpenAgentsInc/openagents/issues/10655) |
| [#10693](https://github.com/OpenAgentsInc/openagents/issues/10693) | Optional input routing: measure and integrate a local ambiguous-line classifier | [#10642](https://github.com/OpenAgentsInc/openagents/issues/10642) |
| [#10694](https://github.com/OpenAgentsInc/openagents/issues/10694) | Optional input recovery: offer corrected commands and honest routing reversal | [#10642](https://github.com/OpenAgentsInc/openagents/issues/10642) |
| [#10695](https://github.com/OpenAgentsInc/openagents/issues/10695) | Optional proposals: add scoped read-only auto-run with revocable admission | [#10642](https://github.com/OpenAgentsInc/openagents/issues/10642), [#10656](https://github.com/OpenAgentsInc/openagents/issues/10656), [#10675](https://github.com/OpenAgentsInc/openagents/issues/10675), [#10669](https://github.com/OpenAgentsInc/openagents/issues/10669) |
| [#10696](https://github.com/OpenAgentsInc/openagents/issues/10696) | Optional interop: reassess public Superlogical and libghostty contracts | [#10651](https://github.com/OpenAgentsInc/openagents/issues/10651), [#10654](https://github.com/OpenAgentsInc/openagents/issues/10654), [#10675](https://github.com/OpenAgentsInc/openagents/issues/10675), [#10676](https://github.com/OpenAgentsInc/openagents/issues/10676) |
| [#10701](https://github.com/OpenAgentsInc/openagents/issues/10701) | Router: expose caller-scoped threads, offers, and resumable run reads | [#10669](https://github.com/OpenAgentsInc/openagents/issues/10669) |
| [#10703](https://github.com/OpenAgentsInc/openagents/issues/10703) | Router: admit bounded execution graphs and funded rework | [#10669](https://github.com/OpenAgentsInc/openagents/issues/10669) |
| [#10720](https://github.com/OpenAgentsInc/openagents/issues/10720) | Router: bind x402 and MPP charge funding to one execution identity | [#10709](https://github.com/OpenAgentsInc/openagents/issues/10709), [#10710](https://github.com/OpenAgentsInc/openagents/issues/10710), [#10669](https://github.com/OpenAgentsInc/openagents/issues/10669) |
| [#10721](https://github.com/OpenAgentsInc/openagents/issues/10721) | Router: define and implement metered Lightning session accounting | [#10720](https://github.com/OpenAgentsInc/openagents/issues/10720), [#10718](https://github.com/OpenAgentsInc/openagents/issues/10718) |
| [#10725](https://github.com/OpenAgentsInc/openagents/issues/10725) | Later markets: design and qualify independently paid workers | [#10699](https://github.com/OpenAgentsInc/openagents/issues/10699), [#10718](https://github.com/OpenAgentsInc/openagents/issues/10718) |
| [#10726](https://github.com/OpenAgentsInc/openagents/issues/10726) | Later markets: design bid comparison and provider selection | [#10725](https://github.com/OpenAgentsInc/openagents/issues/10725), [#10702](https://github.com/OpenAgentsInc/openagents/issues/10702) |
| [#10727](https://github.com/OpenAgentsInc/openagents/issues/10727) | Later markets: design escrow, rework, refunds, and disputes | [#10725](https://github.com/OpenAgentsInc/openagents/issues/10725) |
| [#10728](https://github.com/OpenAgentsInc/openagents/issues/10728) | Later markets: design paid training and evaluation work | [#10725](https://github.com/OpenAgentsInc/openagents/issues/10725), [#10703](https://github.com/OpenAgentsInc/openagents/issues/10703) |
| [#10729](https://github.com/OpenAgentsInc/openagents/issues/10729) | Later economy: design evidence-based contributor payments | [#10718](https://github.com/OpenAgentsInc/openagents/issues/10718) |

## Keeping the plan current

Claim an implementation issue before starting it and release the claim when
stopping. Keep both projects current; the existing repository status scripts
target project 19, so project 20 also needs a status update. The
[project-board guide](../project-board.md#terminal-project-updates) records
the fields and commands. Preserve native blockers and distinguish a ready
issue from one selected for immediate implementation.

Close code-complete work after it lands on main, its own checks pass, and
required host deployment is done. Put owner-only payments, credentials,
physical-device checks, or store releases in `NEEDS_OWNER.md`. Qualification
tooling can be complete while those product claims remain unverified; paid
availability remains disabled until actual supported qualification exists.
An owner verification failure opens a new defect issue.

No GitHub workflow or billed automation is part of this project. Acceptance
runs manually or on supported infrastructure, uses isolated scratch hosts,
and retains actual artifacts and limitations. Update this directory when
scope changes; keep the transcripts and dated historical audits intact.
