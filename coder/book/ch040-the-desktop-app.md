# 40. The desktop app

> **Status: implemented.** OpenAgents Desktop (`apps/openagents-desktop/`,
> `OpenAgents.app`) is the unified native GPU desktop app across macOS
> (Metal + AppKit), Linux (Wayland/X11 + Vulkan), and Windows (Win32 + Vulkan).
> Its Coax state machine and modular UI (`app.coax`, `page.coax`, `page/*.coax`)
> drive primary turns on `oa-worker` (local Psionic Metal/CUDA Inkling 2 Mini
> or OpenAgents Cloud) plus opt-in delegated subagents (`oa-subagent`,
> `agyworker`, `claudeworker`), shipping `--minimal` and `--full` builds with
> per-chat Git worktrees, Checkpoints (`#14026`), Full Auto (`#14063`),
> Always-On Usage HUD (`#14064`), Voice (`#14065`), File Viewer (`#13992`),
> Bitcoin Spark + Nostr Wallet pane (`1acfa5d96e`), Metaverse/GameBuilder pane
> (`#14040`), and Embedded Browser pane (`#14105`).

The terminal UI ([Chapter 36](ch036-a-terminal-ui.md)) keeps Coder inside a
shell the person already has open. OpenAgents Desktop (`apps/openagents-desktop/`,
which superseded the earlier standalone Coder desktop prototype) puts the unified
`openagents/core/` runtime and worker processes inside a native GPU window
built on the shared `components/` display-list carrier: crisp text and layout
on Metal or Vulkan, modular Coax panes in `page/*.coax`, and concurrent chats
running in isolated Git worktrees backed by resident `coder-host`.

## What runs

```sh
apps/openagents-desktop/build.sh --minimal --open       # build minimal OpenAgents.app and launch it
apps/openagents-desktop/build.sh --full                 # build full feature-flagged bundle
coax/bin/coax test apps/openagents-desktop/page.coax --library
```

`build.sh` compiles the Coax UI to C (`coax c app.coax --library` or
`app-v1.coax` for `--minimal`), compiles the GPU shaders (`coax metal` on
macOS; `coax spirv` for `apps/openagents-desktop/linux/` and `windows/`), links
the C/Objective-C carrier and Swift AppKit host (`Window.swift`,
`Worker.swift`, `Store.swift`, `DesktopCore.swift`, `Window+*.swift`) into
`OpenAgents.app`, bundles `oa-worker`, `psionic-inkling-serve`, `gitguard`, and
optional external workers into `Contents/Resources/`, copies fonts and icons,
and signs the bundle.

## Architecture: Modular Coax state machine on cross-platform GPU carriers

The app splits UI logic from platform carriers so state transitions, layout,
and policy stay in pure Coax testable without a window server:

| Layer | Files | Responsibility |
| --- | --- | --- |
| Coax UI & state | `apps/openagents-desktop/app.coax`, `app-v1.coax`, `page.coax`, `page/*.coax` (`state`, `compose`, `editor`, `chats`, `checkpoint`, `filepane`, `gitdiff`, `goal`, `permit`, `usage`, `voice`, `walletpane`, `metaversepane`, `browserpane`, `dashboard`, `transcript`, `todos`) | Conversation model, GPU layout & text editing (`page/editor.coax`), Markdown/syntax-highlighted diffs, subagent cards, saved-chat JSON, keyboard/mouse actions, and 28 `#authority` feature flags (`coax/lib/agent/desktop/flags.coax`) |
| C/GPU carriers | `apps/openagents-desktop/host.h`, `host.c`, `carrier.c`, `apps/shared/host/apple/ui-host.m`, `apps/openagents-desktop/linux/`, `apps/openagents-desktop/windows/` | Compiles Coax UI into an opaque C ABI (`cd_*`), renders the display list on Metal (macOS) or Vulkan (Linux Wayland/X11, Windows Win32), and attaches external GPU panes (`mv_host.c`, `gb_host.c`, `br_host.c`) |
| Platform host | `apps/openagents-desktop/Window.swift`, `Window+*.swift`, `Worker.swift`, `Store.swift`, `Account.swift`, `LocalModels.swift`, `Foreman.swift`, `DaemonClient.swift` | Window lifecycle, IME/clipboard bridge, worker subprocess & `coder-host` RPC management (`oa-worker`, `oa-subagent`, `agyworker`, `claudeworker`), per-chat Git worktrees (`Worktree.swift`), Breez Spark wallet (`Window+Wallet.swift`), and local Inkling model downloads |

### Key shipped capabilities

- **Concurrent chats, headless runs, and `coder-host`:** switching chats swaps
  the active view (`cd_take`) while background chats keep streaming into
  headless Coax pages (`cd_headless`) or survive window closure under the
  resident `coder-host` service (`DaemonClient.swift`).
- **Per-chat Git worktrees and Checkpoints (`#13758`, `#14026`):** each new
  chat inside a Git repository gets an isolated worktree (`~/.openagents/worktrees/REPO/CHAT`)
  with APFS copy-on-write tool cloning, `gitguard`, syntax-highlighted Git
  diffs (`page/gitdiff.coax`), and per-turn rewind checkpoints guarded by
  `#authority` `CheckpointRestoreSafety` (`page/checkpoint.coax`,
  `Window+Checkpoints.swift`).
- **Permissions, Plan Mode, and Full Auto (`#13873`, `#14063`):** interactive
  permission and question cards (`page/permit.coax`), opt-in Full Auto mode
  with Operator Vault autofill (`#14063`), while `WalletPay` stays
  `#forall`-proven bypass-immune (`openagents/core/members.coax`, `#14057`).
- **Always-On Usage HUD & `$0` local tier (`#14064`, `#14072`, `#14087`):**
  cloud-first onboarding with optional local Inkling 2 Mini (`$0.00` compute
  enforced), synced OpenAgents Cloud account credits pill (`page/usage.coax`,
  `Window+Usage.swift`), and per-turn token/cost telemetry.
- **Right-side workspace panes:** unflagged File Viewer (`page/filepane.coax`,
  `#13992`, macOS/Linux/Windows), Voice dictation & streaming TTS
  (`page/voice.coax`, `Window+Voice.swift`, `#14065`), Bitcoin Spark +
  read-only NIP-06 Nostr Wallet pane (`page/walletpane.coax`,
  `Window+Wallet.swift`, `1acfa5d96e`), expandable Metaverse Viewer &
  GameBuilder pane (`page/metaversepane.coax`, `#14040`–`#14045`), and
  Embedded Desktop Browser pane (`page/browserpane.coax`,
  `Window+Browser.swift`, `#14105`–`#14110`).

## Checks

```sh
coax/bin/coax test apps/openagents-desktop/page.coax --library
apps/openagents-desktop/check.sh
apps/openagents-desktop/build.sh --minimal
```

## Dependencies

- [Chapter 9](ch009-microcoder.md): `openagents/core/` (`oa-worker`), Inky,
  and legacy Microcoder.
- [Chapter 10](ch010-drive-claude-code.md) & [Chapter 39](ch039-drive-antigravity.md):
  opt-in delegated Claude Code and Antigravity workers.
- [Chapter 36](ch036-a-terminal-ui.md): shared `coder/tui/` worker binaries.
- [Chapter 41](ch041-worker-protocol-and-sub-agents.md): the JSON-lines wire
  protocol and sub-agent delegation between `Window.swift` / `Worker.swift`,
  `page/*.coax`, and the workers.

## Done means

- [x] `apps/openagents-desktop/build.sh` builds a signed `OpenAgents.app`
  bundle (`--minimal` and `--full`) with Metal shaders, `oa-worker`,
  `psionic-inkling-serve`, and optional external workers; Linux and Windows
  Vulkan hosts compile the same Coax UI.
- [x] `coax/bin/coax test apps/openagents-desktop/page.coax --library` passes
  without a window server.
- [x] Running replies continue in headless pages or resident `coder-host` when
  switching chats, and saved chats reload from disk with worktrees and
  checkpoints intact.
