# Claude Code dev boxes on boat

Stage 0 of [#13474](https://github.com/OpenAgentsInc/openagents/issues/13474).
The plan and the reasons are in
[the audit](../audits/2026-11-13-boat-claude-devboxes.md). This page is the
runbook: how you start a box, work on it, stop it and make a new one. Every
number here was measured on 2026-11-13.

Hosted boat is opted in for dev boxes only: our own agents working on this
public repository. Nothing that serves users runs there.

## What exists

| Thing | Name | State |
|---|---|---|
| Box | `oa-devbox-1` (`bx_fwh3xnws`), `large`: 8 vCPU, 16 GB, 147 GB disk | Stopped. Its disk is kept for at least 14 days |
| Template (saved snapshot) | `oa-devbox-20261113` | Ready. Holds no token |
| Environment | `oa-devbox` | Holds the two tokens. Not the default environment |

The default environment `base` stays "safe for third parties", so a box made
without `--environment oa-devbox` gets no token at all.

## What is on a box

- Ubuntu 24.04, x86_64, passwordless `sudo`. Boat's image brings `claude`,
  `gh`, `git`, `tmux`, Bun 1.3.14, Node 24, Rust, GCC 13, Clang 18 and Chrome
  151.
- Our setup adds the packages `coax/remote/host-startup.sh` installs on a
  build host (build tools, clang, libsecp256k1, libpq, libcurl, PostgreSQL 16
  with no cluster, git-lfs), single-user Nix in `/nix`, and a blob-less clone
  of this repository in `~/oa` (hooks on, through `core.hooksPath
  coax/remote/hooks`, and the compiler in `~/oa/coax/bin/coax`). The file
  `~/.oa-devbox.sh` keeps the Coax caches across a stop (see below) and loads
  Nix.
- Git commits as `AtlantisPleb <chris@openagents.com>`, the identity the
  repository's agent commits use. Each commit still names its agent in an
  `Agent:` trailer.
- **Two secrets, and only these**, both as environment variables from the
  `oa-devbox` environment:
  - `CLAUDE_CODE_OAUTH_TOKEN`: Claude Code logs in with it, no browser.
  - `GH_TOKEN`: a fine-grained token for `OpenAgentsInc/openagents` only
    (Contents and Pull requests). `gh auth setup-git` makes `gh` the git
    credential helper, so `git push` reads the token from the environment.
    It is not in the remote URL or in any file.
- A scan of `/home`, `/etc`, `/opt`, `/root`, `/srv` and `/usr/local` finds
  neither token in any file. Boat passes them to each process, not to disk.
- **Never on a box:** a gcloud key or any GCP credential, a production key,
  anything from `~/work/.secrets`, the owner's SSH keys, a boat API key, and
  any repository other than this one. A box can't reach the build pool
  (`coax/remote/remote.sh`), so it builds and tests on its own 8 cores.

## Start working

Install the CLI once on your Mac and sign in with your key (the key goes in
on stdin, never on the command line):

```sh
curl -fsSL https://boat.dev/install | sh
boat login --key-stdin < key.txt
```

Then these three commands start a session:

```sh
boat resume bx_fwh3xnws --ttl 43200          # 1. start the box for 12 hours
boat ssh -t bx_fwh3xnws tmux new -A -s claude  # 2. attach to its tmux (or make it)
cd ~/oa && git fetch -q && git worktree add ~/wt/ISSUE origin/main && cd ~/wt/ISSUE && coax/build.sh && claude --model claude-opus-5-5   # 3. a session on an issue
```

In step 3, name the worktree after the issue, then give Claude Code the issue
the way you would on the Mac ("Work on #NNNN. Your agent ID is ..."). A fresh
worktree needs `coax/build.sh` once (it installs the compiler from the cache
in seconds); without it the pre-push hook skips `wire-check` and `coax test`.

- Detach with `Ctrl-b d`. The session keeps running. Attach again with step 2.
- `boat extend bx_fwh3xnws --hours 8` sets the time left before auto-stop. A
  Claude Code session in `tmux` does not push the auto-stop deadline; only a
  running `boat prompt` does. Auto-stop is 1 hour after a resume unless you
  pass `--ttl` or extend.
- The first `claude` run in a new folder asks you to trust it. With
  `--dangerously-skip-permissions` it also asks you to accept that mode once
  per box.

## Stop

```sh
boat stop bx_fwh3xnws
```

The stop takes about 4 seconds, and billing stops with it. Push your work
first: the disk survives, running programs do not (see below). A stopped
disk is kept for at least 14 days; the template keeps the set-up state with
no expiry.

## Make a new box from the template

```sh
boat new --from oa-devbox-20261113 --type large --environment oa-devbox --name oa-devbox-2
```

`--environment oa-devbox` is what gives the new box its two tokens. Without it
the box starts with none. Then `cd ~/oa && git pull -q` to bring the clone up
to date.

To refresh the template (a much newer clone, a new toolchain): on the box,
`cd ~/oa && git pull -q && coax/build.sh`, remove worktrees and anything you
don't want copied, check that no file holds a token, and save it under the
same name: `boat snapshot bx_fwh3xnws oa-devbox-20261113`. Never pass a token
with `boat new --env`: a saved snapshot copies those values to every box made
from it.

## Measured

| Step | Time |
|---|---|
| `boat new --type large` to `ready` | 2 s |
| A detached command right after `ready` | Worked at once (6 s after create). The October refusals did not happen |
| apt packages | 57 s |
| Nix (single user) | 134 s, at about 230 KB/s from the Nix CDN |
| Blob-less clone and checkout (2.06 GB of files) | 10 min 39 s, at about 0.5 to 0.75 MB/s from GitHub |
| `coax/build.sh` | 49 s: it installs the published Linux build that matches the tree (`installed the published build c5cf8a2d8b03`), so no bootstrap was needed |
| `COAX_MUTATION=0 coax/build.sh` on a tree with no published build (a bootstrap) | 309 s on 8 vCPU |
| First setup in all | About 15 minutes, most of it the clone |
| `coax/tests/run.sh --only strings lexer` | 165 s, most of it compiling one C++ unit on one core |
| `claude -p "say hi"` on Haiku 5.5 | 4 s |
| Pre-push hook, docs-only push to `main` (simulated, not pushed) | 0 s |
| Pre-push hook, a one-line `.coax` change (simulated, not pushed) | 3 s: tools-check, everglade-parity, layer-check, state-keep |
| `boat stop` | 4 s |
| Resume after more than 10 minutes stopped | 12 s to `ready`, 20 s to the first command's answer. `coax/build.sh` then took 21 s (before the cache fix below) and `claude -p` 3 s |
| Save the template (4.9 GB) | 23 s; saving it again under the same name, 10 s |
| New box from the template | 8 s to `ready`; the compiler ran at +2 s, the first `claude -p` answered at +17 s (the first read of each file downloads it), and a new worktree had its compiler from the cache at +41 s. 49 s in all from `boat new` to a worktree ready to work in |

**The network is the slow part.** Downloads from outside the provider's own
network ran at 0.2 to 1 MB/s (GitHub, the Nix CDN, Hetzner); inside it they
ran at 41 MB/s. A fresh clone costs ten minutes, which is why the template
holds one.

## What survives a stop

Measured with a stop of 11 minutes, so boat had removed the machine:

| Kept | Lost |
|---|---|
| `~/oa`: the clone, its worktrees, local branches and commits | The `tmux` server and the Claude Code process in it |
| `/nix`, the apt packages, `~/.gitconfig` (the credential helper and identity) | `/tmp` |
| `~/.claude`: `claude -c` in the worktree continued the conversation and knew the commit it had made | `~/.cache`: boat empties it, so the Coax caches went with it |
| `~/oa/coax/bin/coax` | The machine: new IP, new hostname, uptime 0 |
| The two tokens: boat passes them again on every start | |

**The cache fix.** The Coax tools keep their caches in `$HOME/.cache/coax-*`,
and the pre-push hook looks for a compiler there when a worktree has none.
The template keeps them in `~/.cache-kept` and links them back on every
login: `~/.oa-devbox.sh`, sourced from the top of `~/.profile` and
`~/.bashrc`. It also loads Nix. On a box made from the template,
`coax/build.sh` in a new worktree installed the compiler from that cache
with no download.

**Claude Code's session ends at a stop.** Before you stop a box, push your
work. After the resume, attach (`boat ssh -t ... tmux new -A -s claude`), go
to the worktree and run `claude -c` to continue the conversation.

## Headless Chrome

Not useful for our pages yet. Chrome 151 runs headless, and with
`--use-angle=swiftshader --enable-unsafe-webgpu` a page gets a WebGPU adapter
(`google`, `swiftshader`). But `https://openagents.com/` never draws its
scene there:

- With the flags `coax/web/tool.sh shot` uses for `CDP_WEBGPU=swiftshader`
  (ANGLE on SwiftShader), the page loses its GPU device about every 300 ms
  ("GPU device lost ... taking a new one") and never reports a scene.
- With Dawn on SwiftShader Vulkan, the page reports `ready` but no scene for
  60 s.
- `tool.sh shot` waits 180 s and takes no shot ("the page never reported its
  scene ready"). A plain screenshot is black.

So shots, goldens and frame times stay on a GPU machine. A page that is not
WebGPU ("plain" to `shot`) should still work; this run didn't need one.

## Cost and limits

- A `large` box costs $0.072 an hour while it runs, billed to the second. A
  stopped box costs nothing; its disk is kept at least 14 days.
- The $20 plan includes 555.56 `default`-hours a period (277 `large`-hours).
  The period ends 2026-11-29 at 8:42 PM CST.
- Saved snapshots: 10 included, then $0.056 a day each. After the 16 old
  ones were deleted on 2026-11-13, the template is the only one.
- Starts: 12 a minute, 60 an hour, 200 a day.
- No GPU on any size. Goldens, frame times and acceptance stay on a GPU
  machine.
- This stage-0 run used 0.96 `large` box-hours (6,895 billed
  `default`-seconds), about $0.07, all inside the plan. `boat limits` before:
  555.56 included hours left, wallet -$1.38, no box running. After: 553.64
  hours left, wallet -$1.38, no box running, 4 of 200 daily starts used.
