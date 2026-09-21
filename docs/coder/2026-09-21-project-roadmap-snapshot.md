# Decision Router and Coder project snapshot

Status: read-only planning snapshot for the [TypeSafe-native Coder analysis](typesafe-agent-analysis.md)
and [roadmap](typesafe-agent-roadmap.md). Repository baseline: `9dd4ddab67`.

The review retrieved every item and issue body from
[project 16, Coder as Decision Router Consumer](https://github.com/orgs/OpenAgentsInc/projects/16)
and [project 15, Decision Router](https://github.com/orgs/OpenAgentsInc/projects/15/views/1),
then read issue state and recent implementation comments. Project 15's
**All work** view has no filter, so its 30-item project inventory covers the
requested view. Project 16 has 29 items. Their union contains 44 issues:
14 consumer issues, including its tracker, and 30 service issues; 15 service
issues also appear on the consumer board.

Board and issue metadata retrieved: 2026-09-21 06:25 UTC.

The tables preserve each board's fields as observed. A dash means the issue
is absent from that board. Each board cell is **status / readiness**. An
open issue can contain shipped slices; a Done status does not make a stale
readiness field accurate. This documentation change does not edit board
fields, issue bodies, or acceptance checkboxes.

## Consumer work

| Issue | Issue state | Project 16 | Scope |
| --- | --- | --- | --- |
| [#9501](https://github.com/OpenAgentsInc/openagents/issues/9501) | open | Todo / Tracking | coder: deliver and prove the flagship Decision Router consumer |
| [#9502](https://github.com/OpenAgentsInc/openagents/issues/9502) | open | Todo / Blocked | coder: route all decision calls through a shared Decision Router client |
| [#9503](https://github.com/OpenAgentsInc/openagents/issues/9503) | open | Todo / Blocked | coder: version decision functions and admit workload-specific policies |
| [#9504](https://github.com/OpenAgentsInc/openagents/issues/9504) | closed | Done / Ready | coder: authorize program effects independently of model selection |
| [#9505](https://github.com/OpenAgentsInc/openagents/issues/9505) | open | Todo / Blocked | coder: bind Decision Router receipts and costs into Coder traces |
| [#9506](https://github.com/OpenAgentsInc/openagents/issues/9506) | open | Todo / Blocked | coder: show decision evidence and program progress in both Coder interfaces |
| [#9507](https://github.com/OpenAgentsInc/openagents/issues/9507) | closed | Done / Ready | coder: ingest scoped tracker work into pinned program task sources |
| [#9508](https://github.com/OpenAgentsInc/openagents/issues/9508) | open | Todo / Blocked | coder: enforce task conflicts and measure semantic independence |
| [#9509](https://github.com/OpenAgentsInc/openagents/issues/9509) | open | In Progress / Ready | coder: verify program outcomes and implement bounded review and suite steps |
| [#9510](https://github.com/OpenAgentsInc/openagents/issues/9510) | open | Todo / Blocked | coder: persist program runs with cancellation, recovery, and whole-run budgets |
| [#9511](https://github.com/OpenAgentsInc/openagents/issues/9511) | open | Todo / Blocked | coder: run bounded child programs with typed dataflow |
| [#9512](https://github.com/OpenAgentsInc/openagents/issues/9512) | open | Todo / Blocked | coder: resolve and trust portable program and question packages |
| [#9513](https://github.com/OpenAgentsInc/openagents/issues/9513) | open | Todo / Blocked | coder: select bounded repository evidence with measured decision functions |
| [#9514](https://github.com/OpenAgentsInc/openagents/issues/9514) | open | In Progress / Blocked | coder: continuously schedule project work under host resource budgets |

## Shared service work

| Issue | Issue state | Project 15 | Project 16 | Scope |
| --- | --- | --- | --- | --- |
| [#9466](https://github.com/OpenAgentsInc/openagents/issues/9466) | closed | Done / Blocked | — | decision-api: authenticate keys and authorize tenant access before inference |
| [#9468](https://github.com/OpenAgentsInc/openagents/issues/9468) | closed | Done / Ready | — | decision-api: keyed HTTP gateway with bounded tenant admission |
| [#9469](https://github.com/OpenAgentsInc/openagents/issues/9469) | open | Todo / Ready | In Progress / Ready | decision-api: versioned NIP-CJ decision jobs with bounded worker admission |
| [#9470](https://github.com/OpenAgentsInc/openagents/issues/9470) | open | Todo / Blocked | Todo / Blocked | decision-api: authenticated capability discovery for available decision lanes |
| [#9471](https://github.com/OpenAgentsInc/openagents/issues/9471) | open | Todo / Blocked | — | decision-api: shared execution receipts for HTTP and relay results |
| [#9472](https://github.com/OpenAgentsInc/openagents/issues/9472) | open | Todo / Blocked | — | decision-api: tenant training with separate training and evaluation data |
| [#9473](https://github.com/OpenAgentsInc/openagents/issues/9473) | open | Todo / Ready | Todo / Ready | decision-api: explicit candidate admission for tenant adapters |
| [#9475](https://github.com/OpenAgentsInc/openagents/issues/9475) | open | Todo / Ready | — | decision-api: publish benchmark snapshots with coverage and evidence limits |
| [#9476](https://github.com/OpenAgentsInc/openagents/issues/9476) | closed | Done / Blocked | — | decision-api: ship caller docs and CLI with the first usable service |
| [#9480](https://github.com/OpenAgentsInc/openagents/issues/9480) | open | Todo / Needs input | — | decision-api: validate the measured-record offering on a caller-owned workload |
| [#9481](https://github.com/OpenAgentsInc/openagents/issues/9481) | open | Todo / Tracking | — | decision-api: track the complete product and delivery plan |
| [#9482](https://github.com/OpenAgentsInc/openagents/issues/9482) | open | Todo / Blocked | In Progress / Ready | decision-api: add batch, multi-label, and multidimensional classification |
| [#9483](https://github.com/OpenAgentsInc/openagents/issues/9483) | open | Todo / Blocked | Todo / Ready | decision-api: schedule bounded inference batches and separate capacity lanes |
| [#9484](https://github.com/OpenAgentsInc/openagents/issues/9484) | open | Todo / Blocked | Todo / Blocked | decision-api: persist batch jobs and deliver resumable results |
| [#9485](https://github.com/OpenAgentsInc/openagents/issues/9485) | open | Todo / Ready | Todo / Ready | decision-api: add measured review, abstention, and opt-in fallback policies |
| [#9486](https://github.com/OpenAgentsInc/openagents/issues/9486) | open | Todo / Ready | In Progress / Ready | decision-api: add backend capabilities and a Rust Laya integration |
| [#9487](https://github.com/OpenAgentsInc/openagents/issues/9487) | open | Todo / Blocked | Todo / Blocked | decision-api: expose inference and documentation through Rust MCP servers |
| [#9488](https://github.com/OpenAgentsInc/openagents/issues/9488) | open | Todo / Blocked | Todo / Blocked | decision-api: publish machine-readable discovery and agent installation metadata |
| [#9489](https://github.com/OpenAgentsInc/openagents/issues/9489) | open | Todo / Blocked | — | decision-api: release client packages and resolve the SDK language boundary |
| [#9490](https://github.com/OpenAgentsInc/openagents/issues/9490) | open | Todo / Blocked | Todo / Ready | decision-api: build self-serve accounts, workspaces, roles, and API keys |
| [#9491](https://github.com/OpenAgentsInc/openagents/issues/9491) | open | Todo / Ready | In Progress / Ready | decision-api: add a versioned monetary ledger and enforce spending limits |
| [#9492](https://github.com/OpenAgentsInc/openagents/issues/9492) | open | Todo / Blocked | Todo / Blocked | decision-api: add plans, checkout, subscriptions, credits, and billing recovery |
| [#9493](https://github.com/OpenAgentsInc/openagents/issues/9493) | open | Todo / Blocked | Todo / Blocked | decision-api: expose usage, balance, activity, and a customer dashboard |
| [#9494](https://github.com/OpenAgentsInc/openagents/issues/9494) | open | Todo / Blocked | — | decision-api: build a classification playground and bounded tool-backed chat demo |
| [#9495](https://github.com/OpenAgentsInc/openagents/issues/9495) | open | Todo / Blocked | Todo / Blocked | decision-api: publish a versioned skill directory with submission and review |
| [#9496](https://github.com/OpenAgentsInc/openagents/issues/9496) | open | Todo / Blocked | — | decision-api: publish and verify the decision workflow recipe library |
| [#9497](https://github.com/OpenAgentsInc/openagents/issues/9497) | open | Todo / Blocked | — | decision-api: accept structured agent feedback with trackable receipts |
| [#9498](https://github.com/OpenAgentsInc/openagents/issues/9498) | open | Todo / Blocked | — | decision-api: package portable deployments and publish service operating policies |
| [#9499](https://github.com/OpenAgentsInc/openagents/issues/9499) | open | Todo / Blocked | — | decision-api: specify and evaluate image-capable typed decisions |
| [#9500](https://github.com/OpenAgentsInc/openagents/issues/9500) | open | Todo / Research | — | decision-api: evaluate confidential hosted inference with explicit threat models |

## Reconcile status with implementation scope

These discrepancies affect delivery planning:

- #9466 and #9476 are closed and Done, but project 15 still says Blocked.
  #9468 is closed and Done. Do not schedule their shipped auth, gateway,
  and caller foundations as missing work.
- #9469, #9482, #9486, and #9491 are In Progress on project 16 and Todo on
  project 15. #9482 also differs in readiness. Each has partial implementation;
  none is complete merely because that foundation landed.
- #9483 and #9490 are Ready on project 16 and Blocked on project 15. Use the
  actual dependency of the proposed slice, not whichever board is convenient.
- #9502 remains Todo/Blocked despite the shipped direct-local SDK slice.
  #9509 is In Progress/Ready, and #9514 is In Progress/Blocked; both have
  functioning foundations and substantial remaining scope.
- #9504 and #9507 are closed and Done. Extend their authority and tracker
  contracts where needed rather than reopening the original delivery claim.
- #9501's body still describes twelve native sub-issues. The inventory now
  has thirteen other consumer issues, including #9514. Use this explicit
  inventory and the current acceptance scope when planning.

## Recent implementation slices

| Owner | Landed evidence | What remains |
| --- | --- | --- |
| #9502 | [Direct-local SDK configuration, `b1df4df7f9`](https://github.com/OpenAgentsInc/openagents/issues/9502#issuecomment-5755918784) | Coder call-site/profile migration, malformed configuration, identities/receipts, and relay integration. Loopback does not attest where inference runs |
| #9509 | [Typed bounded `run-suite`, `e60dd2512f`](https://github.com/OpenAgentsInc/openagents/issues/9509#issuecomment-5756002925) | Real suite adapter, metrics output, review contracts, and broader integration. Fixture success does not accept an artifact for integration |
| #9508 | [Mechanical dependency/conflict acceptance verified](https://github.com/OpenAgentsInc/openagents/issues/9508#issuecomment-5756160695) | General-count semantic questions, independent semantic labels, and complete ATIF/UI coverage |
| #9514 | [Functioning project supervisor and measured limits](https://github.com/OpenAgentsInc/openagents/issues/9514#issuecomment-5755469255) | Full reconciliation, resource classification, whole-run accounting, and automatic bounded verification/integration |
| #9469 | [Pure decision protocol and receipt binding, integrated at `9dd4ddab67`](https://github.com/OpenAgentsInc/openagents/issues/9469#issuecomment-5756199834) | Networked worker/caller, tenant admission, durable settlement, capacity, and live parity; consumer-side typed validation remains required |
| #9482 | [Partial classification HTTP route, integrated at `9dd4ddab67`](https://github.com/OpenAgentsInc/openagents/issues/9482#issuecomment-5756199997) | Binary filtering, Score ranking/rubrics, review/count helpers, exclusions, full item metadata/receipts, and evaluation beyond fixtures; packing and durable jobs remain separate |
| #9486 | [Backend capability record, `81ae574bce`](https://github.com/OpenAgentsInc/openagents/issues/9486#issuecomment-5755699475) | Laya, measurements, and registry/discovery integration |
| #9491 | [Fixed-point monetary ledger, integrated at `823006f7d6`](https://github.com/OpenAgentsInc/openagents/issues/9491#issuecomment-5755834959) | Gateway funding/settlement, authorized account APIs, and commercial integration; no launch pricing follows from this library |

The consumer's earlier implementation inventory predates these slices.
The [updated consumer contract](coder-as-decision-router-consumer.md)
separates their current behavior from the target.

## Dependency decisions for the new roadmap

Start the evidence store, deterministic context builder, and a small native
relevance function under #9513 while #9502/#9503 unify the client and function
contracts. Small bounded native calls do not require completed packing or
every classification mode. The partial #9482 route now provides ordered
Choice, multi-label Noul, and named dimensions with serial native execution;
#9483 packing and the remaining classification modes are still open.
Higher-throughput context selection should consume those shared capabilities
as they land.

Use #9505/#9506 for evidence visibility from the first useful flow. Extend
#9508/#9514 for task-context sharing and low-priority background consumers.
Keep #9510's durable program recovery distinct from #9484's durable inference
jobs. Use #9511/#9512 for composition and packages after there is a useful
workflow to reuse.

#9485 provides optional decision review, not permission to override execution
policy. #9487/#9488 expose and discover service capabilities; they do not
implement Coder's entire progressive tool catalog. #9495 publishes skills;
a local skill's instruction scope and lifetime still belong to Coder.

Accounts, money enforcement, plans, dashboards, and operations are necessary
for their respective hosted product claims. They do not all block a local
repository-answer or repair experiment. Training, image judgments, and
confidential hosted inference remain conditional work. Coder dogfooding is
useful evidence for #9475/#9496 and does not replace #9480's independent
caller pilot.
