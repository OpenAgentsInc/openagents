# Coder automatic updates

How an installed `coder` finds, downloads, verifies, and installs newer
versions of itself and the commands that ship with it (`openagents`,
`microcoder`, and `coder-boundary.exe` on Windows). It ships in 1.0.0, so
anyone on 1.0.0 gets every later version without running the installer
again.

The release contract is the one `scripts/release/coder.sh` already
publishes under
`https://storage.googleapis.com/openagentsgemini-cli-releases/coder`:

| Object | Meaning |
| --- | --- |
| `coder.stable`, `coder.rc` | The channel pointers: one version string each |
| `SHA256SUMS-coder-<version>` | The SHA-256 of every archive of that version |
| `coder-<version>-<platform>.tar.gz` | macOS and Linux: `coder`, `openagents`, `microcoder` |
| `coder-<version>-windows-x86_64.zip` | `coder.exe`, `openagents.exe`, `microcoder.exe`, `coder-boundary.exe` |

The updater reads only these, so the release script and the installers need
no change. A version is immutable once published, and a channel moves only
when every platform has its archive and checksum.

## Prior art

Ported as ideas, not code, from the reference clones under
`~/work/projects/repos/`.

| | Codex (`codex-rs/tui/src/updates.rs`) | Grok Build (`xai-grok-update`) | Coder (this design) |
| --- | --- | --- | --- |
| Where it checks | TUI startup, background task | TUI startup and agent runs, background task; leader process hourly | TUI startup, background thread; `coder update` on demand |
| How often | at most every 20 h | 30 min cache TTL | at most once a day (24 h) |
| Cache | `$CODEX_HOME/version.json` (`latest_version`, `last_checked_at`, `dismissed_version`) | `~/.grok/version.json` (`version`, `checked_at`), atomic tmp+rename | `~/.openagents/coder-new/update.json` (`checked_at`, `latest`, `staged`, `held`), atomic tmp+rename |
| Timeout | none set on the check | 15 s × 3 retries per base URL; 20 min download | 5 s pointer; 30 s sums; 10 min archive, 1 GiB cap |
| Offline | logged, cache unchanged, never blocks | silent "no update", never blocks | silent, cache unchanged, never blocks, retries next launch |
| Discovery | GitHub `releases/latest`; brew cask API; npm registry for npm installs | plain-text channel pointer (`x.ai/cli/<channel>`); npm view; gh release list | plain-text channel pointer `coder.<channel>` |
| Channels | latest only; prereleases never count as newer | stable, alpha, enterprise | stable, rc (rc follows stable too, because publishing stable moves rc) |
| Downgrade | never (strictly greater) | allowed for internal installs (pointer is authoritative) | never (strictly greater); a rolled-back version is held |
| TUI notice | full-screen "Update available" prompt (update / skip / skip version) and a history box | welcome tip "vX available, press ctrl+u to restart" | one line on the context row: "Coder 1.0.1 is ready. Restart Coder to use it." or "… is available. Run coder update." |
| Applies how | never self-replaces; runs `npm i -g`, `brew upgrade`, or the installer after the TUI exits | self-replaces (`~/.grok/bin` symlink swap) or `npm i -g`; WinGet hand-off | self-replaces a standalone install: renames all bundled commands in one step, keeps the previous set |
| When | user picks "Update now" | download in background, active on next launch | download and verify in background; install when Coder quits, or at next start if it was killed |
| Verification | none in the client (installer checks SHA256SUMS) | `--version` smoke test only, no checksum | SHA-256 against `SHA256SUMS-coder-<v>` before staging and again before install; `--version` of every command; on macOS, the same Developer ID team as the running binary and `codesign --verify --strict` |
| Install detection | env set by the npm shim; brew prefix; standalone path | `GROK_INSTALLER`, npm env, `node_modules`, WinGet path | app bundle (OpenAgents Desktop), Homebrew prefix, `node_modules`, source builds (`target/`, `~/.openagents/versions`), unwritable directory → notify only; else standalone |
| Opt-out | `check_for_update_on_startup = false` | `--no-auto-update`, `GROK_DISABLE_AUTOUPDATER`, `auto_update=false` | `CODER_UPDATE=off\|notify\|auto`, `coder update --mode off\|notify\|auto` |
| CI / debug | debug builds compiled out; `0.0.0` skips | debug builds never check | no checks when `CI` is set, on debug builds, or outside the interactive TUI |
| Rollback | none; reinstall a version | all-or-nothing symlink swap; keeps N-1 download | all-or-nothing rename with automatic restore; `coder update --rollback` swaps back and holds that version |
| Windows running exe | junction re-point, exe never touched | copy, else rename locked exe to `.old` and copy | rename the running exe into the backup folder (Windows allows renaming a running image), then rename the new one in |

## Design

### Settings

| Source | Values | Default |
| --- | --- | --- |
| `CODER_UPDATE` (env, wins) | `auto`, `notify`, `off` | — |
| `~/.openagents/coder-new/update-settings.json` `mode` (`coder update --mode`) | `auto`, `notify`, `off` | `auto` |
| `CODER_CHANNEL` (env, wins; the installer reads it too) | `stable`, `rc` | — |
| `update-settings.json` `channel` (`coder update --channel`) | `stable`, `rc` | `rc` when the running version is a release candidate, else `stable` |
| `CODER_BASE_URL` (env; the installer reads it too) | release prefix URL | the bucket's `/coder` prefix |

`auto` downloads and installs on its own; `notify` shows the line and leaves
installing to `coder update`; `off` never contacts the release bucket.

### When Coder checks

Only the interactive TUI checks, and never when `CI` is set, when the mode is
`off`, or in a debug build (unless `CODER_BASE_URL` points at a test
channel). `coder login`, `coder trace`, `--snapshot`, and `--version` never
touch the network for updates. The check runs on a background thread after
the TUI starts; startup never waits on it.

1. On start, the cached `latest` (if newer than this binary) shows the line
   at once, even offline.
2. If the last check is less than 24 hours old (and not in the future), stop.
3. Read `coder.<channel>` (5 s timeout). Anything that is not
   `X.Y.Z[-rc.N]` is ignored. On error, stop silently; the cache keeps its
   old `checked_at` so the next launch tries again.
4. If the version is not strictly newer than this binary, or is the held
   (rolled-back) version, record the check and stop. Coder never downgrades.
5. Otherwise record `latest`. In `notify` mode, or for an install Coder
   must not replace, show the line and stop.
6. In `auto` mode on a standalone install: download
   `SHA256SUMS-coder-<v>` and this platform's archive into
   `~/.openagents/coder-new/updates/`, check the archive's SHA-256 against
   the one entry the sums file names for it, and record it as `staged`. A
   mismatch deletes the download and stages nothing.

The platform is the running binary's own build target (`macos-aarch64`,
`macos-x86_64`, `linux-x86_64[-musl]`, `linux-aarch64[-musl]`,
`windows-x86_64`), so a musl build stays musl.

### Install kinds

| Kind | Detected by (the running binary's resolved path) | Behavior |
| --- | --- | --- |
| Standalone | anything else, with a writable directory | download, verify, install |
| OpenAgents Desktop | inside `*.app/Contents/` | line only: "Update OpenAgents Desktop to get it." |
| Package manager | under a Homebrew prefix (`/Cellar/`, `/homebrew/`, `/linuxbrew/`) or `node_modules` | line only: "Update it with your package manager." |
| Source build | under a Cargo `target/` or `~/.openagents/versions/` (`scripts/install-coder.sh`) | no checks |
| Unwritable | standalone layout, directory not writable | line with the installer one-liner |

### Installing

`coder update` installs at once; in `auto` mode a staged version installs
when Coder quits (after the terminal is restored), or at the next start if
the session was killed, before the TUI opens. A start-time install then
re-executes the new `coder` with the same arguments (on Windows the
current session continues and the new version runs next time).

1. Take `<bin>/.coder-update.lock` (exclusive create; a lock older than 10
   minutes is stale). Another Coder installing at the same time is skipped.
2. Hash the staged archive again and compare with the staged sums entry.
3. Unpack it into `<bin>/.coder-update.<pid>/` (same filesystem as the
   commands). Every command must be a regular top-level file; links,
   directories, and paths with separators are refused.
4. Run each new command's `--version`; it must print `<name> <version>`.
   `coder-boundary.exe` has no version flag and is only checked for presence.
5. On macOS, when the running `coder` is signed by a Developer ID team,
   every new binary must pass `codesign --verify --strict` and carry the
   same team identifier.
6. Rename each installed command into `<bin>/.coder-previous/` (replacing
   the previous backup set), then rename each new command into place. If
   any rename fails, everything already moved is moved back, and the old
   install is unchanged. Windows allows renaming an executable that is
   running, so this also works for a running `coder.exe` or
   `coder-boundary.exe`.
7. Clear `staged`, delete the download, and say "Updated Coder to 1.0.1."

Any failure leaves the installed commands as they were and reports one
sentence ("Checksum mismatch for coder-1.0.1-macos-aarch64.tar.gz. Coder
is unchanged.").

`coder update --rollback` swaps `<bin>/.coder-previous/` back in (the
version it replaces becomes the new backup, so a second rollback undoes the
first) and records that version as `held`, so `auto` does not reinstall it.
An explicit `coder update` clears the hold.

### Commands

```
coder update                 Install the newest version on your channel now.
coder update --check         Say whether a newer version is published.
coder update --rollback      Go back to the version the last update replaced.
coder update --mode MODE     auto (default), notify, or off.
coder update --channel NAME  stable (default) or rc.
```

### The desktop host

OpenAgents Desktop runs the `coder` inside its app bundle (or, without a
bundle, the first `coder` on `PATH`, then `~/.openagents/bin/coder`). A
bundled `coder` never replaces itself; the app's own update carries it. A
host service already running from `~/.openagents/bin` keeps running the
old code until it restarts, which is safe because the files are renamed,
never rewritten in place.

## Tests

`crates/coder-new/src/update.rs` tests: version order and parsing, channel
pointer parsing, sums parsing (duplicates and malformed lines refused),
the once-a-day rule (including a future timestamp), settings and env
precedence, CI and `off` skipping, never downgrading and the hold, install
kind detection, tar and zip unpacking (links and nested paths refused), a
checksum mismatch refusing to stage or install, the all-or-nothing swap with
a failure injected midway, rollback, and an end-to-end check, download,
stage, and install against a fake channel served from a local HTTP server.
