# Handoff: solver, the open-issue loop, and what's in flight (2026-10-11)

For the next agent (cloud or local). It picks up the owner's standing goal:

> Continue using our "solver" flow programmatically to target issues that
> are open, or if conflicts, issues we closed as wontdo. Continue until no
> open issues. As you go, improve/edit our solver to be more reliable and
> cost efficient, including using a combination of models. Focus first on
> using our Claude models, not OpenRouter/paid stuff.

Owner rules that bind this work: see `CLAUDE.md` / `AGENTS.md` at the
workspace root and in this repo, plus these:

- Work on `main` and push.
- Never stash or reset another agent's work; use fresh worktrees from
  `origin/main`.
- Keep commit messages plain, ending with the `Co-Authored-By` line.
- No keyword routing.
- Use Bitcoin-only payments.
- Put no vendor model names in Coder's UI.
- Use plain user-facing copy.
- Make no TestFlight or desktop release builds unless asked.
- Production web deploys need the owner's gcloud login for
  promote/shift (see Deploys).
- Ignore the Clef issues (#11196, #11197, #11306): they belong to Codex.

## 1. What solver is

`coder-new` (the Rust terminal Coder, `crates/coder-new`) can take one
GitHub issue to a pull request in four Claude Agent SDK "hands":

| Hand | Model | Tools | Job |
|---|---|---|---|
| Intaker | Haiku | read-only `gh` (issue/pr list, view, search, project boards) | Picks or reads the issue. Lists and searches only reach issues created in the last 21 days unless a number is named. |
| Briefer | Haiku | Read, Grep, Glob | Judges the deterministic briefing's files and checks, writes the analysis, ends with `SIZE: small\|large`. Reads issue comments (handoffs say what's already done). |
| Coder | Sonnet when the Briefer judged the change small, Opus (`claude-opus-5-5`) for large changes and every hand-back | Read, Edit, Write, Grep, Glob, `run_check` (`check:` and `wasm:` entries) | Makes the change. |
| Tester | Sonnet | read-only plus `run_check` | Runs the checks and ends with `VERDICT: pass \| coder: … \| briefer: …`. |

Then the harness runs the final checks and opens the PR on a `coder/issue-N`
branch. The outcome comes from the required checks, not from the Tester's
verdict.

- Code: `crates/coder-new/src/solver/{mod.rs,hands.rs,tests.rs}`, plus the
  shared issue-run steps in `crates/coder-new/src/issue_run/{setup,decide,agent,verdict}.rs`.
- CLI: `coder-new solver N [--plain] [--json] [--no-pr] [--github OWNER/R]
  [--rounds R] [--coder-model M] [--briefer-model M] [--tester-model M]`.
  `--coder-model` pins one model for every round and turns off the
  Sonnet-first mix.
- In the app: Shift+Tab cycles auto → flash → microclaude → solver. In
  solver mode you chat with the Intaker, and an `ISSUE: owner/repo#N` line
  starts the run.
- **As a plugin** (`4d2ea102e8`, #11347): the live model in auto, flash or
  microclaude can call the `solver` tool. It takes `issue` and an optional
  `no_pr`, streams into a delegation child chat, and returns the outcome,
  PR, cost and run folder.
  - It is on by default. Turn it off with Space in /plugins or
    `plugins disable solver`; the choice persists.
  - Run it without the screen: `coder-cloud-runtime coder delegate solver
    --task "#N --no-pr"` or `coder chat --mode microclaude -p "..."`.
  - `CODER_SOLVER_REPLAY=<events.jsonl>` replays a recording (tests and demos).
- Runs live in `~/.openagents/coder-new/solver-runs/<issue>-<ts>-<id>/`:
  `events.jsonl`, per-hand `*-events.jsonl`, `briefing.md`, `analysis.md`,
  `summary.json` (with `solver.hands[]` per-hand cost),
  `change.patch` and `worktree/`. The shared build folder is
  `~/.openagents/coder-new/target`.

### Build and run it programmatically

```sh
cd ~/work/openagents   # or a clean worktree of origin/main
CARGO_TARGET_DIR=<your target> cargo build -p coder-new --bin coder-new
<target>/debug/coder-new solver 11348 --plain > /tmp/11348.log 2>&1   # needs a Claude login and gh auth
grep -a "Solver ended" /tmp/11348.log
python3 -c 'import json;s=json.load(open("<run>/summary.json"));print(s["cost"]["total_usd"],s["outcome"],[(h["hand"],h.get("cost_usd")) for h in s["solver"]["hands"]])'
```

Before merging a solver PR:
1. Read the diff.
2. Run the touched crates' tests yourself if the run predates
   `b45fbc5dc9`. Earlier runs required only builds.
3. Then `gh pr merge N --squash --delete-branch`. The PR closes its
   issue.

## 2. Solver changes today (all on main)

| Commit | What |
|---|---|
| 41143b93b5 | The Intaker reaches only recent issues (21 days) unless a number is named. Browser-build crates (cdylib with wasm32 code) get a required `wasm:` check. |
| 39f829730f | Final checks run required non-Rust checks and `wasm:` (before this they ran only `check:`, so a required link check always failed with "no result"). Hand-backs carry the failing checks' error lines. Later rounds get the diff so far, marked as their own work. |
| 77d4f1efc5 | Briefer `SIZE: small\|large`. A small change starts on a Sonnet Coder; large changes and hand-backs use Opus. |
| 7e7889e2ca | What must pass comes from the change: touched crates build, and changed Markdown gets `links:docs` (`scripts/coder/check_md_links.py`). The briefed crates' checks run without gating. |
| 9f82d1c3b4 | issue-run decider fails over between doors (Jev → OpenAgents decisions) on 402 or unreachable. |
| b45fbc5dc9 | Cost counts every hand (it was the Coder only). Issue comments reach the hands. **A touched crate's `test:` is required.** The Tester's passing checks aren't rebuilt. |
| 7051cf31d9, 0664f093cf, bf68045351, abce625033, 4af8292800 (earlier) | Touched-crate compile checks, a shared build folder with no doc tests, Claude-only solver, reopening not-planned issues, full-checkout worktrees. |
| 4d2ea102e8 | Solver plugin (see above). |

**Nested workspaces** (committed with this note): crates outside the root
workspace (`crates/openagents-mobile` is its own workspace) get `cargo
check/test --manifest-path <crate>/Cargo.toml` instead of `-p`, using
`cargo metadata --no-deps` for the member list (`workspace_members` in
`issue_run/setup.rs`). #11348's run failed only because of this.

### Results so far

| Issue | Outcome | Cost / wall | Notes |
|---|---|---|---|
| #11338 docs link | PR #11345, merged | $0.107 / 1.5 min | Sonnet Coder |
| #11282 CLI-02 | PR #11341, merged | ~$2.5 / 19 min | Opus |
| #11207 bunny presence | PR #11343, merged | $2.49 / 7.5 min | wasm half checked by hand |
| #11279 PAY-04 daily cap | PR #11351, merged | $1.94 / 25 min | ledger reserve/settle |
| #11319 Where it runs | PR #11352, merged | $0.27 / 18.6 min | needs the next web deploy |
| #11329 router rubric | PR #11350, **not merged** | $0.21 | breaks 4 `coder` digest tests; needs a calibration refit (see 4) |
| #11331 restart recovery | PR #11354, **closed** | $0.83 | its own new test failed; re-run was stopped mid-Coder |
| #11348 phone polish | failed | | `-p openagents-mobile` (nested workspace fix above) |
| #11349, #11356 | stopped mid-run | | for the session limit |

Wall time is mostly concurrent runs waiting on the shared cargo lock.
Run 2 at a time at most.

### Solver ideas not yet done

- **A baseline for required tests:** when `test:<pkg>` fails, rerun only
  the failing tests at the base commit, and don't blame the change for
  tests already broken on main.
- **A docs-only change that touches no crate** gets only `links:docs`.
  A change touching nothing ends "unchecked" with no PR. That's fine, but
  say "already done" when the Briefer finds the work on main from the
  comments.
- **Kill on host death:** Solver's own claude child processes should die
  when the host is killed outright. The normal stop is fine.
- **Make the Tester cheaper:** try Haiku; the harness already gates on
  checks.
- **#11346:** subagents talk to each other (see 3).

## 3. Open issues and where each stands

The loop: take open issues newest-first. If one conflicts with in-flight
work, take a recent closed-as-not-planned one instead. The solver reopens
not-planned issues itself.

- **#11348 Phone polish:** re-run the solver after the nested-workspace
  fix. Mobile tests use `cargo test --manifest-path
  crates/openagents-mobile/Cargo.toml`.
- **#11349 Phone New chat attachments:** needs `POST /v1/threads`, or
  image input in the chat worker, plus a web deploy. The solver run was
  stopped while writing `pages/chat.rs`; start fresh.
- **#11356 /sync off undone by another window:** small; a solver
  candidate (`crates/coder-new/src/account_sync.rs`, `crates/coder-sync`).
- **#11355 200-chat cap per account:** remove the cap for synced Coder
  chats and add paging. The phone list shows only 120. Server change plus
  a web deploy.
- **#11331 restart recovery:** PR #11354 was closed. The approach was
  `Runs::recover()` in `crates/coder-environment-operator/src/studio/claude.rs`
  at server start. Its test `a_server_restart_ends_a_run_left_mid_turn`
  failed at line 1399, and it broke
  `handing_a_paused_run_to_openagents_starts_coders_own_agent_never_claude`.
  Re-run the solver; tests are now required.
- **#11329 router:** PR #11350 (branch `coder/issue-11329`) has WIP
  commit 35acbe026b. It regenerates the route question set (new set
  `chat-router-v5@0fc444292c46`) and `sources.json`, and adds a
  `general.push_readme` golden (3 phrasings, all to work.dispatch on
  Gemini/Vertex).
  - Jev's calibration only has its set id updated, not a refit: TypeSafe
    answers 402.
  - The two **Clef** calibration maps still need a refit on CoderOS-4080,
    so `cargo test -p coder` still fails.
  - Production's chat judge is Clef on the worker VM `oa-coder-worker-1`
    (release 6818ae7d7d). That worker needs its own redeploy after the
    merge; the web deploy alone won't fix production.
  - The exact commands are in the #11329 handoff comment.
  - Rubric edits require regenerating `crates/gym/questions/chat-router-route-v*.json`,
    `crates/coder/fixtures/chat-router/calibration-v2.json` (and the clef
    record), `crates/openagents-chat-app/src/route_map/sources.json` and
    the chat goldens. Follow `docs/web/chat-goldens.md`; f666d394d6 is the
    model commit.
  - Use Vertex/Gemini for refits.
- **#11318 web chat refuses work:** the code is on main (36e8446e25,
  123ac0706d). It needs only the staging check of the auto hand-off and
  the "Run in Coder on {computer}" button (steps in its last comment).
  It waits on #11329, and production also needs the worker VM redeploy
  (see #11318's handoff comment).
- **#11324 web screens gallery:** 059762ec42 and 9c62735dbf are on main,
  not deployed. Every page is marked as a sample, forms and network
  requests are blocked, and a test checks that every served page is in
  the gallery or on a skip list with a reason. It will live at
  https://openagents.com/ui/screens.
  - Left: the full `openagents-web`/`openagents-ui` tests, light and dark
    screenshots, stage, promote/shift, then close the issue. Steps are in
    the #11324 handoff comment.
- **The Ice Cream Man (#11344, closed):** live at
  https://openagents.com/games/the-ice-cream-man (revision
  coder-web-45ef160d88-20261011210455). The guide is at
  /studios/blue-rush/the-ice-cream-man/how-to-play.
  - 41dad2ffe4 (a sturdier browser check) ships with the next deploy.
  - After the MVP: a key hint or minimap, the Giant Booger, co-op, and
    the Roblox port.
- **#11346 subagents talk to each other:** a plugin, on by default and
  toggleable. It gives every delegated subagent a shared channel, overlap
  detection, negotiation and system leases (cargo lock, staging slot,
  build host). The first use is a solver swarm, one solver per issue.
  Not started. Build it on the solver plugin and the delegation code
  (`plugin_tools.rs`, `bundled_runtime.rs`, `delegation_events.rs`,
  `src/solver_plugin/`).
- **#11308–#11313, #11323 (briefed-agent bench program):** the code is on
  main; frozen bench plans need running on oa-dev-env-2/-3
  (`scripts/bench/briefed-ab/run-env.sh`, `docs/inference/briefed-agent-ab.md`,
  `docs/inference/haiku-briefer.md`). A subagent had started #11323 → #11310
  → #11311 → #11309. Check its handoff comments.
  - #11308 needs the Codex/OpenCode engines.
  - #11313 is the acceptance run after #11308–#11311.
  - These are compute-heavy; keep them to Claude and Vertex.
- **Clef (#11196, #11197, #11306):** Codex's. Skip.

## 4. Done today, outside solver

- **Chat sync complete** (f90ddd386c, eae9f57817, #11353):
  - Every local Coder chat, past and new, and background-agent chats
    reach the account, and show on web and phone.
  - New command: `coder sync`.
  - Several open Coders no longer drop each other's `sync.json` records.
  - Not yet verified end-to-end: whether web and phone can drive a
    *running* terminal session live (send, see progress, stop). An agent
    was about to measure it when the session ended. Do this next: the
    owner asked "when can I actually use the mobile and web apps and
    interact with running terminal sessions". The building blocks are
    `crates/coder-new/src/account_sync.rs` (`answer_web_reply`,
    `poll_sync`) and the web's "Coder on {computer}" target.
- **Coder TUI:**
  - Native text selection and copy (709553b39a): drag, double-click,
    triple-click; copies on release.
  - Cmd+A can't reach Coder in Rex/iTerm/Terminal; use Ctrl+Shift+A.
  - Diff background bands, autoscroll, and the sync.json save collision
    are fixed.
  - Gateway errors name OpenAgents.
  - An OpenRouter 402 retries with an affordable output cap.
- **Merged PRs:** #11341, #11343, #11345, #11351, #11352. Also b33f896343
  (CLI tree regenerate).
- **Phone build 58:** code-complete (#11335 closed). The TestFlight
  upload and Android release APK are paused by the owner. The leftovers
  are #11348 and #11349.

## 5. Deploys

- **Stage:** `CLOUDSDK_CONFIG=~/work/.secrets/gcloud-sa-config
  scripts/deploy/web.sh stage`. Check first that no other stage is
  running: `pgrep -fl "web.sh stage"`.
- **Promote and shift:** run `scripts/deploy/web.sh promote --stack …` and
  `shift` with the owner's own gcloud login (no `CLOUDSDK_CONFIG`). The
  automation service account can't act as the production runtime account.
- **Watch:** check https://openagents.com/ and `/api/v1/models` for
  2 minutes after a shift, and `rollback` on any 5xx or 429.
- **Pending for the next web deploy:** #11352 (offline computers in
  Where it runs), the #11324 gallery commits, 41dad2ffe4, and #11329
  once refitted (plus its worker VM redeploy).
- **Latest production revision:** coder-web-45ef160d88-20261011210455
  (from f1db708b06).

## 6. Machine hygiene

- The Mac's disk hit 4 GB and 25 GB free today. Keep it above 30 GB.
  - Big users: `~/work/openagents-target-coderdev` (~120 GB, shared by
    several agents) and `~/.openagents/coder-new/target` (~68 GB, solver's
    shared folder).
  - Remove finished solver worktrees with `git worktree remove --force
    ~/.openagents/coder-new/solver-runs/*/worktree`, then `git worktree prune`.
- CoderOS: keep more than 50 GB free there too, and never delete
  `~/openagents/target/release` on that host.
- Concurrent cargo jobs on one target dir serialize on its lock, so run
  at most 2 solver runs at once per target.
