# 42. Beyond the desktop app

> **Status: partly implemented.** Chapters 40 and 41 document the unified
> native GPU desktop app (`apps/openagents-desktop/`) and its JSON-lines worker
> wire driving `oa-worker` (`openagents/core/`), local Psionic Metal/CUDA
> Inkling, OpenAgents Cloud, and opt-in delegated workers (`oa-subagent`,
> `agyworker`, `claudeworker`). Most of this chapter's roadmap now ships on
> `main`—Linux/Windows Vulkan carriers, resident `coder-host` converged onto
> `oa-worker` (`#14073`), Universal Introspection sidebar & live inspector
> (`#14066`), opt-in Claude sub-agent delegation (`#14104`), Full Auto &
> Operator Vault (`#14063`), conversational capability authoring with two-arm
> ATIF-v1.8 evals (`#14059`), Metaverse/Browser panes (`#14040`, `#14105`), and
> iOS Desktop parity (`#14068`)—while borderless cross-device session control
> (`#14067`), Android parity, a single canonical cross-client session store,
> and universal UI consent/memory revision remain open.

The desktop app proved the core split: modular Coax state (`page/*.coax`)
compiled to native Metal and Vulkan carriers, talking over one JSON-lines
protocol (`mcworker-wire.coax`) to isolated worker processes and resident
`coder-host`. Expanding beyond a single macOS window means bringing that same
Coax state and worker contract across operating systems, live terminal and
external-harness sessions, resident daemons, self-authored capabilities, and
mobile devices.

## Implemented and verified

- **Universal Introspection sidebar & live inspector (`#14066`):**
  [`coax/lib/agent/desktop/introspect.coax`](../../coax/lib/agent/desktop/introspect.coax)
  defines a pure-Coax, zero-allocation session and machine model
  (`IS_KIND_DESKTOP`, `IS_KIND_TUI`, `IS_KIND_HARNESS`, `IS_KIND_REMOTE`) with
  parsers for `tmux list-panes`, `ps` rows, external local harnesses, and
  remote Tailscale/SSH machines.
  [`apps/openagents-desktop/page/introspect.coax`](../../apps/openagents-desktop/page/introspect.coax),
  [`apps/openagents-desktop/page/environments.coax`](../../apps/openagents-desktop/page/environments.coax)
  (promoted into `--minimal` with `Settings -> Connections` un-gated), and
  [`apps/openagents-desktop/Window+Introspect.swift`](../../apps/openagents-desktop/Window+Introspect.swift)
  render collapsible, count-badged sidebar sections (`TUI & Terminal`,
  `Other Harnesses`, `Machines`) with click-to-inspect live output in the main
  pane. Reconciled with `#14071`, discovery of other local harnesses is
  strictly read-only and non-commandable unless opted in under
  `Settings -> Agent`.
- **Cross-platform native carriers (`#14046`):** Linux Wayland/X11 + Vulkan
  ([`apps/openagents-desktop/linux/main.c`](../../apps/openagents-desktop/linux/main.c))
  and Windows Win32 + Vulkan
  ([`apps/openagents-desktop/windows/main.c`](../../apps/openagents-desktop/windows/main.c))
  compile the exact same Coax `page/*.coax` UI and SPIR-V GPU shaders as macOS
  Metal.
- **Resident `coder-host` converged onto `oa-worker` (`#14073`, `#14060`):**
  [`apps/openagents-desktop/DaemonClient.swift`](../../apps/openagents-desktop/DaemonClient.swift),
  [`coder/tui/host.coax`](../../coder/tui/host.coax), and
  [`coder/tui/host-phone.coax`](../../coder/tui/host-phone.coax) let runs
  survive closing `OpenAgents.app`, reattach to live streams, and deliver Stop
  to host-owned runs.
  [`coax/lib/agent/environment/phone-agent-policy.coax`](../../coax/lib/agent/environment/phone-agent-policy.coax)
  (`#authority`) promotes `oa-worker` (`oa.core.v1`: `oa:auto`,
  `oa:gemini-4-neon`, optional `$0` `oa:inkling-2-mini`) as the primary engine
  across local and Tailscale-connected hosts.
- **Opt-in Claude sub-agent delegation (`#14104`):**
  [`coder/tui/claudeworker.coax`](../../coder/tui/claudeworker.coax) and
  [`coder/tui/oa-subagent.coax`](../../coder/tui/oa-subagent.coax) ship on
  macOS `--minimal` and Linux with a non-interactive `--auth` probe (hard 5 s
  deadline, `SSH_CONNECTION=1`, `NO_OPEN_BROWSER=1`), live sign-in status in
  `Settings -> Agent`, mapped permission modes (`Allow All -> bypassPermissions`,
  `Accept Edits -> acceptEdits`, `Plan/read_only -> plan`,
  `Ask/Don't Ask -> dontAsk`), and System One v2 router `available_delegates`
  planning (`#14056`, `#14071`).
- **Conversational capability authoring & two-arm ATIF-v1.8 evals (`#14059`):**
  [`coax/lib/agent/desktop/capability-author.coax`](../../coax/lib/agent/desktop/capability-author.coax)
  and `OA_PACK_CAPABILITY` (`openagents/core/members.coax`,
  `coder/tui/oa-worker.coax`) let OpenAgents draft a `SKILL.md` in chat, run a
  real baseline-vs-candidate two-arm eval measuring latency via `proc_now_ms()`
  and canonical `ATIF-v1.8` SHA-256 `steps_digest`, render an `#authority`
  Promotion Verdict Card, and activate promoted local skills on the next turn
  without a restart (alongside the shared `OA_PACK_KNOWLEDGE` hybrid BM25 +
  dense retriever in `#14058`).
- **Interactive approvals, Plan Mode, Full Auto & Operator Vault (`#13873`, `#14063`, `#14057`):**
  [`apps/openagents-desktop/page/permit.coax`](../../apps/openagents-desktop/page/permit.coax)
  renders inline permission/question cards, while `#authority`
  [`coax/lib/agent/desktop/full-auto.coax`](../../coax/lib/agent/desktop/full-auto.coax)
  and
  [`coax/lib/agent/desktop/operator-vault.coax`](../../coax/lib/agent/desktop/operator-vault.coax)
  enable opt-in Full Auto and Operator Vault autofill while keeping `WalletPay`
  bypass-immune in `openagents/core/members.coax`.
- **Multimodal attachments, workspace panes & Gym benchmarks (`#13992`, `#14040`, `#14105`):**
  `oa-worker` reads images with dimension/byte caps, while desktop ships the
  embedded terminal (`page/termpane.coax`), File Viewer (`page/filepane.coax`),
  expandable Metaverse/GameBuilder pane
  ([`apps/openagents-desktop/page/metaversepane.coax`](../../apps/openagents-desktop/page/metaversepane.coax),
  `#14040`), multi-tab Embedded Desktop Browser pane
  ([`apps/openagents-desktop/page/browserpane.coax`](../../apps/openagents-desktop/page/browserpane.coax),
  `#14105`–`#14110`, with WKWebView on macOS, WebKitGTK 6 on Linux, WebView2 on
  Windows, and `#authority` `browser-policy.coax`), plus built-in gBench,
  Terminal-Bench 4.1, and ATIF-v1.8 scorecards (`evals/session-history/`).
- **iOS Desktop parity (`#14068`):**
  [`apps/openagents-ios/gpu-page.coax`](../../apps/openagents-ios/gpu-page.coax)
  and [`coder/tui/host-antigravity-ext.coax`](../../coder/tui/host-antigravity-ext.coax)
  bring Full Auto (`full-auto.coax`), the Always-On Usage HUD (`GetUsage`),
  Universal Introspection sidebar sections with Tailscale command dispatch
  (`ListIntrospection`, `ReadIntrospection`, `SendSessionCommand`), and
  bi-directional voice (`voice-duplex.coax`) to iOS.

## Checks

Pure-Coax policy, introspection, capability-authoring, and desktop page checks
run directly without a window server:

```sh
coax/bin/coax test coax/lib/agent/desktop/introspect.coax --library
coax/bin/coax test coax/lib/agent/desktop/capability-author.coax --library
coax/bin/coax test coax/lib/agent/desktop/full-auto.coax --library
coax/bin/coax test coax/lib/agent/desktop/operator-vault.coax --library
coax/bin/coax test coax/lib/agent/environment/phone-agent-policy.coax --library
coax/bin/coax test apps/openagents-desktop/page/checks/introspect-checks.coax --library
coax/bin/coax test apps/openagents-desktop/page/checks/agents-checks.coax --library
coax/bin/coax test coder/tui/claudeworker.coax --library
```

## Still open

1. **Canonical cross-client session store & replay (remaining `#14066` work):**
   while Universal Introspection now surfaces Desktop, TUI/tmux, external
   harnesses, and remote machines in one sidebar and live inspector, unifying
   [Chapter 2](ch002-model-visible-means-logged.md)'s hash-chained session log,
   `AJR1` journals, ATIF-v1.8 trajectories, and desktop chat JSON into one
   lossless canonical store across CLI, TUI, desktop, and mobile remains open.
2. **Borderless session control (`#14067`):** seamless live steer, mid-turn
   handoff, approval resolution, and interactive terminal/pane attachment across
   local desktop, Tailnet/LAN/Nostr remote hosts (`#14030`), and background
   cloud runners.
3. **Android parity & cross-device sync (remaining `#14068` work):** bring
   Android to parity with the iOS and desktop Coax GPU surface, approval cards,
   checkpoints, and multi-device session handoff.
4. **Universal consent & memory inspector in the UI:** wire
   [Chapter 18](ch018-consent-before-sharing.md)'s per-destination consent
   ledger and [Chapter 19](ch019-remember-a-fact-then-revise-it.md)'s
   source-linked memory revision UI into `page/*.coax` across every outbound
   provider and fallback path.

## Done means

- [x] Linux and Windows Vulkan builds compile and run the same `page/*.coax`
  UI and worker protocol alongside macOS Metal.
- [x] `OpenAgents.app` attaches to resident `coder-host` converged onto
  `oa-worker` so runs survive closing the window and Stop reaches host-owned
  runs.
- [x] Universal Introspection sidebar sections and live inspector (`#14066`)
  surface local TUI/tmux, read-only external harnesses, and remote machines in
  `--minimal` desktop and iOS (`#14068`).
- [x] Opt-in Claude sub-agent delegation (`#14104`), Full Auto + Operator
  Vault (`#14063`), conversational capability authoring + two-arm ATIF-v1.8
  evals (`#14059`), multimodal Read, embedded terminal/file/metaverse/browser
  panes (`#14040`, `#14105`), and Gym benchmarks ship and pass their Coax
  checks.
- [ ] Desktop, CLI, TUI, iOS, and Android share one lossless canonical session
  log and replay store (`#14066` follow-up) with borderless remote control
  (`#14067`) and Android parity (`#14068`).
- [ ] Chapter 18 consent gates and Chapter 19 source-linked memory revision
  are surfaced in `page/*.coax` across all outbound backends.
