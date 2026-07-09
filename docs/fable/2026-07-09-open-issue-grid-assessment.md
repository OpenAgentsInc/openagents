# Open-issue grid — every open issue, status, blocker, and what we are/aren't doing

Date: 2026-07-09 (early morning)
Status: Fable assessment (flips no promise state). One grid, all 30 open
issues in `OpenAgentsInc/openagents`, written after the overnight parallel
burn-down (MASTER_ROADMAP rev 6.5/6.6) so the states below reflect what
actually landed tonight, verified against `origin/main` and live issue
comments — not stale issue bodies. Sequencing authority remains
`MASTER_ROADMAP.md`.

## How to read the grid

**State** is my honest classification, not the GitHub label:

- **DONE-BUT-OPEN** — code complete and verified; the issue stays open only
  because its literal exit bar needs something outside an agent's reach
  (an owner tap-through, live infra, another lane's exit).
- **ACTIVE** — a lane is genuinely progressing it right now or has fresh
  landed increments with a clear next step.
- **READY** — unblocked, nobody on it; dispatchable today.
- **BLOCKED-OWNER** — waiting on a specific owner action (listed).
- **BLOCKED-INFRA** — waiting on real infrastructure that doesn't exist yet
  (listed precisely).
- **BLOCKED-LANE** — waiting on another issue's exit.
- **EPIC/TRACKING** — an umbrella; closes when its children do.

## The grid

| # | Issue | State | What's actually done | The blocker (precisely) | Next action |
|---|---|---|---|---|---|
| **8467** | EPIC: Khala Code Mobile-Only MVP | EPIC/TRACKING | P0 substrate proven (#8503/#8477 microVM + writeback), QAM-1..7 closed, store artifacts built (TestFlight build 20 valid, Android APK/AAB), credits metering live-proven on a real device | Closes when #8543 (its last gate) closes | Nothing separate — work #8543 |
| **8543** | P0.8 launch readiness | BLOCKED-OWNER | Chat→reply→credit half is device-proven; E2E harness exists | (1) **Owner:** seeded public-safe GitHub test account (NEEDS_OWNER has steps); (2) the pick-repo→push→writeback half of the straight line needs CX-3's in-VM Codex (see #8547); (3) promises/copy pass needs owner sign-off | Owner creates the test account; agents can then run the unattended E2E minus the Codex leg |
| **8546** | CX-2 mobile Codex connect | DONE-BUT-OPEN | Fully implemented + merged (`9b963db890`): mobile routes on the custody rail, Settings UI, multi-account, typed readiness, audited disconnect. Tests/typecheck verified | **Owner:** a real human device-auth tap-through (Settings → Connect → browser short-code → confirm → disconnect). Cannot and must not be automated against live credentials | 5-minute owner action; steps in NEEDS_OWNER.md — then close |
| **8547** | CX-3 Codex in agent computer | ACTIVE | Far more done than the issue body suggests: in-VM broker redemption, `CODEX_HOME`-on-scratch, org-capacity billing, owner-local `codex_app_server` lane all landed; `broker_only` law now threaded into the Rust crates (`8382569313`, cargo green); control plane live in prod from monorepo images (Phase 6, `546e3cf840`) | **BLOCKED-INFRA for the last mile:** no source-controlled rootfs build script for the `agent-computer-gce-1` guest image (hand-debootstrapped), and in-VM `codex_app_server` spawn unwired — needs a nested-virt bake host session. NOT blocked on the private repo anymore (#8591 resolved that) | A bake-host session: write the rootfs build script, bake codex into the image, wire the in-VM spawn, run one real turn |
| **8548** | CX-4 harness/target selection UX | DONE-BUT-OPEN | Picker fed by real accounts, typed dumb `auto` with fallback events (`0d20c34c70`), 452 mobile tests green | Live exit receipts (real connected account producing a provider-backed receipt; live auto-rotation across two accounts) gated on CX-2's owner tap-through + CX-3's live lane | Closes shortly after #8546/#8547 exits |
| **8549** | CX-5 Claude cloud parity | ACTIVE | Owner-local `claude_pylon` lane pre-existed; broker read side landed; tonight the missing **write side** landed (`6f1716823d`: Claude local-auth import route + custody storage + migration 0313; 175/175 tests verified) | Remaining: CLI `pylon auth claude` sync command; mobile "Connect Claude" UI (current UI is Codex-hardcoded); live proof shares CX-3's rootfs/in-VM wall | Small follow-up lane for CLI+mobile UI; live proof waits on CX-3 |
| **8550** | CX-6 session continuity across microVMs | READY | Nothing yet (unstarted) | None hard — the design (durable per-thread account pin + re-prime-and-replay from Khala Sync history) is implementable against landed substrate; a *live* proof shares CX-3's wall | Dispatchable now at fixture tier |
| **8551** | CX-7 multi-account concurrency in cloud | READY | Local-side per-account serialization exists in the fleet supervisor; cloud-side typed queueing unstarted | None hard for source+fixture tier; live tier shares CX-3's wall | Dispatchable now at fixture tier |
| **8552** | CX-8 daily-driver ergonomics | READY (partial substrate) | Steer/interrupt intents exist in the wire contract and now flow over Sync (MH-6); monorepo-scale checkout measurement unstarted | Real value needs CX-3's live lane (the point is steering *cloud* turns from the phone); phone-steering plumbing itself is landed | Fixture-tier lane possible; real dogfood waits on CX-3 |
| **8553** | CX-9 dogfood cutover (P2 exit) | BLOCKED-LANE | — | Definitionally last: needs CX-3 live + CX-2 owner-connected + CX-8 ergonomics. Exit is a full working day of our own coding through Khala Code mobile | Do not dispatch; it's the finish line, not a lane |
| **8558** | OB-1 Sarah sending identity/deliverability | BLOCKED-OWNER | Ramp/cap machinery + suppression substrate exist (LG engine); ledger (OB-6) ready to receive webhook data | **Owner:** sending-subdomain choice + SPF/DKIM/DMARC DNS records + Resend prod arming (`CRM_RESEND_SEND_ENABLED`, keys). All in NEEDS_OWNER | Owner DNS/arming session; then a short agent lane proves headers + opt-out round-trip + cap refusal |
| **8559** | OB-2 Apollo sourcing at volume | ACTIVE (mostly done) | Wave-ingest path landed (`6c7b9cdfe4`: business-pipeline-queue + routes + tests) | Exit bar: two ≥100-prospect **live** waves with clean attribution — needs a real Apollo MCP session driving real segment pulls (owner OAuth is connected; an agent session with MCP access can do this) | Run the two live waves; verify idempotency + suppression; close |
| **8561** | OB-4 draft→approve→send at 100/day | DONE-BUT-OPEN (API layer) | Batch queue/approve API + Sarah reply-routing plumbing + migration landed (`2d39c1faf5`), 49 tests; approval-gate invariant intact | (1) No operator **UI** yet — deliberately deferred to Effect Native rather than build throwaway React; (2) live Sarah email channel now in-repo via #8594's SM-3 rail (was cross-repo) | Small EN-native batch-approval UI lane (catalog is now rich enough); wire SM-3's email rail when it exits |
| **8563** | OB-6 daily sales ledger | DONE-BUT-OPEN | Full aggregation + admin route + Aiur ops panel + digest string landed (`c38b34869a`), 28 tests; honest `not_measured` sentinels | Exit bar: 7 consecutive days over **real sends** — needs OB-1's owner gates; digest cron delivery deliberately not bolted onto the shared scheduler yet | Sits ready; starts accruing the day OB-1 arms |
| **8565** | WEB-1 sales landing (HIGH) | BLOCKED-OWNER (structure done) | React replica + `/new` adaptation live; EN `/stage1` render live on staging; live counters real (pylon-stats repaired); EN parity work continues via EN-4 | **Owner:** final sales-copy sign-off, credit-tier pricing confirmation, root-route cutover decision (+rollback notes). Homepage copy is owner-gated by standing rule | Owner review session on the preview; then the root flip is mechanical |
| **8566** | EPIC EN: Effect Native adoption | EPIC/TRACKING | Massive movement tonight: upstream Phase 2+3 essentially done, Phase 4 catalog v19 (48 components, 13 issues closed in one lane), EN-1 `/stage1`, EN-3 mobile adapter proven, EN-4 first 2 routes converted, MH-7 desktop cockpit proof | Closes with its lanes; nothing separate | Keep the child lanes moving |
| **8570** | EN: deploy component gallery to Cloud Run | ACTIVE (other agent) | Owner said another agent is on this and stops after it | Unknown to me — that agent's lane; I have deliberately not touched it | Await that agent's report; fold into #8571 if it stalls |
| **8571** | EN: host effectnative.org | READY (source done) | Site source + static prerender landed upstream (effect-native #19, verified 181/181) | Deploy: Cloud Run service + **DNS/TLS for effectnative.org** (DNS is an owner action if the domain isn't already in our zone) | Dispatchable: containerize static output, deploy, NEEDS_OWNER the DNS record |
| **8572** | EN-2 catalog demand lane | ACTIVE (continuous) | Demand loop working in practice — tonight's consumers (stage1, cockpit, mobile screen) filed real gaps upstream (#44 fixed same-night); upstream catalog grew v5→v19 on demand | None — this is a standing process lane, not a completable unit | Keep enforcing "no local one-off primitives"; consider closing and folding into #8566's checklist since the process is running |
| **8573** | EN-4 web absorption | ACTIVE | Inventory + burn-down table committed (`dba2b55a38`); `/khala` + `/tassadar` converted with legacy deleted (`acac7e0ff3`, verified) | The big half (`apps/web`, ~55-70 Foldkit routes incl. 43k-LOC autopilot-work) is **explicitly blocked on a serving-cutover decision** — those routes are live production; converting them route-by-route needs the owner's cutover posture for the legacy SPA | Keep converting `apps/start` routes (safe); owner decides the `apps/web` cutover posture |
| **8574** | EN-5 Khala Code desktop conversion | ACTIVE (proof landed) | MH-7 cockpit proof screen landed (`b6cf6c78d0`) — first EN render in the desktop app, real DOM renderer, typed intents. BUT it consumed the old v5 vendored snapshot (EN-8's v19 catalog landed after it started) | Vendored effect-native snapshot in the monorepo is v5; upstream is v19 with everything a desktop needs (tabs, tables, transcript, composer, code editor, terminal, graph figure) | **Highest-leverage next EN lane: bump the vendored snapshot to v19**, then convert real panels |
| **8575** | EN-6 canvas/Verse under canvas contract | BLOCKED-LANE (thin) | Upstream `render-canvas` + `GraphFigure` landed (headless backend + adapter tested against fake port); live three-effect GPU wiring honestly stubbed (effect-native #22 open for that) | Live three-effect backend wiring upstream; then Verse consumers convert. Also sequenced behind EN-5's desktop panels (they host the figures) | Wire the live three-effect backend upstream first |
| **8578** | PY-1 pylon-core extraction | ACTIVE (4 sessions in) | shared/, custody/ (ALL modules now incl. account-connect + auth-health), executor/ (leaves + materializer + assignment-runs), presence/ (fully extracted via DI tonight), RPC contract seed. 16/16 pylon-core tests, baseline-identical app suite every commit | Remaining, precisely: codex-agent-executor/assignment/khala-spawn trace+move; `tips` boundary decision (payment-adjacent — needs a call on where it lives); MCP consolidation (plan written, execution needs its own pass); **wallet needs an RC-binary-capable session** (Spark WASM embed — owner mandate: extra care); CLI re-basing; PY-2's seam deletion | Next session: executor tail + MCP consolidation execution. Wallet only in a session that can run `build:rc-binaries` |
| **8579** | PY-2 desktop = Pylon cockpit | BLOCKED-LANE (partially pre-empted) | MH-7's cockpit proof IS the seed of this pane (same screen, same intents); PY-1's typed RPC contract seeded | PY-1's executor/daemon extraction far enough to delete the stdout-subprocess seam; EN-5's snapshot bump for real chrome | After the v19 snapshot bump + PY-1's next session |
| **8580** | PY-3 retire OpenTUI | BLOCKED-LANE | — | Explicitly gated on PY-2 cockpit-parity receipts (the TUI is the fallback operator surface until then) | Do not dispatch |
| **8588** | MH-9 cloud parity Grok/Claude workers | BLOCKED-LANE | MH wave (0/1/2/5/6/8) all closed; conformance suite ready to accept them | **Strictly after CX-3** — same rootfs/in-VM wall; its capacity must never be reassigned | Do not dispatch until CX-3's live lane exists |
| **8589** | MH-3 Grok Axis A (chat runtime) | ACTIVE (Grok's lane) | `GrokAcpChatRuntime` + session store + projector + `grok_runtime` desktop wiring + live ACP landed (`799b4794fe` etc.); real local dispatch verified tonight (9.5s round-trip) | Grok's own lane — deliberately not touched by our agents. Known debt their lane owns: the 5 desktop typecheck errors from worker-kind widening (a fix lane is running; may collide — it's instructed to yield) | Let Grok finish; hold them to the conformance suite (grok_cli fixtures are red-by-design waiting for them) |
| **8590** | MH-4 Grok Axis B (worker executor + RL probes) | ACTIVE (Grok's lane) | Headless executor real and dispatch-wired (`codex_spawn worker_kind=grok`, mixed fleet runs); RL-4 worktree probes landed; free-window auto-preference builder landed. Accounting groundwork landed on our side tonight (`4f157cc9ff`/`2b8e9f0db2`): `not_measured` is now a schema-legal usage-truth value AND explicitly excluded from the public tokens-served counter in SQL (proven by test) | Their lane. The remaining accounting **producer** gap is precise: the desktop can't POST to the ledger (admin-token route); Grok needs a registered-agent ingest route analogous to `/api/pylon/codex/turns` — the schema+counter now safely accept exactly that row shape. RL-1..3/5/6 probes remain | Grok finishes probes; a small server lane builds the Grok ingest route; then MH-1's grok_cli fixtures can go green |
| **8591** | Cloud repo consolidation | DONE-BUT-OPEN | Everything through Phase 6 executed **live**: crates in-repo, images built from monorepo, prod control plane redeployed + smoked, workroomd staged. Deploy path no longer touches the private repo. Residuals fixed same-night (build paths, script mode, docs) | Two honest leftovers: (1) full mobile Firecracker DoD not re-run this pass (control host is e2-small/no-KVM — that proof lives with CX-3's wall); (2) **temporary firewall 0.0.0.0/0:8787 on the control plane** — owner must confirm testing done, then tighten (NEEDS_OWNER) | Owner: firewall confirmation. Then this can close with the DoD note pointing at CX-3 |
| **8594** | Sarah consolidation into monorepo (SM-1..6) | ACTIVE | Fast progress: SM-0/1/2 checked off (apps/sarah on Bun, `/sarah/api/*`, S-3 guard ported, evals 6/6 CONFIRMED against local server, EN voice shell); SM-3 CRM email rail + continue-handoff landed (`31921bb3ab`) — connects OB-4/OB-5's cross-repo gap | Remaining lanes SM-4..6 (per its plan doc): prod serving cutover of `/sarah` on the monolith, Vercel decommission, DNS. Cutover + DNS are owner-adjacent | Let the lane run; the serving cutover decision rides with WEB-1's owner session |

## What we ARE doing (honest shape of current effort)

1. **Effect Native conversion at full speed** — upstream catalog exploded to
   v19/48 components tonight; first real renders live on web (`/stage1`,
   `/khala`, `/tassadar`), mobile (adapter screen), desktop (cockpit proof).
   The single highest-leverage next step is mechanical: **bump the monorepo's
   vendored snapshot v5→v19** so every EN consumer stops building composites
   out of Card/Text/Button.
2. **The Pylon fold** — pylon-core extraction is 4 sessions deep and most of
   the way there (custody + presence + executor leaves done); the daemon/
   cockpit product shape (PY-2) is seeded by MH-7's screen.
3. **Multi-harness** — the entire MH wave closed except Grok's own two lanes,
   which are actively progressing on their side. Grok delegation genuinely
   works locally today (tested); accounting + typecheck debt from their lane
   is being fixed on ours.
4. **Consolidation** — cloud (done, live) and Sarah (deep in progress) both
   collapsing into the monorepo; the private-repo era is ending.
5. **Sales substrate** — OB-2/3/4/5/6 machinery all landed; the funnel is
   built and tested end-to-end in test mode.

## What we are NOT doing (and why — check these against your intent)

1. **Not sending a single real outbound email.** Every OB lane is built to
   the owner gate and parked: DNS/subdomain/Resend arming (OB-1) blocks
   real sends, which blocks OB-6's real ledger and OB-4's live loop. **The
   entire sales track converges on one owner DNS/arming session.**
2. **Not running the mobile MVP's launch E2E.** #8543 waits on the seeded
   test account (owner) and the in-VM Codex leg (CX-3 infra).
3. **Not doing CX-3's last mile.** Everything source-side is done; the wall
   is real infrastructure (rootfs bake on the KVM host). This is the single
   blocker fanning out to CX-4/6/7/8/9 live tiers, MH-9, and #8543's full
   straight line. **One bake-host session unblocks five issues' exits.**
4. **Not cutting over production surfaces without you.** WEB-1 root flip,
   the `apps/web` legacy-SPA conversion posture, `/sarah` serving cutover,
   and the control-plane firewall tightening are all deliberately parked as
   owner decisions.
5. **Not touching Grok's two lanes** (MH-3/MH-4) beyond fixing debt that
   leaks onto `main` (typecheck, accounting) — claim hygiene.
6. **Not dispatching the finish-line issues** (CX-9, PY-3, MH-9) whose
   entire meaning depends on upstream exits.
7. **Not extracting the Spark wallet** until a session can verify the
   packaged RC binary — deferred three times now, on purpose, per your
   live-rail mandate.

## The three highest-leverage unlocks, ranked

1. **Owner: one gate-clearing session** (~30 min): CX-2 tap-through, seeded
   test account, OB-1 subdomain/DNS/Resend arming, WEB-1 copy/cutover review,
   firewall confirmation. This single session converts ~8 issues from
   blocked to closable/accruing.
2. **Infra: one bake-host session on `agent-computer-gce-1`**: rootfs build
   script + codex bake + in-VM spawn + one real turn. Unblocks CX-3 and the
   five issues stacked behind it.
3. **Agent: bump the vendored effect-native snapshot v5→v19** in the
   monorepo. Mechanical, immediately upgrades every EN surface (desktop
   cockpit, mobile screen, web routes) from primitive composites to the
   real component set, and unblocks EN-5/PY-2's real build-out.
