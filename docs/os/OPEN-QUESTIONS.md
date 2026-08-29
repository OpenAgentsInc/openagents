# Open questions and research avenues

Questions this folder does not answer. Each is a research avenue, not a
hidden decision. Prefer a small evidence note over a new architecture
rewrite when you close one.

## Product and authority

**Q1. Is CoderOS a product we ship, or a profile we publish?**

An ISO with a name is a support surface: hardware, updates, security,
brand. A NixOS flake / Arch overlay / Docker image that makes Coder
excellent is a much smaller promise. Phase 0 is compatible with both.
Phase 2 (ISO) needs an owner decision.

**Q2. Who is the user?**

Omarchy's user is a human who wants a beautiful Linux daily driver and
will pick a vendor coding agent. CoderOS's user might be (a) the same
human, but Coder-first, (b) Coder itself on a build host, (c) both.
(a) and (b) disagree about compositor, themes, and whether LUKS+Hyprland
is in the default image. Dual-role (see [CODEROS.md](./CODEROS.md)) is
the current hypothesis. It is unproven.

**Q3. Where does this live in the repo map?**

This repository is Rust CLI + Cloud crates. An ISO, installer, and
package repo are a new product surface. Phoenix owns web. Omega owns
desktop on macOS. A CoderOS tree might belong in this repo (`docs/os`
today, `os/` later), a sibling repo, or nowhere until Phase 0 is done
in `crates/openagents-cli`. Do not create `os/` packaging here without
an owner decision.

**Q4. How does CoderOS relate to Omega?**

Omega is the macOS/desktop IDE destination. CoderOS is Linux. They
should share Coder, inference, and skills, not a compositor. Whether
Omega-on-Linux (GPUI) is ever a CoderOS GUI is a separate, already-open
GPUI-on-web/money-moving question. Do not assume a GPUI Linux desktop.

## Substrate

**Q5. NixOS module vs Arch overlay vs Debian**

Study order is in [SUBSTRATE.md](./SUBSTRATE.md). Close this only after
Phase 0 GPU+libc evidence exists. The Nix rebase demand is input, not
authority.

**Q6. Can Lua generate Nix well enough that we never teach humans Nix?**

If yes, the “Nix but Lua” comment is a compiler ticket. If no, we either
teach Nix, stay on Arch, or take on a new language. Need a spike: Lua
DSL → flake that enables NVIDIA + `openagents` + a compositor, rebuilt
twice.

**Q7. Guix non-free GPU path**

Is there a documented, repeatable Guix System configuration that loads
NVIDIA userland and runs a GGUF decode at non-toy speed without a
one-off overlay that breaks on the next `guix pull`? If not, Guix is a
peer for ideas, not a base.

**Q8. Lix / Snix / Nickel**

Implementation hosts and config languages around Nix. Worth a study
note if we pick Nix as the store. Not Phase 0.

**Q9. Immutable root + mutable home**

Omarchy snapshots skip `/home`. NixOS often uses impermanence for `/`.
Coder's worktrees, inference store, and Harbor caches want mutable,
large disks. Where do they live so rollback does not strand models or
destroy uncommitted work? This is the same honesty Omarchy already
prints in its snapshot manual.

## Coder and agents

**Q10. Other harnesses on CoderOS**

Omarchy ships nine lazy CLIs. Do we install none, a Coder-only default,
or a hidden `openagents os install-harness codex` for people who still
want it? Default hypothesis: Coder only, other harnesses out of tree.

**Q11. Approval model on a desktop hotkey**

Omarchy auto-approves from the agent keybinding. Coder should not.
What does `Super+A` do instead — open Coder in the normal permission
mode, or open Coder in a “plan” mode? Needs a behavior contract if we
ever ship a desktop profile.

**Q12. Passwordless sudo for agents**

Omarchy offers a 15-minute toggle for “agent is doing system work.”
CoderOS could instead grant Coder a narrow polkit/sudoers snippet
(package install, nothing else). Unresearched. Default: do not copy
the 15-minute hole.

**Q13. Trust directory**

Omarchy `cd`s to `~/Work` because agents will not remember trust for
`$HOME`. Coder's worktree-per-task rule is a stronger version of this.
Should a CoderOS session refuse to start Coder in `$HOME`? Probably
yes; needs an exact rule.

**Q14. Skill layout**

Omarchy symlinks one skill into Claude, Codex, Pi, Antigravity, and
`~/.agents/skills`. Coder should read an OpenAgents-owned skill path.
Do we also publish a CoderOS skill for *other* harnesses so they do
not smash `/usr`? Maybe later. Coder first.

## Inference and hardware

**Q15. First Linux GPU backend**

CUDA (NVIDIA, matches Omarchy's laptop enablement) vs Vulkan
(broader, less model-tuned) vs CPU-only for Phase 0 CI. Hypothesis:
CPU in CI, CUDA on one owned NVIDIA box as the first evidence, Vulkan
as the non-NVIDIA research path.

**Q16. First Linux model artifact**

Same admission problem as the Psionic plan: one digest, one license,
one family. 27B Q8_0 may not fit a cheap Linux box. A small fixture
for CI plus a laptop-class quant for the NVIDIA evidence machine.

**Q17. Driver/kernel pairing**

Omarchy splits NVIDIA GSP vs 580xx. NixOS CUDA pins are infamous.
Whoever owns CoderOS GPU support owns this matrix. It should look
like Omarchy probes (`openagents-hw-nvidia-gsp`) even on NixOS.

**Q18. Apple Silicon Linux**

Omarchy-on-Asahi is community, not first-party. CoderOS Phase 0 may
ignore Asahi. Metal stays the macOS path. Do not promise Asahi+CUDA
or Asahi+Vulkan until someone measures.

## Desktop (Phase 1+)

**Q19. Compositor**

Hyprland is what Omarchy proved. niri, Sway, and a GPUI shell are
alternatives. CoderOS should pick a compositor only after the CLI is
good. Hyprland+Lua is the default research target because that is
where Omarchy's config-as-program idea already lives.

**Q20. Shell**

Quickshell (Omarchy 4.0) vs Waybar+mako vs something we write. The
lesson to take is *one addressable shell*, not Qt specifically.

**Q21. Theme compiler includes Coder TUI**

Should `colors.toml` generate Coder's terminal theme? Yes if we have
a desktop profile. Needs a Coder theme contract.

**Q22. Local Firecracker**

A CoderOS workstation running guest workrooms. Attractive, overlaps
Cloud contracts, easy to do badly. Research only after Docker-on-
CoderOS is ordinary.

## Distribution and ops

**Q23. Where ISOs and packages are hosted**

OpenAgents production is Google Cloud; Cloudflare is DNS-only for
`openagents.com`. Omarchy uses Cloudflare for ISO/package CDN. A
CoderOS ISO must not quietly stand up a Cloudflare Workers-style
runtime. Artifact bucket + existing release script is the starting
pattern (`docs/ops/2026-08-25-cli-release-runbook.md`).

**Q24. Signing and Secure Boot**

Omarchy: ISO signatures, own keyring, Secure Boot off. CoderOS
should not inherit “Secure Boot off” without a decision. Attestable
CoderOS hosts would matter if they ever peer with Cloud.

**Q25. Channels**

CLI already has stable vs `rc.N`. A distro needs at least stable and
dev. Map them deliberately so `openagents` stable on CoderOS stable
does not mean two different clocks.

**Q26. Unattended install secrets**

Omarchy cidata can hold a plaintext disk-encryption passphrase. That
ISO is a credential. If CoderOS grows unattended install, Coder-built
cidata must not repeat a plaintext LUKS secret in a published image.

## Research avenues (concrete)

Work that advances the questions without picking a distro:

1. **Linux doctor spike** — libc, GPU, display, docker, coredump; gnu
   and musl; one NVIDIA box and one CPU-only box.
2. **PTY suite on Linux CI.**
3. **CPU `inference run` on Linux** for the small admitted GGUF.
4. **Read-only NixOS CUDA notes** — reproduce a Qwen GGUF decode on
   NixOS with nixpkgs unstable, record pins and failure modes.
5. **Lua→Nix spike** — ten-line Lua that generates a flake enabling
   `openagents` + nvidia + a terminal; no Hyprland required.
6. **Omarchy skill → CoderOS skill draft** — vendor/user/model/worktree
   paths only, no Hyprland. Lives in this folder or
   `crates/openagents-cli` skills once implementation exists.
7. **Harbor local-lane row on Linux** against Ollama, then against
   Psionic when (3) exists.
8. **Guix NVIDIA feasibility note** — one page, dated, with a yes/no
   on a supported non-free path.

Items 1–3 are Phase 0 product work and belong in the CLI/inference
plans when claimed. Items 4–8 can stay research notes under `docs/os/`
or Fast Follow study packets. They do not authorize an ISO.
