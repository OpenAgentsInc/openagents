# Orchestrator parity: do today's work from openagents.com, coder, and the apps

2026-10-09. Umbrella: [#11181](https://github.com/OpenAgentsInc/openagents/issues/11181).
Source session: [docs/traces/2026-10-09-session-3ba359c8](../traces/2026-10-09-session-3ba359c8/README.md).

## The ask

On 2026-10-09 the owner ran the company through one Claude Code conversation for 22 hours. The coordinator did all of the following:

- **Agents:** launched 180 background agents (168 in their own git worktrees, up to 17 at once) and relayed their results.
- **GitHub:** opened, closed and commented on issues, moved board statuses, and reviewed and merged PRs.
- **Code:** edited code, ran tests, and pushed to main.
- **Deploys:** deployed to staging and production with gcloud through a service account.
- **The owner's computers:** reached CoderOS over Tailscale for adb, screenshots and file copies.
- **Media and files:** analysed video (ffmpeg, whisper, A/V sync) and pasted screenshots, read local files and web pages.
- **Long-running work:** kept memory across sessions, ran wait loops and background shells, and rode out two compactions and a usage limit.
- **Traces:** uploaded traces.

The owner wants all of that from three places:

- the live website (openagents.com),
- the terminal (`coder`),
- our apps (desktop app, and the phone app on iOS and Android).

This page lists each capability, where it stands on each surface today, what is missing, the plain design, and the issue that builds it.

## Where each capability lives

- **Website:** the full workplace. It runs agents on a Cloud environment or on one of your computers.
- **Terminal (`coder`):** the same workplace in a shell. Agents run on this machine in worktrees, or are sent to a Cloud environment.
- **Desktop app:** hosts the terminal and runs agents locally. It is the "computer" that the website and phone reach.
- **Phone:** supervises. It shows the agent list, push notices, Approve/Deny, Stop and Message, chat, and a screenshot of a computer. It never runs agents itself.

## Status today (22 capabilities)

| Surface | Works | Partial | Missing |
| --- | --- | --- | --- |
| Website (live openagents.com) | 1 | 6 | 15 |
| Terminal (`coder`) | 8 | 9 | 5 |
| Apps (desktop / phone) | 2 | 9 | 11 |

The website count is low because much of the agent work is already built but answers only on localhost. `guard()` in `crates/openagents-web/src/lib.rs` keeps these routes local:

- `/environments*`
- `/chat/{id}/claude`
- `/chat/{id}/continue`

The deploy also never passes `--environments`. The live chat is a Q&A box with no tools (`crates/openagents-web/src/ask.rs`). Turning that work on is the first step ([#11162](https://github.com/OpenAgentsInc/openagents/issues/11162)).

## Parity table

Status words:
- **works:** a person can do it today.
- **partial:** some of it, or only in a narrower form.
- **missing:** nothing usable.

Paths are under `crates/` unless shown otherwise.

| # | Capability | Web | Terminal | App (desktop / phone) | Gap | Proposed design | Issues | Phase |
|---|---|---|---|---|---|---|---|---|
| 1 | A chat that does real work (tools, runs on a computer) | missing on live. Built but localhost-only: `openagents-web/src/lib.rs` `guard()`; `ask.rs` has no tools | works: `coder-new` tools `Run`, `openagents_cli`, `acp_subagent`, `microcoder`, `jev` (`bundled_runtime.rs:243-324`) | partial. Desktop: local Coder lane (`openagents-desktop/src/worker.rs`). Phone: only through a paired computer | Live web chat can't touch code or a computer | Serve Environments and Claude Code runs to signed-in users. Turn the composer's Model and Tools pickers back on (commented out in `pages/chat.rs`) | [#11162](https://github.com/OpenAgentsInc/openagents/issues/11162) | This weekend |
| 2 | Start several background agents at once, each in its own worktree | missing: one Claude Code run per chat (`pages/chat_work.rs`) | works: the `agent` tool and `/agent ENGINE TASK` start background agents in parallel, each in its own worktree and branch (`coder-new/src/fleet.rs`, registry in `crates/agent-fleet`) | missing | No fan-out anywhere | Background agent tool, a worktree per child, a disk floor, a shared build pool. See design (a) | [#11163](https://github.com/OpenAgentsInc/openagents/issues/11163), [#11164](https://github.com/OpenAgentsInc/openagents/issues/11164) | This weekend (coder), Monday (web) |
| 3 | Completion notices back into the chat | partial: a finished run's answer joins its chat (`observe`); sidebar status over SSE (`pages/chat_live.rs`) | works: each agent's report joins the chat as the next input, mid-turn or as a new turn (`coder-new/src/fleet_app.rs`) | partial. Desktop: OS notices for done, failed and question (`mac_notify.rs`, `win_notify.rs`). Phone: no agent push (iOS push is wallet-only, `PushRegistration.swift`) | Parent can't keep talking while children work | A child's final report arrives as a turn input in the parent. Push to the phone | [#11163](https://github.com/OpenAgentsInc/openagents/issues/11163), [#11164](https://github.com/OpenAgentsInc/openagents/issues/11164), [#11165](https://github.com/OpenAgentsInc/openagents/issues/11165) | This weekend to next week |
| 4 | Live list of running agents with status and cost; stop, resume, message | partial: Stop only (`/environments/{id}/runs/{run}/stop`), localhost-only | works: `/agents` panel with status, time, tokens and dollars; stop, message, resume; `agent_list`/`agent_message`/`agent_stop` tools | partial. Desktop/phone: stop and steer Coder tasks (`chat.rs:3937`). `openagents-chat-app/src/subagents.rs` is unused | No single agent list on any surface | One agent list API, shown as `/agents` in coder, an Agents panel on web, and an Agents screen in the apps | [#11163](https://github.com/OpenAgentsInc/openagents/issues/11163), [#11164](https://github.com/OpenAgentsInc/openagents/issues/11164), [#11165](https://github.com/OpenAgentsInc/openagents/issues/11165) | This weekend to next week |
| 5 | Hand work to Claude Code, Codex, Cursor, Grok | partial: Claude Code only, localhost-only | works: `acp_subagent` (`coder-new/src/acp_discovery.rs`), `openagents coder delegate` | works. Desktop: Codex, Claude Code, Grok Build, Devin, OpenCode (`docs/desktop/local-coder.md`). Phone: through the computer | Web picks no engine | Engine choice in the web agent fleet | [#11164](https://github.com/OpenAgentsInc/openagents/issues/11164) | Monday |
| 6 | Create, comment on and close GitHub issues | missing: GitHub connection reads repos and branches only (`projects/mod.rs`) | partial: `openagents issue claim\|release\|done\|status\|pickup` (`openagents-cli/src/issue.rs`); everything else is raw `gh` | missing | No create/comment/close verbs; nothing on web | Full CLI verbs, then the same typed tools in web chat with a confirm card | [#11166](https://github.com/OpenAgentsInc/openagents/issues/11166), [#11167](https://github.com/OpenAgentsInc/openagents/issues/11167) | This weekend (CLI), Monday (web) |
| 7 | Move GitHub Project board status | missing | partial: claim, release and done move status; no free move | missing | No "move to X"; no web | `openagents project add\|move`; web tool asks for the `project` scope when first used | [#11166](https://github.com/OpenAgentsInc/openagents/issues/11166), [#11167](https://github.com/OpenAgentsInc/openagents/issues/11167) | This weekend, Monday |
| 8 | Edit code, run tests, commit, push | missing on live (Claude Code on an Environment is localhost-only, `coder-environment-operator/src/studio/claude.rs`) | works: `Read`, `Edit`, `Write`, `Grep` and `Glob` tools beside `Run`, under the same approval policy; edits show as diffs (`coder-new/src/file_tools.rs`) | partial. Desktop: through local engines. Phone: through the computer | Web off | Turn web on | [#11162](https://github.com/OpenAgentsInc/openagents/issues/11162), [#11168](https://github.com/OpenAgentsInc/openagents/issues/11168) | This weekend, Monday |
| 9 | Review and merge PRs | missing | partial: only `gh pr` via `Run`; host-side `coder/src/review.rs`, `task/agent_host_merge.rs` | missing | No review or merge tool | `openagents pr review\|merge`; web tool with confirm | [#11169](https://github.com/OpenAgentsInc/openagents/issues/11169) | Next week |
| 10 | Deploy to staging and production, with approvals | missing | partial: shell gcloud works, but there is no deploy tool. Approvals only in programmatic chat (`coder-new/src/approval.rs`); default is full access | missing | No deploy ability and no production gate | Deploy plugin around `scripts/deploy/web.sh`. Policy: staging allowed, production asks the owner (TUI, web card, phone push). See design (c) | [#11170](https://github.com/OpenAgentsInc/openagents/issues/11170) | Monday |
| 11 | Run commands on your own computers | partial: replies to a synced Coder run on that computer (`coder_sync.rs`, `pages/chat.rs` `reply_to_coder`) | works: `openagents computer exec\|shell\|tail`, `openagents ssh`, `openagents host serve` (tailnet in `coder-host/src/tailnet.rs`) | works. Phone: remote terminal (`TerminalScreen.swift`/`.kt`); pairing by QR, code, SSH or Nearby. Desktop: is a host | Web can't run a command directly | Computer tools in web chat when a computer is linked | [#11171](https://github.com/OpenAgentsInc/openagents/issues/11171) | Monday |
| 12 | Screenshots and file transfer to and from computers | missing | missing: only `remote artifacts\|apply` for cloud jobs | missing | No screenshot, push or pull | `openagents computer screenshot\|push\|pull` over the host channel; inline images; phone Screenshot and Files | [#11171](https://github.com/OpenAgentsInc/openagents/issues/11171) | Monday |
| 13 | Analyse video and audio (probe, frames, transcription, A/V sync) | missing | missing: no ffmpeg or whisper tools (shell only if installed) | missing | No media tools; today's sync fix went the wrong way twice | `media` plugin: probe, frames, transcribe, av_offset (signed, against an independent reference), retime. See design (e) | [#11172](https://github.com/OpenAgentsInc/openagents/issues/11172) | Next week |
| 14 | Paste screenshots and images into the chat | missing: attach button commented out (`pages/chat.rs` `composer()`) | works: a pasted screenshot, a dropped path or an `@path` attaches; a model that takes images gets the image, any other gets a note with the file's path (`coder-new/src/attachments.rs`) | missing: off since 2026-10-01 (#10093/#10095); pipeline kept (`openagents-chat-app/src/attachments.rs`) | Images never reach a model | Send images to vision routes through the gateway; re-enable the composer pickers. See design (g) | [#11173](https://github.com/OpenAgentsInc/openagents/issues/11173), [#11174](https://github.com/OpenAgentsInc/openagents/issues/11174) | This weekend (coder), Monday (web, apps) |
| 15 | Attach local files and PDFs | missing | works: a dropped path or `@path` to an image or PDF attaches it (5 MB images, 20 MB PDFs) | missing | No file attachment | Path drop attaches in coder; web composer accepts PDFs and text files and stores them with the chat | [#11173](https://github.com/OpenAgentsInc/openagents/issues/11173), [#11174](https://github.com/OpenAgentsInc/openagents/issues/11174) | This weekend, Monday |
| 16 | Fetch web pages and search the web | missing: only the site's own docs at `/mcp/docs` | missing: only `curl` via `Run` | missing | No fetch or search tools | `web_fetch` and `web_search` tools shared by coder and web chat; refuse private IPs | [#11175](https://github.com/OpenAgentsInc/openagents/issues/11175) | This weekend |
| 17 | Memory that lasts across sessions | missing (ledger B10) | works: AGENTS.md/CLAUDE.md loaded nearest first plus `~/.openagents/AGENTS.md`; user and project notes with `remember`/`forget`/`recall` and `/memory` (`coder-new/src/memory.rs`); account sync is [#11182](https://github.com/OpenAgentsInc/openagents/issues/11182) | missing | No memory anywhere | Load AGENTS.md/CLAUDE.md; per-user and per-project memory entries synced to the account; Settings > Memory. See design (f) | [#11176](https://github.com/OpenAgentsInc/openagents/issues/11176) | This weekend |
| 18 | Background shells, wait loops, monitors | missing | partial: none in the TUI; host rules exist (`openagents background`, `openagents-cli/src/background.rs`) | partial. Desktop: Background page can pause and resume rules (`background_pane.rs`). Phone: watchers shown read-only (`computers_home.rs`) | The chat can't start a background command and be told when it ends | `Run` with background mode, a `monitor` tool, `/loop`. See design (h) | [#11177](https://github.com/OpenAgentsInc/openagents/issues/11177) | Monday |
| 19 | Scheduled tasks | missing: only the retention sweep | partial: host background rules | partial: view and pause only | Can't schedule a prompt | Scheduled prompts become host rules on a computer or Cloud environment; list and edit on web and in the apps | [#11177](https://github.com/OpenAgentsInc/openagents/issues/11177) | Monday |
| 20 | Capture the whole orchestration as a trace and upload it | partial: ATIF upload and `/trace/{id}` (`openagents-web/src/traces.rs`), but no parent-and-child tree | works: `coder trace upload\|list`, subagents as `subagent_trajectories` (`coder-new/src/trajectory.rs`) | partial: transcripts viewable; no upload | No tree view; Claude Code session import done by hand | `coder trace upload --claude-session`; tree on `/trace`. See design (i) | [#11178](https://github.com/OpenAgentsInc/openagents/issues/11178) | Monday |
| 21 | Chats follow you between surfaces | works: `/coder/sessions`, `/coder/sync` | works: `coder login`, `/sync on\|all\|off`, auto-update (`coder-new/src/account_sync.rs`, `update.rs`) | partial: no web-account sign-in on desktop or phone | Apps not on the account | Already tracked | [#11107](https://github.com/OpenAgentsInc/openagents/issues/11107) | (open) |
| 22 | Long sessions: compaction, usage-limit pause, cost in dollars | partial: plan hours and API spend limits (`plan.rs`, `api_keys.rs`); usage-limit pauses "not wired" (`chat_work.rs`) | partial: Retry-After handled (`provider.rs:863-1003`); no summarizing compaction; cost summed but not shown (`ui.rs:202-255`) | partial: engine and usage shown on desktop | Long runs can't compact; nobody sees $ | Summarize near the limit, pause and auto-resume on usage limits, $ per chat and per agent | [#11179](https://github.com/OpenAgentsInc/openagents/issues/11179), [#11164](https://github.com/OpenAgentsInc/openagents/issues/11164) | Monday |

Desktop terminal hosting has its own issue ([#11180](https://github.com/OpenAgentsInc/openagents/issues/11180), next week). The desktop app runs Coder locally but has no embedded terminal (`openagents-desktop/src/control.rs:43`) and no split panes.

## Designs, in plain words

### (a) Background agents

**One model on every surface.**
- An agent is a child chat with its own transcript, its own worktree and branch, an engine, and a place to run:
  - this machine,
  - one of your computers,
  - or a Cloud environment.
- The parent chat starts one with "background: on".
- The parent keeps talking.
- When the child finishes, fails or asks a question, a short notice with its report joins the parent chat as the next input.

**The agent list.**
- One list API, shown in three places:
  - `/agents` in coder,
  - the Agents panel in a web chat,
  - the Agents screen in the apps.
- Each row shows name, engine, where it runs, status, elapsed time, tokens and dollars.
- Actions: Stop, Message (steer a running agent), Resume (a stopped one), Open transcript.

**Guards learned from today.**
- No new worktree agent below a free-disk floor.
- Worktrees share a pool of build folders.
- A worktree is never removed while its agent runs. A lease in `coder-lease` enforces this.
- An agent's own wait loops end when it hands back, so one finished agent sends one notice.

**Web.** A chat can fan out N runs on one Cloud environment or on a linked computer through sync. Each run posts one result row.

**Phone.** It shows the same list and gets a push for done, failed, question and approval.

### (b) GitHub issues and the board

There is one set of typed GitHub tools:
- issue: create, comment, close, reopen,
- project: add, move,
- PR: open, comment, review, merge.

In the terminal they are `openagents issue|project|pr` commands, which the model calls. On the web they are chat tools that use the user's GitHub sign-in. The `project` scope is asked for only the first time a board tool runs.

Every write on the web shows a confirm card first. Closing an issue still moves its board item to Done (#11108).

### (c) Deploys with approvals

A deploy is a plugin ability that wraps `scripts/deploy/web.sh`: stage, promote the same image by digest, then smoke-test.

The policy is data:
- staging: allowed,
- production: ask the owner.

A production request stops and asks in three places:
- in the TUI,
- as a web confirm card,
- as a phone push with Approve and Deny.

The answer is recorded with the image digest and who approved it. The same gate covers other risky abilities: DNS, secrets and payments.

### (d) Your computers

Computers already pair and stay reachable through the host and Tailscale. We add three things to the host channel:
- `screenshot`, of the desktop or an attached Android device,
- `push` and `pull` for files, with a size cap and a sha256 check,
- run-command as a chat tool.

Screenshots show inline in the chat. The phone's Computers screen gets Screenshot and Files buttons.

### (e) Media

A `media` plugin runs on your computer or a Cloud environment. It has five tools:
- **probe:** streams and durations.
- **frames:** a contact sheet, returned as images.
- **transcribe:** whisper on the host, or the gateway.
- **av_offset:** audio-versus-picture offset and drift, signed, measured against an independent reference rather than the recorder's own meter.
- **retime.**

A fix is reported done only after av_offset re-measures it.

### (f) Memory

**What coder loads.** The repo's AGENTS.md and CLAUDE.md, nearest first, plus the workspace root file.

**What it keeps.** Small memory entries per user and per project, with an index like `MEMORY.md`. The tools are remember and forget, and `/memory` shows the entries.

**Sync.** With sync on, entries go to the account. The web chat reads the same memory, and Settings > Memory lists, edits and deletes entries. Delete means gone everywhere.

### (g) Pasted images and files

**Terminal.** Pasted images and dropped paths become attachments. They go to vision routes through the gateway, and on to Claude Code or Codex children.

**Web.** The composer gets paste, drop and pick for images, PDFs and text files. Files are stored with the chat, screened for secrets, and deleted with it.

**Apps.** The apps turn their attachment picker back on, using the same API.

### (h) Background and scheduled work

**Terminal.** `Run` gains a background mode: output goes to a file and a notice arrives when the command ends. A `monitor` tool turns each output line into an event, with a timeout. `/loop INTERVAL PROMPT` repeats a prompt.

**Scheduled prompts.** "Every weekday at 9" becomes a host background rule on a computer or Cloud environment. It posts into its chat. It is listed and edited on the web and in the apps.

### (i) Traces of a whole orchestration

coder already saves children as `subagent_trajectories`.

Two additions:
- `coder trace upload --claude-session` converts a Claude Code session folder into ATIF with every child, screened for secrets: the main JSONL plus `subagents/*.jsonl`.
- `/trace/{id}` shows the agent tree with per-agent tokens, time and cost.

Web agent fleets save the same shape automatically.

## Build order

**Start these five now, in parallel.** They touch separate files:

| Order | Issue | Main files | Why first |
|---|---|---|---|
| 1 | [#11163](https://github.com/OpenAgentsInc/openagents/issues/11163) coder background agents | `crates/coder-new` (tool loop in `provider.rs`, new agents module, `ui.rs`) | The core of today's session; every other surface reuses its agent list |
| 2 | [#11162](https://github.com/OpenAgentsInc/openagents/issues/11162) web: agent work live | `crates/openagents-web/src/lib.rs` `guard()`, deploy config, `pages/chat.rs` composer | Most of it is built; without it the website does no work |
| 3 | [#11166](https://github.com/OpenAgentsInc/openagents/issues/11166) CLI issue and board verbs | `crates/openagents-cli/src/issue.rs`, new `project.rs` | Small; unblocks the web GitHub tools |
| 4 | [#11176](https://github.com/OpenAgentsInc/openagents/issues/11176) memory | new `crates/coder-new/src/memory.rs`, the instruction assembly in `provider.rs` (lines 233-241, away from the tool loop) | Long sessions depend on rules and memory; small overlap with #1, easy to rebase |
| 5 | [#11178](https://github.com/OpenAgentsInc/openagents/issues/11178) orchestration traces | `crates/coder-new/src/trace_upload.rs`, `crates/openagents-web/src/traces.rs` | Separate files from #1 and #2 |

**Next, in this order:**
- **Remaining weekend items:** [#11173](https://github.com/OpenAgentsInc/openagents/issues/11173) images in coder and [#11175](https://github.com/OpenAgentsInc/openagents/issues/11175) web fetch/search. Both add tools to `crates/coder-new`, so start them after #11163 lands its tool-registry change.
- **Monday:**
  - web fleet [#11164](https://github.com/OpenAgentsInc/openagents/issues/11164) (needs #11162)
  - web GitHub tools [#11167](https://github.com/OpenAgentsInc/openagents/issues/11167) (needs #11166)
  - deploy approvals [#11170](https://github.com/OpenAgentsInc/openagents/issues/11170)
  - computers [#11171](https://github.com/OpenAgentsInc/openagents/issues/11171) (`coder-host`, `openagents-cli/src/computer.rs`; can start any time)
  - web attachments [#11174](https://github.com/OpenAgentsInc/openagents/issues/11174)
  - file tools [#11168](https://github.com/OpenAgentsInc/openagents/issues/11168)
  - background and monitors [#11177](https://github.com/OpenAgentsInc/openagents/issues/11177)
  - long sessions [#11179](https://github.com/OpenAgentsInc/openagents/issues/11179)
- **Next week:**
  - phone and desktop supervision [#11165](https://github.com/OpenAgentsInc/openagents/issues/11165)
  - PR review and merge [#11169](https://github.com/OpenAgentsInc/openagents/issues/11169)
  - media [#11172](https://github.com/OpenAgentsInc/openagents/issues/11172)
  - desktop terminal [#11180](https://github.com/OpenAgentsInc/openagents/issues/11180)

**Done when** the owner can redo today's session, start to finish, from openagents.com plus `coder`, and supervise it from the phone.
