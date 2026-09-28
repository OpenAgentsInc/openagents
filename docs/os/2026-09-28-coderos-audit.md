# CoderOS audit: what moves from `~/coder` into OpenAgents

**Status: proposal (2026-09-28).** Nothing has moved yet. This document is
the plan for the first move, and the
[migration assessment](../coder/design/coder-suite-migration.md#coderos-and-execution-environments)
and milestone M12 in the [migration tracker](../coder/migration-status.md)
are the requirements it has to meet.

## Summary

CoderOS in `~/coder` is a NixOS flake under `os/`: 25 modules, 24 shell
scripts, 7 package definitions, one host, and a 1,300-line README, about
11,000 lines of Nix and shell in all, written from 2026-09-07 over 165 commits. The modules
are already parameterized well. Almost every feature is off by default and
takes the account name as an option, so most of what is personal sits in one
file, `os/hosts/coderos-4080.nix`.

The problem is what the modules run, not how they are written. About half of
them start a program that exists only in `~/coder` and has no counterpart
here: the old `coder-terminal`, `coder-runner`, the Autopilot main session,
the inference daemon, and the game pilot. Moving those modules would publish
configuration for programs this repository cannot build.

The recommendation:

1. Publish a generic `os/` flake in this repository that exposes NixOS
   modules, not a host. It carries the base system, the Hyprland desktop,
   the desk command, capture, Tailscale, the Android shell, and a new module
   that installs the OpenAgents Coder host as a service.
2. Ship an example host, `os/examples/workstation/`, derived from
   `coderos-4080` with every personal value replaced by a placeholder.
3. Keep your own machine in a private flake that imports the public one. That
   flake holds your hardware file, SSH keys, username, microphone, and the
   personal modules (Zoom, Battle.net, the deck). A one-line override points
   it at your working checkout, so you edit the public modules and your host
   in one rebuild.
4. Leave the custom compositor, the camera daemon, and hand tracking for a
   second phase, as the migration assessment already says.

## What is in `~/coder/os` today

| Part | Size | What it does |
| --- | --- | --- |
| `flake.nix` | 1 host, 5 packages, 3 shells | Pins `nixpkgs` to one revision; builds the compositor, CoderQuest, the camera daemon, the inference daemon, and the ROCm compiler; offers `android`, `deck`, and `inference` shells. |
| `modules/coderos/` | 25 files, about 4,500 lines | The base system, and one module per optional capability. |
| `bin/` | 24 scripts, about 5,400 lines | Launchers and session tools that the modules wrap with `writeShellApplication`. |
| `pkgs/` | 7 files | Nix builds of Rust binaries from the `~/coder` workspace, plus the CUDA 13.2 toolkit and the ROCm HIP compiler. |
| `hosts/` | 2 files | `coderos-4080` and its generated hardware file. |
| `etc/`, `tests/` | 3 files, 1 crate | The git hooks configuration, and a cuTile GPU smoke test. |
| `README.md` | 1,300 lines | How to rebuild a host, and a guide to every capability. |

Around it, `~/coder` holds the Rust programs the flake builds, 20 shell tests
under `ops/tests/` that exercise the scripts, and design documents under
`docs/os/` and `docs/coderos-*.md`.

### The Rust the flake builds

Each package builds from the `~/coder` workspace. The closures below come
from `cargo metadata` on 2026-09-28.

| Binary | Workspace crates it needs | Lines of Rust | Notes |
| --- | --- | --- | --- |
| `coder-desk` (`bins/coder-desk`) | `coder-desk`, `coder-binds`, `coder-contract` | about 7,300, plus the `desk` module | Every script under `os/bin/` calls it. It needs only the `desk` module of `coder-contract` (1,156 lines), not the other 25,000 lines of that crate. |
| `coder-compositor` | the above, plus `coder-wm` and `coder-hands` | about 15,500 of its own | A Smithay 0.7 compositor with hardware and nested backends. `coder-hands` pulls in `~/coder`'s `jev` and `coder-jev` for its judge feature. |
| `coderos-camera` | `coder-hands` | about 2,800 of its own | Links a static ONNX Runtime fetched by digest. |
| `coder-hands-measure` | `coder-hands`, `quest-hands` | about 3,000 | A measurement tool for the gesture rules. |
| `coder-quest` | 31 crates, including `libghostty-vt` and the `quest-*` crates | large | A spatial desktop experiment. |
| `coder-inference-daemon` | `coder-gptoss`, `coder-harmony`, `coder-stage`, `coder-fleet` | about 40,000 | Local model serving on the GPU. |

Cargo reports the compositor and the camera daemon as depending on
`coder-runner` and most of the old workspace. That comes from feature
unification in the `~/coder` lockfile, not from what the code uses: the
compositor's own `Cargo.toml` names only `coder-binds`, `coder-contract`,
`coder-desk`, `coder-hands`, and `coder-wm`.

`~/coder` pins Rust 1.95.0 and this repository pins 1.97.1. None of the
crates above depends on the GPUI fork that required 1.95, so the newer
toolchain is not expected to block them. Nobody has built them on 1.97.1 yet.

## Decision for each part

Each row says what happens to a part: **carry** moves it with only the
scrubbing that [Before anything moves](#before-anything-moves) lists,
**rewrite** replaces it with a version over this repository's programs,
**private** keeps it in your own host flake, and **drop** leaves it in
`~/coder`.

### Base system and host plumbing

| Part | Decision | Reason |
| --- | --- | --- |
| `default.nix`: boot, SSH, Nix settings, `nix-ld`, Docker, base packages | **Carry** | Generic, and every host needs it. |
| `default.nix`: `security.sudo.wheelNeedsPassword = false` | **Rewrite** as an option that defaults to requiring a password | The right choice for a single-person machine, and the wrong default for a distribution. Your host turns it off. |
| `default.nix`: `coderos.claim` | **Drop** | Runs `coder-runner` from a checkout to hold a claim on the old Coder fleet. `coder-setup` and `coder-host` replace the fleet claim here. |
| `default.nix`: `safe.directory = "*"` | **Rewrite** to name the checkouts the flake builds from | A wildcard trusts every repository on the machine for root's git. |
| `tailscale.nix` | **Carry** | `coder link` in `crates/coder-setup` already reaches hosts over Tailscale. |
| `coder-update.nix` and `bin/coder-update` | **Rewrite** | Builds the old `coder-terminal` from `OpenAgentsInc/coder` and swaps a symlink. Here, `crates/coder-service` already owns trial updates, state snapshots, and rollback. The new module builds `coder` from a checkout of this repository and hands the binary to `coder-service update --to <sha256>`, so rollback is the Rust one. Keep the bounds: `TasksMax`, `MemoryMax`, and the free-space floor. |
| New: `coder-host.nix` | **Write** | Installs `coder host serve` as the systemd user unit `coder-service` generates, with linger. This is the M11 portable host running on CoderOS, and it replaces `writer.nix`. |
| `writer.nix` | **Drop** | Serves `coder-terminal serve` on the old surface lane. The Coder host and NIP-HOST replace it. |
| `git-hooks.nix` and `etc/` | **Drop** | Enrolls checkouts of `OpenAgentsInc/coder` in its pre-push gate. This repository has no `.githooks`, and you don't run gates before a push. |
| `cpu-limits.nix` | **Carry the mechanism, keep the values private** | The power-limit and `max_perf` levers are generic. The numbers belong to one degrading i7-14700K and move to your host file. |
| `firmware.nix` | **Private** | System76 Thelio firmware tools. A later `hardware/` profile could publish it if a second Thelio needs it. |
| `proofs.nix` | **Drop** | An 18-line hook for a verifier package that nothing here supplies. |
| `inference.nix`, `rocm.nix`, `pkgs/cuda-toolkit.nix`, `pkgs/rocm-hip-compiler.nix`, `pkgs/coder-inference-daemon.nix`, `tests/cutile-smoke/`, `bin/coderos-inference`, `bin/coderos-hipcc` | **Drop** | GPU serving for `coder-gptoss` and the Psionic research. None of those crates is in this repository, and the Luna pivot does not call for local serving. Your host keeps the NVIDIA driver and the container toolkit, which are ordinary NixOS options. |

### The desktop

| Part | Decision | Reason |
| --- | --- | --- |
| `desktop.nix`: the Hyprland session, tty1 autologin start, the bind table, monitors | **Carry** | Generic and reviewed. It stays the default compositor, which the migration assessment already requires. |
| `desktop.nix`: `directory`, `command` | **Carry**, retarget | `command` defaults to this repository's `coder`. `directory` stays an option with no personal default. |
| `desk.nix`, `pkgs/coder-desk.nix`, `bins/coder-desk`, `crates/coder-desk`, `crates/coder-binds` | **Carry** | Every script asks the session through `coder-desk`. Move the `desk` module out of `coder-contract` into `crates/coder-desk` so the other 25,000 lines of that crate stay behind. |
| `capture.nix`, `bin/screen-record`, `bin/dictate-toggle`, `bin/beep-sound` | **Carry** | Keyboard-driven screen recording and dictation. The microphone rules are options, and your Blue and C930e values move to your host. |
| `presentation.nix`, `bin/presentation-mode` | **Carry** | Generic and self-contained. |
| `recording-hud.nix`, `bin/recording-hud` | **Carry after camera** | It docks under the camera circle, so it waits for phase 2. |
| `recording.nix` | **Drop for now** | A grant for the old Coder's typed `recording` tool. Nothing here reads it. Bring it back when a Rust tool here does. |
| `browser.nix`, `bin/coder-browser`, `bin/coder-open-url` | **Carry the launcher, hold the grant** | `SUPER + B` and Chromium with a loopback DevTools port are useful alone. The grant file feeds the old `browser` tool, so it stays out until a tool here reads it. |
| `android.nix`, `bin/android-emulator`, and the flake's `android` shell | **Carry** | This repository ships `bins/coder-android` and `bins/openagents-android`, and the shell already names the first. |
| `bin/coder-chord` | **Drop** | Forwards pane chords into CoderQuest windows. CoderQuest does not move. |
| `bin/coder-close` | **Rewrite** | `SUPER + W` asks `coder activity` whether a session is busy before it closes the window. This repository's `coder` has no `activity` command. Until it has one, the key closes after a second press on every Coder window, which keeps the safety without the check. |
| `bin/coder-pane` | **Carry** | Opens a tile through the desk command. |
| `agent-panes.nix`, `agent-notices.nix`, `bin/agent-panes`, `bin/agent-notices` | **Drop for now** | They read the Autopilot main session's child registry over the old surface lane. A later version can read the Coder host's task list over NIP-HOST instead. |
| `deck.nix`, `bin/coder-deck-open`, the flake's `deck` shell | **Private** | Opens the company's slide decks, which live in `~/coder` and build on the GPUI fork. |
| `zoom.nix`, `bin/coder-zoom`, `bin/coder-zoom-open-url` | **Private** | Generic code, but an account-backed video client is a personal choice. It moves to your host flake unchanged. |
| `battlenet.nix`, `gaming.nix`, `bin/coder-battlenet` | **Private** | Battle.net and World of Warcraft, and the grant for the old `coder-gamer` pilot. |
| `pkgs/coder-quest.nix` | **Drop** | CoderQuest is an experiment with 31 crates behind it; Verse covers the spatial surface here. |

### Phase 2: the custom compositor, camera, and hands

| Part | Decision | Reason |
| --- | --- | --- |
| `bins/coder-compositor`, `pkgs/coder-compositor.nix`, `bin/coder-compositor-session`, `crates/coder-wm` | **Carry in phase 2** | Real, tested on `coderos-4080` since 2026-09-17, and optional. The migration assessment sets its gate: input routing, recovery, display changes, focus, and an escape path when the shell fails. `trialTty` already gives that escape path. |
| `camera.nix`, `camera-grant.nix`, `pkgs/coderos-camera.nix`, `bins/coderos-camera`, `bin/camera-overlay`, `bin/camera-toggle` | **Carry in phase 2** | One daemon owns the camera and serves a loopback node, recordings, and hand landmarks. Its ONNX Runtime is fetched by digest, which fits the pinned-dependency rule. |
| `hands.nix`, `crates/coder-hands`, `bins/coder-hands-measure` | **Carry in phase 2**, port the judge | The gesture rules move as they are. The judge calls `~/coder`'s `jev`; port it to this repository's `crates/jev` rather than bringing a second Jev client. |

### What that adds up to

| Decision | Modules | Scripts |
| --- | --- | --- |
| Carry in phase 1 | 8 (`default`, `desktop`, `desk`, `capture`, `presentation`, `tailscale`, `android`, and the `browser` launcher) and the `cpu-limits` mechanism | 8 |
| Rewrite or write | `coder-update` and a new `coder-host` | 2 (`coder-update`, `coder-close`) |
| Carry in phase 2 | `camera`, `camera-grant`, `hands`, `recording-hud`, and the compositor | 4 |
| Private | `firmware`, `deck`, `zoom`, `battlenet`, `gaming` | 4 |
| Drop | `claim`, `writer`, `git-hooks`, `proofs`, `inference`, `rocm`, `recording`, `agent-panes`, `agent-notices` | 6 |

Phase 1 moves about 4,700 lines of Nix and shell and about 8,500 lines of
Rust. It adds no GPU toolkit and no Smithay dependency to this workspace.

## How your own setup keeps working

You want a public CoderOS that anyone can install, and a machine that stays
exactly yours and is quick to change. Three layers do that.

### The layout

```text
openagents/                      public, this repository
  os/
    flake.nix                    exposes modules, packages, and shells; no host
    modules/coderos/             generic modules, every capability off by default
    bin/                         the scripts the modules wrap
    pkgs/                        Nix builds of Rust binaries in this workspace
    examples/workstation/        a complete example host with placeholders
      configuration.nix
      README.md                  how to copy it into a flake of your own

coderos-hosts/                   private, yours (a separate repository)
  flake.nix                      imports openagents' modules
  hosts/coderos-4080.nix         today's host file, unchanged in spirit
  hosts/coderos-4080-hardware.nix
  modules/                       zoom.nix, battlenet.nix, gaming.nix,
                                 deck.nix, firmware.nix
```

The public flake exports `nixosModules.coderos`, the same module set
`./modules/coderos` is today. A host flake imports it:

```nix
{
  inputs.openagents.url = "github:OpenAgentsInc/openagents?dir=os";
  inputs.nixpkgs.follows = "openagents/nixpkgs";

  outputs = { nixpkgs, openagents, ... }: {
    nixosConfigurations.coderos-4080 = nixpkgs.lib.nixosSystem {
      system = "x86_64-linux";
      modules = [
        openagents.nixosModules.coderos
        ./hosts/coderos-4080.nix
        ./modules/zoom.nix
        ./modules/battlenet.nix
      ];
    };
  };
}
```

A flake fetched with `?dir=os` still carries the whole repository, so the
package definitions that build from `../..` keep working.

### The edit loop

Most days you change a module and your host together. Point the input at your
working checkout for that rebuild:

```sh
sudo nixos-rebuild switch \
  --flake ~/coderos-hosts#coderos-4080 \
  --override-input openagents "git+file://$HOME/openagents?dir=os"
```

Nix reads tracked files from the working tree, including uncommitted edits, so
a change to `~/openagents/os/modules/coderos/desktop.nix` is live without a
commit. A new file needs `git add` first, because a git flake ignores
untracked files. When the change is right, commit it here, push, and run
`nix flake update openagents` in `coderos-hosts` to pin the new revision.

A change only to your own machine, such as a new bind or a microphone gain,
stays in `coderos-hosts` and never touches this repository.

### Why not a gitignored host directory here

A gitignored `os/hosts/local/` looks simpler, but a git flake cannot see
untracked files, so Nix would not find the host at all. Keeping the host in
this repository instead would publish your SSH keys, username, microphone
serial, and hardware inventory, and the migration assessment already rules
out private host inventories. A separate private flake is the smallest shape
that works.

### The example host

`os/examples/workstation/configuration.nix` is `coderos-4080.nix` with its
choices kept and its identity removed:

- `user` becomes `"you"`, `authorizedKeys` becomes an empty list with a
  comment, and `hostName` becomes `"coderos"`.
- The NVIDIA block, CPU limits, `rasdaemon`, zram, and PostgreSQL move to
  comments that say when a machine wants them.
- The desktop turns on Hyprland, capture, presentation, the browser launcher,
  and the Coder host service. The compositor, camera, and hands stay off
  until phase 2.
- Zoom, the deck, Battle.net, and gaming do not appear.

`nix flake check` evaluates the example against a stub hardware file, which
is the first proof that a machine that isn't yours can build.

## Before anything moves

`~/coder` is closed source and this repository is public. Moving these files
publishes them, which is your decision as the owner of both, and it is the
exception to the rule in `AGENTS.md` against copying from `~/coder`. Record
it in each move's commit message. Scrub each file before it lands:

- Remove the SSH public keys, including the `devin-psionic` key, the
  username, home paths, and the example tailnet address `100.64.0.9`.
- Rewrite links to `OpenAgentsInc/coder` issues. That repository is private,
  so the links are dead here. Keep the dated observation the link supported,
  such as "On 2026-09-11 a test run took all 125 GB", and drop the number.
- Remove the fundraising deck's name from comments.
- Remove the host's PostgreSQL loopback trust. It served `ops/gate.sh` and is
  not part of CoderOS.
- Retarget comments that name `~/coder` paths, such as `bins/coder-terminal`
  and `docs/autopilot/`, or delete them when the thing they name doesn't
  move.

Add a line to `AGENTS.md` naming `os/` and its Nix and shell as
infrastructure in the same sense as the retained Python and shell tooling, so
nobody reads the scripts as permission for another product language. The
larger scripts, `screen-record` at 1,218 lines and `presentation-mode` at
428, hold product behavior. Port them to `coder-desk` subcommands over time
rather than growing them.

## The order of work

1. **Flake skeleton.** Add `os/flake.nix` with the pinned `nixpkgs`, the
   `nixosModules.coderos` export, and the `android` shell. Add the base module
   with the sudo option.
2. **Desk command.** Move `crates/coder-desk`, `crates/coder-binds`, and
   `bins/coder-desk`, with the `desk` protocol types folded into
   `coder-desk`. Run their tests on 1.97.1.
3. **Desktop.** Move `desktop.nix`, `desk.nix`, `capture.nix`,
   `presentation.nix`, `tailscale.nix`, `android.nix`, and their scripts,
   scrubbed. Port the matching tests from `~/coder/ops/tests/`.
4. **Coder host.** Write `coder-host.nix` and the new `coder-update.nix` over
   `coder-service`.
5. **Example host.** Add `os/examples/workstation/` and a `nix flake check`
   that evaluates it.
6. **Your host flake.** Create the private `coderos-hosts` flake from
   `coderos-4080.nix` and the five private modules. Build its toplevel and
   compare it with what `coderos-4080` runs before switching. Keep
   `~/coder/os` until one switch from the new flake has held for a week.
7. **Phase 2.** The compositor, `coder-wm`, the camera daemon, and hands,
   each behind its own option, with the judge ported to `crates/jev`.

Steps 1 to 5 close most of M12's configuration work. M12 still needs a clean
install, missing-hardware behavior, disk pressure, an interrupted upgrade,
and rollback verified on a machine other than `coderos-4080`.

## Open questions

- **Where does the private flake live?** A private GitHub repository is the
  simplest. A directory under `~/work` with no remote works too, but then the
  machine is the only copy of its own description.
- **Does `coderos-4080` switch its default compositor back to Hyprland** while
  the Coder compositor waits for phase 2? It has run the Coder compositor on
  tty1 since 2026-09-17. Your private flake can keep building it from `~/coder`
  in the meantime, so nothing forces the switch.
- **Does the example host ship Docker on by default?** The base module turns
  it on because the Microcoder container path uses it. A distribution might
  prefer it as an option.
