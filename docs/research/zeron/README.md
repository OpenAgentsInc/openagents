# Zeron desktop parity gap analysis

Assessment date: 2026-10-15.

The subsequent [Coder Desktop plan](../../coder-desktop/plan.md) selects a
separate Zeron fork as the first OpenAgents SDK reference client. This report
preserves the original comparison and remains the workflow checklist; its
porting sequence is not the default implementation route for the new app.

OpenAgents has a working chat desktop with local Coder, saved conversations,
streaming, questions, approvals, queues, image input, change review, a terminal,
settings, and desktop packaging. It does not yet provide Zeron's integrated
coding workspace. Full parity requires a substantial expansion of the desktop
application, especially file editing, source control, remote workspace control,
provider selection, browser previews, and voice.

This report compares the checked-in implementations at these revisions:

| Repository | Revision | Baseline |
| --- | --- | --- |
| OpenAgents | `2a4c682f209089cfc67f042f7f360003705ea305` | `main`, 2026-10-15 |
| Zeron | `6ac8859998909bc5afd55838afb0635c281f31c3` | Local `~/zeron`, version `0.2.113`, 2026-10-14 |

Both checkouts were clean when the investigation began. These are source
baselines, not claims about binaries installed on a person's computers or the
versions running in production. The report documents gaps; it does not implement
the parity work.

## Read the report

| Document | Contents |
| --- | --- |
| [Scope and evidence](scope-and-evidence.md) | Comparison rules, confidence, implementation boundaries, and source references |
| [Navigation and chat](navigation-and-chat.md) | Shell, projects, sidebar, composer, queue, transcript, and conversation navigation |
| [Workspace and source control](workspace-and-source-control.md) | Files, editor, search, worktrees, actions, diffs, Git history, and pull requests |
| [Agents and remote work](agents-and-remote.md) | Provider coverage, models, accounts, sessions, desktop remote control, synchronization, and terminals |
| [Media and appearance](media-and-appearance.md) | Images, capture, voice, browser, previews, interactive visuals, themes, typography, and settings |
| [Platforms and quality](platforms-and-quality.md) | Platform differences, installation, updates, accessibility, lifecycle, performance, and validation |
| [Delivery plan](delivery-plan.md) | Dependency order, implementation work packages, acceptance scenarios, and completion rules |
| [Coverage map](coverage-map.md) | Current Zeron surface inventory, cross-references, stale documentation, and remaining investigation |
| [Coder SDK integration](coder-sdk-integration.md) | Existing SDK contracts, desktop intersections, proposed extensions, ownership boundaries, and first implementation slices |
| [Coder Desktop plan](../../coder-desktop/plan.md) | Separate Zeron fork, OpenAgents SDK consumption, ownership, delivery stages, and external-client acceptance |

The register contains **174 assessed items**: 68 missing desktop workflows,
80 partial workflows, 24 verification gaps, and 2 product differences requiring
a decision. These are planning items of different sizes, not a feature score.

| Register | IDs | Items |
| --- | --- | --- |
| Navigation, composer, transcript | NAV-01–14, COM-01–16, TRN-01–14 | 44 |
| Files, projects, source control, pull requests | WRK-01–20, GIT-01–16 | 36 |
| Providers, remote work, terminals | RUN-01–16, REM-01–12, TERM-01–08 | 36 |
| Media, browser, voice, visuals, appearance | MED-01–06, WEB-01–08, VOI-01–07, VIS-01–03, CUS-01–16 | 40 |
| Platform/lifecycle and performance | OPS-01–12, PERF-01–06 | 18 |

## The most consequential gaps

1. **A workspace beside the conversation.** Zeron combines a file explorer,
   editable file tabs, browser tabs, diffs, terminals, child conversations,
   subagent transcripts, and pull requests in a shared panel system. OpenAgents
   has separate change and terminal panes, but no comparable surface host.
2. **Direct control over coding sessions.** Zeron exposes the selected agent,
   model, reasoning, provider options, repository, branch, and worktree before
   a send. OpenAgents starts with hosted OpenAgents chat and routes coding work
   to Coder. Requests in words and Coder defaults do not provide equivalent
   discoverability or per-session controls.
3. **Files and source control.** Zeron has owner-routed file reads and writes,
   autosave and conflict handling, file mutations, indexed search, several diff
   scopes, Git history, and native pull request inspection. OpenAgents' reviewed
   task diff and Publish flow cover only part of this workflow.
4. **A desktop as a remote workspace client.** OpenAgents already has extensive
   paired-device, host, history, and task infrastructure. Its desktop's main
   coding path still emphasizes this computer. Zeron exposes remote projects,
   files, provider settings, terminals, and session controls through the same UI
   used for local work.
5. **Media and personalization.** Zeron adds local dictation, a Codex voice
   orchestrator, Appshots, integrated browsing and development previews,
   generated-image viewing, interactive agent visuals, imported themes,
   independent font controls, and richer layout preferences.

## Existing work to preserve

Avoid treating every Zeron feature as a new backend project. OpenAgents already
has these useful foundations:

- Shared Rust chat state and presentation, bounded transcript readers,
  optimistic drafts, retry behavior, and a Rust desktop renderer.
- Coder execution, provider failover and capacity awareness, isolated worktrees,
  ATIF traces, questions, approvals, plans, and task continuation.
- Durable task commands and protected queue edits; exact-review publishing and
  shared review-comment and subagent models.
- PNG/JPEG input, now enabled, with explicit restrictions on which routes can
  carry images.
- Paired hosts, encrypted history observation, remote task authority, PTY and
  terminal-emulation crates, SSH installation, and host service updates.
- System/light/dark appearance, text sizing, reduced motion, notifications,
  provider keys, native macOS menus, and release/update paths.

Some of these capabilities are in shared libraries or other applications and
still need desktop integration. Each chapter identifies that distinction.

## What “full parity” means

Parity means that a person can complete the same supported workflow with
comparable feedback, recovery, keyboard access, and platform coverage. It does
not require copying GPUI, Loro, Cloudflare services, WorkOS, provider credential
storage, or Zeron's code. Product implementation remains Rust under this
repository's contract. Platform glue must stay thin.

The register uses stable IDs and explicit acceptance conditions. A missing
feature is not closed by adding a button, exposing a CLI command, or finding a
similar type elsewhere in the workspace. Conversely, different implementation
details are not gaps when the user outcome is equivalent.

No percentage of parity is reported: the rows differ greatly in size and risk,
and this investigation did not run either complete application. The performance,
native interaction, and installed-release comparisons remain named validation
work. Any deliberate omission needs an explicit product decision; this report
does not silently exclude expensive features from “full parity.”
