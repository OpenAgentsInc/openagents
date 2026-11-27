# 77. A Windows host

> **Status: partly implemented.** The Win32 + Vulkan OpenAgents Desktop host
> ([`apps/openagents-desktop/windows/`](../../apps/openagents-desktop/windows),
> [`apps/shared/host/windows/`](../../apps/shared/host/windows), `#13782`, `#14046`),
> Windows OS core seam ([`coax/lib/agent/desktop/os-windows.coax`](../../coax/lib/agent/desktop/os-windows.coax)),
> and shared `#authority` host conformance admission check
> ([`metaverse-engine/host/conformance.coax`](../../metaverse-engine/host/conformance.coax))
> run on `cpu`, grounded by
> [`openagents/examples/windows-host-seam.coax`](../examples/windows-host-seam.coax)
> and [`openagents/examples/windows-host-seam.out`](../examples/windows-host-seam.out),
> and are consumed by [`coax/lib/agent/desktop/os-posix.coax`](../../coax/lib/agent/desktop/os-posix.coax),
> [`apps/openagents-desktop/host-core.coax`](../../apps/openagents-desktop/host-core.coax),
> and [`apps/openagents-desktop/windows/build.sh`](../../apps/openagents-desktop/windows/build.sh).
> Remaining: native Windows CI hardware qualification (real Vulkan driver, IME,
> Authenticode signing) and Metaverse Engine Windows frame-parity conformance.

## Premise

Six of the seven platforms Episode 289 named have a host. This chapter builds
the seventh: a Windows host for the engine and the platform layer. It is an
additional platform. The web and phone apps ship without it (chapter 76), and
Windows is qualified on its own evidence.

## Draft

Episode 289 lists them: Windows, Mac and Linux, iOS and Android, web and
terminal. When the initial port audit ran, Windows had nothing behind it.
Since then, **the Win32 + Vulkan OpenAgents Desktop host has shipped** in
[`apps/openagents-desktop/windows/`](../../apps/openagents-desktop/windows)
(`#14046`, with WebView2 integration in `browser_webview2.c`, cross-build
tooling in `build.sh`, Wine smoke harnesses `wine-smoke.sh` and
`wine-window.sh`, and Linux/Windows `Inkling-2-Mini` CUDA serving and variant
selection on RTX 4080-class GPUs). What remains open for this chapter is the
full **Metaverse Engine** Windows host (`metaverse-engine/host/windows/`)
passing [`conformance.coax`](../../metaverse-engine/host/conformance.coax)
with pixel-for-pixel frame parity against Linux Vulkan.

A host is the layer between Coax code and a platform. It opens a window, reads
input, plays sound, talks to the graphics card, opens sockets and reads files.
The OpenAgents Metaverse Engine defines what a host must provide, and has conformance checks that a
host must pass. The tree has hosts for the browser, for Apple's platforms, for
Linux Vulkan, and the Win32 + Vulkan desktop carrier in
[`apps/openagents-desktop/windows/`](../../apps/openagents-desktop/windows).

The graphics decision (audit decision 5) is settled: **Vulkan** is the shared
desktop graphics interface across both Linux and Windows, avoiding a separate
Direct3D backend. Windowing and input use native Win32, embedded browser panes
use WebView2, and Coax compiles cleanly to Windows executables via C output.

Windows matters more than its place at the end of the book suggests. Chapter
70's sellers are people with gaming computers, and most gaming computers run
Windows. Psionic's CUDA serving backend (`psionic-inkling-serve`) and the
desktop Win32/Vulkan carrier already target NVIDIA Windows hardware; finishing
full Metaverse Engine host conformance completes the seventh platform.

Because a host is a contract with checks, this is a bounded job. The chapter
is done when the Windows host passes the same conformance run as the others
and the same realm draws the same frame.

## What we build

Shipped today:

- Win32 + Vulkan OpenAgents Desktop host ([`apps/openagents-desktop/windows/`](../../apps/openagents-desktop/windows), `#14046`), including WebView2 pane support (`browser_webview2.c`), build scripts (`build.sh`), and Wine smoke checks (`wine-smoke.sh`, `wine-window.sh`).
- Windows/Linux NVIDIA CUDA serving support and `Inkling-2-Mini` variant catalog selection.

Remaining for full chapter completion:

- A Windows host for the Metaverse Engine (`metaverse-engine/host/windows/`): window, input, Vulkan graphics, sound, sockets, and files passing the engine conformance suite.
- Full Pylon and CLI release qualification on native Windows CI hardware.
- The host conformance run passing on Windows, with a frame that matches the
  other hosts.

It builds on what exists today:

- [`apps/openagents-desktop/windows/README.md`](../../apps/openagents-desktop/windows/README.md)
- [`apps/openagents-desktop/windows/main.c`](../../apps/openagents-desktop/windows/main.c)
- [`apps/openagents-desktop/windows/browser_webview2.c`](../../apps/openagents-desktop/windows/browser_webview2.c)
- [`metaverse-engine/host/README.md`](../../metaverse-engine/host/README.md)
- [`metaverse-engine/host/conformance.coax`](../../metaverse-engine/host/conformance.coax)
- [`metaverse-engine/host/build-vulkan.coax`](../../metaverse-engine/host/build-vulkan.coax)
- [`coax/platform`](../../coax/platform)
- [`apps/openagents-cli`](../../apps/openagents-cli)
- [`psionic/lib/pylon`](../../psionic/lib/pylon)

### The chapter's check

- **Source:** [`coax/lib/agent/desktop/os-windows.coax`](../../coax/lib/agent/desktop/os-windows.coax),
  [`metaverse-engine/host/conformance.coax`](../../metaverse-engine/host/conformance.coax),
  [`apps/openagents-desktop/windows/main.c`](../../apps/openagents-desktop/windows/main.c),
  [`apps/shared/host/windows/ui-host.c`](../../apps/shared/host/windows/ui-host.c),
  [`openagents/examples/windows-host-seam.coax`](../examples/windows-host-seam.coax),
  and [`openagents/examples/windows-host-seam.out`](../examples/windows-host-seam.out)
  (remaining: native Windows hardware conformance and pixel-for-pixel Vulkan frame parity).
- **Trace:** assembles the deduplicated Git-for-Windows worker `PATH`
  (`C:/Users/alice/.local/bin;C:/Program Files/Git/usr/bin;c:\program files\git\cmd;C:\Windows\system32`),
  constructs the `robocopy` folder-clone invocation with `/XJ` junction guard
  (`ok=true`), and verifies exact host conformance probe receipt admission
  (`conformance ok=true`).
- **Refusal:** `robocopy` error exit codes (`>= 8` or `< 0`) refuse
  (`dcwin_robocopy_ok=false`); POSIX case-sensitive comparison rejects
  mismatched case; and `hc_core` refuses any dropped frame, wrong host version,
  or 1-word tampered event/asset probe.
- **Independent check:** `coax/bin/coax run openagents/examples/windows-host-seam.coax`
  and `--release` match [`openagents/examples/windows-host-seam.out`](../examples/windows-host-seam.out),
  alongside `apps/openagents-desktop/windows/args-test.c`, cross-build
  `coax/remote/check.sh windows-build`, and Wine smoke/window harnesses in
  [`apps/openagents-desktop/windows/`](../../apps/openagents-desktop/windows).

Planned paths: none. The Win32 + Vulkan desktop host lives in
[`apps/openagents-desktop/windows/`](../../apps/openagents-desktop/windows)
and [`apps/shared/host/windows/`](../../apps/shared/host/windows), and future
Metaverse Engine Windows host conformance extends
[`metaverse-engine/host/`](../../metaverse-engine/host).

## Sources

- **Episodes:**
  - [Episode 289, *OpenAgents*](../../docs/transcripts/289.md): 4:00, Windows, Mac and Linux
  - [Episode 290, *Enter the Verse*](../../docs/transcripts/290.md): 5:00, gaming computers
  - [Episode 283, *Coder for Gamers?*](../../docs/transcripts/283.md): gamers as an audience
  - [Episode 280, *CoderOS*](../../docs/transcripts/280.md): an operating system of our own
- **Old code at `coder-v1.0.0`, crates:** `rust-native`, `rust-native-desktop`, `openagents-desktop`, `terminal-app`
- **Also:**
  - The audit's gap table, row P16, and its decision 5
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P16

## Open questions for the owner

1. Do we need a Windows machine in the build fleet, or do we build from
   another platform and test on a rented one?

## Source listing

Run the Chapter 77 example in [`openagents/examples/windows-host-seam.coax`](../examples/windows-host-seam.coax) (committed transcript [`openagents/examples/windows-host-seam.out`](../examples/windows-host-seam.out)):

<!-- source: openagents/examples/windows-host-seam.coax windows-host-seam -->
```jai
whs_run :: () -> bool {
  // 1. Shipped Desktop Windows seam: worker PATH assembly (Git for Windows + dedup) and robocopy argv.
  pbuf: [2 * DC_PATH] u8;
  wpath := dcwin_worker_path("C:/Users/alice", "C:/Program Files", "c:\\program files\\git\\cmd;C:\\Windows\\system32", pbuf[..]);
  fb: [DC_PATH] u8; tb: [DC_PATH] u8;
  argv: [DCWIN_ROBOCOPY_ARGS] string;
  dcwin_robocopy_argv("C:/repo/coax/bin", "C:/wt/a/coax/bin", fb[..], tb[..], argv[..]);
  rc_ok := dcwin_robocopy_ok(0) && dcwin_robocopy_ok(1) && dcwin_robocopy_ok(7);
  print("worker_path=%\n", wpath);
  print("robocopy bin=% src=% dst=% guard=% ok=%\n", argv[0], argv[1], argv[2], argv[6], rc_ok);

  // 2. Shared Metaverse Engine host conformance receipt check (metaverse-engine/host/conformance.coax).
  assets := [2][8] int.[[8] int.[7, 0, 4, 0, 0, 0, 4, 3541452541], [8] int.[7, 1, 0, 0, 0, 0, 0, 2166136261]];
  frames := [2][8] int.[[8] int.[1, 1134559232, 1127481344, 1073741824, 0, 1, 0, 0], [8] int.[1, 1134559232, 1127481344, 1073741824, 1015021568, 1, 0, 0]];
  events := [13][8] int.[
    [8] int.[2, -6, 15, 1, 0, 0, 0, 0], [8] int.[2, -6, 15, 0, 0, 0, 0, 0], [8] int.[2, 128578, 0, 1, 0, 0, 0, 0], [8] int.[5, 1, 0, 0, 0, 0, 0, 0],
    [8] int.[6, 1107296256, 0, 0, 0, 0, 0, 0], [8] int.[3, 1103626240, 1116274688, 0, 0, 17, 0, 0], [8] int.[3, 1103626240, 1116274688, 2, 0, 17, 0, 0],
    [8] int.[3, 1096810496, 1099956224, 0, 1, 19, 0, 0], [8] int.[3, 1096810496, 1099956224, 2, 1, 19, 0, 0],
    [8] int.[4, 901, 0, 0, 0, 0, 7, 2634860734], [8] int.[4, 901, 0, 0, 0, 0, 128, 794728389], [8] int.[4, 900, 0, 0, 0, 0, 4, 3469047761], [8] int.[8, 3, 0, 0, 0, 0, 0, 0]
  ];
  conf_ok := hc_core(1, 73, 0, 0, assets[..], frames[..], events[..]);

  // 3. Fail-closed refusals: robocopy error exit (>= 8 or -1), POSIX case mismatch,
  // dropped conformance frame, wrong host version, and 1-word tampered event probe.
  ref_rc := !dcwin_robocopy_ok(8) && !dcwin_robocopy_ok(-1);
  ref_posix_case := !dc_same_dir("/usr/bin", "/usr/BIN", false);
  ref_dropped := !hc_core(1, 73, 1, 0, assets[..], frames[..], events[..]);
  ref_ver := !hc_core(2, 73, 0, 0, assets[..], frames[..], events[..]);
  bad_ev := events;
  bad_ev[0][1] += 1;
  ref_tamper := !hc_core(1, 73, 0, 0, assets[..], frames[..], bad_ev[..]);
  print("conformance ok=% refuse=(rc=%,case=%,dropped=%,ver=%,tamper=%)\n",
    conf_ok, ref_rc, ref_posix_case, ref_dropped, ref_ver, ref_tamper);

  return wpath == "C:/Users/alice/.local/bin;C:/Program Files/Git/usr/bin;c:\\program files\\git\\cmd;C:\\Windows\\system32" &&
         argv[0] == "robocopy" && argv[1] == "C:\\repo\\coax\\bin" && argv[2] == "C:\\wt\\a\\coax\\bin" &&
         argv[6] == "/XJ" && rc_ok && conf_ok &&
         ref_rc && ref_posix_case && ref_dropped && ref_ver && ref_tamper;
}
```

Output (`coax/bin/coax run openagents/examples/windows-host-seam.coax`, identical in `--release`):

```text
worker_path=C:/Users/alice/.local/bin;C:/Program Files/Git/usr/bin;c:\program files\git\cmd;C:\Windows\system32
robocopy bin=robocopy src=C:\repo\coax\bin dst=C:\wt\a\coax\bin guard=/XJ ok=true
conformance ok=true refuse=(rc=true,case=true,dropped=true,ver=true,tamper=true)
ok=true
```
