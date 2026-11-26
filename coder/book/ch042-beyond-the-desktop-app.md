# 42. Beyond the desktop app

> **Status: unimplemented — design draft.** Chapters 40 and 41 document what
> `apps/openagents-desktop/` ships today: a unified native GPU app across
> macOS, Linux, and Windows driving `oa-worker` (`openagents/core/`), local
> Psionic Metal/CUDA Inkling, OpenAgents Cloud, and opt-in delegated workers
> (`oa-subagent`, `agyworker`, `claudeworker`). Several items from this
> chapter's original roadmap have now shipped; this draft separates those
> completed milestones from the genuinely open multi-device tracks (`#14066`,
> `#14067`, `#14068`).

The desktop app proved the core split: modular Coax state (`page/*.coax`)
compiled to native Metal and Vulkan carriers, talking over one JSON-lines
protocol (`mcworker-wire.coax`) to isolated worker processes and resident
`coder-host`.

## Already shipped since the original draft

- **Cross-platform native carriers:** Linux Wayland/X11 + Vulkan
  (`apps/openagents-desktop/linux/`) and Windows Win32 + Vulkan
  (`apps/openagents-desktop/windows/`, `#14046`) compile the exact same Coax
  `page/*.coax` UI and GPU shaders.
- **Resident `coder-host` execution:** `DaemonClient.swift` and
  `coder/tui/host.coax` let runs survive closing `OpenAgents.app` and
  reattach to live streams, with Stop reaching host-owned runs (`ebabbbec4c`).
- **Interactive approvals, Plan Mode, and Full Auto:** `page/permit.coax`
  renders inline permission/question cards (`#13873`), `#14063` adds opt-in
  Full Auto with Operator Vault autofill, and `openagents/core/members.coax`
  enforces bypass-immune `WalletPay` (`#14057`).
- **Multimodal attachments and right-side panes:** `oa-worker` reads images
  with dimension/byte caps, while desktop ships the embedded terminal pane
  (`page/termpane.coax`), File Viewer (`page/filepane.coax`, `#13992`),
  Metaverse/GameBuilder pane (`page/metaversepane.coax`, `#14040`), and
  Embedded Browser pane (`page/browserpane.coax`, `#14105`).
- **Built-in Gym & benchmarks:** `GymBenchmarkView.swift`, `GymGBenchView.swift`,
  `GymTerminalBenchView.swift`, and `evals/session-history/` run gBench,
  Terminal-Bench 4.1, and ATIF-v1.8 friction scorecards directly from desktop.
- **System One v2 routing & opt-in external engines:** `openagents.router.v2alpha1`
  (`#14056`) routes across packs, local Inkling, OpenAgents Cloud, and opt-in
  delegated external CLIs (`#14071`, `#14104`).

## Still open

1. **Universal session introspection (`#14066`):** unify Chapter 2's hash-chained
   session log, `AJR1` journals, ATIF-v1.8 trajectories, and desktop chat JSON
   so any CLI, TUI, desktop, or mobile client inspects and replays every run
   from one canonical store without translation loss.
2. **Borderless session control (`#14067`):** seamless steer, handoff,
   approval resolution, and live terminal/pane attachment across local desktop,
   Tailnet/LAN/Nostr remote hosts (`#14030`), and background cloud runners.
3. **Full mobile parity (`#14068`):** bring iOS (`apps/openagents-ios/`) and
   Android clients to full parity with desktop threads, approval cards,
   checkpoints, and session handoff.
4. **Universal consent & memory inspector in the UI:** wire Chapter 18's
   per-destination consent ledger and Chapter 19's source-linked memory
   revision UI across every outbound provider and fallback path.

## Done means

- [x] Linux and Windows Vulkan builds compile and run the same `page/*.coax`
  UI and worker protocol alongside macOS Metal.
- [x] `OpenAgents.app` attaches to resident `coder-host` so runs survive
  closing the window and Stop reaches host-owned runs.
- [x] Interactive approval cards, Full Auto, multimodal Read, embedded
  terminal/file/metaverse/browser panes, and Gym benchmarks ship in desktop.
- [ ] Desktop, CLI, TUI, and mobile share one canonical session log and
  introspection model (`#14066`) with borderless remote control (`#14067`)
  and full mobile parity (`#14068`).
- [ ] Chapter 18 consent gates and Chapter 19 source-linked memory revision
  are surfaced in `page/*.coax` across all outbound backends.
