# Coder Desktop fork notes

Coder Desktop (the app is called **Coder**) is a fork of
[Zeron](https://github.com/zeronsh/zeron) by Wing, MIT. This file is the
working guide for keeping the fork current: the upstream pin, how to merge,
every identity seam we changed, and the platform matrix. Integration patches
(themes, SDK adapters) are logged in
[docs/downstream-patches.md](docs/downstream-patches.md). Plans and issues
live in [OpenAgentsInc/openagents](https://github.com/OpenAgentsInc/openagents)
under `docs/coder-desktop/`; all issues are filed there.

## Upstream

| | |
| --- | --- |
| Upstream remote | `upstream` = `https://github.com/zeronsh/zeron.git` |
| Our remote | `origin` = `https://github.com/OpenAgentsInc/coder-desktop.git`, branch `main` |
| Last merged upstream | `3b2f78027a2c31622c39c35716da4d6e1af8de15` (v0.2.113 + 2, 2026-10-15) |
| License | MIT. `LICENSE` keeps Zeron's copyright (Wing); `THIRD_PARTY_NOTICES.md` stays as upstream ships it. |

Update the "Last merged upstream" row after each merge.

## Merging upstream

```sh
git fetch upstream
git merge upstream/main
# resolve conflicts, then:
RUSTUP_TOOLCHAIN=1.98.1 cargo build -p zeron
RUSTUP_TOOLCHAIN=1.98.1 cargo test -p coder-brand -p zeron -p zeron-update
RUSTUP_TOOLCHAIN=1.98.1 scripts/run-macos-dev.sh
```

After a merge, search for new upstream identity strings so nothing slips back
in:

```sh
rg -n 'zeron\.sh|sh\.zeron|27654|"zeron"|\.zeron\b' -g '!target' -g '!docs' -g '!edge'
rg -n '"[^"]*\bZeron\b[^"]*"' -g '*.rs' crates/ui/src
```

Rules that keep merges easy:

- Keep internal names: crates stay `zeron-*`, the binary stays `zeron`,
  modules and code structure stay upstream's.
- Read product identity from `crates/brand` (`coder-brand`). Add a constant
  there rather than repeating a value at a call site.
- The `ZERON_*` environment variables keep their names. They are developer
  overrides; only their defaults are ours.
- Don't run `cargo fmt --all` over the tree: upstream's code isn't formatted
  with the current stable rustfmt, and reformatting it would turn every merge
  into a conflict. Format the files you change.

## Identity and branding seams (patch log)

The source of truth is [`crates/brand/src/lib.rs`](crates/brand/src/lib.rs).

| Seam | Zeron | Coder Desktop | Where |
| --- | --- | --- | --- |
| App name (bundle, menus, dock, window title, notifications) | Zeron | **Coder** | `coder_brand::NAME`; `dist/macos/Info.plist`; `crates/ui/src/app_menus.rs`; `crates/ui/src/lib.rs` (window title, now set on every platform; macOS hides it in the titlebar); `crates/ui/src/notify.rs` |
| Formal name and attribution | — | **Coder Desktop**; "Based on Zeron by Wing, MIT" | `coder_brand::LONG_NAME`, `ATTRIBUTION`; About panel credits (`app_menus.rs`); `NSHumanReadableCopyright`; `README.md` |
| macOS bundle ID | `sh.zeron.app` | `com.openagents.coder.desktop` | `coder_brand::BUNDLE_ID`; `Info.plist`; notification identity (`notify.rs`) |
| Dev bundle ID | `sh.zeron.app.dev` | `com.openagents.coder.desktop.dev` | `coder_brand::DEV_BUNDLE_ID`; `Info-dev.plist`; `scripts/run-macos-dev.sh` |
| Bundle names | `Zeron.app`, `Zeron Dev.app` | `Coder.app`, `Coder Dev.app` | `scripts/package-macos.sh`, `scripts/run-macos-dev.sh` |
| Data directory | `~/.zeron`, `%LOCALAPPDATA%\Zeron` | `~/Library/Application Support/Coder Desktop` (macOS), `$XDG_DATA_HOME/coder-desktop` (Linux), `%LOCALAPPDATA%\Coder Desktop` (Windows) | `apps/zeron/src/paths.rs`; `main.rs` exports the resolved directory as `ZERON_DATA_DIR` before anything starts, so engine subsystems and child processes use it too |
| Dev data directory | `target/macos-dev/data` | unchanged, `target/macos-dev/data` (keeps existing dev sessions) | `scripts/run-macos-dev.sh` |
| Worktrees, scratch folders, managed adapters | `~/.zeron/{worktrees,scratch,adapters}` | `~/.coder-desktop/{worktrees,scratch}`; adapters under the data directory | `coder_brand::HOME_WORK_DIR`; `crates/engine/src/repos.rs`; `crates/proto/src/projectless.rs`; `crates/harness/src/adapter_install.rs` |
| Single-instance lock | `{data}/engine.lock` | same file, in Coder's data directory | `crates/engine/src/instance_lock.rs` (unchanged) |
| Engine IPC port | 27654 (dev script 49777) | **27854** (dev script unchanged, 49777) | `coder_brand::DEFAULT_IPC_PORT`; `apps/zeron/src/main.rs`; `crates/mcp/src/lib.rs` |
| URL scheme | `zeron://` | `coder-desktop://` | `coder_brand::URL_SCHEME`; `Info.plist`; `crates/ui/src/lib.rs`, `links.rs`; `crates/client/src/auth.rs`; Windows installer registry |
| Background service | launchd `sh.zeron.app`, systemd `zeron.service` | `com.openagents.coder.desktop`, `coder-desktop.service` | `coder_brand::LAUNCHD_LABEL`, `SYSTEMD_UNIT`; `apps/zeron/src/daemon.rs`; `crates/update/src/lib.rs` |
| Keychain | Provider entries only | OpenAgents app sessions use `<bundle ID>.account`, scoped by profile and deployment; provider entries stay separate. | `crates/coder-account/src/platform.rs`; `crates/ui/src/openagents_account.rs` |
| Icon | Zeron | OpenAgents app icon (`dist/coder/icon-1024.png`, from `openagents/bins/openagents-ios/.../icon-1024.png`) | `dist/coder/coder.icns`, `coder.ico`, `coder.png`; packaging scripts; `dist/windows/zeron.rc` |
| Windows installer | AppId `AD5DEC34-…`, `Programs\Zeron` | new AppId `0FBFAAA3-AC9B-4542-8219-5C1638A1FB18`, `Programs\Coder` | `dist/windows/zeron.iss`; `crates/update/src/windows.rs` |
| User-visible strings | "Zeron …" | "Coder …" | menus, update dialog, permission prompts, notices, slash-command descriptions, link actions, settings text, agent-login pages |

Regenerate the icons after changing `dist/coder/icon-1024.png`:

```sh
T=$(mktemp -d); mkdir "$T/coder.iconset"
for s in 16 32 128 256 512; do
  sips -z $s $s dist/coder/icon-1024.png --out "$T/coder.iconset/icon_${s}x${s}.png"
  sips -z $((s*2)) $((s*2)) dist/coder/icon-1024.png --out "$T/coder.iconset/icon_${s}x${s}@2x.png"
done
iconutil -c icns "$T/coder.iconset" -o dist/coder/coder.icns
python3 -c 'from PIL import Image; im=Image.open("dist/coder/icon-1024.png").convert("RGBA"); im.save("dist/coder/coder.ico", sizes=[(n,n) for n in (16,24,32,48,64,128,256)]); im.resize((512,512), Image.LANCZOS).convert("RGB").save("dist/coder/coder.png")'
```

## Hosted services turned off

A plain start of Coder makes no request to Zeron's services.

| Service | Upstream default | Coder Desktop | Switch |
| --- | --- | --- | --- |
| Update feed | `https://edge.zeron.sh/releases` | Off. No background checks; "Check for Updates…" says Coder doesn't update itself yet and points at openagents.com/download. Set our feed in one place when it exists. | `coder_brand::UPDATE_FEED`; `crates/update/src/lib.rs` (`feed_configured`, `release_base`); dev override `ZERON_RELEASES_URL` |
| Sign-in (WorkOS AuthKit via the edge) | Production client ID baked in | Off. Local workspaces remain independent. OpenAgents account sign-in uses the public SDK and the app’s own Keychain entry ([#11829](https://github.com/OpenAgentsInc/openagents/issues/11829)). | `coder_brand::WORKOS_CLIENT_ID` |
| Sync, relays, preview signaling (edge) | `https://edge.zeron.sh` | Off. The default edge URL is `https://edge.invalid`, which never resolves. | `coder_brand::HOSTED_EDGE_URL`, `DISABLED_EDGE_URL`; dev override `ZERON_EDGE_URL` |
| Release and download links | GitHub `zeronsh/zeron` releases | `https://openagents.com/download` | `coder_brand::DOWNLOAD_URL` via `zeron_update::RELEASES_PAGE` |
| Telemetry, analytics, crash reporting | none in the app | none | — |
| Website, docs, Discord, sponsor links | README only (none in the app) | README points at openagents.com | — |

Not changed, and not called by the app: `edge/` (Zeron's Cloudflare Worker
and curl installer), `apps/landing`, `apps/www-redirect`, and the GitHub
workflows that deploy or release to Zeron's infrastructure (`deploy.yml`,
`release.yml`, `testflight.yml`). Our repository has none of their secrets.

## Still says Zeron

Known places a user can still see "Zeron" or "zeron":

- The binary and CLI are `zeron` (`zeron mcp`, `zeron daemon`, CLI help and
  messages). The MCP server agents see is named `zeron`, and its tool
  descriptions and agent prompts mention Zeron.
- Worktree branches are named `zeron/<name>` (`crates/engine/src/repos.rs`,
  `titles.rs`; many tests pin the prefix).
- The theme family named "Zeron", the project file `zeron.json`, the
  `TERM_PROGRAM=Zeron` terminal variable, and the Codex/ACP client title.
- Sign-in and sync screens (unreachable while sign-in is off).
- Linux and Windows packaging file names (`zeron.desktop`, `zeron.exe`,
  `zeron-update.json`), and Linux's install layout (`~/.zeron/app`,
  `~/.local/bin/zeron`).
- The iOS app under `apps/ios` is unchanged and out of scope.

## Platform matrix

| Platform | Upstream packages | Coder Desktop status |
| --- | --- | --- |
| macOS (Apple silicon) | `.dmg` + app tarball (`scripts/package-macos.sh`, `release.yml` on `macos-latest`) | Dev bundle built and run on 2026-10-15 (macOS 26, Apple silicon): bundle identity, window title, data directory and no hosted requests checked. `package-macos.sh` updated, not yet run. |
| macOS (Intel) | Not in the release matrix | Not checked. |
| Windows x86_64 | Inno Setup installer + portable `.exe` (`scripts/package-windows.ps1`, `windows-2022`) | Identity edited (installer AppId, names, scheme, icon, data directory); not built or run. |
| Linux x86_64 / aarch64 | Tarball with `install.sh` (`scripts/package-linux.sh`, Ubuntu 24.04) | Data directory and icon changed; install layout still Zeron's (collides with a Zeron install). Not built or run. |
| iOS | TestFlight (`testflight.yml`) | Out of scope. |

## iOS identity (#11841)

- `apps/ios/Zeron.xcodeproj` and `Zeron/Info.plist`: OpenAgents Coder,
  `com.openagents.coder`, isolated extension/test bundle IDs, and
  `openagents-coder` URL scheme. Upstream has no App Group entitlement.
- `Zeron/App/AppModel.swift`: isolated keychain services; stored upstream
  credentials do not start a session. `-demo` and debug `-dev` remain available.
- `Zeron/Shell/SignInViewController.swift`: signed-out screen with an offline
  demo; no WorkOS sign-in control. OpenAgents sign-in follows in #11848.
- `crates/client/src/auth.rs`: production server is `https://sync.openagents.com`.
  `crates/mobile/src/client_ffi/mod.rs` exports that setting to Swift.
- `Zeron/Assets.xcassets/AppIcon.appiconset`: OpenAgents icon from the public
  monorepo's `bins/openagents-ios/host/App/Assets.xcassets/AppIcon.appiconset`.
- `Zeron/Shell/MoreViewController.swift` and `Zeron/Zeron-MIT.txt`: About
  attribution and the complete upstream MIT notice.
- `Zeron/App/PushNotifications.swift`, Settings, and `Zeron.entitlements`:
  no permission prompt, token registration, or push entitlement in the first
  release. Push returns through OpenAgents in #11853.
- `ZeronLiveActivity/VoiceLiveActivity.swift` and `SceneDelegate.swift`:
  isolated Live Activity deep link. Dispatch and signpost names are isolated.
- `crates/mobile/src/layout/fixture.md`: demo links use OpenAgents.
- `scripts/ios/build-core.sh`: honor an external Cargo target directory and
  preserve the selected Rust toolchain (default 1.98.1) across Xcode's clean
  build environment.

After upstream merges, review these seams, run the iOS identity tests, and
scan the built app with `python3 scripts/ios/verify-identity.py PATH_TO_APP`.

Verification scope: upstream has existing workspace formatting differences;
format only the Rust files changed here rather than rewriting the workspace.
