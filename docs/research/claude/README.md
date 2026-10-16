# Claude desktop architecture

Assessment date: 2026-10-15.

This report describes the shipped macOS Claude desktop application installed at
`/Applications/Claude.app`. The inspected build is version `2.37337.1`, bundle
identifier `com.anthropic.claudefordesktop`, signed by Anthropic PBC on 13
October 2026. It is an Electron 44.7.0 application whose product package is
`@ant/desktop`. The interface is a set of renderer documents around a small
native host. Product features that leave the web UI — computer use, Cowork,
device pairing, local terminals, a GitHub MCP server, Microsoft sign-in, an
in-app browser handoff, and an iOS Simulator helper — are separate native
binaries and archive entries beside that host.

The application was not launched. No chat, token, cookie, or
`~/Library/Application Support/Claude` file was opened. Facts below are file
contents, signatures, linkage, and static structure of this installed build.
They are not observed runtime behavior.

## Read the report

| Document | Contents |
| --- | --- |
| [Scope and evidence](scope-and-evidence.md) | What was inspected, how claims are marked, and the digests that identify the build |
| [Identity and packaging](identity-and-packaging.md) | Bundle identity, signature, entitlements, Electron fuses, document types, and the Developer ID profile |
| [Process architecture](architecture.md) | Boot, windows, the IPC bus, preload trust, utility processes, schemes, and permission decisions |
| [Surfaces and deep links](surfaces.md) | `claude://` routing, login callbacks, desktop actions, and the shell around the web view |
| [Sessions and code](sessions.md) | Code-session files under `claude-code-sessions`, `~/.claude.json` trust, the pty cwd, SSH `ssh -G`, askpass, and saved passwords |
| [Cowork](cowork.md) | Session files, folder grants in `claude_desktop_config.json`, the sessions bridge, Copper Heron, direct MCP, artifacts, and scheduled tasks |
| [Virtual machine](vm.md) | Bundle download, `smol-bin` disk copy, Swift start, host loop, and Yukon Silver |
| [Computer use and devices](computer-use.md) | TCC grants, teach and watch-record, Sable Otter, framebuffer, simulator, Buddy, Bluetooth |
| [Extensions and integrations](extensions-and-integrations.md) | MCP hosts, the node host's refusal to spawn Electron, plugins, GitHub, Office, Chrome |
| [Web UI and previews](web-and-preview.md) | `ion-dist`, the artifact frame shell, host-tools port, launch previews, browser shim |
| [Enterprise configuration](enterprise.md) | Managed-config schema, third-party setup, device-code confirmation, Microsoft 365 labels |
| [Application](application.md) | ASAR layout, package manifest, archive inventory, and the earlier graph counts |
| [Native modules](native.md) | Host stub, Swift and Rust-like addons, helpers, Go MCP server, MSAL, and linkage |
| [Bundle and resources](bundle.md) | Frameworks, images, iOS Simulator helper, and asset catalog |
| [Coverage](coverage.md) | What this reading established, and what is still unexecuted |

## The architecture in short

1. **A thin Electron host loads one archive.** `Contents/MacOS/Claude` links
   only Electron Framework and the system C library. Its arm64 text is 348
   bytes. The product is `Contents/Resources/app.asar`. Node-as-node and
   `--inspect` are fused off. The archive is the only app the fuses will load.
2. **The main process is one 7 MB chunk behind a compile-cache bootstrap.**
   Build commit `ba51ab579cc4029765083ec9f9f0bd23d37a32ee`, production, not a
   nest build. It creates the windows, registers `cowork-artifact`,
   `cowork-file`, `claude-media`, and `claude-simulator`, and forks utility
   processes with a `{type: "init"}` port.
3. **Renderers get a typed bus, not Node.** Channels are
   `$eipc_message$_<uuid>_$_<namespace>_$_<Type>_$_<method>`. A preload
   exposes a world only for a top frame on an allowlisted origin. The main
   view's worlds are `claude.web`, `claude.settings`, and the smaller
   simulator, skills, Office, engine, locale, and Buddy worlds.
   `LocalSessions` alone lists 337 methods.
4. **Three products share the shell.** Chat is the shipped `ion-dist` page.
   Code is `LocalSessions`: a JSON file per session under
   `claude-code-sessions`, trust in `~/.claude.json`, a pty whose cwd is the
   worktree, git, `gh`, and SSH. Cowork is `LocalAgentModeSessions` (a JSON
   file per session under `local-agent-mode-sessions`, folder grants in
   `claude_desktop_config.json`, an in-process sessions bridge) plus a Linux
   VM (`claudevm.bundle`, downloaded from `downloads.claude.ai`, booted
   through the Swift addon) plus `cowork-artifact:` documents that can call
   `mcp__<server>__<tool>` and nothing else.
5. **`claude://` is one function.** Hosts `cowork`, `code`, `claude.ai`,
   `login`, `resume`, and `sable-otter` navigate or import. `preview` and
   `debug-handoff` are rejected in this non-nest build. Google and magic-link
   callbacks are dropped unless this process started them. Managed config can
   disable the scheme and fail closed.
6. **Privileged work stays out of the page.** Computer use is a panel plus
   TCC grants plus `computer_use.node`. Pairing is a Buddy window and a
   separate Nordic UART bridge. The MCP node host refuses to spawn Electron
   and tells the user to turn off the built-in Node. Preview partitions and
   the browser shim's `pane` reply are how a local URL opens inside the app.

## What this report is not

It is not a line-by-line reconstruction of the hydrated claude.ai modules, and
it is not a claim about what a signed-in session does. Control flow in the
chapters above is the shipped script, read as text. The VM was not started,
no deep link was opened, and no MCP server was spawned. A compiler path
embedded in a binary identifies a compiler commit. It does not authenticate
package versions. Hashed assets under `ion-dist/assets/v1/` were sampled
through the HTML that loads them, not executed.
