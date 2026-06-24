# Autonomous QA as a Khala example flow — computer-use agent that writes & runs e2e tests

> **Status: PARTIALLY BUILT — 2026-06-24. Not done.** A lot landed on `main`
> (the runner, drivers, distilled tests, OSS bundle, docs), but key pieces are
> partial, placeholder, or deploy-pending — most importantly the **shareable
> result surface**, which is being redesigned as an **agent trace** at
> `/trace/{uuid}` (ATIF) and is **not built**. See **§0** for the honest
> done / partial / not-done breakdown. Not a product promise or public-claim copy.
>
> _(original status line:)_ design exploration + planned **Khala example flow**, 2026-06-24.
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

## 0. STATUS — honest delivery map (done / partial / NOT done)

A lot landed on `main` overnight 2026-06-24 (epics #6174 + #6181, all child issues
closed) — **but "issues closed" is not "shipped and finished."** Several pieces are
partial, placeholder, or deploy-pending. Read this as the honest breakdown, not as
"all done." Legend: ✅ done + verified · 🟡 partial/placeholder · ❌ not done yet.

**The biggest not-done — the shareable result URL.** What shipped was framed as a
"chill-eval" at `/pro/evals/<id>`, and that's wrong three ways: (a) the page renders
committed **fixture data**, not real runs; (b) wrong name — this is an **agent
trace** (a session record), not an "eval" (a scored benchmark); (c) wrong URL —
a *shareable* artifact must not live under the `/pro/` operator console. The
redesign: a generic shareable **agent trace** at
`https://openagents.com/trace/{uuid}` in **ATIF** format —
spec'd in [`docs/traces/README.md`](../traces/README.md), **not built yet**. The
QA process emitting one real ATIF trace + a beautiful render is the current build.

Live + real today: <https://openagents.com/docs/autonomous-qa> ·
<https://openagents.com/QA-RUNNER.md> · the flow on PR #6197.

### His requirements → status (honest)

| # | He asked for | Delivered | Where |
|---|---|---|---|
| Thesis | "verify an agent's work **without running anything locally** — just read the e2e test + its output" | Commitments → **verify verdict** (CONFIRMED/REFUTED/INCONCLUSIVE) + a committed e2e test that's green vs prod; reviewer confirms from the PR artifacts | #6192, PR #6197 |
| 1 | Real dev tools (Chrome, terminal) | Computer-use: real Chrome (CDP/Playwright) + PTY terminal + container + native-desktop (macOS AX) drivers | #6175, #6186, #6199 |
| 2 | Develop, then **distill into committed tests** | Session → a committed, re-runnable `*.e2e.test.ts` (the differentiator vs videos-only) | #6174 distiller |
| 3 | Pluggable targets (dev/prod, same test) | Multi-target registry (dev/staging/prod/selfhost, prod read-only) | #6190 |
| 4 | Fast + cross-OS (mac/Win/Linux) | Parallel sharding + crash-safe harness; terminal/container/native-desktop/firecracker backends across OSes | #6193, #6186, #6199, #6200 |
| 5 | **OSS + local** | MIT `@openagentsinc/qa-runner` — self-contained bundle, **bring-your-own model, no OpenAgents login** | #6191 |
| 6 | Video output | Playwright `recordVideo` + a data-driven ffmpeg **compose** layer (title/verdict cards) | #6187 |
| 7 | "Chill evals" — compare agents across MCP/config changes | 🟡 the comparison **runner** computes real results — but the **shareable surface is NOT done**: `/pro/evals` renders fixtures, and it's being redesigned as a shareable **agent trace** at `/trace/{uuid}` (ATIF), see [`docs/traces`](../traces/README.md) | #6183 runner ✅ / trace surface ❌ |
| candid | "messy harness; the **last 10%**" | Reviewed, hardened harness (timeouts/retries/artifact-flush/parallel) + quality-bar doc | #6193 |
| money | hosted VMs + GitHub Sponsors he'd pay for | Hosted **Khala driver on own-infra at $0** (operator-credit exemption) + **QA control API** + `/pro` console + Cloud-VM runner/provisioner | #6180, #6196, #6184, #6176/#6200 |

### Extras we added beyond the ask
- **gh-attach inline PR videos**, repeatable in CI (every PR auto-embeds a Khala-QA video) — #6185.
- **OpenRouter** Effect provider so the driver is reliable + model-agnostic (`openrouter/free` $0 test) — #6182.
- **Failure-learning** (report strategies + evidence-only Blueprint/GEPA candidate-feedback) — #6195.
- **Run = verified receipt** → governed skill candidate → Blueprint run-record → INERT settlement seam — #6188.
- Anti-fabrication verify discipline; honest pass/fail everywhere; public docs.

### NOT done yet — be clear about this
- ❌ **Shareable trace surface.** The `/trace/{uuid}` page + ATIF store + `POST
  /api/traces` ingest is **unbuilt**; `/pro/evals` is fixtures. This is the
  headline gap. Spec: [`docs/traces/README.md`](../traces/README.md). In progress:
  the QA process emitting one real ATIF trace + a beautiful render.
- ❌ **"Eval"/comparison view** (compare N traces) — depends on the trace surface.
- 🔁 **Cloud execution substrate — corrected, was over-engineered.** The original
  plan (firecracker/sek8s provisioner needing a Linux **KVM host**, #6200/CND-056)
  was the wrong assumption: we run on **Cloudflare Workers**, which has native,
  GA, no-host-to-stand-up primitives — **Browser Rendering** (managed Chrome via
  `env.BROWSER` + Playwright/Puppeteer/CDP) for the browser, and **Sandbox SDK /
  Containers** (`@cloudflare/sandbox` / `@cloudflare/containers`, DO-bound, up to
  4GB+vCPU) for terminal/isolation. Verified against Cloudflare's full docs.
  The firecracker direction (#6200) is **superseded**; the real substrate is
  tracked in **#6205**. A local Docker container backend (#6186) covers it for now.
- 🟡 **npm publish** — `@openagentsinc/qa-runner` is publish-ready but **not on
  npm**; external install is clone-build / local-tarball until an owner runs the
  one publish command.

> droid-control / executor comparison and the requirement-by-requirement scorecard
> that motivated the build are in §5a–§5c below.

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
7. **"Chill evals" — compare agent performance across changes.** *(Added after
   the requester's follow-up, §5a.)* A first-class, low-ceremony way to answer
   *"I want to see how agents perform with these MCP changes"* — run the same
   scenario(s) across variants (a config/MCP-set/model change, before vs after)
   and see the comparison (pass-rate, latency, behavior deltas) with the videos.
   Evals, but exploratory and watchable — not a formal benchmark rig.

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

### What it looks like in practice — demo video play-by-play

The requester sent a ~18s screen recording (`dLqy5DZJ-OP4D5_D.mp4`, 3266×2160
retina, no audio) of **two merged GitHub PRs on `RhysSullivan/executor`, each
reviewed purely by watching the e2e recording embedded in the PR.** His words:

> "Here's what this looks like in practice — two PRs that were opened using my
> testing framework. The first one runs the real OpenCode flow, opens the auth
> URL in the browser, and returns. The second one is a recording of the web UI —
> once you experience it you realize how good it is."

This is the §3 thesis, live: the **PR body + embedded video are the review
artifact.** Frame-by-frame:

| ~t | What's on screen |
|---|---|
| 0–1s | **PR #1002** — "Self-host MCP OAuth: connect-card path, approval screen, real-agent e2e + framework dwells" (Merged, **+879 −19**, 20 files, 16 checks). Body "**The flow, recorded**": *"The real OpenCode binary connecting to a self-hosted instance over MCP OAuth — it asks to connect, you approve on the consent screen, it's connected. One spliced recording of the agent terminal and the browser (paced by the new `E2E_FILM` dwells)."* The embedded film opens in a **terminal**: `$ opencode mcp auth executor` → `MCP OAuth Authentication` → `Starting OAuth flow.` |
| 2–3s | Film cuts to the **browser**: a **"Connect OpenCode?"** consent screen — *REQUESTING CLIENT: OpenCode; will be able to: confirm identity, read basic profile, read email, stay connected (refresh without re-approving)*; an MCP-server note; **Deny / Allow** (Allow highlighted green). |
| 4–5s | Browser shows **"Authorization Successful — You can close this window and return to OpenCode."** (the real OAuth round-trip completed). |
| 6–7s | The film loops back to the terminal (connected) — proving the *real* OpenCode binary + *real* MCP OAuth + *real* browser approval, end to end, not a mock. |
| 8–10s | Recording scrolls to **PR #996** — "Adds a browser e2e scenario covering the tool **policy UI** end to end, and fixes a bug it surfaced in the tool-detail policy badge" (Merged, **+286 −14**, 4 files, 16 checks). The embedded **web-UI film** shows the executor console (BETA): left nav Integrations / **Policies** / Organization / Billing; the **Policies** page (*Override default approval behavior… Add policy*; Active policies: `…records.create` → **Block**, `…records.*` → **Require approval**). |
| ~13s | Film drives the **Records API** tools tree (Accounts / Tools tabs; `records` → `create` / `list`; a **List records** detail with Schema / TypeScript / **Run**, Parameters / Response) — the per-tool and category policy surfaces. |
| ~16s | Brief console **skeleton/loading** state; PR section heading **"The bug it caught"** is visible (the scenario surfaced a real tool-detail policy-badge bug, fixed in the same PR). |
| ~17–18s | Film shows the **Accounts** tab with **two connected workspace accounts** (`alphab…`, `betab…` keys, *Add connection*) — the scenario "connects two accounts and asserts the rules govern both," persisted server-side. |

**Why it lands (and what we adopt):** (1) the entire review is *watch the film +
read the PR* — no clone/install/click; (2) the films are real binaries / real
OAuth / real UI, not stubs; (3) the **`E2E_FILM` "dwells"** deliberately slow
machine-speed actions so a human can actually watch them — a small but decisive
UX detail; (4) one scenario both *demonstrates* a feature and *catches a real
bug* in the same PR. An OpenAgents build should produce exactly this artifact —
with **Khala** as the agent driving the recorded session and an OpenAgents Cloud
VM as the stage (frames extracted to
`scratchpad/vidframes/` during this analysis; the source mp4 is on the
requester's Desktop, not committed).

## 4. Audit of `executor/e2e` (the folder he linked)

The thing to study is `projects/repos/executor/e2e` (he linked
[`/tree/main/e2e`](https://github.com/RhysSullivan/executor/tree/main/e2e)). It is
not a toy — it is a well-factored cross-target QA harness. Folder-level map of
what's actually there:

| Path | What it is |
|---|---|
| `src/scenario.ts` | The ONE way a test is written. A scenario body is an Effect; **its requirements ARE its capability declaration** — it yields services (`Api`/`Browser`/`Cli`/`Mcp`/`Telemetry`/`Billing`/`Restart`/`TtlControl`/`OpenCode`) and *nothing else*. Yielding a service the target lacks surfaces as Effect's missing-service defect, which the runner **classifies into a vitest skip with the missing service named in the matrix**. Per run it writes a small `result.json` + whatever artifacts the surfaces produced. "Correctness lives in the test code and its vitest assertions — there is no recording layer." |
| `src/target.ts` | The `Target` interface (deployment seen from outside): `baseUrl`, `mcpUrl`, `capabilities` (`api`/`browser`/`mcp-oauth`/`billing`), `newIdentity()` (fresh isolated user+org — isolation via fresh identity, *no resets*), optional `mcpConsent`, `setAccessTokenTtl`, `restart`. |
| `src/services.ts` + `src/surfaces/{api,browser,cli,mcp,telemetry}.ts` | The capability services and their wire implementations. `browser.ts` = Playwright over the real web UI, dark mode, with **mp4 video + Playwright trace + per-step screenshots + `failure.png`**, wrapped in `acquireUseRelease` so a vitest timeout still flushes the video/trace instead of leaking Chromium. `telemetry.ts` boots a motel OTLP store and points the target's real exporter at it, so a scenario can assert on **spans the server actually exported** (catches "observability silently went dark"). |
| `src/vm/{types,tart,ec2,build-binary}.ts` | Cross-OS VM substrate. `VmOs = macos\|linux\|windows`; `tart` (macOS+Linux on Apple Silicon), `ec2` (ephemeral Windows). A `VmHandle` can `ssh`, `push`, **`reboot` for real**, and `tunnel` — so restart-persistence / boot-time auto-start are tested honestly, not faked. |
| `src/clients/{chat-theater,replay-brain,opencode,agent-chat-tui}.ts` | The "product-as-used" renderers for watchable recordings: chat-theater (real MCP calls, no inference), replay-brain (scripted LLM wire + real third-party client), opencode integration. |
| `src/{timeline,trace-harvest,ports}.ts`, `src/viewer/manifest.ts` | Timeline beats, trace harvesting, per-checkout **port locking** (concurrent worktrees can't collide), and the viewer manifest. |
| `targets/{registry,cloud,selfhost,selfhost-docker,cli,desktop,cloudflare,local}.ts` | The multi-target registry; pick with `E2E_TARGET`, attach to a live instance with `E2E_<TARGET>_URL`. |
| `scenarios/*.test.ts` (28) + `cloud/`, `selfhost/`, `local/` | Cross-target product journeys (api-tools, auth-methods, oauth-*, policies, mcp-execute, restart-persistence, …); the `cloud/`/`selfhost/`/`local/` dirs hold target-specific ones. |
| `setup/*.globalsetup.ts` + `*.boot.ts` | Per-target (and per-OS) boot: each app boots its OWN dev server or attaches to a running one. |
| `vitest.config.ts` | **One vitest project per target** (`cloud`, `selfhost`, `selfhost-docker`, `cloudflare`, `desktop`, `desktop-packaged`, `local`, `cli-{macos,linux,windows}`); `fileParallelism:false` for shared-instance targets; 180s–360s timeouts. |
| `desk/`, `viewer/`, `scripts/{film,pr-media,record-*,serve,summary}.ts` | The Desk (virtual Linux desktop: chat in xterm + headed browser + one `ffmpeg x11grab`), the matrix viewer with Playwright-trace links, and shareable PR-media generation. |

**What it already nails (adopt, don't reinvent):** the `Target` abstraction +
project-per-target ("write once, run on dev/prod/selfhost" — requirement #3); the
**capabilities-as-Effect-requirements → auto-skip + matrix** design (elegant — no
hand-maintained `needs` list to drift); the artifact set (mp4 + trace +
screenshots + `result.json`) with flush-on-timeout (requirement #6); the cross-OS
real-reboot VM layer (requirement #4); black-box + review-by-reading discipline;
port-locking for parallel runs; recording tiers.

> Audit catch (doc drift): `e2e/AGENTS.md` still documents the old
> `scenario("...", { needs: ["api"] }, ...)` signature, but `src/scenario.ts`
> moved to requirements-as-capabilities (no `needs`). Minor, but it's exactly the
> single-source-of-truth lesson we just applied to OpenAgents routing — keep the
> contract and its doc coupled.

## 5. Where it stops, and how OpenAgents improves on it

What executor does **not** close — and where an OpenAgents build adds value:

1. **No autonomous loop (the core ask).** Scenarios are human/scripted-authored;
   the Desk *films* them. Missing: **an agent that drives the real tools itself to
   develop/explore.** → OpenAgents adds **Khala** as the driver (Probe runtime +
   computer-use tools), so exploration is autonomous.
2. **No session→test distiller.** Recordings are films, not lowered into committed
   scenarios. → Add a **distiller** that turns a Khala session timeline into a
   black-box `Target` scenario meeting the executor quality bar — making the
   *committed test the default deliverable of agent work*, not a human afterthought.
3. **Runs aren't receipts (no verification/economic layer).** A green run + video
   is trusted by reading; there's no independent verifier, tamper-evidence, or
   metering. → OpenAgents wraps each run in the **Tassadar verification-class +
   revenue-loop receipt** model: a run becomes a *dereferenceable, optionally
   independently-verified receipt* — which also unlocks the **hosted-VM
   monetization** natively (metered run → settled receipt).
4. **VM substrate is bespoke + heavy** (tart/ec2; macOS needs a real VNC GUI
   session per `notes/testing-on-mac.md`; cost/perf-per-PR unaddressed). →
   OpenAgents makes per-run isolation a **Cloud primitive**: `cloud` (`oa-node` /
   `oa-workroomd`) over `firecracker` microVMs / `sek8s` confidential runners, so
   cross-OS and scale come from infra we already own — this is the literal "use
   VMs and infra from OpenAgents Cloud" the example flow demonstrates.
5. **Target-adapter onboarding is undocumented for arbitrary apps.** `Target`
   assumes the product can mint a fresh isolated identity (+ optional restart). →
   Define the **minimal adapter** a third-party app must provide (auth,
   fresh-identity, optional restart) so any product can be a target.
6. **Computer-use is browser-centric.** Beyond Chrome it's only the Electron
   `desktop` target. → "the same tools you develop with" may need a broader
   computer-use surface (arbitrary GUI in the VM), which the Cloud-VM substrate
   makes natural.

So the build is: **keep executor's elegant Target / capabilities / artifact model,
add the Khala-driven computer-use agent + session→scenario distiller, and run it
on OpenAgents Cloud VMs with each run wrapped as a verified receipt.**

## 5a. The requester's candid assessment — where the "last 10%" is

Asked directly *"what's wrong with your implementation?"*, the requester was
refreshingly honest, and his answer sharpens exactly where OpenAgents adds value:

> "Mainly that it's **messy and I haven't actually reviewed the test setup that
> much**. It also doesn't cleanly handle things like *'I want to see how agents
> perform with these MCP changes'* (sorta evals but in a chill way). My setup is
> **decent but the last 10% would go so far** on this."

Three takeaways, each a concrete target:

1. **Messy / under-reviewed harness → a clean, opinionated, *reviewed* harness is
   the product.** His substrate is decent but he hasn't hardened or audited the
   test setup itself. The OpenAgents version's job is the **last 10%**: a typed
   (Effect) harness whose own test setup is reviewed and trustworthy, with the
   ergonomics that make the good ideas usable — not a from-scratch reinvention.
   This is why we *adopt* his Target/capabilities/artifact model (§4) rather than
   rebuild it, and spend our effort on the finishing: the autonomous Khala driver,
   the distiller, the receipt/verification wrapper, and the eval mode below.
2. **"Chill evals" is a missing first-class mode (new requirement #7).** *"See how
   agents perform with these MCP changes"* is a **comparison across variants**, not
   a single pass/fail run: hold the scenario fixed, vary the agent's config — an
   MCP-server set, a tool policy, a model/brain, a before/after of a change — and
   show the delta (pass-rate, latency, behavior) with the videos side by side.
   Crucially it's *chill*: exploratory and watchable, not a formal benchmark with
   ceremony. This maps cleanly onto what we already have: the `qa-runner`'s
   brain × backend × **target** already varies the run; add a **variant axis**
   (MCP/tool/config/model) and a comparison report, and reuse the benchmark
   harness's percentile/aggregate math (`inference/benchmark/report.ts`) for the
   numbers. It is the executor multi-target idea generalized — vary the *agent*,
   not just the deployment — and it is the daily-driver loop an MCP author (his
   actual job — `executor` is an MCP tool catalog) wants.
3. **"The last 10% would go so far" = the opportunity is finishing, not founding.**
   The high-leverage work is precisely the parts he hasn't built: the autonomous
   develop→distill loop, a reviewed harness, and the chill-eval comparison. That
   is the brief.

Implications folded into the rest of this doc: requirement #7 above; a **variant
axis + comparison report** added to the build plan (§7) and architecture (§6, the
runner already has the brain/backend/target seams to hang it on); and an explicit
"reviewed, typed harness" quality bar as a first-class deliverable, not an
afterthought.

## 5b. Prior art #2 — Factory `droid-control` + Automated QA (and where we differ)

After the executor exchange, someone from Factory pointed Rhys at **`droid-control`**
("droid-control is fully OSS"). It is the closest thing to this concept shipped by
a serious team, so it's worth a hard look. Local clone:
`projects/repos/factory-plugins/plugins/droid-control` (Factory-AI/factory-plugins).
There are two related Factory pieces:

- **`droid-control`** (OSS plugin): terminal/browser/desktop automation with three
  commands — **`/demo`** (polished before/after Remotion videos), **`/verify`** (test
  a claim as an *investigator* → CONFIRMED/REFUTED/INCONCLUSIVE), **`/qa-test`**
  (drive a flow → step-level pass/fail table). Architecture: a thin command parses
  args into **commitments**, an orchestrator routes by *target × stage × artifact*
  to small **atom skills**, and every workflow flows **capture → compose → verify**.
  Four drivers: `tuistory` (virtual PTY), `true-input` (real terminal emulator via
  Wayland/KVM/QEMU), `agent-browser` (Playwright+CDP, web/Electron), `desktop-control`
  (native via `trycua/cua`). Video via **Remotion** (23 components, 6 presets).
  Per-OS platform subdocs (linux/macos/windows).
- **Automated QA** (Factory built-in `/install-qa` + `/qa`, NOT in the OSS repo):
  generates a `config.yaml` (environments, personas, `path_patterns`→apps,
  `production: read-only` restrictions), runs diff-scoped per-app sub-skills, posts a
  PR-comment report, and has **failure learning** (suggest / auto-commit / open-PR).

### What we should LEARN from it (adopt these)
1. **Commitments → capture→compose→verify, with `verify` checking the artifact
   against the original commitments.** A command starts as a checklist it must
   satisfy, not "make something impressive" — an explicit anti-drift guardrail. Our
   qa-runner should adopt "commitments verified at the end of the run."
2. **The `/verify` investigator + anti-fabrication framing** ("if the claim is
   false, that's a valid finding; never stage evidence to match the expected
   outcome"). This is *exactly* our "verified, not trusted / no fake green / honest
   FAIL" discipline — independent validation of our thesis. Worth codifying a named
   `verify` verdict (CONFIRMED/REFUTED/INCONCLUSIVE) in the runner.
3. **Driver breadth + platform isolation.** Four drivers across web/Electron, PTY,
   real-terminal, and **native desktop**, with per-OS subdocs — broader surface and
   OS coverage than our current Chrome+PTY. Generalize our backend abstraction this
   way (req #4); native-desktop (`trycua/cua`) is the notable add.
4. **Remotion-grade polished video.** Data-driven props (the droid never writes
   JSX), before/after side-by-side, title cards, keystroke overlays, presets. Our
   videos are raw Playwright `recordVideo` (evidence-grade); theirs are demo-grade.
   A **compose/polish layer** (data-driven, not hand-written) is worth stealing for
   shareable demos — directly serves requirement #6 and the "send to Rhys" use case.
5. **Diff-scoped runs + `config.yaml` single source of truth + `production:
   read-only` restriction + failure learning.** Map the git diff to affected
   apps/scenarios; treat prod as read-only by policy; feed new failures back. All
   good ergonomics for our runner and the `/pro` console.
6. **"UX for droids" context discipline** — load only the atom relevant to the
   current step (temporal relevance), not one giant prompt. A clean principle for
   our skills/tools.

### Does it EXACTLY fit what Rhys wants? (scorecard vs §2)
| Req | droid-control | Note |
|---|---|---|
| 1. Real dev tools | ✅ | terminal/browser/desktop drivers |
| 2. Develop → **distill into committed tests** | ❌ | produces **videos + pass/fail reports**, *not* a checked-in, re-runnable test file. `/qa-test` reports a table; nothing lands in the repo as a regression asset. (Automated QA generates markdown sub-skills — prose flows, still not executable committed tests.) |
| 3. Pluggable targets (dev/prod) | ✅ | `config.yaml` environments |
| 4. Cross-OS | ✅✅ | stronger than us — native desktop + real terminal + 3 OSes |
| 5. OSS + local | ⚠️ | **see caveats below** |
| 6. Video | ✅✅ | Remotion, polished — stronger than us |
| 7. **"Chill evals" — compare agents across MCP changes** | ❌ | no variant-comparison/eval mode. `/demo` does before/after of two **branches** (a code change), never before/after of an **MCP/agent config**. Rhys's actual job (MCP-tool author) is exactly this — uncovered. |

**The "OSS + local" caveats (req #5, ⚠️):** the plugin source is public, but
(a) there is **no LICENSE file** in the repo — "fully OSS" is asserted, not
licensed; (b) it is **coupled to Factory's `droid` runtime** — the commands are
`droid` slash-commands and the architecture states "the agent operating the tool
is also the runtime," so you run it *inside Factory's agent*, not as a standalone
library; (c) **`agent-browser` is Factory's own CLI** (docs.factory.ai), not a
public OSS tool; (d) **Remotion requires a paid company license** for orgs >3
employees. So it's "OSS-ish plugin source, Factory-runtime-locked" — not a
framework Rhys can drop into his own CI without adopting Factory's agent.

### Where OUR approach has extra benefits (beyond droid-control)
1. **Distill → a committed, re-runnable e2e test** (req #2, the thing droid-control
   does NOT do). We emit a video *and* a checked-in executor-style `Target` test
   that runs in CI forever. Evidence becomes a **regression asset**, not an
   ephemeral artifact. This is the core differentiator.
2. **Run = a verified, dereferenceable receipt** tied to the Tassadar verification
   floor and (later) Bitcoin settlement — runs become an economy (pay-per-run,
   hosted VMs, author rev-share), not just files. droid-control's evidence is
   ephemeral.
3. **Khala as the driver — model- and runtime-agnostic, on our own infra.** Driven
   by our own model on our own hourly hardware (zero per-token cost; the
   balance-gate exemption), OpenAI-compatible, not locked to a vendor agent. A real
   LICENSE, runnable locally or on OpenAgents Cloud VMs without a Factory dependency.
4. **The "chill evals" / variant-comparison mode** (req #7) — vary the MCP set /
   tool policy / model and compare. The exact thing droid-control lacks and Rhys
   most wants.
5. **The `/pro` hosted operator console** — a product surface to watch runs, review
   distilled tests, and (later) the capability marketplace; not just a CLI plugin.

### Net
droid-control **validates the thesis** (evidence-first, investigator/anti-
fabrication, capture→compose→verify) and is **ahead of us on surface/OS breadth and
video polish** — we should learn both. But it is **not** what Rhys ultimately wants:
it doesn't distill into committed tests, has no agent/MCP-change comparison eval,
and is Factory-runtime-coupled with license caveats. Our edge is committed-test
distillation, run-as-receipt+settlement, a model/runtime-agnostic own-infra driver,
chill-evals, and the `/pro` console — while adopting droid-control's
commitments→verify contract, multi-driver/cross-OS breadth, and Remotion-grade
compose layer.

## 5c. Tooling to adopt: `gh-attach` (the "post evidence to the PR" glue)

The Factory engineer also pointed at **`gh-attach`** (`ain3sh/gh-attach`, Go; local
clone `projects/repos/gh-attach`) — "a neat lil tool that will help with any of
these PR QA pipelines." It is small and exactly fills a gap we have.

**What it does:** uploads the same attachment kinds GitHub's web UI supports for
issues/PRs/discussions — **images, video, PDFs/code/archives** — and returns the
embeddable result (`--md` markdown links, `--auto` `![](url)` for images, `--url`
raw, `--json`). One command: `gh-attach clip.mp4 report.png --repo owner/repo --json`.
Installable as a `gh` CLI extension (`gh attach …`).

**Why it matters for us:** GitHub's REST API does **not** natively support
attaching media to issue/PR comments — gh-attach drives the web upload path that
does. Our `qa-runner` already produces a `session.mp4`, screenshots, and a
`result.json`; the missing step is **posting that evidence into a PR/issue comment**
the way Factory's Automated QA and droid-control do (inline screenshots/video in the
report). `gh-attach` is that glue: distill → run → `gh-attach` the video+shots →
post the report comment with the media embedded + the link to the committed e2e
test. It also serves the same role inside the `/pro` console's run-sharing.

**Adopt-not-vendor:** it's a tiny standalone Go binary (MIT-class), runtime-agnostic
(no Factory/`droid` coupling, unlike droid-control), so we can use it directly in
the QA CI step / `qa-runner` reporting without taking on a dependency on anyone's
agent. Add it to the Phase 2.5/CI reporting step.

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
- **Phase 2.5 — "chill evals" (variant comparison, requirement #7 / §5a).** Add a
  *variant axis* to the runner (MCP-set / tool-policy / model-brain / before-after
  of a change) and a comparison report: same scenario(s), N variants, side-by-side
  pass-rate + latency + behavior deltas with the videos. Reuse the benchmark
  harness aggregates (`inference/benchmark/report.ts`); keep it low-ceremony and
  watchable. This is the daily-driver loop for an MCP author ("how do agents do
  with these MCP changes?").
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
- `projects/repos/executor/e2e/AGENTS.md` — scenario/target/recording contract (note: doc-drift on the `needs` signature, see §4).
- `projects/repos/executor/e2e/src/scenario.ts` — the test harness (capabilities-as-Effect-requirements → auto-skip + matrix; `result.json` + artifacts).
- `projects/repos/executor/e2e/src/target.ts` — the `Target`/`Identity`/`Capability` interface.
- `projects/repos/executor/e2e/src/services.ts` + `src/surfaces/{api,browser,cli,mcp,telemetry}.ts` — capability services; `browser.ts` = Playwright + mp4/trace/screenshots with flush-on-timeout.
- `projects/repos/executor/e2e/targets/` — the multi-target registry; `vitest.config.ts` — one project per target.
- `projects/repos/executor/e2e/src/vm/` (`types.ts`, `tart.ts`, `ec2.ts`) — cross-OS VM substrate.
- `projects/repos/executor/e2e/scenarios/` (28 cross-target journeys) + `cloud/` / `selfhost/` / `local/`.
- `projects/repos/executor/e2e/desk/` + `viewer/` + `scripts/{film,pr-media}.ts` — virtual-desktop filming, run viewer, shareable media.
- `projects/repos/executor/e2e/notes/testing-on-mac.md` — macOS GUI-session realities.
- OpenAgents: `docs/khala/khala.md` (verified-work/receipt framing),
  `packages/probe` (agent runtime), `cloud/` (managed run isolation),
  `projects/repos/firecracker` + `projects/repos/sek8s` (microVM / confidential runners).
