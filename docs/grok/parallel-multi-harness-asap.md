# Parallel multi-harness ASAP — analysis

Date: 2026-07-08
Status: Grok analysis / opinion (flips no promise state)
Audience: owner + fleet implementers
Primary spine: `docs/fable/MASTER_ROADMAP.md` rev 6.3

## 0. The ask, restated

Ship **parallel coding agents** across **Codex, Claude Code, and Grok CLI**,
steered from **Khala Code (mobile + desktop)** over **Khala Sync**, with UI
on **Effect Native** — as fast as substrate and safety floors allow.

This is not a new product idea. The monorepo already encodes most of the
architecture. What is missing is:

1. Treating **Grok** as a first-class harness (not an external chat tool)
2. Closing the **ChatRuntime** and **workerKind** abstractions so a third
   engine is a schema + adapter, not a product rewrite
3. Making **mobile ↔ desktop** supervision of fleets a Sync problem, not a
   second orchestration stack
4. Running **Effect Native conversion** and **multi-harness wiring** as
   *parallel fleet lanes* that share contracts, not serial roadmaps that
   wait on each other

---

## 1. What MASTER_ROADMAP already decides

### 1.1 Product sequence (does not block multi-harness prep)

```text
P0 MVP tested / store artifacts
 → P1 Sarah + sales landing + outbound
 → P2 Your Codex on agent computers (daily-driver cutover)
 → P3 Standing employees
 → …
```

P2 is still "your Codex in the cloud" — the highest-leverage dogfood for
*one* harness on agent computers. Multi-harness parallelization is **not
P2's only job**, but P2 is the cloud execution substrate multi-harness
workers will share.

### 1.2 Effect Native full conversion (rev 6) — ASAP, parallel

Rev 6: entire UI converges on Effect Native ASAP via conversion waves
CV0–CV5. Safety floor: tests, QAM gates, behavior contracts, P0 store
artifacts stay green. Substrate critical path lives in public
`OpenAgentsInc/effect-native`.

Implication for multi-harness UI:

| Surface | EN binding | Multi-harness UI that belongs there |
| --- | --- | --- |
| Mobile | EN-3 #8568 burn-down; new screens EN-native | Harness pill, accounts, fleet peek, approvals |
| Desktop | EN-5 #8574 full conversion (React rewrite cancelled) | Fleet cockpit, multi-harness chat, Inbox |
| Web cockpit twin | EN-4 / catalog growth | Read-only fleet + spend (later) |

**Do not** build multi-harness chrome in throwaway React shells that EN
will delete. New multi-harness panels author Effect Native contracts (or
neutral RPC + thin host until the panel lands under EN-5).

### 1.3 Pylon fold (rev 6.3) — one human surface

Khala Code desktop becomes the primary human surface for Pylon-class
capacity (daemon + typed RPC; OpenTUI retired after cockpit parity).
Fleet parallelism is a **desktop/mobile product feature**, not a
terminal ritual.

### 1.4 Parallelism doctrine already in the roadmap

MASTER_ROADMAP repeatedly says: **ASAP means maximal parallel lanes,
never skipping substrate.** That is the correct doctrine for multi-
harness too: many adapters, one contract; many workers, one claim
registry; many UIs, one Sync plane.

---

## 2. The two axes (load-bearing — do not conflate)

From `2026-07-01-episode-245-completion-and-multi-harness-orchestration.md`:

| Axis | Question | Today | Target |
| --- | --- | --- | --- |
| **A — Chat harness** | Who owns *this conversation* (thread, tools, approvals, session files)? | Codex default; Claude runtime partially landed; Khala legacy | `codex \| claude \| grok \| auto` |
| **B — Delegation target** | Who runs *spawned workers* when fleet/dispatch fans out? | Codex primary; Claude ~80% Pylon lane; UI `workerKind: codex\|claude\|auto` | `codex \| claude \| grok \| auto` |

These axes must stay independent:

- Chat can be **Grok** while workers are **Codex** (Grok plans, Codex
  implements — classic planner/coder split).
- Chat can be **Claude** while a mixed fleet runs **Codex + Grok**
  workers on claimed issues.
- Chat can be **Codex** with `auto` workers that pick the free quota.

Conflating A and B recreates the pre-pivot mess (one mode string that
tries to mean "UI engine" and "worker kind").

---

## 3. Substrate inventory (do not rebuild)

### 3.1 Already multi-harness-shaped

| Piece | Location / note |
| --- | --- |
| Neutral chat turn events | `KhalaCodeDesktopChatTurnEvent` stream — harness-agnostic |
| Claude + Codex runtimes | `claude-app-sdk-chat-runtime`, `codex-app-server-chat-runtime` |
| Harness setting | `harness-setting.ts` (`claude_runtime` env path exists) |
| FleetRun + supervisor | `fleet-run-supervisor.ts`; maps `workerKind` → runnerKind |
| Claim / planner doctrine | fleet-fanout doc Lanes A/B — one claim per work unit |
| Deterministic delegate | `khala.fleet.delegate` program |
| Claude Pylon lane | `claude-agent-executor.ts`, capacity refs, ~80% parity |
| `agent_definition.v1` | Harness kinds already include `codex`, `claude_code`, … |
| Adapter kinds | `AgentRuntimeAdapterKind` includes `claude_code`, `codex`, `opencode`, `hermes` — **no `grok` yet** |
| Planner/coder/judge design | oh-my-pi audit → roles × harness matrix |
| Khala Sync | Postgres outbox + DO hubs + client stores; chat mutators landed |
| Grok CLI (external) | Headless `-p`, ACP `agent stdio`, worktrees, sessions, MCP — `docs/grok-cli/` |

### 3.2 Explicit gaps for a third harness (Grok)

| Gap | Why it blocks ASAP |
| --- | --- |
| No `grok` / `grok_cli` in schema literals | Definitions and adapter kinds cannot name it |
| No Axis A `GrokChatRuntime` | Desktop/mobile cannot chat with Grok as the harness |
| No Axis B Grok worker executor | Fleet cannot spawn Grok workers with claims + metering |
| No capacity / account model for Grok | Rate limits, multi-account, exact token rows |
| No Sync projection for multi-harness fleet state | Mobile sees partial fleet truth |
| No EN-native harness pill / fleet cards | UI thrash under conversion |

### 3.3 June 29 lessons (still the law)

From fleet fan-out coding instructions:

1. **Sustained concurrency with refill**, not one-shot batch
2. **Typed claims**, never "each worker greps the backlog"
3. **Typed verification gate** on merge — never vibes
4. **Visible + controllable** in product UI (and therefore on mobile via Sync)
5. Throughput is a **product feature**, not an operator shell ritual

Adding Grok does not relax any of these. A third engine that skips claims
is how you recreate duplicate-PR collapse at larger scale.

---

## 4. Why Grok CLI is a good third harness

Not "another chatbot." Grok Build CLI is already an **agent runtime** with
properties that map cleanly onto Khala Code:

| Grok capability | Khala Code mapping |
| --- | --- |
| `grok -p` + `--output-format json\|streaming-json` | Headless worker / fixture-friendly turns |
| `grok agent stdio` (ACP JSON-RPC) | Preferred integration: tool host owns FS/terminal; Grok is the brain |
| Sessions under `~/.grok/sessions` | Desktop session catalog + resume/fork |
| `grok -w` worktrees | Align with monorepo clean-worktree + claim isolation |
| MCP (`grok mcp …`) | Same MCP economy as Codex/Claude; can host `khala_fleet` tools |
| Permission modes / sandbox | Map to Khala approval + agent-computer isolation |
| Multi-agent dashboard | Inspiration for Fleet cockpit grouping; do **not** fork a second dashboard product |
| `--always-approve` / `dontAsk` | Fleet unattended vs interactive approval postures |
| Exact-ish automation surface | Pair with OpenAgents exact token rows — **do not trust vendor self-report as ledger** |

### 4.1 Preferred integration style: ACP first, shell second

**Primary:** spawn `grok agent stdio`, speak ACP (`initialize` →
`authenticate` → `session/new` → `session/prompt`), stream
`session/update` agent message chunks into the neutral turn event model.

**Secondary:** `grok --no-auto-update -p "…" --output-format streaming-json`
for simple workers and CI smokes where ACP is overkill.

**Avoid as product core:** scraping the interactive TUI. The TUI is for
humans; Khala Code is the product TUI.

Auth for automation: `XAI_API_KEY` or cached `grok login` / device-code on
the capacity host. Never put API keys in Sync payloads or workroom
receipts.

### 4.2 Role fit (planner / coder / judge)

From the oh-my-pi / multi-role design, a natural default matrix (opinion):

| Role | Default harness | Rationale |
| --- | --- | --- |
| Architect / plan | Claude or Grok | Long-context plan + DAG; Grok strong for synthesis |
| Coder | Codex (primary), Grok optional | Existing own-capacity density + dogfood |
| Judge / review | Claude or Grok | Second opinion; never merge authority |
| Advisor / watchdog | Claude or Grok read-only | Steers; does not own the coding thread |
| Fleet workers | `auto` across free capacity | Claims + quotas decide |

Merge authority remains: **verify command + human/product acceptance**,
never the judge model alone.

---

## 5. Target architecture (one picture)

```text
┌──────────────── Khala Code mobile (EN/RN) ────────────────┐
│  Approvals · fleet peek · harness preference · push       │
└──────────────────────────┬────────────────────────────────┘
                           │ Khala Sync (scopes: threads, agents,
                           │ fleet_run projections, approvals)
┌──────────────────────────▼────────────────────────────────┐
│              Khala Code desktop (EN-5 target)             │
│  ChatRuntime selector (Axis A)                            │
│    codex | claude | grok | auto                           │
│  FleetRun supervisor + claim planner (Axis B)             │
│    workerKind: codex | claude | grok | auto               │
│  Neutral turn event bus → UI + Sync capture               │
└─────────────┬───────────────┬───────────────┬─────────────┘
              │               │               │
        Codex app-server  Claude Agent SDK  Grok ACP/stdio
              │               │               │
              └───────────────┴───────────────┘
                              │
                    Pylon / agent computers
                    (dispatch, isolation, meters)
                              │
                    exact token_usage_events
                    + work claims + verify gates
```

Invariants:

1. **One claim registry** across all worker kinds
2. **One Sync plane** for cross-device truth
3. **One credits / token ledger** with provenance labels
4. **One chat event model** for all harnesses
5. **Harness-specific state** stays in harness homes
   (`~/.codex`, `~/.claude`, `~/.grok`) with desktop mapping files only

---

## 6. ASAP parallelization plan (lanes that can run together)

The goal is maximal concurrent implementation **without** skipping
contracts. Lanes below are intentionally independent once schemas land.

### Wave 0 — contracts (unblocks everyone) — 1–3 days, serial critical path

| Task | Outcome |
| --- | --- |
| W0.1 | Add `grok` / `grok_cli` to `AgentDefinitionHarnessKind` + `AgentRuntimeAdapterKind` (+ raw event kind) |
| W0.2 | Extend `FleetRun.workerKind` and UI enums: `codex \| claude \| grok \| auto` |
| W0.3 | Document Axis A/B in a shared package comment + `INVARIANTS` if needed |
| W0.4 | Neutral `ChatRuntime` interface stabilized (Codex + Claude already nearly there) |

**Exit:** schemas merged; no behavior change required.

### Wave 1 — Grok adapters (parallel sublanes)

| Lane | Axis | Exit receipt |
| --- | --- | --- |
| G-A1 | A | `GrokAcpChatRuntime`: startTurn/startThread/interrupt; streams into neutral events |
| G-A2 | A | Session catalog mapping desktop session ↔ Grok session id; resume/fork |
| G-B1 | B | Pylon (or desktop host) executor: Grok headless/ACP worker with worktree + claim pin |
| G-B2 | B | Capacity advertisement + readiness probe (`grok version`, auth, model list) |
| G-B3 | B | Exact metering path: wall time + any available usage fields labeled; never invent tokens |

**Exit:** fixture-tier chat with Grok; one claimed Grok worker closeout in tests.

### Wave 2 — Fleet multi-kind (depends on W0 + partial G-B)

| Lane | Exit |
| --- | --- |
| F1 | Supervisor schedules mixed workerKinds under one FleetRun |
| F2 | Planner scores candidates with harness affinity (optional) without breaking claim uniqueness |
| F3 | MCP `khala_fleet` verbs accept workerKind including `grok` |
| F4 | Role registry (`architect\|coder\|judge`) × harness matrix (G1 from oh-my-pi) |

**Exit:** fixture FleetRun targetConcurrency≥3 with mixed codex/claude/grok simulated workers; zero claim collisions.

### Wave 3 — Khala Sync surfaces (parallel with Wave 2)

| Lane | Exit |
| --- | --- |
| S1 | FleetRun + worker cards projected into Sync scopes (read on mobile) |
| S2 | Approval / steer mutators from mobile to desktop/daemon |
| S3 | Cross-device dogfood: start run on desktop, pause from phone, resume |

**Exit:** MC-class receipt: phone sees live fleet state without polling hacks.

### Wave 4 — Effect Native product chrome (parallel with Waves 1–3 once contracts stable)

| Lane | EN issue | Exit |
| --- | --- | --- |
| U1 | EN-3 | Mobile harness pill + accounts list EN-native |
| U2 | EN-5 | Desktop fleet cockpit panels EN-native (no new Foldkit/vanilla DOM) |
| U3 | EN-2 | Catalog demand: list/virtualized worker rows, severity chips, plan DAG card |

**Exit:** no multi-harness UI written only for a dying shell.

### Wave 5 — Agent computers + multi-harness (rides P2)

| Lane | Exit |
| --- | --- |
| C1 | Codex-in-VM remains linchpin (CX-3) — do not starve this for Grok novelty |
| C2 | Grok worker on agent computer **after** Codex path is real (same isolation/meter rails) |
| C3 | Claude worker on agent computer parity |

**Exit:** at least one non-Codex harness completes a metered cloud turn with reclaim evidence.

---

## 7. What to parallelize vs what must stay serial

### Parallelize hard

- Schema extension + three adapters (Codex polish, Claude parity, Grok greenfield)
- Fixture tests per harness
- Sync projections for fleet (read path) while desktop remains write authority
- EN catalog demand tickets for fleet UI components
- QA scenarios per harness (fixture tier)

### Keep serial / gated

- Claim registry uniqueness semantics (one implementation)
- Token ledger write path (one truth)
- Agent-computer money gates (P2 CX linchpin)
- Store-submission / mobile straight-line green (P0 safety floor)
- Promise/copy for any public "multi-agent army" claims

### Anti-patterns

| Anti-pattern | Why it fails ASAP |
| --- | --- |
| Second fleet state store "for Grok" | Sync + claims diverge |
| Shell-only Grok orchestration | Returns to June 29 ritual |
| UI rewrite in React "just this week" | EN-5 deletes it |
| Auto-approve everything to go faster | Authority model + safety floor |
| Waiting for full Claude desktop parity before starting Grok | Grok is greenfield; contracts unlock both |
| Starving CX-3 to chase three-harness demos | No cloud substrate for anyone |

---

## 8. Sequencing vs MASTER_ROADMAP phases

Opinionated overlay (does not rewrite MASTER_ROADMAP authority):

| Now (parallel) | Why |
| --- | --- |
| Wave 0 schemas | Cheapest unlock |
| Grok Axis A adapter + Claude chat parity continuation | Two independent Axis A lanes |
| Fleet claim/supervisor hardening (Codex) | Existing density |
| EN CV0/CV1 + EN-3 mobile burn-down | UI substrate |
| P1 sales (separate capacity) | Revenue lane; do not put sales agents on coding claim registry |

| Next | Why |
| --- | --- |
| Grok Axis B workers + mixed FleetRun | Needs W0 + supervisor |
| Sync fleet projections | Needs stable FleetRun schema |
| P2 CX-3 Codex-in-VM | Cloud dogfood |
| Grok/Claude on agent computers | After CX-3 rails exist |

| Later | Why |
| --- | --- |
| Public multi-harness marketing | Promise registry |
| Role presets as product copy | After fixture+live smokes |
| auto routing ML/policy | After exact economics per harness |

---

## 9. Metrics that prove parallelism works

| Metric | Good | Bad |
| --- | --- | --- |
| Concurrent claimed workers (mixed kinds) | Sustained target N | Spikes then idle |
| Duplicate claims / duplicate PRs | 0 | >0 |
| % closeouts with verify green | High, labeled | Vibes merges |
| Token rows with harness + role provenance | 100% of metered | Missing labels |
| Mobile fleet projection lag | Seconds, typed | Manual refresh only |
| Operator minutes per N workers | Falling | Linear babysitting |
| Harness switch cost (chat) | One pill + resume | New app / lost context |

---

## 10. Risks specific to three-harness fleets

1. **Quota chaos** — three vendors, three rate shapes; `auto` must be
   explicit and typed (`account_exhausted`, never silent swap).
2. **Semantic drift** — same prompt, different tools; pin verify commands
   and repo/commit in every claim.
3. **Worktree pile-up** — Grok and monorepo worktrees both create
   directories; GC policy required (`grok worktree gc` + our worktree hygiene).
4. **Auth sprawl** — device-code vs API key vs OAuth; capacity hosts need
   a readiness matrix, not hope.
5. **Cost blindness** — subscription-covered Codex vs API Grok must show
   on the economics surface (oh-my-pi G5) before `auto` optimizes wrong.
6. **EN conversion thrash** — dual-writing UI systems mid-fleet-build;
   mitigate by RPC-first + EN-native only for new chrome.

---

## 11. Recommendation (single paragraph)

**Do it now as a contract-first, three-adapter program under the existing
Axis A/B and FleetRun/claim laws — with Grok entering through ACP/stdio
as a first-class harness, Khala Sync carrying fleet and approval
projections to mobile, and Effect Native owning all new multi-harness
UI.** Keep P2 Codex-on-agent-computers as the cloud linchpin; do not let
the romance of three logos skip claims, verify gates, or Sync. ASAP is
**parallel lanes on shared schemas**, not one mega-PR that rewrites the
desktop three times.

Companion: [`grok-cli-as-third-harness.md`](./grok-cli-as-third-harness.md)
for adapter-level design and concrete file/package touch list.
