# OpenAgents for Windows

OpenAgents Desktop on Windows 10 and 11, x64 and Arm64 (GitHub #13782). It is the same
app as on Linux: the page (`page.coax`), the desktop core (`coax/lib/agent/desktop/`,
through `host-core.coax`) and the entry logic (`host-main.c`) are shared. Windows adds a
window host, a file chooser, and C effects for processes, files and sockets.

## Build

From a Mac (or Linux) with zig and the Vulkan headers:

```sh
brew install zig vulkan-headers
apps/openagents-desktop/windows/build.sh --zip ~/work/openagents-windows-dist
```

That writes `OpenAgents-windows-x64.zip` and `OpenAgents-windows-arm64.zip`. Each holds
one folder, `OpenAgents`, with:

| File | What |
| --- | --- |
| `OpenAgents.exe` | The app: a GUI program with the OpenAgents icon, version information and a manifest |
| `mcworker.exe` | The Microcoder worker the app starts for each reply |
| `claudeworker.exe` | The Claude Code worker for Claude sub-agents |
| `agyworker.exe` | The Antigravity ACP worker (`agy.exe` / `.cmd` launchers) |
| `coder-host.exe` | The resident daemon (`AF_UNIX` local socket on Windows 10 1803+ / 11) |
| `resources/` | The shaders (`components.spv`, `composition.spv`), fonts and icons |
| `NOTICE` | The fonts' and icons' licenses |

No DLLs ship beside them. The C runtime is the Universal C Runtime that Windows 10 and 11
include, and Vulkan comes from the graphics driver (below).

| Command | What it does |
| --- | --- |
| `windows/build.sh` | Builds both arches into `windows/build/x64` and `windows/build/arm64` |
| `windows/build.sh --arch x64` | One arch |
| `windows/build.sh --check` | Builds both and keeps nothing; any compile or link error fails it. `coax/remote/check.sh windows-build` runs it |
| `windows/wine-smoke.sh` | On a Linux build host: the headless smoke under wine (below) |

## Run

Unzip, open the `OpenAgents` folder and double-click `OpenAgents.exe`. The program is not
signed yet, so SmartScreen says "Windows protected your PC": choose **More info**, then
**Run anyway**.

The app keeps its files where the Mac and Linux apps keep theirs, under the user's folders:

| What | Where |
| --- | --- |
| Chats | `%APPDATA%\Coder\chats` |
| Settings and keys (`openrouter-key`, `claude-key`) | `%APPDATA%\Coder` |
| The log | `%LOCALAPPDATA%\OpenAgents\logs\openagents.log` (a new one each launch; the last is `openagents.log.old`) |
| Worker journals | `%USERPROFILE%\.openagents\coder\sessions` |

A reply needs a model key. Set the OpenRouter key in settings (**Models → OpenRouter key**), or put
`OPENROUTER_API_KEY=...` in `%USERPROFILE%\work\.secrets\openrouter.env` as on a Mac.
Microcoder runs commands with `sh` and the POSIX tools from
[Git for Windows](https://git-scm.com/download/win): the app puts Git's `cmd` and
`usr\bin` folders on the worker's `PATH`. Requests go out through `curl.exe`, which
Windows 10 (1803 and later) and 11 include.

## Why Vulkan and not Direct3D 12

The window draws with Vulkan through `vulkan-1.dll` and `VK_KHR_win32_surface`, sharing the
Linux renderer (`apps/shared/host/vulkan-ui.c`, `metaverse/gpu/vkc.c`, `vks.c`, `vkp.c`) and
its SPIR-V shader. A Direct3D 12 host would need a second renderer and the shader in HLSL,
and every drawing change would land twice. NVIDIA, AMD and Intel drivers for Windows 10
and 11 install `vulkan-1.dll`, and so do the Qualcomm drivers on Arm64 laptops
(Snapdragon X). The app loads it when it starts (`apps/shared/host/windows/vk-load.c`): on a
computer without it, or with a driver too old, the app says so in a message box in place of
the system's "vulkan-1.dll was not found". If Arm64 devices without Vulkan turn out to
matter, Direct3D 12 is the fallback to add, behind the same `ui-host.h`.

## What works and what does not yet

Every control does something. One that needs a part of the Mac app that is not here says
so in a short note.

| Works | Not on Windows yet |
| --- | --- |
| The window: keyboard, mouse, wheel, typing through the input method, the clipboard (text and files copied in Explorer), files dropped on the window, per-monitor DPI with live glyph atlas reload (`WM_DPICHANGED`) | A drop shows no "Drop to attach" while files are carried over the window (that needs an OLE drop target) |
| Chats: new, open, rename, delete, pinned-chat drag reorder, kept across restarts | The Gym (3D workbench) |
| Microcoder (`mcworker.exe`), Claude (`claudeworker.exe`), Antigravity (`agyworker.exe`), steering and the follow-up queue, sub-agents, Foreman chat & tier | Remote Nostr relay TLS (`wss://`) in `coder-host.exe` (Windows Schannel not wired yet; local `AF_UNIX` daemon works) |
| Resident daemon (`coder-host.exe` over Winsock `AF_UNIX`) | Authenticode code signing and MSIX/NSIS installer (`NEEDS_OWNER.md`) |
| Attachments through the system's file chooser, the working folder chooser, PNG/JPEG image previews (`coax/lib/image/image.coax` + `composition.spv`) | WebP/HEIC image decoding |
| The file pane: open, edit, save, open in the default app | |
| The chat budget, the thread goal, the OpenRouter and Claude keys | |
| Each chat in its own worktree, when the folder is a Git repository | |
| Ed25519 + SHA-256 verified update feed check, download and atomic swap (`curl.exe`) | Signed release feed publishing & Authenticode verification on downloaded binaries |

Signed update manifests (`openagents.desktop.update.v1`) are checked by the core
(`oa_core_update_verify`, Ed25519): `OPENAGENTS_UPDATE_MANIFEST=PATH` makes **Check for
updates** read one, and `OpenAgents.exe --check-update PATH` checks one from a terminal.

## TODO: signing and an installer

Not done, and not faked:

- **Authenticode signing.** `OpenAgents.exe` and `mcworker.exe` are unsigned, so SmartScreen
  warns on first run. Signing needs a code-signing certificate held by the company
  (`NEEDS_OWNER.md`), then `signtool sign /fd sha256 /tr <timestamp URL>` or
  `osslsigncode` from the Mac, in `build.sh` before the zip.
- **An installer.** The zip is unpacked by hand. An MSIX or Inno Setup installer would add a
  Start menu entry, file association and uninstall; it should be signed with the same
  certificate.

## Checks

| Check | What it covers |
| --- | --- |
| `apps/openagents-desktop/check.sh` | Builds and runs `windows/args-test.c`: the Windows command-line quoting, path and environment rules (`coax/platform/win32-args.h`) on the Mac |
| `coax/lib/agent/desktop/*.coax` tests | The core's Windows paths, worker `PATH` and environment, and the robocopy arguments (suites) |
| `coax/remote/check.sh windows-build` | Both arches compile and link |
| `windows/wine-smoke.sh` | Under wine with Mesa's software Vulkan: the page drawn offscreen, a chat kept and read back, an untrusted update refused, and `mcworker.exe` started until it says it is ready |
| `windows/wine-window.sh` | The real window under wine on a virtual screen: typing, sending (the reply goes through `mcworker.exe`), a note from a control that is not here, and settings, with a picture of each |

Wine is not Windows. The input method, the file chooser, a real driver's Vulkan,
Git for Windows and a full reply are checked on a Windows computer (`NEEDS_OWNER.md`).

## Files

| File | What |
| --- | --- |
| `main.c` | The Windows side of `../host-main.c`: paths, the log, message boxes, UTF-8 arguments and `wWinMain` |
| `build.sh` | Coax to C and SPIR-V, the icon and resources, then `zig cc` for each arch, and the zips |
| `app.manifest` | UTF-8 code page, per-monitor DPI, Windows 10 and 11 |
| `args-test.c` | The command-line checks |
| `wine-smoke.sh`, `wine-window.sh` | The smoke and the window under wine |
| `../../shared/host/windows/ui-host.c` | The Win32 window host on the shared Vulkan renderer |
| `../../shared/host/windows/portal.c` | The file and folder chooser (`IFileOpenDialog`) |
| `../../shared/host/windows/vk-load.c` | Vulkan loaded from `vulkan-1.dll` when the app starts |
| `../../../coax/platform/*-win32.c` | Processes, files, sockets and the tool entry for Windows |
| `../../../coax/lib/agent/desktop/os-windows.coax` | The core's Windows rules |
