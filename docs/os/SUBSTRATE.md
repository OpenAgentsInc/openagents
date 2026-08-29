# Substrate: Arch, NixOS, Lua, Guix, or a new distro

This document compares bases for a later CoderOS image. **Phase 0 does not
need a choice.** It runs on Linux we already have. Choosing a substrate
before Coder is excellent on Linux would make the ISO the product.

Three comments in circulation, restated as positions:

| Position | Claim |
| --- | --- |
| **Nix rebase** | Omarchy is the right desktop; rebuild it on NixOS so the machine is a function of a config. |
| **Nix-shaped, Lua config** | Keep the Nix (or Nix-like) store and evaluation model; replace the Nix language with Lua. |
| **Guix** | Same functional-store idea as Nix, but the language is Guile Scheme and the system is Guix. |

CoderOS is not obligated to pick any of them. A fourth position is “stay on
Arch (or Debian) and steal Omarchy's migration runner.” A fifth is “write a
new distro.” All are research, not a decision.

## What we actually need from a substrate

From [CODEROS.md](./CODEROS.md) and [OMARCHY.md](./OMARCHY.md):

1. **GPU that works** — NVIDIA CUDA and/or Vulkan, AMD, Intel. Local Qwen
   3.8 is not a Metal-only product on Linux.
2. **Reproducible CLI and toolchain** — the same `openagents` binary, Rust
   toolchain, Harbor images, and admitted GGUF the gym used last week.
3. **Rollback that a human and Coder both understand.**
4. **A config Coder can edit without forking the vendor tree.**
5. **A package set that includes non-libre firmware and NVIDIA userspace.**
   A Coder laptop with a dGPU is not a linux-libre appliance.
6. **An update transaction** that cannot be bypassed by accident.
7. **A build-host profile** of the same tree (headless, no compositor).

Beauty, tiling, and Quickshell are Phase 1 desktop choices. They do not
select the store.

## Arch (Omarchy's base)

Strengths: NVIDIA install scripts that match real laptops; rolling
security; AUR depth; Omarchy already proved the desktop product on it;
CoderOS could overlay without inventing a package manager.

Weaknesses: the machine is mutable. Omarchy's whole migration runner exists
because of that. Reproducibility is “here is an ISO and a channel,” not
“here is a lockfile of the world.” Two CoderOS laptops drift unless we
become as disciplined as Omarchy about `omarchy update` as the only path.

Fits if we want the fastest path to a daily-driver ISO that feels like
Omarchy and we are willing to own migrations forever.

## NixOS (the rebase demand)

The demand is real. NixOS Discourse threads ask for an Omarchy-class
batteries-included flake. Community ports exist. NixOS's answer to
vendor/user drift is to rebuild rather than migrate.

Strengths that rhyme with CoderOS:

- Content-addressed store. Our model store is already
  `~/.openagents/inference/models/<digest>.gguf`. A system store that also
  thinks in hashes is a friendlier home than `/usr`.
- `nixos-rebuild` / generation rollback is the honest cousin of Omarchy
  snapshots, and it *can* include more of the system if we put state in
  the store.
- Flakes + lockfiles are a natural Coder artifact: Coder edits a module,
  updates a lock, CI builds the closure.
- The OpenAgents Linux installer already special-cases NixOS: the glibc
  loader is not at `/lib64/ld-linux-x86-64.so.2`, so the **musl** CLI
  artifact is the one that runs. We have already paid that tax.

Weaknesses:

- The Nix language is the complaint. Modules, overlays, `mkForce`, and
  `nixpkgs` pin drift are a real onboarding cost. Coder can write Nix;
  humans who wanted Omarchy often cannot.
- CUDA on NixOS works and is painful (pin `nixpkgs`, `cudaPackages`,
  driver/kernel pairing). This is the CoderOS-critical package, not hello
  world.
- Rebuilding the world for a bar widget is the cost Omarchy refused.
  Hyprland/Quickshell live in a fast-moving Arch world; a Nix port of
  Omarchy is already known to lag.
- “Rebase Omarchy” still leaves us maintaining Hyprland desktop opinions
  that are not Coder.

**Research hypothesis, not a decision:** Nix (or Lix/Snix) as the *store
and build*, with **Lua (or similar) as the desktop and CoderOS module
language** that generates Nix. That is closer to the “Nix but Lua”
comment than a literal rebase of Omarchy's bash onto `configuration.nix`.

Related Nix-family research (untrusted refs):

- [henrysipp/omarchy-nix](https://github.com/henrysipp/omarchy-nix) /
  [mrosseel/omarchy-nix](https://github.com/mrosseel/omarchy-nix) — Omarchy
  experience on NixOS.
- [Lix](https://lix.systems) / [Snix](https://snix.dev) — Nix implementation
  work in a different governance/language-host shape.
- [Nickel](https://github.com/nickel-lang/nickel) — typed config language
  inspired by Nix; integration with nixpkgs is still the hard part.
- Clan, Determinate Nix, `nixos-anywhere` — fleet deploy patterns if
  CoderOS ever means more than one machine.

## “Nix but Lua”

Restate the comment: *why rebase onto Nix if the pain is the Nix language?
Make a Nix-like distro configured in Lua.*

Facts that bear on it:

- Omarchy 4.0 already moved Hyprland to Lua. Lua won as a *desktop* config
  language independently of Nix. That is evidence people will edit Lua to
  bind keys and name monitors. It is not evidence Lua can express
  `mkDerivation`, build sandboxes, and nixpkgs.
- Nix's difficulty is not only syntax. It is laziness, the module merge
  algebra, and the size of nixpkgs. A Lua frontend that emits the same
  derivations still has to confront nixpkgs. A Lua that *replaces*
  nixpkgs is a new distro pretending not to be.
- Existing Lua-in-Nix experiments are supplemental (evaluate Lua, return
  a Nix attrset). None is a distro.
- Coder already lives in Lua-adjacent ecosystems (Neovim, Hyprland, WezTerm).
  A CoderOS desktop DSL in Lua is a small, good idea even if packages stay
  Nix or pacman.

**Split the question:**

| Layer | Language that may win | Why |
| --- | --- | --- |
| Package build / store | Nix, Guix, or pacman+git | Existing ecosystems; do not rewrite |
| System modules (services, users, GPU) | Nix modules, Guix services, or a thin Lua that generates them | Coder can generate this |
| Desktop / keybinds / theme / agent window class | Lua | Omarchy already proved this |

“A new distro that's Nix with Lua” is only worth it if we cannot generate
Nix from Lua. Generating Nix from Lua is a compiler. Writing a new store
is a decade. Prefer the compiler.

## Guix and Scheme

Guix is the functional-store idea with Guile Scheme as the language, GNU
Shepherd instead of systemd, and a FSF-shaped package set. Guix 1.5.0
(early 2026) improved hardware and KDE support. Scheme is a real language
with a real standard library; that is the whole pitch versus Nix.

For CoderOS the blocking issues are not taste:

- **linux-libre and non-free firmware.** Guix System defaults to a kernel
  and package set that omit NVIDIA userspace and much laptop firmware.
  Extra channels exist. Using them is fighting the project's identity to
  get the one GPU we need for local models.
- **Package breadth.** nixpkgs is the largest repo on Repology; Guix is
  smaller and libre-constrained. Rust, Docker, NVIDIA, Chrome, and
  proprietary agent CLIs (if we even ship them) are all harder.
- **Shepherd vs systemd.** Omarchy's crash-to-agent path is
  systemd-coredump. Harbor, Docker, and our Cloud mental model assume
  systemd. Shepherd is coherent in Guix and a tax for us.
- **Scheme literacy.** Coder can write Scheme. The humans who wanted
  Omarchy wanted Lua and a pretty ISO, not a Lisp machine.

Guix remains a **research peer**: same store thesis as Nix, different
language. Study it for module-as-code and Shepherd as “the init system is
also Scheme.” Do not pick it as the CoderOS ISO base unless an owner
decision accepts linux-libre-shaped GPU pain or a permanently unofficial
non-free channel.

## A new distro

Cost: kernel policy, installer, package repository, signing, mirrors,
security updates, hardware enablement. Omarchy did this on top of Arch
(own ISO, own repo, own mirror, own keyring) and still needed a foundation
and a core team. Doing it *and* inventing a store language is two products.

A new distro is justified only after Phase 0+1 prove that no existing
substrate can express the CoderOS profile. We are not there.

## Working ranking (research, not admission)

Until Phase 0 exists, treat this as a study order, not a build order:

1. **Phase 0 on all of them.** Debian/Ubuntu, Fedora, Arch, NixOS (musl
   artifact), maybe WSL. Prove Coder + inference. See
   [LINUX-CLI.md](./LINUX-CLI.md).
2. **NixOS module / flake as the first Phase 1 prototype**, because the
   store matches the model cache, rollback is native, and Coder can emit
   flakes. Desktop config in Lua that generates or wraps that module.
   CUDA enablement is the first hard test, not Hyprland ricing.
3. **Arch overlay as the fast desktop prototype**, if we want Omarchy-like
   feel before Nix CUDA is civilized. Throw it away if Nix catches up.
4. **Guix as a paper comparison and maybe a package-manager-on-foreign-distro
   experiment**, not as the ISO.
5. **New language / new distro** last, and only with an owner decision.

## What not to do

- Do not fork `omarchy-nix` and sprinkle Coder into it.
- Do not start a Lua-to-Nix compiler before `openagents inference doctor`
  tells the truth on a Linux NVIDIA box.
- Do not treat “lots of people asked for NixOS Omarchy” as authority to
  skip Phase 0.
- Do not hide a linux-libre GPU limitation behind “Scheme is nicer.”
