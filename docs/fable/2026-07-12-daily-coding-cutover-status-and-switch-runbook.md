# Daily Coding Cutover — Status Refresh and Switch Runbook

- **Date:** 2026-07-12
- **Author:** Fable (agent)
- **Status:** capability status refresh + operator runbook — not roadmap
  authority; sequencing stays `docs/sol/MASTER_ROADMAP.md`
- **Refreshes:** [`2026-07-11-daily-coding-capability-audit.md`](./2026-07-11-daily-coding-capability-audit.md)
  (the mining methodology, frequency data, and oracle matrix there remain the
  frequency authority; this document supersedes only its *status* snapshot)
- **Live status authority:** `apps/openagents-desktop/src/capability-registry.ts`
  enforced by `apps/openagents-desktop/tests/capability-evals.test.ts`. When
  this document and the registry disagree, the registry is right — that is the
  point of the registry.
- **Issue:** #8712 (EP250); cutover acceptance is CUT-27 #8707

## 1. What changed since the 2026-07-11 audit

The audit graded 40 daily-coding capabilities (mined from one month of real
`~/.claude` + `~/.codex` archives: 1.9M records, 12,266 human turns, 262,903
shell calls) against the desktop app at `c95b260c22`. In the ~24 hours since,
the EP250 lanes plus CUT-17 through CUT-24 landed directly against its ranked
gap list.

| Status | At audit (07-11, tables) | Now (registry on `main`) |
| --- | ---: | ---: |
| `ui_available` | 15 | **24** |
| `programmatic_only` | 4 | **0** |
| `partial` | 13 | **14** |
| `missing` | 8 | **2** |

Landed since the audit (each with registry row flips and oracles):

- **PTY terminal** (gap #1, 8,333 `write_stdin`/mo) — workspace-bounded PTY
  terminals + local preview lifecycle (CUT-20, `b57bf71fac`). D3 is
  `ui_available`.
- **Plan/todo progress** (gap #2) — live-updating plan/todo cards render and
  final plan state persists (EP250 wave-2, `6bddf331e6`). J2 `ui_available`,
  J4 `partial` (plan-mode toggle/review residual).
- **Typed Git/GitHub flows** (gap #3, 13,300 ops/mo) — commit/push/issue/PR
  typed UI surface (`a507c2ddfe`, reconciled `7416202ad8`) on top of CUT-19's
  conflict-safe review (`6ff137e5c0`). E2/E4/E5 are `ui_available`; E3
  worktree creation remains agent-driven.
- **Child interrupt** (half of gap #4) — interrupt-a-running-child is
  UI-driven; messaging an in-flight child stays blocked on provider surfaces
  (G4 `partial`).
- **User MCP servers** (gap #5) — MCP-servers settings panel (`cee701c03a`)
  over the wave-1 runtime substrate. I2 is `ui_available`.
- **History + search** (gaps #6/#11) — `~/.claude` import merged with
  `~/.codex`, source-tagged and loss-accounted, plus free-text title + bounded
  content search (`f94d202b8d`, CUT-22 `083f3f3654`). H3/H4 are `partial` only
  for residuals (workflow-journal edges surface as explicit gaps; content
  index bounded to most-recent sessions).
- **Image input** (gap #7) — composer image path (`fda911a6dc`). I1
  `ui_available`.
- **Skills** (gap #8) — typed local skills invocation (`20a200e786`). I3
  `ui_available`.
- **Stop button** (gap #9) — mid-turn interrupt is UI-driven (A2 `partial`
  only because queued-followup steering is A3's queue-until-idle, not
  mid-stream steer).
- **Editor** — grant-scoped documents, conflict-safe lifecycle, draft
  recovery (CUT-18, `091574d5bc`); Files workspace with lazy tree, search,
  mutations (CUT-17, `de0bb06ef7`).
- **Runtime/provider selection substrate** — verified bundled provider
  runtimes, exact provider-target binding, conversation-runtime selection
  (CUT-21 lanes, `25b35b4d24`/`655a0b772b`); a model *picker* is still absent
  (A4 `partial`).
- **Preferences, accessibility, notifications, diagnostics** (CUT-24,
  `64dcbb52e9`); permission-posture selection (`ff8cc0699b` lane); Fleet
  cockpit projection/controls/attention (CUT-25 lanes, `cc9cead0e1`,
  `bb170226b6`, `83efc87477`).

## 2. Current board — every non-`ui_available` row

The other 24 rows (A1, B1–B3, C1–C3, D1, D3, E1, E2, E4, E5, F1–F2, G1–G2,
I1–I3, J1–J3, K2) are `ui_available` with both oracles named. What remains:

| ID | Capability | Status | What is actually missing |
| --- | --- | --- | --- |
| A2 | Mid-turn interrupt | partial | Stop works; *steering* a live turn is A3's territory |
| A3 | Follow-up queueing | partial | queue-until-idle only; no mid-stream steer; local lane only |
| A4 | Model selection | partial | pinned `FABLE_LOCAL_MODEL`; `model_effective` visible, no picker |
| D2 | Background processes | partial | delegate children run async; no general background-process surface/indicator |
| E3 | Worktree isolation | partial | branch list/create/checkout UI wired; worktree *creation* is agent Bash only |
| G3 | Child completion notify | partial | no notification-on-complete surface |
| G4 | Message running children | partial | interrupt is UI-driven; messaging blocked (codex exec non-interactive; no per-subagent SDK message API) |
| G5 | Scheduled automation | partial | fleet workspace + pylon registry exist; no local scheduling / `/loop` equivalent |
| H1 | Resume | partial | SDK resume automatic per thread; no resume picker; Codex children never resume; live rung pending |
| H2 | Session fork | **missing** | no fork surface or seam |
| H3 | History import | partial | landed; Claude workflow-journal edges and async background-agent lifecycle surface as explicit gap/unknown |
| H4 | Session search | partial | landed; content index bounded to most-recent sessions, not whole archive |
| H5 | Compaction | partial | SDK auto-compacts; no UI marker/control, no boundary-integrity harness |
| I4 | File attachments / @-mentions | **missing** | no composer attachment or mention path |
| J4 | Task/todo tracking | partial | live plan/todo card renders; plan-mode toggle/review residual |
| K1 | Multi-workspace | partial | `workspace.choose` exists; single active workspace, no switching between concurrent roots |

**Rung honesty (audit §"six rungs"):** these statuses are code-landed and
fixture-proven, with live-proof receipts for a subset (`fable-turn`,
`fleet-workspace`, `fleet-usage-check`, redaction). The `thread-resume` and
`commit-push` live rungs are pending, distribution is unsigned (CUT-26 #8706
open), and owner cutover acceptance is CUT-27 #8707. Nothing below claims a
rung it has not reached.

## 3. Switch runbook — routing daily coding through the app now

The decision this enables: **default to OpenAgents Desktop for interactive,
single-repo coding sessions today**, keep the CLI consciously for the patterns
in §3.4, and convert every fallback into a registry/issue receipt (§4).

### 3.1 One-time setup

1. **Build and launch** (from the monorepo root, clean `origin/main`):

   ```bash
   bun install
   bun run dev:openagents-desktop     # builds dist/ and launches Electron
   ```

   Equivalent: `bun run --cwd apps/openagents-desktop dev`. There is no signed
   installer yet (CUT-26); running from source is the supported dogfood path.

2. **Connect isolated provider accounts.** Both lanes refuse the default
   provider homes by design — the app never touches `~/.claude` or `~/.codex`,
   so your live CLI sessions cannot be clobbered.

   - **Claude (Fable lane):** `pylon auth claude --token <setup-token>` (or
     `CLAUDE_CODE_OAUTH_TOKEN`); creates a sibling `~/.claude-pylon-<ref>`
     home. The lane discovers every ready sibling home deterministically.
   - **Codex (delegation + Codex lane):** `pylon auth codex` (paste-free
     device login) or `khala fleet connect`; isolated homes under the Pylon
     account registry. More distinct accounts = more parallel budget.
   - Verify: `pylon accounts list --json` shows `readiness.state: "ready"`
     rows; the app's Settings surface and Fleet workspace project the same
     registry (a status dot lights only from decoded fresh evidence).

3. **Pick the workspace root**: `⌘O` (`workspace.choose`) onto the repo you
   are working in. Grants are root-scoped; hidden/ignored/secret-shaped paths
   are withheld by contract.

### 3.2 The daily loop, mapped

| CLI habit (from the audit's frequency data) | In-app equivalent |
| --- | --- |
| `claude` / `codex` REPL turn | Chat workspace; harness chip picks **Fable** (Claude) or **Codex** lane per new chat (`⌘N`) |
| Mid-turn `Esc`/interrupt (240/mo) | Stop button on the streaming turn |
| Follow-up while running | type and send; delivered queue-until-idle at turn completion |
| `grep`/`rg`, `sed`/`cat` (90k/mo) | agent tools render as typed cards; Files workspace tree + search for human reads |
| Edit/patch (37k/mo) | agent Edit/Write cards; Files editor with SHA-256 revision-guarded save, drafts survive restart |
| `git status`/`diff` (16.8k/mo) | Review workspace: typed status/diff, staged/unstaged, discard-with-confirm |
| `git commit`/`push`, `gh issue`/`pr` (13.3k/mo) | typed commit/push/issue/PR surface (EP250 E2–E5) or agent-mediated |
| Interactive REPLs / `write_stdin` (8.3k/mo) | Terminal workspace: workspace-bounded PTY + local preview |
| Screenshots into the turn (347/mo) | paste/attach image in composer |
| `/loop`, skills (354/mo) | typed local skills from composer; **no `/loop` equivalent yet — keep CLI (G5)** |
| Stripe/Expo/docs MCP (~858/mo) | Settings → MCP servers panel |
| Subagents, cross-provider (2,027/mo) | Agent tool + `mcp__codex__delegate`; child cards, child interrupt |
| "find that conversation where…" | History workspace: unified `~/.claude` + `~/.codex` import, free-text search |
| Token/usage awareness | usage ledger per (provider, accountRef) in Fleet workspace |

### 3.3 Known caps to not re-discover

Read these before your first long session; they are constants, not bugs:

- `FABLE_LOCAL_MAX_TURNS = 16` agentic turns per user turn and
  `FABLE_LOCAL_TIMEOUT_MS = 180_000` per turn — a long autonomous burn will
  hit these caps by design; split work or delegate to Codex children
  (`FABLE_LOCAL_DELEGATION_TIMEOUT_MS = 600_000`).
- Model pinned to `claude-fable-5` (A4); Codex children pin their own
  model/effort constants.
- History context window into the model is the last 12 messages at 2,000
  chars each — very long threads compress hard; start fresh threads at task
  boundaries (there is no fork, H2).
- One active workspace root (K1); switching roots re-grants.
- Question cards time out at 10 minutes (`FABLE_LOCAL_QUESTION_TIMEOUT_MS`).

### 3.4 What consciously stays on the CLI for now

Route these through `claude`/`codex` (or the headless Pylon fleet) until their
rows flip, and say so in the fallback log (§4.1):

1. **Multi-repo / umbrella-workspace sessions and worktree fanout** (K1 + E3)
   — the app is single-root; the CLI worktree habit (1,079/mo) has no typed
   surface.
2. **`/loop`-style autonomous cadence** (G5) — 222 uses/mo; no local
   scheduler. The standing Pylon + fleet intake covers the *fleet* shape but
   not the local supervision loop.
3. **Long unattended burns** — the 16-turn/180s caps make the app the wrong
   host for hours-long autonomous work; use fleet dispatch
   (`pylon khala request --workflow codex_agent_task …`) which the app's Fleet
   workspace can then supervise.
4. **Session forking** (H2) and archive-wide content search (H4 residual).

Everything else defaults to the app first.

### 3.5 Fleet-scale work from the app

For work you would have run as a fleet wave: keep the standing owner Pylon
running (`pylon provider go-online`, heartbeat per the repo runbook), dispatch
through the Khala → Pylon → Codex path, and use the Desktop **Fleet
workspace** as the cockpit: pylon registry, account readiness/usage (decoded
fresh evidence only), authoritative work cockpit rows, confirmed runtime
controls, and attention resolution (CUT-25 lanes). Do not treat a lit dot or a
green row as completion — closeout receipts remain the completion truth
(`pylon khala closeout <assignmentRef> --json`).

## 4. Rough-edge improvement loop

The registry makes friction cheap to convert into permanent product pressure.
The discipline (from the audit §5, now enforced in code):

### 4.1 When you hit friction while dogfooding

1. **Log the fallback at the moment it happens** — one line: what you tried,
   which capability ID it maps to (A1–K2), and what you fell back to. If it
   maps to no existing row, that is itself the finding: the taxonomy has a
   hole, add a row.
2. **Classify it:**
   - *Regression* (a `ui_available` row failed) → the capability-evals suite
     should already be red; if it is not, the oracle is too weak — strengthen
     the oracle in the same change as the fix. Concrete reproducible bugs use
     the strict-bug issue form.
   - *Residual on a `partial` row* → check the row's note; if your friction is
     the noted residual, add weight to the owning issue (EP250 #8712 or the
     owning CUT issue) rather than filing a duplicate.
   - *New capability* → new registry row with status, both oracle references,
     and `blockedOn`; the meta-test forces the distribution to be updated
     honestly.
3. **Land fixes with the contract discipline:** every capability change
   updates `capability-registry.ts` and its two oracles in the same change
   (`ui_available` REQUIRES both oracles named and green; `missing` requires
   `blockedOn`). UX expectations stated in a session land in the
   behavior-contract registry per the repo mandate — never conversation-only.
4. **Verify gate:** `bun run --cwd apps/openagents-desktop verify` (typecheck,
   full test sweep, bundle build, headless Electron smoke) is the clean-tree
   gate for any desktop change. New human-visible capabilities extend
   `LiveProofStepName` in `src/live-proof.ts` and journal PNG receipts.

### 4.2 Priority order for the remaining rows

From the audit's frequency authority crossed with what agent-mediation cannot
substitute for, the burn order for the residue is:

1. **K1 multi-workspace + E3 typed worktrees** — the single biggest daily
   blocker for this owner's actual style (multi-repo, 1,079 worktree calls/mo).
2. **G5 local scheduling** — a typed `/loop` equivalent (recurring prompt +
   interval + stop condition) over the existing fleet/pylon seam.
3. **D2/G3 background-work ergonomics** — a background-activity indicator and
   notify-on-complete (CUT-24 landed the notification substrate; wire it).
4. **A4 model picker** — CUT-21's runtime/target binding landed; expose it.
5. **H1 resume picker / H2 fork** — the history workspace already renders the
   catalog; add "continue this thread" and "fork from here."
6. **I4 attachments/@-mentions** — composer path exists for images; extend to
   files.
7. **A3 mid-stream steer, H5 compaction marker, H4 full-archive index** — real
   but lower-frequency.

G4 child *messaging* stays parked on provider limitations (codex exec is
non-interactive; no per-subagent SDK message API) — revisit when the provider
surfaces move, and keep the interrupt path as the supported control.

### 4.3 Cutover exit

The switch is *declared* (not just practiced) at CUT-27 #8707: sustained owner
dogfood through the installed app, the capability registry green at its stated
rungs, CUT-26 signed distribution, and the legacy-flow lockout. Until then the
operating rule is: **app first, CLI consciously, every fallback becomes a
receipt.**
