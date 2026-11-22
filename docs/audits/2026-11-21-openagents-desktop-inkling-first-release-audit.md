# OpenAgents Desktop: audit for an Inkling-only first release

Date: 2026-11-21. Audited at `origin/main` `053f4638f9`.

The plan this audit serves: the first release of OpenAgents Desktop runs on
one model, Inkling 2 Mini, on this Mac. Almost everything else goes behind
feature flags that are off. Benchmarks and evals measure Inkling only.

This document says what the app holds today, how Neon, Claude and Inkling are
wired, what one flag exists, what blocks an Inkling-only release, and what to
change in the benchmarks. It changes no code.

## 1. Summary

1. **The app is far wider than the release.** `apps/openagents-desktop/` is
   about 30,000 lines of Swift, 28,500 lines of Coax and 4,700 lines of C. It
   holds about 30 features beyond chat: Foreman, Gym, a wallet, pull requests
   and issues, an iOS Simulator pane, a terminal, a web preview, voice, MCP,
   scheduled tasks, remote environments and more.
2. **One flag exists.** `experimentalFeatures` hides the account and login,
   Gym and Foreman. Everything else is always on.
3. **Neon is the default in code, in the evals and in the README.** Inkling is
   one of up to 13 kinds of entry in the model picker, and it shows only after
   a 17.6 GB download on a Mac with 32 GB of memory or more.
4. **Other models appear on their own.** Claude, Antigravity, Codex and
   Cursor entries appear when their command is installed. Gemini models
   appear when the worker finds a Vertex or Gemini sign-in on the machine. An
   Inkling-only release must turn these off, not only hide settings rows.
5. **The Inkling path has gaps that matter more than the flags.** The
   numbers we have for Inkling were measured on `llama-server`. The app ships
   a different server, `psionic-inkling-serve`. The app starts it with an
   8,192-token context while the worker plans for 32,768. The download has no
   checksum. See section 5.
6. **The in-app Benchmarks tab is already Inkling-only.** The eval tiers that
   gate changes (Tier 2, Tier 4, session history) default to Gemini 4 Neon,
   cost money, and keep Neon baselines. See section 6.
7. **There is no finished Inkling pass rate.** The one completed run is 2 of
   9 tasks on a development slice, on `llama-server`. The 18 held-out tasks
   have not run on any Inkling setup.

## 2. What the app is today

The app is `OpenAgents.app`, bundle `com.openagents.desktop`, version 1.0.0,
macOS 14 or later. A Linux build (Wayland and Vulkan) and a Windows build
share the page and the desktop core.

| Part | Where | Size |
| --- | --- | --- |
| The page: everything drawn in the window | `page.coax`, `page/*.coax`, `page/layout/` | 25,500 lines, 38 check files |
| Settings | `settings.coax` | 1,000 lines |
| The Mac host | `*.swift` | 30,300 lines |
| The GPU carrier and C hosts | `host.c`, `host.h`, `host-main.c`, `carrier.c` | 4,700 lines |
| The desktop core bridge | `desktop-core.coax`, `host-core.coax` | 1,000 lines |
| Workers, built from `coder/tui/` | `mcworker`, `claudeworker`, `agyworker`, `coder-host` | bundled in `Contents/Resources` |

How a message runs: the page writes a task line, `Worker.swift` carries it to
a worker, and the daemon (`coder-host`) owns the worker so a reply goes on
after the app quits. Microcoder (`mcworker`) is the agent loop for Gemini,
OpenRouter and Inkling. Claude Code and Antigravity each have their own
worker.

The bundle also carries `gitguard`, `oasim`, `ple-run`, `lean-run`,
`tbench-run`, the Harbor adapter, `proof-export`, `capability-eval`,
`agents-catalog`, `tester-cli`, `foreman-ctl`, the Tier 2 and Tier 4 eval
tasks, and the Breez Spark framework.

## 3. Models today

### 3.1 The model picker

`page/models.coax` builds the picker. An entry shows when its condition
holds.

| Entry | Runs on | Shows when |
| --- | --- | --- |
| Gemini 4 Neon (the default), Carbon, Hydrogen, Argon | Microcoder, Vertex AI or the Gemini API | A Gemini key is set in Settings, or the worker finds a sign-in on the machine (`mcworker --auth`: a service account, ADC, or a key in `~/work/.secrets` or the environment), or experimental features are on and you are logged in |
| An OpenRouter model | Microcoder, OpenRouter | You set one in OpenRouter Models |
| Antigravity | `agyworker` | The `agy` command is installed |
| Inkling 2 Mini (Local) | Microcoder, the local Psionic Metal server | The weights are downloaded and the Mac has 32 GB or more |
| Hybrid Executor [E] | Microcoder, the same local server | Base weights and four weight modules pass SHA-256 checks |
| Inkling + Neon (Hybrid) | Local first, then Vertex Neon | Inkling is installed and a Gemini sign-in exists |
| Codex, Cursor | Their own CLIs | The command is installed (or `CODER_CODEX_BIN`, `CODER_CURSOR_BIN`) |
| ACP Registry agents | A registered binary | You add one in Settings |
| Claude Fable 5.2, Opus 5.5, Sonnet 5.5, Haiku 5.5 | Claude Code through `claudeworker` | The `claude` command is installed |

Sub-agents have their own levels (`page/state.coax` `CP_LEVEL`): hydrogen,
carbon, neon, openrouter, inkling, haiku, sonnet, opus, fable.

### 3.2 Where Neon is the default

- `CP_MODEL_DEFAULT` is 0, which is `gemini-4-neon`. A new chat, an unknown
  `--model` name and a removed OpenRouter model all fall back to it.
- Settings > Models > "New chats start on" has two choices: Neon or Claude.
- A send with no sign-in fails with "No Gemini sign-in found: set up Vertex
  AI…". With no model at all, the picker says "Install Antigravity CLI (agy)
  or download a Local Model".
- `evals/openagents-desktop/tier2/run_tasks.py` has `DEFAULT_MODEL =
  "gemini-4-neon"`.
- The README opens with "a conversation with Microcoder on Gemini 4 Neon"
  and does not mention Inkling.

### 3.3 How Claude is wired

Claude arrived in `56755c553b`. It has three parts:

- **A main model.** A chat on a Claude entry runs on Claude Code in place of
  Microcoder, through the Coax Claude Agent SDK (`coax/lib/agent/claude`).
- **Sub-agents.** Microcoder can hand a task to Claude on any of the four
  models.
- **A key.** OpenAgents > Claude Key… stores an API key or a Claude Code
  token in `~/Library/Application Support/Coder/claude-key`. Without it, the
  Claude login on the Mac is used.

Claude needs no Gemini sign-in. Its entries appear whenever `claude` is on
the path, with no setting to turn them off.

### 3.4 How Inkling is wired

Inkling arrived in `6789db8deb` (#13825, #13826). It is a 26.4B-parameter
looped mixture-of-experts model with 3.4B active, as a 17.6 GB `IQ4_NL` GGUF.

- **Download.** Settings > Models > Local Models starts `/usr/bin/curl -fSL
  -C -` from `huggingface.co/ggml-org/Inkling-2-Mini-GGUF` into
  `~/.openagents/models/inkling-2-mini/`. The file counts as installed when
  it is at least 16,000,000,000 bytes (`LocalModelStore`,
  `Window+Dialogs.swift`).
- **Server.** `LocalModelServer` starts `psionic-inkling-serve` from the
  bundle on `127.0.0.1:18794` with `--ctx 8192`, and waits up to 45 seconds
  for `/health`. The server speaks `/v1/chat/completions` and
  `/v1/responses` on Psionic's Metal kernels
  (`psionic/platform/inkling-serve.c`, `psionic/lib/inkling/kernels.coax`).
- **Run.** `Window+Runs.swift` sets `PSIONIC_BASE_URL` and
  `PSIONIC_MODEL=inkling-2-mini` for the worker. The task line has
  `local = true` and `agents = false`, so an Inkling chat has no sub-agents.
- **Worker.** `coder/tui/microcoder_keys.coax` plans a strict JSON schema
  request first for `inkling-2-mini`, then a plain one. `coax/lib/gateway/
  card.coax` gives the model a 32,768-token window.

Linux and Windows have no Inkling path: the runtime is Metal only.

### 3.5 The picker's index arithmetic

The picker has no list. Each entry's position is computed by counting the
entries before it, and the same counting is repeated in `cp_models`,
`cp_agy_idx`, `cp_inkling_idx`, `cp_hybrid_idx`, `cp_inkling_neon_idx`,
three places in `page/providers.coax`, `page/host-api.coax` and
`settings.coax`. Each one also has a "was on X, keep it on X" repair for when
the set changes. Adding flags on top of this multiplies the cases. Build one
list of entries once, and have every lookup read it, before adding the model
flags.

## 4. Flags today

One flag exists: `experimentalFeatures` (`Window+Settings.swift`
`Experimental.on`). It turns on with the Settings > General row, with
`defaults write com.openagents.desktop experimentalFeatures -bool YES`, with
`--experimental`, or with `OA_EXPERIMENTAL=1`. The page holds it as
`s[0].experimental`.

It gates:

- Log in, the account, and its menu items
- Foreman: its sidebar row, menu, settings page and control server
- Gym: its sidebar row, its four modes and its request route
- Gemini models for a logged-in account

Everything else has no gate. The other switches in the code are not feature
flags: `chatWorktrees` and `tailnetPairing` (both on unless turned off), and
test or path overrides such as `CODER_TEST_LAUNCH`, `CODER_UPDATE_URL` and
`CODER_AGY_BIN`.

The page's checks set `experimental = true` so they cover every feature.
New flags need the same: checks run with a flag on, plus one check per flag
that the feature is gone when it is off.

## 5. What blocks an Inkling-only release

These are in the Inkling path itself. Flags don't fix them.

1. **The measured server is not the shipped server.** The optimization lab
   (`docs/verification/microcoder-neon-inkling-lab.md`) ran Inkling on
   `llama-server` with a 32K context and grammar-constrained JSON. The app
   ships `psionic-inkling-serve`. That file has no handling of
   `response_format`, `json_schema` or a grammar, so the worker's strict
   request is not enforced there. The lab found that the lack of a strict
   schema caused empty first replies. Measure Inkling on the shipped server,
   or ship the server that was measured.
2. **Context is 8,192 in the app and 32,768 in the worker's plan.**
   `LocalModelServer` passes `--ctx 8192`. The worker sizes prompts and
   compaction for 32,768. Pick one number and use it in both.
3. **The download is not verified.** A file over 16 GB passes. There is no
   SHA-256 check, so a corrupt or changed file runs. The Hybrid Executor's
   store already verifies digests; Inkling's does not.
4. **A stuck start is silent.** The server's output goes to the null device,
   the port is fixed, and the health check needs the text `psionic-metal`. A
   `llama-server` on the same port (the lab uses 18794 too) gives a 45-second
   wait and then "Timed out". Keep a log and say what holds the port.
5. **No completed quality number.** Development slice, `llama-server`:
   Inkling 2 of 9 (2,098 seconds), Neon 4 of 9. The later Inkling run is
   marked in progress. The 18 held-out tasks have not run.
6. **First run assumes a cloud model.** With nothing set up, the app points
   at Antigravity and Gemini. An Inkling-only app needs a first screen that
   checks memory, offers the download, shows its progress and size, and
   opens a chat when it ends.
7. **Hardware limit.** Inkling needs Apple Silicon with 32 GB or more. On a
   smaller Mac the release has no model. The app must say so on first run.
   Linux and Windows have no first-release model.
8. **Three tracking issues are open:** #13823 (Inkling on Metal), #13824
   (tool calls and the Responses API), #13825 (Inkling in Desktop).
9. **Server lifetime.** The app starts the server; the daemon owns the
   worker. Check what happens to a running Inkling reply when the app quits.

Smaller items:

- `GymBenchmarkView.swift:1018` has a path under one person's home folder as
  a fallback for the server binary.
- `build.sh` builds the Inkling runtime twice: once at line 89 with errors
  ignored, once at line 140.
- `removeInkling()` deletes the weights with no check for a running reply.

## 6. Benchmarks and evals

### 6.1 What exists

| Surface | What it measures | Model today | Cost |
| --- | --- | --- | --- |
| Gym > Benchmarks (`GymBenchmarkView.swift`) | Psionic Metal against llama.cpp Metal on 10 coding tasks and two Lean suites: pass, wall time, tokens per second, time to first token, boot time, peak memory | Inkling 2 Mini on both sides | Free. Needs `~/.openagents/bin/llama-server` |
| Gym > Terminal-Bench (`GymTerminalBenchView.swift`, `coder/tui/tbench-run.coax`, `evals/terminal-bench/`) | Two arms side by side on 3, 12, 66 or 89 Harbor Docker tasks, and hillclimb rounds | Three policies, all `inkling-2-mini`, plus an "OpenRouter · Claude Sonnet" arm that has no policy file | Free for the local arms. Needs Docker and Harbor |
| Gym > PLE 3D, Gym > Lean · RH | PLE runs and Lean proofs, with a 3,500-line 3D flythrough | Not model benchmarks | — |
| Tier 0, Tier 1, compaction, follow-ups (`evals/openagents-desktop/`) | The page, the wire, 20 host scenarios, on a mock worker | None | Free |
| Tier 3 | 10 flows on the real app with a mock worker, 20 captures | None | Free |
| Tier 2 | 15 live tasks: single turn, recall, sub-agents, goal and budget, safety | Gemini 4 Neon, Hydrogen sub-agents | About $0.50 a run |
| Tier 4 | 4 golden trajectories and a 25-task slice | Gemini 4 Neon | About $1.20 a run |
| Session history (`evals/session-history/`) | 27 tasks mined from real sessions, five buckets | The lab compares Neon, Inkling, Qwen on a second machine, and two Neon and Inkling combinations | Neon runs cost money |
| Executor H7 (`psionic/tools/executor-h7-qual.coax`) | Synthetic checks reported under a Terminal-Bench name | None | Free. See `2026-11-21-terminal-bench-audit.md` |

All of Gym is behind `experimentalFeatures`.

### 6.2 What to change

1. **One model, one server.** Every live eval defaults to `inkling-2-mini`
   on the server the app ships. `run_tasks.py --model inkling-2-mini`
   already works (`host_driver.c` line 983); make it the default and have
   the runner start the server.
2. **New baselines.** `tier2/baseline/` and `tier4/baseline/` hold Neon
   results. Record Inkling baselines and compare against those.
3. **Time, not dollars.** Inkling costs $0, so the budget tasks (d2, d3) and
   the cost columns measure nothing. Replace cost with wall time, tokens per
   second and time to first token. Raise the 300-second task limit: Inkling
   took about 100 seconds on the two easiest tasks.
4. **Drop what Inkling can't do.** The sub-agent tasks (c1 to c3) can't run:
   an Inkling chat has no sub-agents. Mark them not applicable instead of
   failed.
5. **Finish the session-history run.** All 27 tasks, development and
   held-out, on the shipped server. This is the release's pass rate.
6. **Park the other arms.** Neon-only runs, the Qwen second device, the
   `inkling-neon` and `neon+inkling` combinations, `PSIONIC_PEERS` pooling
   and the OpenRouter Claude arm go behind the same flags as their models.
7. **Keep llama.cpp as the comparison.** The Benchmarks tab's Psionic
   against llama.cpp race is the right test of the runtime we ship. It
   stays, for developers.
8. **Report by memory size.** 32 GB is the floor. Record boot time, peak
   memory and speed at 32, 48 and 64 GB or more.
9. **Fix the names.** Rename the H7 buckets so synthetic checks don't carry
   the Terminal-Bench name, as the earlier audit says.

The release scorecard is then four numbers for Inkling on Psionic Metal: the
session-history pass rate, the Tier 2 pass rate on the tasks that apply,
tokens per second, and time to first token.

## 7. Feature inventory and the first-release state

"On" means part of the first release. "Flag" means behind a flag that is off.

### 7.1 On

| Feature | Files |
| --- | --- |
| Chat with Inkling: send, stop, steer, queue, the reply, tool rows, thoughts | `page/transcript.coax`, `page/followups.coax`, `Window.swift`, `Window+Runs.swift` |
| The local model: download, status, remove, the server | `Window+Dialogs.swift` (`LocalModelStore`, `LocalModelServer`) |
| Saved chats, the sidebar, rename, delete, fork | `Store.swift`, `page/chats.coax`, `page/persist.coax` |
| The working folder, attachments, `@` files | `page/attach.coax`, `page/mention.coax` |
| The file pane | `page/filepane.coax`, `Window+Files.swift` |
| The command palette and Find | `page/palette.coax`, `page/find.coax` |
| Permission and question cards, the to-do tray | `page/permit.coax`, `page/todos.coax` |
| Goal (`/goal`) | `page/goal.coax` |
| Runs that outlive the app | `DaemonClient.swift`, `coder-host` |
| Each chat in its own worktree, the git guard | `Worktree.swift`, `Window+Worktrees.swift` |
| Settings: General, Models (local only), About | `settings.coax` |
| Updates | `Updater.swift`, `Window+Update.swift` |
| Notifications | `Notifications.swift`, `page/notify.coax` |
| Agent instructions | `AgentInstructions` in `Window+Settings.swift` |

### 7.2 Behind a flag, off

| Flag | What it hides | Gated today |
| --- | --- | --- |
| `models.gemini` | Neon, Carbon, Hydrogen, Argon, the Gemini key, the sign-in probe, the Gemini sub-agent levels | No |
| `models.claude` | The four Claude entries, Claude sub-agents, Claude Key…, "New chats start on" | No |
| `models.openrouter` | OpenRouter Models…, its picker entry and sub-agent level | No |
| `models.antigravity` | The Antigravity entry and its four settings rows | No |
| `models.external` | Codex, Cursor, the ACP Registry, handoff and fan-out (`page/providers.coax`) | No |
| `models.executor` | Hybrid Executor, weight modules, checkpoint and shadow rollout, Local Pylon, settled sats, verified traces | No |
| `models.inklingNeon` | Inkling + Neon, and Inkling as a sub-agent of a cloud chat | No |
| `account` | Log in, the account, link this Mac's key | Yes (`experimentalFeatures`) |
| `foreman` | Foreman, its menu, tiers, control server, `foreman-ctl` | Yes |
| `gym` | Benchmarks, Terminal-Bench, PLE 3D, Lean · RH, `ple-run`, `lean-run`, `tbench-run` | Yes |
| `wallet` | The Spark wallet, the wallet RPC, the Nostr identity, the settle helper | No |
| `forge` | "PRs & Issues" in the sidebar, review tours | No |
| `git` | The Git actions toolbar, the diff viewer, checkpoints and rewind | No |
| `simulator` | The iOS Simulator pane and `oasim` | No |
| `terminal` | The terminal pane | No |
| `preview` | The web preview and SnapShot Region | No |
| `voice` | Dictation and its settings page | No |
| `mcp` | MCP servers, OAuth, the Skills catalog | No |
| `scheduled` | Scheduled tasks, webhooks, Update Everything | No |
| `environments` | Remote SSH, Boats, server exposure, keep awake | No |
| `pairing` | Pairing a phone over Tailscale (on by default today) | No |
| `usage` | The usage and quota dashboard, price overrides, the cost and budget display | No |
| `customize` | Themes, VS Code themes, keybindings, languages | No |
| `proof` | Thread Proof and the ATIF trace export, sync opt-in | No |
| `capabilities` | New Local Capability (two-arm eval) | No |
| `agents` | The Agents page (Coder, Tester, Game Builder) | No |
| `scenes` | Open 3D Scene…, the scene panel | No |
| `rich` | Prompt stash and history and the other composer upgrades in `page/rich.coax` | No |

`git` is the one worth a second look: a coding agent that edits files with
no way to see the diff is a weak first release. Recommendation: leave it
behind a flag for the first build, and make it the first flag to turn on.

## 8. Flag design

1. **One registry, in Coax.** A table of named flags in the desktop core
   (`coax/lib/agent/desktop/`), so macOS, Linux and Windows read the same
   list. Each flag has a name, a default and one line of description.
2. **Three ways to turn one on:** `defaults write com.openagents.desktop
   flag.NAME -bool YES`, `OA_FLAGS=name,name` in the environment, and
   `--flag name` at launch. `experimentalFeatures` stays as a developer
   alias that turns every flag on, so current workflows keep working.
3. **Off means gone.** No sidebar row, menu item, palette command, settings
   page, settings search hit, keyboard shortcut, slash command or model
   entry. Auto-detection does not run: with `models.claude` off the app
   does not look for `claude` at all.
4. **Off means not started.** No control server, wallet, pairing listener,
   webhook listener or probe starts for a feature that is off.
5. **The page gets one flags value** from the host, as it gets
   `experimental` today, and each part asks it. No part reads defaults
   itself.
6. **Checks.** Page checks keep running with every flag on. Add one check
   per flag that the feature is absent when off, and one check of the
   release set: a page with every flag off has Inkling as its only model.
7. **Kept chats.** A chat kept on a model whose flag is off opens
   read-only-on-that-model: it shows as it was, and the next message runs on
   Inkling and says so.
8. **Later: build-time exclusion.** Runtime flags are enough for the first
   release. The bundle still carries the workers and tools for hidden
   features. Leaving them out of the release build is a follow-up.

## 9. Order of work

1. Make the model picker one list (section 3.5).
2. Add the flag registry and move the three existing gates onto it.
3. Gate the models: `models.gemini`, `models.claude`, `models.openrouter`,
   `models.antigravity`, `models.external`, `models.executor`,
   `models.inklingNeon`. Make Inkling the default model.
4. Fix the Inkling path: one server measured and shipped, one context size,
   a verified download, a start that reports errors, a first-run screen.
5. Gate the rest of section 7.2, feature by feature.
6. Point the evals at Inkling (section 6.2) and record baselines.
7. Run the 27 session-history tasks on the release build and publish the
   number with the release.
8. Update the README and the specs (section 10).

## 10. Stale text found

- `apps/openagents-desktop/README.md` calls the app "Coder" throughout and
  describes it as a Gemini 4 Neon app. It does not mention Inkling, Codex,
  Cursor, ACP, the wallet, Gym's modes, voice, MCP or the terminal.
- `evals/openagents-desktop/README.md` names `apps/coder-desktop/build.sh`
  and `Coder.app`. The folder is `apps/openagents-desktop` and the app is
  `OpenAgents.app`.
- Chat files live in `~/Library/Application Support/Coder/`, and several
  environment variables keep the `CODER_` prefix. These still work; they
  are noted so a rename is a decision, not an accident.
- `docs/coder/desktop-evals-spec.md` cites the deleted `bench/terminal-bench/`
  (already listed in `2026-11-21-terminal-bench-audit.md`).
- `build.sh` line 89 says the Inkling server is "for Gym Benchmarks". It is
  now the release's model runtime.
