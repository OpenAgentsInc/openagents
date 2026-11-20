# Desktop core

The portable core of the OpenAgents desktop apps (GitHub #13787, master #13786). It holds the
logic that `apps/coder-desktop` keeps in Swift today, written once in Coax, so the macOS, Linux and
Windows hosts call the same code. The on-disk formats and paths match the Swift app's, so the
Swift app and the core can each read what the other writes.

| Module | Replaces | What it does |
| --- | --- | --- |
| `paths.coax` | path rules in `Worktree.swift`, `Store.swift`, `DaemonClient.swift` | Pure rules: worktree root, repository slug, chat folder, branch `coder/CHAT`, the Coder folder on each system, the daemon folder. No system calls. |
| `worktree.coax` | `Worktree.swift` (`ChatWorktree`) | A per-chat git worktree under `~/.openagents/worktrees/SLUG/CHAT` (or `$CODER_WORKTREES`), branched from `origin/main` (else `HEAD`). It clones in the folders that `.coder/worktree.json` lists under `"clone"` with copy-on-write, and cuts symbolic links that point back into the main checkout. It removes the worktree only when it is clean. |
| `store.coax` | `Store.swift` (`CoderStore`, the key files, `ChatWorktree.enabled`) | The chat store: `chats/index.json` (Swift `CoderChat`, including the worktree fields) and `ID.json`. It also handles keep, register, rename, pin and remove, the `0600` key files (never the Keychain) and the worktree setting. The index codec is `coder/tui/desktop-store.coax`. |
| `daemon.coax` | `DaemonClient.swift` (`CoderDaemonClient`) | One `openagents.environment.daemon.v1alpha1` request per connection on the owner-only socket `HOST/service.sock` (or the socket that `daemon.json` names). It also starts `coder-host serve` when no daemon answers. The codec is `coax/lib/agent/environment/daemon-wire.coax`. |
| `worker.coax` | `Worker.swift` (`CoderWorker`) | Starts the Microcoder worker and carries one JSON line each way. When the daemon owns runs, the worker runs as a daemon run through `DAEMON/runs/ID.{spec,in,out,state,attached}`, the files that `coder/tui/host-runs.coax` keeps. Otherwise it runs as a child process with pipes. The core builds the worker's PATH and environment the way the app does. |
| `update.coax` | `Updater.swift` (`UpdateVerifier`, `UpdateInstaller.verifyArchive`) | `openagents.desktop.update.v1`: checks the envelope's Ed25519 signature over the payload before it reads the manifest. Then it checks the schema, the pinned bundle and team ids, that the SemVer is newer, an https URL, the size and the SHA-256, and finally the downloaded archive's exact size and SHA-256. |
| `os.coax` | | The platform seam (below). |
| `os-posix.coax` | | The seam on macOS and Linux. |
| `smoke.coax` | | The headless smoke. |

The core does not cover the Mac-only installer steps: unpacking with `ditto`, checking the code
signature with `codesign`, swapping the bundle and relaunching. Each host keeps those, because
they differ on every system.

## The platform seam

Nothing in this folder calls a `#foreign` except the seam file. `os.coax` loads the seam file for
the current build. macOS and Linux share `os-posix.coax`, which checks `dcos_os` at run time for
the few places the two systems differ. A Windows port adds `os-windows.coax` with the same
procedures and loads it from `os.coax` in place of `os-posix.coax`. No other file changes.

| Procedure | Must do |
| --- | --- |
| `dcos_os () -> int` | Return `DC_OS_MAC`, `DC_OS_LINUX` or `DC_OS_WINDOWS`. |
| `dcos_getenv (name, out) -> string` | Return an environment variable, or `""` when it is unset. |
| `dcos_home (out) -> string` | Return the user's home folder, with no trailing slash. |
| `dcos_now_ms`, `dcos_now_us` | Return wall-clock time since 1970, in milliseconds or microseconds. |
| `dcos_random (out) -> bool` | Fill `out` with system entropy (used for run and command ids). |
| `dcos_real (path, out) -> string` | Resolve symbolic links. When the path does not exist, return its standardized form. |
| `dcos_kind`, `dcos_read`, `dcos_read_at`, `dcos_size` | Report a file's kind (`FS_NONE`, `FS_FILE`, `FS_DIR`, `FS_OTHER`), read its bytes, read from an offset, or return its size (`-1` when it is missing). |
| `dcos_write (path, data) -> bool` | Replace the file atomically: write a temporary file, then rename it. |
| `dcos_write_private (path, data) -> bool` | The same, with the file readable by this user only (keys and run specs). |
| `dcos_append (path, data) -> bool` | Append to the end of the file (a run's input). |
| `dcos_mkdirs`, `dcos_mkdir_private`, `dcos_remove`, `dcos_list` | Make a folder and its parents, make one folder readable by this user only, remove a file or a whole folder, or return a folder's names in sorted order. |
| `dcos_run (argv, env, out) -> DcRun` | Run a program to its end, with empty stdin and stderr joined to stdout. Kill it after `DC_RUN_MS`. |
| `dcos_clone_tree (from, to) -> bool` | Copy a folder, copy-on-write where the file system supports it (APFS `cp -c`, `cp --reflink=auto`). |
| `dcos_cut_links (dir, source) -> int` | Remove the symbolic links under `dir` that resolve into `source`. |
| `dcos_spawn (program, cwd, env, unset) -> int` | Start a worker with pipes to its stdin and stdout and with stderr dropped. Return a handle. |
| `dcos_send`, `dcos_line`, `dcos_close`, `dcos_alive` | Write to the worker, read its next line within a timeout, close its input (then end it), or check whether it is still running. |
| `dcos_spawn_detached (program, args, log) -> int` | Start a program that outlives this one, in its own session, with output appended to `log`. Return its pid. |
| `dcos_unix_call (path, line, out, timeout_ms) -> int` | Send one request line over a local socket and read one reply line. Refuse a peer that is not this user. |
| `dcos_setting`, `dcos_set_setting` | Read or write an app setting. On macOS this is the app's defaults domain, `com.openagents.coder` (`CODER_SETTINGS_DOMAIN` overrides it). Elsewhere it is the Coder folder's `settings.json` (`CODER_SETTINGS_FILE` overrides it). |

## Where things live

| What | macOS | Linux | Windows |
| --- | --- | --- | --- |
| Coder folder | `~/Library/Application Support/Coder` | `$XDG_DATA_HOME/Coder` or `~/.local/share/Coder` | `%APPDATA%\Coder` |
| Chats | `CODER/chats` (`CODER_DESKTOP_DIR`) | same | same |
| Keys | `CODER/claude-key`, `CODER/openrouter-key`, mode `0600` | same | same |
| Worktrees | `~/.openagents/worktrees` (`CODER_WORKTREES`) | same | same |
| Daemon | `$CODER_HOME` or `~`, then `.openagents/coder/host` | same | same |

## Checks

The `#test` blocks in each module are suites, so `coax/web/tool.sh suites` runs them. To run one
module's checks:

```sh
coax/bin/coax test coax/lib/agent/desktop/update.coax --library
```

The smoke runs every module against the real system in a temporary folder. It runs a worktree
round trip, reads an `index.json` that the Swift app wrote, round-trips settings, talks to a
stand-in daemon over its socket, exchanges lines with a worker in both directions, and verifies the
signed update manifest in `proto/coder/desktop-update-v1.json`. The manifest is signed with a test
key that no release trusts. Run the smoke from the repository root:

```sh
coax/web/tool.sh coax/lib/agent/desktop/smoke.coax
coax/remote/check.sh desktop-core            # the same, as a pipeline step
```

`coax/remote/check.sh coder` and `coder-since` run the smoke when a change reaches the core.
