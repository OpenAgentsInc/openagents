# OpenAgents as the Codex workroom — first deployable shape audit

- Class: historical-analysis
- Date: 2026-07-13
- Status: point-in-time product audit and MVP recommendation
- Dispatch: no; the master roadmap and live issues remain authoritative
- Owner: Sol product analysis
- Source snapshot: `OpenAgentsInc/openagents` `c68ed0b2a06bd2c615eea29ee7a3acc1245aecad`
  plus live GitHub issue state at `2026-07-13T13:37:50Z`
- Product Spec:
  [`specs/openagents/codex-workroom-mvp.product-spec.md` @ `spec_revision: 1`](../../specs/openagents/codex-workroom-mvp.product-spec.md)

## Executive finding

**OpenAgents' first deployable Codex product should be a signed, local-first
Desktop workroom around Codex—not another agent engine, a chat skin, or the
entire OpenAgents platform at once.**

The relationship is exact enough to guide the product:

```text
OpenCode owns agent execution       Codex owns agent execution
OpenChamber owns the workroom       OpenAgents owns the workroom
```

OpenChamber's strongest contribution is not a component library. It is the
coherent product frame around OpenCode: sessions are cheap to resume; typed
work, blockers, files, Git, terminal, and review remain near the conversation;
and several clients can reach the same runtime. Codex already supplies the
corresponding engine seam for OpenAgents: a typed Thread → Turn → Item app-server,
durable rollout history plus indexed state, explicit subagent topology,
approvals, containment, and a replay-aware remote-control protocol.

The smallest complete OpenAgents adaptation is therefore:

> A developer can install OpenAgents Desktop, use it without an OpenAgents
> account, connect a named Codex account without touching the default Codex
> home, grant a repository, start or resume a Codex thread, supervise typed
> live work and child agents, resolve blockers, inspect the resulting files and
> Git diff, quit and reopen, and return to one honest session outcome.

This definition is narrower than the current R0–R7 program and CUT-27. It does
not change their sequence, split their issues, or authorize a public cutover
claim. It identifies the first independently understandable product inside
that program. The [Product Spec](../../specs/openagents/codex-workroom-mvp.product-spec.md)
is the durable intent artifact; this audit is its point-in-time evidence.

## Question and method

The question was: **what OpenChamber is to OpenCode, what must OpenAgents be to
Codex in its first deployable form?**

The audit reviewed:

- the current [`MASTER_ROADMAP.md`](./MASTER_ROADMAP.md), Sol index, Desktop
  architecture, OpenCode parity audit, coding cutover plan, CUT-27 readiness
  audit, checked-in issue sources, and current Desktop guarantees;
- the teardown set about
  [OpenChamber](../teardowns/2026-07-12-openchamber-product-teardown.md),
  [OpenCode Desktop](../teardowns/2026-07-10-opencode-desktop-app-teardown.md),
  [OpenCode V2](../teardowns/2026-07-10-opencode-v2-architecture-teardown.md),
  [OpenCode's Effect architecture](../teardowns/2026-07-10-opencode-effect-architecture-teardown.md),
  [Codex](../teardowns/2026-07-10-codex-agent-runtime-teardown.md), and the
  [OpenAgents adaptation](../teardowns/2026-07-10-openagents-product-adaptation-analysis.md);
- all 11 open `OpenAgentsInc/openagents` issues at the snapshot, with bodies and
  current comments reviewed for the relevant product issues; and
- all 186 issues closed from `2026-07-08T00:00:00Z` through the snapshot,
  scanning every title/label/disposition and reading the relevant bodies and
  closure evidence in depth.

The reproducible issue queries were:

```sh
gh issue list --repo OpenAgentsInc/openagents --state open --limit 500
gh api --paginate -X GET search/issues \
  -f q='repo:OpenAgentsInc/openagents is:issue is:closed closed:>=2026-07-08' \
  -f sort=updated -f order=desc -f per_page=100
```

Issue state is coordination evidence, not implementation truth. Where a closed
issue, current guarantee, and receipt named different proof rungs, this audit
kept the narrowest proven rung.

## The product analogy

| Concern | OpenChamber around OpenCode | OpenAgents around Codex |
| --- | --- | --- |
| Engine owner | OpenCode owns sessions, model/tool execution, permissions, files, Git, and PTY | Codex owns Thread → Turn → Item execution, tools, approvals, sandboxing, rollout history, and provider-native child threads |
| Product owner | OpenChamber owns the persistent workroom, navigation, review, attention, and cross-shell reachability | OpenAgents owns the hardened workroom, stable product identity, typed command/outcome projection, review, and supervision |
| Adapter seam | OpenChamber server and SDK adapt OpenCode plus host capabilities | Host-owned Runtime Gateway adapts Codex app-server plus grant-bounded workspace services |
| Identity | Reconnect to the same OpenCode host/session | Stable OpenAgents session/work-context refs map to Codex thread refs; neither host path nor provider ID becomes product identity |
| Persistence | OpenCode state, OpenChamber metadata, and client caches coexist | Codex rollout/index stays engine evidence; OpenAgents persists admitted intents, product mappings, bounded projections, and durable outcomes only |
| Client trust | Broad authenticated server surface; Electron renderer receives runtime credentials | Sandboxed tokenless renderer; fixed schema-decoded queries, intents, and events only |
| First product value | Make OpenCode feel like an ongoing place to work | Make Codex work findable, inspectable, steerable, reviewable, and restart-safe in a signed app |
| Later expansion | Web, mobile, VS Code, relay, schedules, goals, voice | Khala Sync, mobile, Fleet, remote workrooms, portable sessions, managed targets, voice, and durable goals after their own gates |

The analogy has a boundary. OpenChamber principally reconnects clients to an
existing host. The broader OpenAgents roadmap promises canonical cross-device
state and eventually generation-fenced movement between target classes. Those
are stronger later contracts; they are not needed to explain or accept the
first local Codex workroom.

## What the MVP is

### Product sentence

**OpenAgents Desktop Codex Workroom is the open, installable operating surface
for local Codex work: one place to find sessions, run and steer turns, inspect
agents and blockers, review workspace effects, and recover honest state.**

Codex remains independently useful and owns its agent loop. OpenAgents earns
its place by reducing reorientation and uncertainty around that loop.

### Required user journey

1. Install and open the signed/notarized Desktop artifact on the first
   supported macOS target.
2. Enter local-first mode without an OpenAgents account. If Codex is missing,
   incompatible, signed out, rate-limited, or policy-disabled, show the exact
   prerequisite or refusal instead of a fake empty workroom.
3. Select or connect one **named isolated** Codex account. Device auth never
   writes to the default `~/.codex` home.
4. Grant one repository and open a recent thread or create a new one.
5. Submit one durably admitted task. Observe typed text, reasoning summary,
   plan, tool/file-change activity, usage, and terminal state.
6. Resolve one question, approval, or plan-review blocker, or explicitly stop
   or steer the active turn.
7. Inspect the full child-agent roster, open one child's independent
   transcript, and follow its causal edge from the parent timeline.
8. Inspect the bounded file tree, Git status, and exact diff beside the
   conversation.
9. Reload the renderer and quit/relaunch the app without duplicating the turn,
   losing the catalog, flattening children, or inventing completion.
10. Export bounded diagnostics and complete update/rollback/uninstall checks
    without exposing credentials, prompts, absolute repository roots, or raw
    provider payloads.

### Minimum workroom surface

| Surface | MVP requirement | Why it is irreducible |
| --- | --- | --- |
| Session rail | Metadata-first named top-level sessions, status/attention, paging, resume/new/fork/archive/delete | A persistent workroom must make repeated work cheap to find and resume |
| Causal timeline | Typed Thread/Turn/Item projection for text, plans, tools, patches, blockers, usage, errors, interruption, terminal outcome | Strings and spinners cannot explain what Codex is doing |
| Composer and controls | Send, stop, steer-current-turn, queue-next-turn, account/model selection, durable question/approval/plan response | The app must control the engine, not merely watch history |
| Agent topology | Full parent/child graph, inline causal child cards, lifecycle, independent transcripts, explicit unknown/gap state | Codex subagents are product state, not one flattened status line |
| Review | Grant-bounded file tree, Git status, exact diff, links from tool/file events | A coding workroom must show the effect of the work beside the request |
| Runtime state | Named account/model/version/readiness plus typed auth/quota/policy failures | A blank pane must not hide a missing engine or unusable account |
| Recovery | Durable intent before dispatch, exact retry reconciliation, projection/log repair before live resubscribe, one restart disposition | Persistence without recovery is not continuity |
| Distribution | Signed compatible app/runtime set, update, rollback, reinstall, redacted diagnostics | “Deployable” means a reproducible artifact, not a source checkout |

A full editor, interactive PTY, destructive Git operations, and commit/push/PR
workflow may remain present in the broader Desktop product. They are not
required to make this first Codex workroom independently useful or to accept
this MVP.

## Architecture and authority boundary

The MVP should preserve the current architecture sentence:

```text
sandboxed Effect Native renderer
        |
fixed typed projections and intents
        |
host-owned Runtime Gateway
        |----------------------|
Codex app-server          workspace/Git capabilities
```

The boundary has eight non-negotiable consequences:

1. Codex owns the model/tool loop. Runtime Gateway is an adapter and lifecycle
   supervisor, never a second conversation engine.
2. The app-server path is primary. An embedded transport is an optimization
   through the same Codex request processor, not a privileged alternate API.
3. The renderer receives no bearer/provider/Pylon credential, loopback secret,
   raw IPC, raw provider event, process handle, or general filesystem handle.
4. OpenAgents assigns stable product refs and stores host-private mappings to
   Codex thread/account/workspace identity. Provider refs and paths do not
   become portable identity.
5. Every mutating intent is admitted durably before dispatch with owner/tier,
   work context, causal parent, idempotency, generation, and delivery semantics.
6. Current projection, durable log/history, and volatile live stream are
   distinct. Reconnect repairs from durable state before accepting live facts.
7. Questions, permissions, and plan review remain host-owned durable blockers;
   a mounted dialog, active goal, or model prose never grants authority.
8. Raw Codex history stays owner-local by default. Bounded projections and
   opt-in public-safe receipts never contain prompts, repository bodies,
   credentials, account identity, or stable private refs.

## Issue audit

### Open set at the snapshot

All 11 open issues belonged to the Sol program:

| Issues | Role | MVP implication |
| --- | --- | --- |
| [#8566](https://github.com/OpenAgentsInc/openagents/issues/8566), [#8574](https://github.com/OpenAgentsInc/openagents/issues/8574), [#8597](https://github.com/OpenAgentsInc/openagents/issues/8597) | Program, Desktop, and mobile parents | The MVP is a bounded product inside these parents; it is not a replacement queue |
| [#8707](https://github.com/OpenAgentsInc/openagents/issues/8707) | Codex/Claude installed-app cutover | Directly relevant but broader: it requires Codex **and** Claude plus physical iOS/Android evidence for each counted task |
| [#8741](https://github.com/OpenAgentsInc/openagents/issues/8741) | Persistent-audio owner acceptance | Not an MVP blocker; voice is explicitly outside the first Codex workroom |
| [#8748](https://github.com/OpenAgentsInc/openagents/issues/8748)–[#8753](https://github.com/OpenAgentsInc/openagents/issues/8753) | Real host movement through signed portable-session dogfood | Later differentiation; none is manufactured by local session reachability |

The live CUT-27 comments are especially instructive. The installed named-Codex
repository task, typed Git review, durable terminal, and restart continuity
were reported as proven. CUT-27 remains open because both tested named Claude
accounts return `provider_disabled` and the literal same-task physical-device
receipts remain missing. That supports a Codex-first product definition, but
current roadmap authority still forbids converting it into the broader
Codex/Claude default-surface declaration.

### Relevant recently closed evidence

| Closed issue(s) | What the closure contributes | Boundary retained here |
| --- | --- | --- |
| [#8655](https://github.com/OpenAgentsInc/openagents/issues/8655) | Closed Runtime Gateway protocol/lifecycle | A protocol receipt alone is not an installed product journey |
| [#8674](https://github.com/OpenAgentsInc/openagents/issues/8674), [#8675](https://github.com/OpenAgentsInc/openagents/issues/8675) | Loss-accounted Codex history, subagent inspector, and real-Electron trace acceptance | Historical import is read-only and does not by itself authorize resume or dispatch |
| [#8696](https://github.com/OpenAgentsInc/openagents/issues/8696) | Composer, questions, approvals, and runtime controls | Fixture and component proof do not replace the counted installed Codex journey |
| [#8699](https://github.com/OpenAgentsInc/openagents/issues/8699), [#8700](https://github.com/OpenAgentsInc/openagents/issues/8700) | Typed Git review/context plus bounded PTY/preview | MVP requires review; interactive PTY/preview breadth is optional for this first shape |
| [#8701](https://github.com/OpenAgentsInc/openagents/issues/8701), [#8703](https://github.com/OpenAgentsInc/openagents/issues/8703) | Named runtime/account/model plus MCP/skill/plugin/permission/settings integration | MVP requires one compatible named Codex lane, not general provider or extension breadth |
| [#8706](https://github.com/OpenAgentsInc/openagents/issues/8706) | Signed/notarized/stapled installed artifact, update, rollback, reinstall, and diagnostics evidence | Distribution evidence must be repeated against the exact MVP compatibility set |
| [#8744](https://github.com/OpenAgentsInc/openagents/issues/8744) | Durable local-turn journal and one bounded Codex continuation after process restart | Does not prove byte-identical live-stream reattachment, autonomous goals, or scheduled work |
| [#8676](https://github.com/OpenAgentsInc/openagents/issues/8676), [#8677](https://github.com/OpenAgentsInc/openagents/issues/8677) | Same-ref mobile continuation and command/event fault convergence | Valuable stronger-program evidence; mobile is not required for the local Codex MVP |
| [#8547](https://github.com/OpenAgentsInc/openagents/issues/8547), [#8636](https://github.com/OpenAgentsInc/openagents/issues/8636) | Managed Agent Computer and bounded hybrid-routing acceptance | Neither proves portable movement and neither is needed for the first local workroom |
| [#8745](https://github.com/OpenAgentsInc/openagents/issues/8745)–[#8747](https://github.com/OpenAgentsInc/openagents/issues/8747) | Portable-session contract, durable authority, and target-scoped broker | Schema/control-plane/broker closure is not a real move; [#8748](https://github.com/OpenAgentsInc/openagents/issues/8748) owns that proof |
| [#8593](https://github.com/OpenAgentsInc/openagents/issues/8593) | ProductSpec v0.1 tooling and OpenAgents extensions | Specs declare intent; behavior contracts, Eval Suites, receipts, promises, roadmap, and issues enforce it |

The issue history shows that much of the required substrate is already
code-landed or accepted at stronger scopes. The product gap is not another
capability inventory. It is one narrow installed Codex workroom contract and
one release journey that users can understand without inheriting every later
program exit.

## Explicit cuts

The first deployable shape does **not** require:

- Claude, Grok, or provider-neutral parity;
- an OpenAgents account, hosted Sync, or mobile/web/VS Code/PWA clients;
- Fleet, multi-account dispatch, markets, payments, settlement, or public
  proof surfaces;
- managed Agent Computers, remote workrooms, owner-managed targets, provider
  adapters, host movement, or failback;
- a full editor, interactive PTY, commit/push/PR/merge, or destructive Git;
- arbitrary MCP installation, third-party plugins in the authority process,
  marketplace, or model-authored Code Mode;
- autonomous Session Goals, schedules, or background workflow execution;
- voice, computer use, browser automation, or ambient memory; or
- a public statement that the broader Codex/Claude/mobile cutover is complete.

These cuts are definition discipline, not deletion instructions. Existing
broader capabilities and roadmap gates retain their own owners and proof.

## MVP acceptance and proof

The exact release candidate should pass one packaged real-host journey matching
the required user journey above, plus deterministic failure cases for:

- Codex missing, incompatible, signed out, quota-exhausted, rate-limited, and
  policy-disabled;
- duplicate/conflicting intent identity, lost acknowledgement, stream gap,
  renderer reload, app-process restart, and stale generation;
- missing child history, unknown event/item type, explicit redaction/gap,
  revoked workspace grant, and Git post-image conflict; and
- update interruption, rollback/downgrade refusal, diagnostics export,
  uninstall/reinstall, and cleanup.

The acceptance bundle must name app and Codex versions, artifact digest,
account **class** but no account identity, stable public-safe test refs,
commands, timestamps, expected failure dispositions, and exact proof rung. It
must not contain prompts, transcript bodies, repository content, local paths,
credentials, raw runtime events, or provider payloads.

## Risks and falsifiers

| Risk | Falsifier / required response |
| --- | --- |
| OpenAgents becomes a second Codex engine | Any provider/model/tool loop implemented outside the Codex adapter blocks launch and must be removed or separately justified |
| The workroom is only a prettier chat | A user cannot find an old session, understand child/tool state, resolve a blocker, and inspect the exact diff in one journey |
| Local-first is nominal | First useful Codex work requires OpenAgents sign-in, hosted availability, or remote authority |
| Persistence is mistaken for recovery | Restart can leave a silent idle state, duplicate dispatch, reopened terminal work, or unowned pending turn |
| Renderer trust expands for speed | Any credential, generic IPC, raw provider event, absolute root, or general filesystem/process handle crosses preload |
| Compatibility is brittle | A Codex update silently changes semantics instead of producing an explicit incompatible/unavailable state |
| Scope grows back to the whole platform | MVP acceptance becomes gated by Claude, mobile, Fleet, cloud, portability, voice, goals, or marketplace work |
| Issue closure becomes proof | The launch claim cites a closed label without the exact current artifact and real-host receipt |

## Recommendation

Adopt
[`specs/openagents/codex-workroom-mvp.product-spec.md` @ `spec_revision: 1`](../../specs/openagents/codex-workroom-mvp.product-spec.md)
as the durable what/why for this bounded product. Keep the master roadmap and
live issues as sequence and coordination authority. Do not open a parallel epic
or change public promises from this audit alone.

The decisive scope test is simple:

> If Codex were removed, this MVP would have no engine. If OpenAgents were
> removed, the developer would lose the persistent workroom, typed
> supervision, integrated review, and restart-safe product state.

That is the relationship OpenChamber demonstrates for OpenCode, expressed in
the smallest deployable OpenAgents form.
