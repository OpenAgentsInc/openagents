# Autonomous QA as a Khala example flow — computer-use agent that writes & runs e2e tests

> **Status:** design exploration + planned **Khala example flow**, 2026-06-24.
> Not a product promise or public-claim copy. The intent: build this as a
> **flagship example of what Khala can do** — Khala driving real developer tools
> (Chrome, a terminal) inside **OpenAgents Cloud VMs/infra**, developing against a
> product, then distilling the session into committed e2e tests whose passing run
> (with video) is the review artifact. It is a showcase of Khala-as-agent-platform
> over Cloud primitives (VM isolation, compute, artifacts), not a one-off tool.
>
> **Origin:** the shape comes from an external request by the author of
> [`RhysSullivan/executor`](https://github.com/RhysSullivan/executor) (local
> clone: `projects/repos/executor`) — he wants an OpenDevin-style *autonomous QA*
> agent, offered to be the **first customer** / **GitHub Sponsor**, and to get on
> a call. We are NOT opening a PR to his repo; `executor` is studied here as
> **prior art** for the substrate, and he is a natural **design partner / first
> customer** for the hosted tier. The build target is an owned Khala example flow.

## 1. The ask, in his words

> "I am desperate for an *OpenDevin* — but not in the background-agent sense, in
> their **autonomous QA** sense. You give the agent the same tools that you use
> to develop and use your product (think Codex computer-use, but where the
> session can be turned into e2e tests as well). You should be able to verify an
> agent's work **without running anything locally**, purely by looking at the
> e2e test and its output."

This is **not** "another background coding agent." It is a QA substrate: an agent
that operates the product the way a developer does — a real browser, a real
terminal — and whose work product is a **committed, reviewable e2e test plus a
playable video of it passing.** Review collapses to *read the test, watch the
video.*

## 2. Requirements (his list, organized)

1. **Real developer tools.** The agent gets a real Chrome, a real terminal, etc.
   — the same surface a human uses to build and exercise the product.
2. **Develop, then distill.** The agent can *develop/iterate* on the app using
   those tools, and once done can **turn the session into tests committed to the
   repo.**
3. **Pluggable targets.** The same (or near-same) test runs against multiple
   **targets** — "test the dev server," "test production" — without rewriting it.
4. **Fast + cross-environment.** Run quickly, and against different OSes —
   **macOS, Windows, Linux.**
5. **Open source + local.** Must be OSS and runnable locally.
6. **Video output.** Emit videos that play back the run so a reviewer can watch
   what happened.

Monetization he'd accept: a **hosted product** with VMs to run tests on; and/or
**GitHub Sponsors** (he is "so desperate for a good version of this" he'd sponsor
it outright).

## 3. The thesis (why this matters for agent work)

The deepest idea here is a **verification contract**: *the test source is the
review artifact.* When an autonomous agent claims it built or fixed something,
the reviewer should not have to clone, install, boot, and click. They read a
black-box scenario that reads like a product guarantee, watch the recorded run,
and see it pass against the real target. This is exactly the bar OpenAgents
already holds for Khala — *verified, not trusted; receipts underneath* — applied
to product QA. **A passing e2e run with video IS a verification receipt.**

The corollary that makes it agent-native: the agent earns trust by producing the
same artifact a human reviewer would demand, not by self-attesting. Computer-use
is how it explores; the distilled test is how it *proves*.

## 4. Prior art: `executor/e2e` (what's already strong)

The requester's own `executor` e2e suite (`projects/repos/executor/e2e`,
`e2e/AGENTS.md`) is further along than a prototype and is the right substrate to
extend. What it already nails — and what an OpenAgents build should adopt rather
than reinvent:

- **The `Target` interface** (`e2e/src/target.ts`): one deployed shape of the
  product seen purely from the outside — `baseUrl`, `mcpUrl`, `capabilities`
  (`api` | `browser` | `mcp-oauth` | `billing`), `newIdentity()` (fresh isolated
  user+org — isolation via fresh identity, *no resets*), optional `restart()` and
  `setAccessTokenTtl()`. **Scenarios are written once against this interface and
  run on every target** — directly satisfying requirement #3.
- **A target registry** (`e2e/targets/`: `cloud`, `selfhost`, `selfhost-docker`,
  `cli`, `desktop`, `cloudflare`, `local`) selected by `E2E_TARGET`; attach to a
  live instance with `E2E_<TARGET>_URL`.
- **Cross-OS VM substrate** (`e2e/src/vm/`, `VmOs = macos | linux | windows`):
  `tart` for macOS+Linux on Apple Silicon, `ec2` for ephemeral Windows; a
  `VmHandle` can `ssh`, `push`, **`reboot` for real**, and `tunnel` — so
  restart-persistence and boot-time auto-start are tested honestly. This already
  reaches requirement #4 (macOS/Windows/Linux), incl. the macOS GUI-session
  gotchas in `e2e/notes/testing-on-mac.md`.
- **Video + trace + screenshots.** Browser scenarios record `session.mp4`, a
  Playwright trace, and per-step screenshots (`failure.png` on failure). **The
  Desk** (`e2e/desk/run.sh`) films a scenario on one virtual Linux desktop — the
  chat renderer in a visible xterm, the browser as a real headed window, one
  `ffmpeg x11grab` — replacing `session.mp4` with a single film. A `viewer/`
  plays the scenario × target matrix and links traces into Playwright's viewer.
  This satisfies requirement #6.
- **Black-box discipline + review-by-reading** (`e2e/AGENTS.md`): drive only
  public surfaces (typed API, web UI, MCP, CLI); never import app internals or
  poke the DB; named `step(label, fn)` groups read as user actions; scenario
  names read as product guarantees ("Billing · the free plan stops org creation
  after 3"). This is the thesis in §3, already codified.
- **Deterministic, parallel-safe**: no sleeps (wait on conditions); per-checkout
  port locking (`e2e/src/ports.ts`) so concurrent worktrees never collide.
- **Recording tiers** for product-as-used films: chat-theater (real MCP calls,
  no inference), replay-brain (scripted LLM wire, real third-party client), and
  real-inference evals.

## 5. The gap (his "so much more potential")

What executor does NOT yet close is the **autonomous loop** — and that is the
whole request:

- Today, scenarios are authored by humans (or scripted), and the Desk *films*
  them. The missing piece is the **agent that drives the real tools itself** to
  develop/explore, **then distills that session into a committed scenario.**
- "Verify without running anything locally" needs the distilled test to be the
  *default deliverable of agent work*, with the video and trace attached — not a
  thing a human writes afterward.
- Faster runs, broader/cheaper OS coverage, and a hosted runner are scaling work,
  not architecture.

So the build is: **wrap executor's Target/VM/artifact substrate with an
autonomous computer-use agent and a session→scenario distiller.**

## 6. Proposed architecture (how OpenAgents could build it)

OpenAgents already owns most of the hard parts; the request is largely a
*composition*.

```
            ┌──────────────── one VM/sandbox per run (macOS/Win/Linux) ───────────────┐
  task ──►  │  Computer-use Agent (Probe runtime + Khala)                              │
            │    tools: real Chrome (CDP/Playwright), PTY terminal, fs, MCP, the app   │
            │    1) DEVELOP/EXPLORE the product through those tools (recorded)         │
            │    2) DISTILL the session → a black-box scenario against `Target`        │
            │    3) RUN the scenario on the run's target; emit video + trace + result  │
            └──────────────────────────────────────────────────────────────────────────┘
                         │ commits scenario to repo            │ artifacts
                         ▼                                     ▼
                 scenarios/*.test.ts                  session.mp4 + trace.zip + result.json
                 (the review artifact)                (the playable receipt)
```

- **The agent = Khala driving the Probe runtime + computer-use tools.** The model
  is **Khala** (`openagents/khala` — one model, no variants) end-to-end; Probe
  owns session lifecycle, tool execution, approvals, and transcripts. Give it the
  *same* tools a developer uses: a real Chrome over CDP/Playwright, a real PTY
  terminal, filesystem, and MCP. This is "Codex computer-use, but the session
  becomes a test" — and because Khala is the driver, the QA agent rides the
  verified-work + receipt machinery natively. **This is the example flow: Khala
  exercising OpenAgents Cloud VM/infra primitives to do real work and prove it.**
- **Develop → distill.** The exploration session is captured as a typed timeline
  (actions, selectors-as-intent, terminal commands, network/console). A
  **distiller** lowers that timeline into a black-box `scenario(...)` written
  against executor's `Target` interface — role-based locators, named `step`s,
  outcome assertions — i.e. it *generates the spec the reviewer reads.* The
  distiller's output must obey the executor quality bar (no tautologies, assert
  on values, deterministic waits).
- **Targets & OS matrix.** Reuse executor's `Target` registry and `src/vm`
  substrate. Map OpenAgents infra to the VM layer: `firecracker` microVMs and
  `sek8s` confidential GPU/k8s (both already tracked under `projects/repos/`) for
  Linux runners, `tart`/Apple-Silicon hosts for macOS, ephemeral cloud for
  Windows. `cloud/` (`oa-node` / `oa-workroomd`) is the natural home for managed
  run isolation and artifact custody.
- **Artifacts = receipts.** Each run writes `result.json` + `session.mp4` +
  `trace.zip` + step screenshots, dereferenceable like a Khala receipt. Reviewing
  agent work = open the receipt: read the scenario, watch the video, see green
  against the named target. No local run.
- **Open source + local-first.** The runner, distiller, and Target/VM substrate
  stay OSS and run on a developer's machine (or one self-hosted VM) — requirement
  #5. The hosted tier is purely *more/faster VMs*, never a lock-in for the core.

### Where it sits in the OpenAgents map

| Concern | Owner |
|---|---|
| Agent runtime + computer-use tools + approvals | `probe` / `packages/probe` |
| Model driving the agent (verified-work lane) | Khala (`openagents/khala` — single model) |
| Run isolation (microVM/confidential), artifact custody | `cloud` + `firecracker`/`sek8s` references |
| Target/VM/scenario/artifact substrate | study `executor` as prior art (no upstream PR) |
| Verification framing (run = receipt) | Tassadar verification-class + revenue-loop receipts |

## 7. Build plan (phased, honest-scope)

- **Phase 0 — adopt the substrate.** Stand up executor's `e2e` Target + VM +
  viewer against an OpenAgents surface (e.g. the `openagents.com` web app) as a
  consumer; prove "write once, run on dev + prod" with video artifacts. No new
  agent yet.
- **Phase 1 — computer-use agent.** Probe runtime + Khala drives real Chrome +
  PTY inside one Linux microVM; it can *develop/explore* a task and produce a
  recorded session (video + timeline). Review the film.
- **Phase 2 — the distiller.** Turn a recorded session into a committed black-box
  scenario against `Target`, meeting the executor quality bar. The deliverable of
  an agent task becomes *(scenario + video + green run)*.
- **Phase 3 — multi-OS + speed.** macOS (tart) and Windows (ephemeral) runners;
  parallel sharding; fast attach-to-running-instance loop.
- **Phase 4 — hosted runners + sponsors.** Managed VM pool for runs (the
  monetization), GitHub Sponsors for the OSS core, receipts wired to settlement.

## 8. Open questions (for the call)

- Distiller fidelity: how much of a recorded session becomes assertions vs
  narration, and how to keep generated tests from asserting implementation
  detail (the executor quality bar is the rubric — can the agent self-grade
  against it?).
- The Target contract for arbitrary external apps: executor's `Target`/`Identity`
  assumes the product can mint a fresh isolated identity. What's the minimal
  adapter a third-party app must provide (auth, fresh-identity, optional restart)?
- "Same tools you develop with": which exact computer-use toolset is in scope v1
  (Chrome + PTY + fs + MCP?) and what stays out (native desktop apps beyond the
  Electron `desktop` target?).
- Cost/perf envelope for the OS matrix; what's cheap enough to run on every PR.
- Trust: should a generated scenario require human acceptance before it counts as
  a guarantee, or is "green on target + video" sufficient (the §3 question)?

## 9. Next steps (build it as a Khala example flow)

The decision is made: **build this as an owned Khala example flow** demonstrating
Khala using OpenAgents Cloud VMs/infra. No PR to `executor`.

- **Headline demo (the example flow):** Khala (`openagents/khala`), running in an
  OpenAgents Cloud VM with real Chrome + a terminal, performs a real task against
  `openagents.com` (e.g. *log in and run a `/gym/oss` benchmark*, or *verify
  `/login` works*), then distills the session into a committed black-box e2e
  scenario — and the playable **video + green run** is the receipt. This single
  flow showcases Khala-as-agent-platform + Cloud VM isolation + infra + the
  verified-work/receipt model in one artifact.
- **Sequencing:** stand up the owned substrate (Target/VM/artifacts, modeled on
  `executor` as prior art) → give Khala the computer-use toolset in Probe → run
  the headline demo in a Cloud VM with video → add the session→scenario distiller
  → multi-OS + hosted runners.
- **Design partner:** keep the `executor` author in the loop as first
  customer / potential GitHub Sponsor and take the call when useful, but the
  deliverable is the owned Khala flow, not an upstream contribution.

## References

- Requester repo: <https://github.com/RhysSullivan/executor> · e2e dir:
  <https://github.com/RhysSullivan/executor/tree/main/e2e> · local clone:
  `projects/repos/executor`.
- `projects/repos/executor/e2e/AGENTS.md` — scenario/target/recording contract.
- `projects/repos/executor/e2e/src/target.ts` — the `Target`/`Identity`/`Capability` interface.
- `projects/repos/executor/e2e/targets/` — the multi-target registry.
- `projects/repos/executor/e2e/src/vm/` (`types.ts`, `tart.ts`, `ec2.ts`) — cross-OS VM substrate.
- `projects/repos/executor/e2e/desk/` + `viewer/` — virtual-desktop filming + run viewer.
- `projects/repos/executor/e2e/notes/testing-on-mac.md` — macOS GUI-session realities.
- OpenAgents: `docs/inference/khala.md` (verified-work/receipt framing),
  `packages/probe` (agent runtime), `cloud/` (managed run isolation),
  `projects/repos/firecracker` + `projects/repos/sek8s` (microVM / confidential runners).
