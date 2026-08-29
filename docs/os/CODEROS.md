# CoderOS

A Linux distribution whose default job is **Coder**: running it, being
operated by it, and compiling and serving local models for it.

Omarchy showed that a strongly opinionated Linux desktop, with coding agents
as first-class applications and a CLI that addresses every GUI action, can
pull people onto Linux. CoderOS takes that product lesson and aims it at
one agent and one inference stack we own, rather than at a menu of nine
vendor CLIs on top of Arch plus Ollama.

## What it is

CoderOS is a **Coder-native Linux**. Three properties define it:

1. **Coder is the default operator.** The distro installs, updates, themes,
   diagnoses, and builds through commands Coder can call. A human can still
   drive the same commands. The GUI is a view over that CLI, not a second
   control plane.
2. **Coder is a first-class resident.** The OpenAgents CLI, its TUI, local
   inference, Harbor/gym, and worktree workflow are packaged, themed, and
   keyed as tightly as Omarchy keys Claude Code and Codex. They are not an
   optional AUR extra.
3. **Coder builds the distro.** Package lists, desktop config, GPU drivers,
   model admission, and ISO contents are artifacts Coder can change, test,
   and ship. The OS is a product Coder maintains, not a snapshot of one
   person's dotfiles.

The third point is the one Omarchy does not make. Omarchy is DHH's taste,
distributed. CoderOS is a machine that Coder can keep true.

## What it is not

- Not a rebase of Omarchy onto NixOS. Community ports already exist
  (`henrysipp/omarchy-nix`, `mrosseel/omarchy-nix`). They copy a desktop.
  They do not give us Coder.
- Not a promise that we will invent a new kernel, a new libc, or a new
  package language in order to ship a nicer config file.
- Not a replacement for Omega on Desktop, Phoenix on the web, or Firecracker
  guests in Cloud. Those stay where they are. CoderOS is the Linux host
  someone sits at, and later the Linux host Coder builds on.
- Not an instruction to weaken Coder approvals, Keychain rules, or
  `INVARIANTS.md` because a desktop hotkey wants unattended agents.
- Not the current local-inference program. That program's first supported
  platform is Apple Silicon Metal. CoderOS *needs* the Linux backends that
  program deferred. It does not override the plan.

## Dual role

Omarchy is a workstation. CoderOS has two roles that share one image family:

| Role | Who sits there | What must be true |
| --- | --- | --- |
| Workstation | a human using Coder all day | beautiful, fast, opinionated desktop; Coder keyed; local models; GPU that works |
| Build host | Coder, Harbor, gym, later Firecracker guests | reproducible toolchain, native Linux CLI, Docker/amd64, content-addressed models, no GUI required |

Phase 0 is the CLI and inference on *existing* Linux. The workstation ISO
is a later product, and only if Phase 0 is something we would install
ourselves. The build-host image can be a profile of the same tree (no
compositor, no themes) rather than a second distro.

## Sequencing law

**Do not start with an ISO.** Start with Coder on Linux being excellent.

Omarchy's power is not Arch. It is years of daily-driver scar tissue:
migrations, hardware probes named after laptop models, a CLI that is the
menu, a vendor/user filesystem split, and an agent skill that teaches that
split to the other operator. We do not have that scar tissue for Coder on
Linux yet. Burning an ISO first would freeze the wrong opinions.

Order:

1. **Phase 0 — Linux CLI and local models.** Coder and `openagents inference`
   are first-class on current Linux (Debian/Ubuntu, Fedora, Arch, NixOS).
   See [LINUX-CLI.md](./LINUX-CLI.md). This is the only phase that may start
   without a new substrate decision.
2. **Phase 1 — CoderOS profile.** A declared system configuration (NixOS
   module, Guix system, Arch overlay, or something else — undecided) that
   installs Phase 0 plus the desktop ideas taken from Omarchy. Lives on top
   of an existing distro. No installer ISO required.
3. **Phase 2 — ISO.** Only after a human can spend a week on the Phase 1
   profile and Coder can rebuild it. The ISO is a delivery vehicle for a
   configuration we already trust.

Skipping to Phase 2 is how you get a pretty Arch fork with our logo on it.

## Relationship to Omarchy

Take the *product architecture*, not the packages. The transfer list is in
[OMARCHY.md](./OMARCHY.md). The short version:

Take: CLI-addressable desktop, vendor/user split, user-state migrations,
hardware quirks as code, agents as a window class, OS skill for the agent,
theme as a compiler, update as a transaction, GPU that just works.

Do not take: nine-vendor agent menu as the identity, auto-approve from a
hotkey, Ollama/LM Studio as the local-model story, Arch-plus-pacman as
destiny, passwordless sudo as the agent privilege model, Secure Boot off
as a default we copy without a new decision.

## Relationship to local models

Today Coder Local talks to Ollama. The accepted program in `docs/psionic/`
replaces that with in-process Psionic Qwen 3.8 inside the `openagents`
binary, first on Metal.

CoderOS is the Linux half of that story:

- The model store (`~/.openagents/inference/models/<digest>.gguf`) is a
  content-addressed cache. A distro that already thinks in content hashes
  (Nix, Guix) fits that store. A distro that does not can still host it.
- Linux needs CPU plus CUDA and/or Vulkan. Metal will not save a CoderOS
  laptop.
- `openagents inference doctor` is the hardware skill. Omarchy's
  `omarchy-hw-nvidia-gsp` is the pattern: named probes, not a wiki page.
- Weights stay out of git and out of the ISO. The ISO may carry *admission
  metadata* (digest, license, family). The user or Coder fetches the blob.

Until the Psionic Linux backends exist, CoderOS workstation demos will
still be Ollama. That is a temporary compatibility lane, not the product.

## Relationship to Cloud

Cloud computers and managed sandboxes remain Google Cloud / Firecracker /
GCE. CoderOS does not become the production isolation boundary.

A CoderOS workstation *may* later run local Firecracker workrooms for
Coder, the way Omarchy runs Docker and a Windows VM. That is a Phase 1+
research item, not Phase 0. The Cloud contracts stay the authority for
hosted isolation.

## Success

CoderOS is succeeding when all of these are true:

1. `curl https://openagents.com/install.sh | bash` on a fresh Linux box
   yields a Coder TUI that is as good as the macOS one, including
   clipboard, notifications, GPU detection, and `--prompt`.
2. `openagents inference run --gguf <qwen38>` walks load through generate
   on that box without spawning Ollama, on CPU and on the machine's GPU.
3. `openagents coder --local` completes a multi-round tool turn against
   that in-process engine.
4. Every desktop action we care about has a stable CLI address Coder can
   call, and a skill file that says which paths Coder may write.
5. Coder can rebuild the Phase 1 profile from a declared config and a
   pinned lock, and a human can roll back a bad update.

(1) does not require a new distro. (2) and (3) require Psionic Linux
backends. (4) and (5) are the distro. Do them in that order.
