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

## Sync protocol merge checkpoint (#11847)

If an upstream merge changes `edge/`, `crates/sync`, `crates/proto`, or
`crates/rpc`, run the Rust server conformance suite before completing the merge.
Use a clean OpenAgents monorepo worktree and the candidate fork commit:

```sh
/path/to/clean-openagents-worktree/scripts/coder-sync/conformance.sh \
  --fork-repo /path/to/candidate-fork --pin FULL_FORK_COMMIT
```

The script updates the shared client pin and lockfile, builds that exact
headless engine, and runs chat, registry, device, blob, and image fixture flows
against temporary Postgres and a fake bucket. Commit the pin and lockfile in
OpenAgents after the suite passes; push the corresponding fork commit to our
`origin/main`. A local candidate commit works before publication. Never push
to `upstream`. Keep the previous pin if the suite fails, and turn each mismatch
into a regression test before retrying. Notifications remain disabled until
OpenAgents push-gateway delivery lands in #11853.

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
- iOS keeps the upstream project and target names; identity and account seams are listed below.

## Platform matrix

| Platform | Upstream packages | Coder Desktop status |
| --- | --- | --- |
| macOS (Apple silicon) | `.dmg` + app tarball (`scripts/package-macos.sh`, `release.yml` on `macos-latest`) | Dev bundle built and run on 2026-10-15 (macOS 26, Apple silicon): bundle identity, window title, data directory and no hosted requests checked. `package-macos.sh` updated, not yet run. |
| macOS (Intel) | Not in the release matrix | Not checked. |
| Windows x86_64 | Inno Setup installer + portable `.exe` (`scripts/package-windows.ps1`, `windows-2022`) | Identity edited (installer AppId, names, scheme, icon, data directory); not built or run. |
| Linux x86_64 / aarch64 | Tarball with `install.sh` (`scripts/package-linux.sh`, Ubuntu 24.04) | Data directory and icon changed; install layout still Zeron's (collides with a Zeron install). Not built or run. |
| iOS | TestFlight (`testflight.yml`) | OpenAgents Coder identity and shared account flow; simulator verified. Store upload is an owner step. |

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

## Shared OpenAgents sign-in (#11848)

- `coder-account` and `zeron-ui` reuse the public SDK device-code flow delivered
  in #11824. The SDK pin includes the explicit first-party transport bearer
  accessor. A shared controller rechecks active credentials every 15 seconds;
  an outage preserves the login, while revocation clears it.
- `coder-account::PlatformStore` uses the Security framework on both macOS and
  iOS. Entries are scoped to app, profile, API, and website. A signed-out marker
  prevents silent restore when the OS refuses credential deletion.
- `zeron-mobile::client_ffi::account` exposes account state and actions to Swift.
  Tokens stay in Rust and Keychain. Sync caches include account, sync server,
  and account-service identities. Logout closes issued clients and removes only
  that account's cache, preserving the restore fence.
- `AppModel.swift` and `SignInViewController.swift` mount this shared flow and
  the existing SDK-approved website. Legacy stored WorkOS credentials are never
  restored. Demo data has a separate directory. Debug-only fixture arguments
  use dedicated loopback accounts and suppress browser opening.
- `zeron-client` accepts OpenAgents sessions and redacts every credential in
  debug output. `zeron-client` and `zeron-engine` disable inherited WorkOS
  configuration and entry points by default. The explicit `upstream-workos`
  compatibility feature is reserved for upstream comparisons; OpenAgents
  builds must never enable it. The UI forwards it only for its two legacy
  WorkOS integration tests. The default engine regression proves an inherited
  WorkOS session is ignored and remains unchanged.
- `OpenAgentsAccountTests` exercises sign-in, synced sessions, and sign-out
  against `scripts/coder-sync/test.sh --ios-auth-fixture CONFIG_PATH` in the
  monorepo. Pass its addresses as `TEST_RUNNER_CODER_ACCOUNT_API`,
  `TEST_RUNNER_CODER_ACCOUNT_WEBSITE`, and `TEST_RUNNER_CODER_ACCOUNT_SYNC`.
  Build with `CODE_SIGNING_ALLOWED=NO`; run Keychain UI tests with local
  simulator signing (`CODE_SIGNING_ALLOWED=YES CODE_SIGN_IDENTITY=-`). Xcode
  supplies simulator entitlements without a developer-account login. The app
  writes only booleans to `Documents/account-fixture-result.json` after logout;
  require `signed_in`, `saw_session`, and `keychain_empty` to be true.

After upstream merges, retain the disabled WorkOS defaults and account/cache
boundaries, regenerate UniFFI bindings, and rerun these account tests before
advancing the server conformance pin.

## Desktop-to-phone sync (#11849)

- `coder-brand::HOSTED_EDGE_URL` defaults to `https://sync.openagents.com`.
  WorkOS and the upstream updater stay disabled. A local engine joins no rooms
  until the shared SDK controller supplies a verified account lease.
- `coder-account::desktop` shares one controller with the embedded engine per
  data directory. Normal desktop logins keep the same Keychain service as
  #11824. An explicit `CODER_DESKTOP_CREDENTIAL` is authenticated by the SDK and
  held in memory; the launcher removes it from the environment before threads
  and agent processes start. Its logout marker is separate from saved logins.
- `Auth` forwards SDK account changes and bearer authority to the existing sync
  transports. A synced runtime captures its account ID and refuses a replacement
  account's token. Slow verification does not block account-change events;
  local startup waits at most 250 ms for an in-progress restore.
- `EngineProfile::openagents` separates chats and uploads by account, account
  deployment, and sync URL. It never claims upstream's global uploads. Existing
  local-to-synced switching and the optional local-chat import remain in place.
- `AppAccount` and `zeron login/logout/status/headless` use that controller.
  Headless callers can supply a temporary credential through the environment;
  no second plaintext session file is written. The old CLI test that treated a
  WorkOS `session.json` as a login is replaced by SDK engine integration tests
  for sign-out, revocation, and account replacement.
- Mobile and desktop reuse the same cache-scope helper. The mobile credential
  import uses the existing SDK storage/cancellation/logout lifecycle.
- `account-fixture` in `zeron-mobile` adds a one-use loopback credential delivery
  helper for simulator checks. It keeps account tokens out of Swift and Xcode
  logs. Rust refuses this feature outside the simulator; the build script also
  refuses release builds and detects a mismatched staged core. Normal committed
  bindings exclude it. Swift enables its call only with
  `CODER_ACCOUNT_FIXTURE`. The debug app's `-account-fixture` uses separate
  account/cache, display-profile, device-ID, and draft preferences.
- The mock harness accepts the typed boolean `modelOptions.mockQuestions` for
  one run. The live fixture seeds only its first turn with questions; later
  turns stream until stopped, avoiding a race between a second question and
  queue controls.
- `mobile_sync_fixture` owns a fresh headless process, scratch home/data, and
  random IPC port. It checks listener ownership before IPC, refuses port 49777,
  and removes only its own registry entries. `OpenAgentsLiveSyncTests` exercises
  both deployed Rust services. The repeatable check and cleanup are in the
  monorepo's `docs/mobile/coder-desktop-phone-e2e.md`.

## iOS release preparation (#11850)

- The app and Live Activity extension share version 1.0.0, build 1000, recorded in
  `apps/ios/release/version.json`. The candidate must be checked against the
  existing App Store Connect record before upload.
- Removed the upstream signing team. `scripts/ios/archive.py` prepares a local
  Release archive, checks matching identities/versions and release contents,
  and stops before export/upload. Signing uses only an explicitly supplied
  owner's local team setup; it never downloads profiles or opens a login.
- The privacy manifest declares account-linked sync content and optional voice
  data for app functionality, without tracking. The inherited automatic export
  exemption answer is removed; the owner reviews the Rust TLS inventory and
  answers Apple's encryption questions.
- Push permission requests and the push entitlement remain absent. Test notes
  distinguish simulator-qualified sync from owner device checks and upcoming
  public chats/cloud computers. Release fields and exact owner buttons are in
  `apps/ios/release/README.md`. The app-specific privacy notice is published in
  `apps/ios/release/privacy.md` and supplements the general OpenAgents policy.

## Authorized TestFlight upload (#11850)

- The owner selected marketing version 1.0.0 for both app and extension. Build
  1000 was checked against the existing App Store Connect record before upload.
- `archive.py --distribution-certificate` selects an installed distribution
  certificate and per-target App Store profiles named for their bundle IDs.
  The team remains an explicit argument, never a committed project setting.
- Upload is a separate, explicitly authorized operation; archive preparation
  continues to stop before export or upload. Active worktrees, the owner’s
  desktop app, and its data are outside the release workflow.

## Account chats on the phone (#11851)

The Sessions list includes **OpenAgents chats**, using the desktop's public
SDK history and account-bound reader. Rust shares ordering, explicit paging,
full text parts, and on-demand file access; UIKit draws the controls. Local
Coder sessions stay separate. Sign-out clears both views. Run
`scripts/ios/check-history.py` for the isolated native paging, file, archive,
project, and sign-out check; `--public-history` accepts only disposable chat
IDs supplied by the monorepo's `scripts/smoke/sdk-history.py`.
