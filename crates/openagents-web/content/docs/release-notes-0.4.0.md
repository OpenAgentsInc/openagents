# Coder 0.4.0 release notes

0.4.0 is one version for the service, the web bundle, the desktop app, the
phone app, and the terminal. These notes describe what changed since
terminal 0.3.5, the last stable release.

[`CHANGELOG.md`](changelog.md) lists every change with its issue or commit.
[`coder-terminal-0.4.0.md`](../releases/coder-terminal-0.4.0.md) records what each
channel points at.

Several parts of this release are opt-in, and the Gym now requires a switch.
See [what is not here yet](#what-is-not-here-yet).

## Install or update

One command installs or updates the terminal:

```sh
curl -fsSL https://openagents.com/releases/install-terminal.sh | sh
```

The installer detects your operating system, your architecture, and
whether the machine uses glibc or musl. It verifies the download against the
release's checksum file, and installs the command `coder` into
`~/.openagents/bin`. It prints an `export PATH` line when that directory is
not on your `PATH`.

`CODER_TERMINAL_CHANNEL` picks a channel. The bare command follows `stable`.
`rc` follows the newest release candidate:

```sh
curl -fsSL https://openagents.com/releases/install-terminal.sh | CODER_TERMINAL_CHANNEL=rc sh
```

A channel now moves only to a version every supported platform can install.
Before this release, a candidate built on one machine could move the `rc`
pointer after publishing a single macOS artifact, and every Linux install
that followed the channel failed with one unhelpful line. The publish reads
the bucket back after the upload and leaves the pointer where it was when
any platform is missing (#374).

The installer also tells three failures apart. It reads the version's
checksum file and says whether the version is unpublished or the bucket is
unreachable, whether the version has no build for your platform, or whether
the download itself failed. For a platform that was never built, it names
the newest version that does have a build.

[`../ops/linux.md`](../ops/linux.md) is the new page that takes a Linux machine from
a clean install to a running Coder, by release binary and by source build.

## Plugins

A plugin extends Coder with one tool. It is a WebAssembly module and a
manifest that pins it: the manifest declares the artifact's SHA-256 digest,
the typed input and output, the read-only directories the module may open,
and the time and memory it may spend. A plugin has no network access, writes
no files, and sees nothing you did not grant it. Coder refuses to load a
module whose bytes do not match its manifest.

### Turn one on

Press **Ctrl+P**, or enter `/plugins`, to open the Plugins screen. Move to an
entry, press Enter or Space to enable it, wait for the applied message, and
press Esc to return to the chat.

Your choice persists under `~/.openagents`, applies while the chat is idle,
pins the exact package version, and refuses a package that changed or was
withdrawn. A reinstall or an upgrade keeps the choice.

### What you can turn on

The terminal reads a managed catalog from the service, caches the list, and
downloads and verifies a package the first time you enable it. The catalog
seed in this repository lists twelve plugins, and the service serves what an
administrator published:

| Plugin | What it does |
| --- | --- |
| `repo_search` | Answers several literal searches over the workspace in one call. |
| `repo_search_bounded` | Ranks files by how many of your queries each one covers. |
| `rust_outline` | Outlines Rust files, finds a symbol's definition, and checks a name. |
| `ast_grep_bounded` | Searches Rust syntax by node kind, symbol, or structural pattern. |
| `test_report` | Turns captured pytest, Cargo, Jest, Vitest, or Go output into failures. |
| `cargo_diagnostic_filter` | Replaces a failed Cargo command's output with its diagnostics. |
| `git_diff_summary` | Compresses a long `git diff` into file changes and counts. |
| `shell_digest` | Replaces a long successful command's output with a digest, and keeps the original on disk. |
| `progress_filter` | Drops recognized progress lines from successful installs. |
| `env_facts` | Probes the container before a Gym attempt's first turn and puts the facts in the first message. |
| `gate_preflight` | Checks branch state and gate evidence before you run the gate. |
| `word_stats` | Counts bytes, characters, words, and lines in text you supply. |

The build also carries `repo_context`, which returns task-relevant
definitions with their callers, callees, and tests in one bounded call. It
is not in the catalog yet.

Five of these run for the host rather than for the model: `env_facts`,
`shell_digest`, `git_diff_summary`, `progress_filter`, and
`cargo_diagnostic_filter`. The model cannot call them and never sees them
declared, so enabling one adds no tool to your requests.

### Nothing is on by default

Every plugin is off until you turn it on. That is deliberate, and it is not
an oversight to fix later.

A plugin joins the default set only after the Gym records it helping: no
task worse on the mean of calls, seconds, or prompt tokens, the medians
agreeing with the means on the metric that earned the keep, and a
confirmation batch on fresh trials. Nothing has passed that test, so the
default set is empty. [`../plugin-suite.md`](../plugin-suite.md) states the
rule and the measurements behind it, including why `env_facts` stays opt-in
after helping one task and hurting another.

### Write your own

`coder-cli` builds and installs a plugin from a checkout:

| Command | What it does |
| --- | --- |
| `coder-cli plugin init <dir>` | Writes a plugin skeleton with a manifest and fixtures. |
| `coder-cli plugin build <dir>` | Compiles the module and records its digest in the manifest. |
| `coder-cli plugin inspect <manifest>` | Prints the identity, grants, limits, and schemas the manifest declares. |
| `coder-cli plugin test <manifest>` | Runs the fixtures against the compiled module. |
| `coder-cli plugin install <manifest>` | Verifies one package and selects its version in the catalog lock. |
| `coder-cli plugin upload <package>` | Uploads a package to the draft your account owns. |
| `coder-cli plugin publish <package>` | Asks an administrator to publish your draft. |
| `coder-cli plugin mine` | Lists the plugins your account owns, with their state. |

An administrator reviews an upload before it reaches the catalog. Your name
stays on the listing, a release is written once, and a rollback is a pointer
change rather than an edit. [`../plugin-catalog.md`](../plugin-catalog.md)
is the full contract.

Two limits are worth knowing before you start. `coder-cli plugin` cannot
uninstall or list a locally installed package (#354), and `plugin test` can
report `ok` against a stale artifact after a build that failed (#356).

## Delegations

Coder can hand a piece of work to another agent and read its output in your
transcript. This release changes how many run, where they run, and how you
read them.

**No cap unless you ask for one.** A delegation runs until the agent
finishes. A call that names no turn cap and no clock gets neither, so a long
piece of work is not cut off at a default you never chose. This arrived in
`0.3.5` and is on the 0.4.0 card because most people last installed `0.3.1`.

**A fresh worktree each.** A second delegation that writes no longer waits
for the first to release the directory. It gets a fresh worktree of the
repository on its own branch and reports the branch when it starts, so two writing
delegations run at once without stepping on each other (#277).

**A rail under the composer.** Running delegations appear in a rail below
your input bar, each with its number, its agent, and a short summary of what
it is doing. Arrow keys move through the rail, and Down returns you to the
input bar.

**Alt or Ctrl and a number** opens that delegation full screen. `/open
<number>` does the same from anywhere. Ctrl and a digit does not survive
every terminal: without keyboard enhancement, only Ctrl+4 through Ctrl+7
carry a distinct byte, so Alt and a digit is the binding that reaches all
nine. Ctrl+3 stays ambiguous with Esc, and nothing downstream can tell them
apart (#393).

**The full screen scrolls while it streams.** A delegation open full screen
used to swallow every key until the turn ended and then replay them at once.
The frame loop takes your key first and draws once per change, so you scroll
a delegation's output while it is still arriving (#278).

**`c` on a delegation's screen** lists the children that delegation
supervised, and `/children` lists every child in the session. You can stop
one by its exact attempt.

**Delegating to a cloud agent.** `/handoff` and the delegate call reach
agents that run on this computer and agents that run in the cloud. A
delegated model is named the way you type it.

## Turns and the transcript

**Esc ends the whole round.** Before, Esc ended the response and the round
carried on. Now it ends the round. A turn in flight takes two presses: the
first arms the stop and the input bar says `Esc again to stop the turn`, and
the second stops it. Any other key stands the pending stop down. Leaving a
panel with Esc closes the panel and never reaches the turn (#280, #353).

**Ctrl+C clears the input, and quits on a second press.** A second Ctrl+C on
an already empty input leaves the terminal. The rule is on the key line in
`/help` (#279).

**A stopped turn says `turn cancelled`,** and a stream that ends without an
answer shows a notice. This also
arrived in `0.3.5`.

**A cut turn names what cut it.** A turn the model door ends early used to
reach the transcript as one fixed string that dropped the cause. It now
names the output limit, a content filter, or whatever the door reports. A
turn that reached the output limit having written nothing goes again on its
own, once, because reasoning counts against the allowance and a second turn
usually lands. A turn that wrote something keeps what it wrote, a second
empty cut reaches you rather than looping, and a turn you stopped is never
repeated (#363).

**The context rail tells the truth.** A session used to refuse its third
turn at about 6,000 real tokens with the rail reading `context ~99%`. Three
faults stacked: the service published no limit, the terminal enforced a
32,768-token stand-in as though a door had published it, and the estimate
counted every byte as a token. The terminal now enforces only a limit the
door published, counts four bytes to a token, and draws a percent only
against a published limit. Against a stand-in it shows the estimate itself
(#337, #347).

**A dropped connection picks up where it left off.** An interrupted turn
resumes after the connection recovers, and background work saved before the
outage continues rather than being abandoned (#341).

**Your session file keeps the whole answer.** Every answer in every file
under `~/.openagents/sessions/` was stored as the one or two words the first
streamed fragment carried, so a resumed session redrew every answer
truncated. The recorder now rewrites the row as the answer grows (#370).

## Images

Paste an image or drop an image path onto the composer to attach it. PNG,
JPEG, GIF, WebP, and HEIC files up to 4 MB each work. A HEIC from an iPhone
is converted to JPEG before it is sent; on a machine without `sips`, Coder
tells you to export the file as JPEG or PNG instead of failing without a
reason. A dropped path is a path, not an unknown command (#280).

Multiline pastes keep their line breaks.

## Reading files in the terminal

Click a file path in the transcript to open it in a file viewer with syntax
highlighting. Esc or **[ back ]** closes the viewer. A link with a line
anchor scrolls to that line (#340).

The bottom rail of the composer shows this session's memory and processor
use. The sample is throttled, and a stale figure is hidden rather than shown
as current.

## Signing in without a browser

An agent or a machine with no browser can sign in two ways:

- `coder-terminal login --agent` prints a code, and you approve it.
- `CODER_TOKEN` takes an API key that starts with `coder_sk_`, minted with
  `coder-cli key mint`.

Both appear on the signed-out card and in `/help` (#326).

## Autopilot

`coder autopilot <goal>` takes a goal in plain language and runs an agent
against it without further input:

```sh
coder autopilot "fix the failing migration test and commit the repair"
```

| Flag | What it does |
| --- | --- |
| `--dry-run` | Prints what would run and exits without changing anything. |
| `--agent <name>` | Picks which agent executes the cycles. |
| `--max-turns <n>` | Bounds the number of cycles. Defaults to 10. |
| `--budget-cents <n>` | Bounds the spend across the run. |
| `--dir <path>` | Sets the working directory. Defaults to the current one. |
| `--verbose` | Reports progress in more detail. |

This is a first version, and it is smaller than the flag list suggests. The
loop stops after the first cycle that exits cleanly, so `--max-turns` bounds
a retry after a failure rather than driving a multi-step plan to completion.
Read the goal you give it as one delegated task, not as a standing
instruction. Issue #395 is the tracking issue.

## The Gym needs a switch now

The Gym screen shipped in 0.3.x on Ctrl+G with no switch. It is now opt-in.
Start the terminal with `CODER_ENABLE_GYM=on` to get the screen, the `/gym`
commands, and the Ctrl+G binding. With the variable unset, the terminal
hides Gym navigation and help, ignores Ctrl+G, and rejects `/gym`.

The headless `coder-terminal gym` commands work either way.
[`../plugins/terminal-plugins.md`](../plugins/terminal-plugins.md) covers the controls and
the task suites.

## Documentation on the web

The service serves a public documentation page at `/docs`. The index lists
each page with its title and a one-line description, and `/docs/<slug>`
renders one page. The first page is **Plugins in Coder**: what a plugin is,
the four properties the sandbox holds to, and how to write one.

Public pages live in `docs/public/` in the repository, which keeps internal
documents off the web by construction rather than by a filter. The old
`/doc` alias is gone; `/docs` is the only route.

This page was written on 2026-09-07 and was not yet on `main` when these
notes were drafted. Confirm it is live before you link a reader to it.

## Connected computers

A machine you pair holds a claim and takes runs the cloud places on it. A
machine that paired, claimed, and then waited for work used to read
`offline` ninety seconds later and disappear from the computer list, while
the same connection went on taking runs placed on it by name. Holding the
claim now counts as the heartbeat, so a waiting machine stays listed with
its state, and the list carries every computer your account paired (#388).

On a Linux virtual console, the spinner drew as garbage. A console addresses
at most 512 glyphs and loads 256, and the braille block is 256 code points
on its own, so no console font can carry braille and ASCII together. Coder
picks an ASCII spinner where the environment cannot render braille, and
`--glyphs` overrides the choice (#390).

Pairing a machine still needs a source build of this repository. The
released terminal cannot pair one yet (#386).

## Chat sync across devices

Chat sync stays off, as it has since `0.3.4`. With `CODER_CHAT_SYNC` unset, the terminal registers no
device, opens no thread, imports no earlier session, publishes nothing, and
draws nothing about sync. `/sync` and `/publish` are not among the commands
`/help` lists, and typing one returns an unknown-command error.

The work this release does is underneath: a session log with a stable record
identity, a durable journal for a run's critical writes, and a design that
separates what stays on your machine from what an opted-in device sends.
The remaining phases are open issues (#358, #359, #360, #361), and the phone
is tracked separately (#352).

## Where your files live

Everything Coder writes for you lives under `~/.openagents`. A terminal from
0.2.1 wrote its history under `~/.config/coder`, and 0.4.0 stopped reading
that path, one release after the move shipped. Move the file yourself if you
still want that history.

## Bugs fixed

Twenty-two defects you can encounter, by issue number:

- #272: A delegation adapter's commit stamp went stale in a worktree, and
  the headless command said only that no agent runs.
- #273: The terminal retried a refused thread call at once, and the service
  did not rate-limit it.
- #276: A delegation whose adapter was killed read like a finished answer,
  and the terminal could not tell its own binary was replaced under it.
- #277: A second writing delegation waited on the directory's lock instead
  of getting a fresh worktree.
- #278: A delegation open full screen froze every key until the turn ended,
  then replayed them.
- #279: Ctrl+C quit on the first press, with no rule saying it would.
- #280: Esc ended the response but the round carried on, and a dropped HEIC
  read as an unknown command.
- #312: Coder answered a delegated question from its own priors instead of
  waiting for the delegate.
- #337: A turn was dropped without a word when it passed a context limit
  nothing handled.
- #344: The terminal did not build for `windows-x86_64`.
- #345: A native delegation failed when build output changed under it.
- #346: A read-only native delegation searched by listing directories and
  never converged.
- #347: The terminal refused turns at an invented 24,576-token limit and
  counted bytes as tokens.
- #353: Esc while leaving the plugin screen cancelled the running turn.
- #363: A turn that ended incomplete said only that it stopped early, named
  no cause, and never retried.
- #369: A round with two delegate calls wedged the turn, and nothing
  noticed.
- #370: A session file kept only the first fragment of every agent message,
  so a resumed session redrew every answer truncated.
- #372: A replaced websocket stream stranded its generation with no closing
  frame, so a client waited for an answer that could no longer arrive.
- #374: The `rc` channel pointed at a Mac-only build, so every Linux install
  failed.
- #388: The computer list omitted a paired machine that was holding a claim
  and taking runs.
- #390: The braille spinner drew as garbage on a Linux console.
- #393: Only Ctrl+4 through Ctrl+7 opened a delegation on a terminal without
  keyboard enhancement.

One more fix with no issue number: a 20-millisecond timer
that sampled each running delegation with a blocking process call starved
the frame loop, and one person waited 44 minutes on a turn the door had
already finished. The sampling is throttled and the loop stays live.

[`CHANGELOG.md`](changelog.md) lists the repairs to the build, the gate, and
infrastructure you do not use directly.

## What is not here yet

Known limits in 0.4.0, each with its open issue:

- Every lane publishes a context window an eighth of the real one, so the
  rail understates your headroom (#371).
- A prompt typed after `/clear` during a turn sits queued while the session
  looks idle (#375).
- A displaced generation keeps running and charging with nobody able to read
  it (#377).
- The Windows installer cannot tell a missing build from a failed download
  (#378).
- The released terminal cannot pair a machine, so a connected computer needs
  a source build (#386).
- The terminal draws marks a Linux console font cannot carry (#396).
- The model gets no search tool but the shell, and no search plugin has
  earned a place in the default set (#397).
- A turn lost to a malformed function call tells you nothing and takes the
  round with it (#401).
- `/resume` has no interactive session picker (#274).
- `coder-cli plugin` cannot uninstall or list a locally installed package
  (#354), a catalog lock that fails to verify silently disables every
  installed plugin (#355), and `plugin test` can report `ok` against a stale
  artifact (#356).

Chat sync, multi-device threads, and the phone at parity with the terminal
are designed and partly built, and none of them is on in this release.
