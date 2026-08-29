# Omarchy, as a donor

Pinned clone: `~/work/projects/repos/omarchy` at
`56fbaf4689e3eb6867c0b7f375ae49964f183774`, tag `v4.0.0`. Upstream:
[omacom/omarchy](https://github.com/omacom/omarchy). Manual source lives in
`manual/`. Site: [omarchy.org](https://omarchy.org).

This is a teardown for CoderOS. It is not an adoption plan for Arch, Hyprland,
Quickshell, or Omarchy's agent list.

## What it is

Omarchy is DHH's omakase Linux: **Arch + Hyprland + Quickshell**, MIT-licensed,
ISO-delivered, with a core team and (as of late August 2026) an Omacom
Foundation. Version 4.0 (“Quattro”, 2026-08-14) rebuilt the desktop shell into
one Quickshell process (bar, menu, notifications, OSD, lock), converted
Hyprland config to Lua, and made coding agents a first-class application
class.

The interesting claim is not “Arch can be pretty.” It is that a desktop Linux
install is **mutable state co-owned by the vendor and the user**, and that
this is a data-management problem. Omarchy answers with a vendor tree
(`/usr/share/omarchy`), a user tree (`~/.config`), Lua load order so the
user's file calls the vendor's, and a directory of timestamped idempotent
migrations for state `pacman` cannot own.

Shaun Li's architecture study of 4.0
([“The Distro Is Mostly a Migration Runner”](https://shaunli.com/blog/20-omarchy-4-quattro-architecture-study/))
is the best external reading of that claim. The clone confirms it: `migrations/`
is a first-class directory, `bin/` is hundreds of `omarchy-*` scripts, and
`bin/omarchy` is a router that turns that naming convention into a browsable
CLI *and* the graphical menu.

## Stack (observed)

| Layer | Omarchy 4.0 choice |
| --- | --- |
| Base | Arch Linux, own package repo + own Arch mirror (stable lags ~1 month) |
| Boot / snapshots | Limine, snapper; rollback restores root, **not** `/home` |
| Encryption | LUKS mandatory on the ISO path; Secure Boot / TPM off |
| Compositor | Hyprland, config in Lua (`config/hypr/hyprland.lua` loads vendor then user) |
| Desktop shell | One Quickshell process; plugins on disk |
| Terminals | Foot default; Alacritty, Kitty, Ghostty themed |
| Dev toolchains | mise lazy stubs (Ruby, Node, Bun, Rust, …) |
| Containers | Docker installed; user **not** in `docker` group by default |
| Agents | Lazy mise stubs: Claude Code, Codex, OpenCode, Antigravity, Copilot, Crush, Grok, Pi, Oh My Pi, Ori |
| Local LLMs | LM Studio and Ollama, optional, under *Install > AI* |
| GPU | `install/hardware/nvidia.sh` plus named probes (`omarchy-hw-nvidia-gsp`, `omarchy-hw-nvidia-without-gsp`) |
| Update | `omarchy update` snapshots, updates packages, runs migrations; direct `pacman -Syu` is blocked |
| Agent skill | `default/agents/skills/omarchy/SKILL.md` symlinked into Claude/Codex/Pi/Antigravity/`~/.agents/skills` |

## Ideas to take

### 1. The desktop is a CLI

Every menu entry has a command-line address (`omarchy menu summon style.theme`,
`omarchy theme set <name>`, `omarchy bar move …`). The GUI is a browser over
that CLI. An agent that can run commands can operate the machine without
screenshots.

CoderOS should have this property for Coder from day one of a desktop
profile. If a setting exists in a panel, `openagents os …` (name TBD) can
do it, `--help` documents it, `--json` lists it.

### 2. Vendor tree vs user tree, taught to the agent

`/usr/share/omarchy` is the package. `~/.config` is the user. The Omarchy
skill states this as a hard rule: **never write the vendor tree; reading it
is encouraged.** That is the minimum safety rail once an agent edits
dotfiles on a real machine.

CoderOS should ship an equivalent skill for Coder: which paths are CoderOS
(overwritten on update), which paths are the user's, which paths are the
model store, which paths are git worktrees. Same document a human
contributor would need.

### 3. User config loads vendor config

`hyprland.lua` is a program, not a merged document:

```lua
require("default.hypr.omarchy")
require("hypr.monitors")
require("hypr.input")
-- …
```

Opt-out is a flag (`omarchy_default_bindings = false`), not a fork of the
vendor file. Package updates can improve defaults without rewriting
`~/.config`.

The crack Omarchy itself documents: `shell.json` is canonical once you
touch it — no deep merge — so customising the bar forks you off future
default widgets. CoderOS should decide merge vs fork per surface, and write
the decision down.

### 4. Migrations for state packaging cannot own

Timestamped, idempotent, per-user completion under
`~/.local/state/omarchy/migrations/`. They wait for pacman, run as the
user, and notify if they ran without a login.

A Nix/Guix substrate shrinks this class (packages rebuild instead of
mutating). It does not delete it. User-state format changes, systemd user
units, and “this laptop's firmware now needs a new module” remain
migrations even on an immutable root. Steal the ledger shape.

### 5. Hardware quirks as named probes

`omarchy-hw-dell-xps-oled`, `omarchy-hw-framework16`,
`omarchy-hw-nvidia-gsp` return exit codes for scripts. That is tacit
laptop knowledge checked into git. CoderOS needs the same for CUDA vs
Vulkan vs CPU, NVIDIA GSP vs 580xx, AMD ROCm, Intel, and “this box can
hold Qwen 3.8 Q8_0 in VRAM.” `openagents inference doctor` is the start
of that catalog.

### 6. Agents as one window class

`bin/omarchy-agent` launches whatever the default CLI is, but always under
`--app-id=org.omarchy.agent`, and `cd`s from `$HOME` to `~/Work` because
“agents refuse to remember trust for `$HOME`.” Window rules and themes
then target *the agent*, not `claude` vs `codex`.

CoderOS should do this for `openagents coder` (and only optionally for
other harnesses). One class, one theme, one trust directory.

### 7. OS hands work to the agent

`omarchy-agent-crash` turns a coredump into a prompt plus a skill path, then
`exec omarchy-agent --prompt`. A notification is a clickable handoff.

CoderOS can hand Coder crashes, failed Harbor trials, failed inference
loads, and failed updates the same way. The prompt must include facts, not
vibes, and must point at a skill.

### 8. Theme as a compiler

A theme is a `colors.toml`. Omarchy generates Foot, Alacritty, Ghostty,
Kitty, btop, Chromium, Hyprland, Neovim, Helix, VS Code, Obsidian, and the
shell from it. Coder's TUI should be another output of the same compiler,
not a private color scheme.

### 9. Update as a transaction, with an honest rollback

Snapshot, packages, migrations, post-update hook. Direct package-manager
upgrades are blocked because they skip the rest. Rollback restores root
and **says it does not restore `/home`**.

CoderOS should be this honest even if the substrate is Nix (`nixos-rebuild`
rollback) or Guix (`guix system roll-back`). The user-state caveat remains.

### 10. Channels

`stable` (lagged mirror), `rc`, `edge`, `dev` (git checkout). Matches how
we already think about CLI channels (`stable` vs `rc.N`). A CoderOS ISO
that only has “latest git” will hurt the first non-author user.

## Ideas to refuse, or invert

### Auto-approve from the hotkey

`bin/omarchy-agent` starts each vendor CLI in its “don't stop to ask” mode
(`claude --permission-mode auto`, `codex --approve-for-me`,
`grok --permission-mode bypassPermissions`, …). Combined with a 15-minute
passwordless-sudo toggle, that is a sharp default.

Coder already has an approval model. CoderOS must not launch Coder in a
bypass mode just because a keybinding started it. Unattended mode is an
explicit Coder setting, not an OS hotkey side effect. Document it if we
ever add it.

### Agent identity as a vendor menu

Omarchy's identity in 4.0 is “pick Claude, Codex, OpenCode, …”. That is
the right product for DHH's audience. CoderOS's identity is Coder, with
other harnesses as optional installers if we want them at all. Do not ship
nine stubs and call the last one Coder.

### Ollama / LM Studio as the local-model product

Fine as escape hatches. The CoderOS local path is `openagents inference`
and in-process Psionic. Do not make LM Studio the onboarding story.

### Pacman guard as a personality

Blocking `pacman -Syu` is coherent on Arch because Omarchy owns the update
transaction. On NixOS the equivalent is “don't `nix-env -i` around the
flake.” The lesson is *one update path*, not “we too must wrap pacman.”

### Secure Boot off, TPM off, Bluetooth keyboard cannot unlock LUKS

Documented constraints, not silent ones. CoderOS should not copy “Secure
Boot must be off” without a new owner decision. A Coder-operated machine
that cannot attest is a weaker Cloud peer.

### Cloudflare in front of the ISO and package mirror

Omarchy's own security page says Cloudflare protects ISOs, packages, and
the Arch mirror. OpenAgents production infrastructure authority is Google
Cloud; Cloudflare is DNS-only for `openagents.com`. A CoderOS ISO CDN is
a later ops decision and must not quietly revive Cloudflare as a runtime
or package authority.

## Community Nix ports

Omarchy's manual (`manual/49-omarchy-on.md`) points at Henry Sipp's NixOS
port as a community starting point, with an explicit “may or may not stay
up to date.” That is the honest status of “rebase Omarchy onto NixOS”:
unofficial, chasing a moving Arch desktop, not a second first-party OS.

Use those ports as evidence that **people want the Omarchy experience on a
declarative substrate**. Do not use them as a fork point for CoderOS. We
would inherit Hyprland/Quickshell churn and none of Coder.

## What “Omarchy for Coder” compresses to

Four sentences:

1. Make every action a command.
2. Split vendor state from user state and teach Coder the split.
3. Record hardware and update scar tissue as code (probes, migrations,
   channels, honest rollback).
4. Put Coder, not a vendor menu, in the agent window class, with local
   models that are ours.

That is the donor. The substrate is a separate question.
