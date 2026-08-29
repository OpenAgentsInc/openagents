# Phase 0: Coder and Psionic as first-class Linux software

CoderOS starts here. No ISO. No new package language. The artifact is the
OpenAgents CLI (and later the in-process inference library) behaving as if
Linux were the development machine, not a cross-compile afterthought.

Apple Silicon remains the first *supported* local-inference platform in
`docs/psionic/PLAN.md`. This document does not change that. It says what
must be true on Linux before a CoderOS profile is worth drawing.

## What already exists

| Surface | Linux state today |
| --- | --- |
| Installer | `https://openagents.com/install.sh` already names `linux-x86_64`, `linux-x86_64-musl`, `linux-aarch64`, `linux-aarch64-musl`. |
| NixOS | musl artifact is the one that runs; glibc loader is in the Nix store. Documented in the CLI release runbook. |
| Coder Local | Ollama lane (`--model ollama:…`, `--local`). Works wherever Ollama works. |
| Gym / Harbor | Runbook already requires a **native Linux CLI build**; Terminal-Bench images are amd64. |
| Inference program | In-process GGUF path landed on `main` for mmap/Metal wrap; CUDA is a second-platform packet and must not default on in the macOS artifact. |
| Clipboard | Linux branch exists in `crates/openagents-cli/src/coder/clipboard.rs`; needs a real Wayland/X11/tmux matrix. |
| Auth | Linux keychain path exists in `auth.rs`; must not use macOS Keychain APIs or prompt-y `security` calls. |

Linux is already a release target. It is not yet a first-class *product*
target.

## Definition of first-class

A Linux user who never owned a Mac should be able to:

1. Install with the public script and land on a `openagents` that is the
   same command surface as macOS.
2. Log in, run `openagents coder`, and get a TUI that is not missing
   panes, copy, paste, notifications, or `--prompt` flushing.
3. Run `openagents inference doctor` and see **this machine's** backends:
   CPU always; CUDA if NVIDIA + driver; Vulkan if that is the admitted
   path; Metal never claimed.
4. Add an admitted GGUF and complete `inference run` through generate
   without Ollama.
5. Point Coder at that engine (`--model psionic:<id>` then `--local`)
   for a multi-round tool turn.
6. Run Harbor/gym against the Linux binary without copying a Darwin
   artifact into a container.
7. Fail with a named reason when a backend or libc is wrong (NixOS musl
   vs Debian glibc, missing NVIDIA userland, too little VRAM), not
   `No such file or directory` on a file that exists.

Until (1)–(3) and (7) are true, an ISO is theatre. Until (4)–(5) are
true, CoderOS cannot claim “Psionic local models.” Until (6) is boring,
we cannot let Coder build CoderOS in CI.

## Workstreams

Independent enough to claim separately. Linux-CLI quality does not wait
on CUDA kernels.

### A. Install and libc

- Keep the four Linux artifacts. Do not rename glibc to `-gnu`; the
  installer contract forbids it.
- `openagents inference doctor` and `openagents os doctor` (if we add it)
  should print which artifact is running (`gnu` vs `musl`) and why.
- NixOS, Alpine, distroless, and “Debian with musl installed beside
  glibc” are already installer test cases in Phoenix
  (`test/openagents_web/install_script_test.exs`). Add a CoderOS note
  when we publish a recommended distro list — as recommendations, not
  support promises.
- Verify `linux-aarch64` on a real ARM laptop/SBC, not only x86_64
  GitHub runners.

### B. TUI, clipboard, notifications, tty

Omarchy is a Wayland desktop. Coder on Linux will run in Foot/Kitty/Ghostty
under Hyprland, in tmux over SSH, and in Harbor with no display.

- Clipboard: Wayland (`wl-clipboard`), X11 (`xclip`/`xsel`), tmux, and
  “none” must each be detected. The existing Linux branch should be
  tested, not assumed.
- Notifications: `notify-send` / `org.freedesktop.Notifications` on a
  desktop; silent skip in Harbor.
- Truecolor, OSC 52, and the interactive PTY suite
  (`cargo test -p openagents-cli --test coder_interactive_pty`) on Linux
  as a first-class CI job, not only Darwin.
- `--prompt` flush/Done behavior (#343) confirmed on glibc and musl.

### C. GPU and `inference doctor`

Omarchy's lesson: named probes, not a wiki.

Minimum probes:

| Probe | Question |
| --- | --- |
| `cpu` | always |
| `nvidia-smi` / driver | CUDA userland present? |
| GSP vs legacy NVIDIA | which driver family, as Omarchy splits 580xx vs nvidia-open |
| `vulkaninfo` | Vulkan devices |
| ROCm | AMD |
| VRAM / unified memory | can this box hold the admitted Qwen 3.8 quant? |
| libc | gnu vs musl |

`docs/psionic/PLAN.md` keeps CUDA out of the macOS artifact. The Linux
artifact is allowed — as a **feature**, not a default of Darwin — to
enable `inference-cuda` / `inference-vulkan`. First Linux generate may
be CPU-only; doctor must still tell the truth about the GPU we are not
using yet.

### D. In-process inference on Linux

Depends on the Psionic import, not on an ISO.

- CPU backend on `linux-x86_64` and `linux-aarch64` (gnu and musl as
  far as SIMD and linking allow).
- CUDA packet: Linux x86_64 first, then aarch64 if we care about
  Grace/Jetson-class boxes.
- Vulkan as the non-NVIDIA GPU path if CUDA is not universal.
- Same GGUF admission, same store path, same teach-mode status lines as
  [docs/psionic/CLI.md](../psionic/CLI.md).
- Ollama remains until replacement gates pass. No silent fallback.

Weights still live in the user store. A CoderOS profile may *seed*
admission metadata (digest + URL + license). It must not ship 20 GB in
the ISO.

### E. Coder Local on Linux against Psionic

Same selection table as the Psionic plan:

| User input | Engine |
| --- | --- |
| `--model ollama:<name>` | Ollama |
| `--model psionic:<id>` | in-process |
| `--local` after the switch | Psionic |

Linux-specific: tool subprocesses, sandboxing, and Harbor's amd64 images
must see a Linux `openagents` that was built for Linux. Cross-testing
from macOS via Docker is necessary but not sufficient; a Linux host
(workstation or CI) must run the PTY suite and a small local-lane gym
row.

### F. Agent-operable OS CLI (thin)

Even before a distro, define a small `openagents linux` or `openagents os`
group that only does things we can implement on any distro:

- `doctor` — libc, GPU, display, audio, docker, coredump, inference store
- `clipboard` / `notify` wrappers used by the TUI
- `crash` — facts from systemd-coredump, prompt for Coder (Linux
  desktop/systemd only; no-op elsewhere)

Do not wrap `apt`/`dnf`/`pacman`/`nixos-rebuild` in Phase 0. That is how
we accidentally pick a substrate. Phase 1 adds the substrate-specific
update transaction.

### G. Gym on Linux as the canary

The autoimprove runbook already says the suite runner builds and installs
a native Linux CLI in each Harbor environment. Make “CoderOS Phase 0 is
not done” equivalent to “we cannot run `tb2-quick` on a Linux host with
the local lane.” That is a sharper gate than screenshots of Hyprland.

## Explicit non-goals for Phase 0

- Hyprland, Quickshell, themes, ISO, installer branding.
- Shipping Claude Code / Codex stubs. Coder is the agent.
- Passwordless sudo, auto-approve hotkeys, Secure Boot policy.
- Replacing Cloud Firecracker with a local hypervisor.
- Importing the full Psionic monorepo or enabling CUDA on macOS artifacts.

## Suggested first claims (when someone implements)

Order is the dependency order, not a mandate to open issues (repository
policy still keeps GitHub issues for bugs):

1. Linux doctor: libc + GPU + display + docker, tests on gnu and musl.
2. Interactive PTY suite green on a Linux runner.
3. CPU `inference run` through generate on Linux for a small admitted
   GGUF (CI fixture, not 27B).
4. CUDA or Vulkan packet on one real NVIDIA box, recorded as evidence,
   not as a support matrix.
5. One local-lane gym row on Linux against Psionic when (3) exists;
   against Ollama until then.

## Handoff into Phase 1

When A–E are boring, the substrate document becomes actionable: we know
which GPUs we handle, which libc we ship, and whether Coder can live in
a flake. Then a NixOS module or Arch overlay is a packaging of Phase 0,
not a hope that the desktop will make the CLI good.
