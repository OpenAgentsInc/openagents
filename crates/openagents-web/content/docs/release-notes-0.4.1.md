# Coder 0.4.1 release notes

0.4.1 is a patch release. One version covers the service, the web bundle,
the desktop app, the phone app, and the terminal. These notes describe what
changed since 0.4.0.

[`CHANGELOG.md`](changelog.md) lists every change with its issue or commit.
`ops/release.sh --check` reports which channel points at 0.4.1.

## Install or update

One command installs or updates the terminal:

```sh
curl -fsSL https://openagents.com/releases/install-terminal.sh | sh
```

The bare command follows `stable`. `CODER_TERMINAL_CHANNEL=rc` follows the
newest release candidate. The installer verifies the download against the
release's checksum file and installs the command `coder` into
`~/.openagents/bin`.

On Windows, the installer now tells a version with no Windows build from a
download that failed, and names the newest version that does have a build
(#378).

## Highlights

**The release pipeline runs without hand-set credentials.** A staging roll
used to present the credential this machine was signed in with, which
staging does not hold, so every staging canary failed, no image earned a
soak record, and every production deploy needed `--unsoaked`. The deploy now
mints one credential for the canary and the door check, and the soak mints
its own (#389, #427). The terminal publish reads the gate note before it
builds anything, and refuses in seconds instead of after a seven-platform
build (#411). The release's conversation check reads what a turn did, so a
turn that chained three commands into one row passes (#412).

**A stalled provider no longer loses a run.** A guest gave up on the door's
first word at the same moment the service was retrying it, so one late
answer ended a run that had made progress, and every trial of the 2026-09-07
nightly ended that way. The guest now waits 180 seconds for the first word
and takes the turn again up to twice. A run the provider stalls on every
attempt settles as `stalled` and can continue. A family that graded no trial
reads as missing evidence, which the release guard refuses (#415).

**A displaced generation stops charging.** When a second frame took a
socket's stream, the generation behind the first one kept running upstream
and kept spending, with no frame able to read it. The generation is now
cancelled at the moment its stream is replaced, the meter stops there, and a
`generation_displaced` event records it. A displaced thread follow is told
it ended and can follow again from its last cursor (#377).

**The terminal keeps a whole answer across a mid-stream notice.** A
delegation notice, a command row, or an outage line that landed while an
answer streamed split the answer in two, and the exchange kept only the
half after the split. The exchange now keeps the whole answer, and the
screen still draws the halves as separate entries (#406).

**`/clear` during a turn.** `/clear` while a turn runs cancels the
foreground work, and a prompt typed after it starts the next conversation
instead of sitting queued behind a session that looks idle (#375).

**The frame loop stays live.** The loop serves every ready event source
within a bounded round, so a delegation's updates and a plugin's results
arrive while another source is busy (#376).

**A Box host holds its own credential.** A host used to authenticate with
the pool's one secret and then name itself, so any holder of the secret
could register under a managed host's name and take its placement jobs. A
host now holds a key derived for its own name, a key issued for one host is
refused under another name, and `coder box-host-revoke <name>` refuses
every key that host was issued while every other host keeps working (#398).

**Plugins say what they refused.** A catalog lock that fails to verify
reports `catalog_refused` and names the failed pin, instead of disabling
every installed plugin without a word, and `coder-cli plugin list` shows
the verified pins (#355). `plugin test` refuses a stale or failed build
with `artifact_stale` through a receipt that ties the artifact to the
source that built it (#356). `coder-cli plugin uninstall` unpins and
removes a local package (#354).

**A released terminal pairs a machine.** `coder pair`, `coder confirm`,
and `coder claim` connect a computer to your account from the released
terminal. A machine with no browser pairs with `coder pair --no-confirm`,
and another signed-in machine confirms the code (#386). 0.4.0 needed a
source build for this.

**`/resume` opens a session picker.** `/resume` with no argument lists
recent sessions in a panel. Up, Down, `j`, and `k` move, Enter opens, and
Esc closes. A number or an id after `/resume` still opens that session
directly (#274).

## Terminal

- `/clear` during a turn cancels the foreground work and keeps the next
  prompt (#375).
- An entry that lands while an answer streams no longer cuts what the
  exchange keeps (#406).
- The frame loop serves every ready source within a bounded round (#376).
- A delegation report that arrives during a turn waits for the next request,
  so the door no longer refuses an exchange that ends with the model's own
  turn (#394).
- A turn lost to a tool call the door cannot read says what the round lost
  and that your prompt stays in the exchange (#401).
- The marks a Linux console font lacks draw in ASCII under `TERM=linux` or a
  locale without UTF-8 (#396).
- `/resume` with no argument opens a session picker (#274).
- `coder pair`, `coder confirm`, and `coder claim` connect a machine from
  the released terminal (#386).
- A delegated turn writes its own session file under
  `~/.openagents/sessions/`, with every tool call and result. `--export` and
  the headless answer name it (#405).
- The composer draws at the bottom of a terminal taller than 128 rows
  (#420).
- The full-screen delegation view follows the tail as cells arrive, and
  holds a position you scrolled back to (`2d09bfd797`).
- `/tile <number>` opens a delegation in a window beside the parent where
  the desktop tiles windows, up to four at once (#408).
- The `windows` tool reads the desktop session Coder runs in and opens a
  window in it. `describe` answers what is on your screen in words (#409).
- The `browser` tool steers the Coder Browser on a CoderOS host, reads
  frames, dialogs, and long pages, and hands a sign-in to you with
  `handoff` (#421, #422, #426).
- A browser click that submits something asks for your yes on the command
  row first (`bc8e1fe56b`).
- The `recording` tool records the screen of a CoderOS host (#413).
- A thread follow says when it has caught up, bounds its replay, and reports
  a failed follow (#358).
- The terminal hides a thread action the service does not offer (#360).
- The terminal follows a peer terminal's threads without the service, over a tailnet it joins as its own device with an auth key; `/peer` adds, removes, and lists peers, and a followed thread opens read-only with its writer named (#429).
- The model gets `code_search` and `where_is` beside `shell`, and the prompt says what each is for, so a lookup is one call instead of a run of shell commands (#397).

## Service

- A displaced generation is cancelled when its stream is replaced. The
  meter stops there, and `generation_displaced` records it (#377).
- A run the provider stalls settles as `stalled` and can continue. A guest
  waits 180 seconds for the first word and takes the turn again up to twice
  (#415).
- Every lane publishes its measured context retention (#371).
- `whoami` carries thread capabilities as data (#360).
- Admission answers `422 unsupported_profile` for a profile no pool host can
  place (#399).

## Plugins

- A catalog lock that fails to verify reports `catalog_refused` and the
  failed pin. `coder-cli plugin list` shows verified local pins (#355).
- `plugin test` refuses a stale or failed build with `artifact_stale`
  (#356).
- `coder-cli plugin uninstall` unpins and removes a local package (#354).

## Box and CoderOS

- A Box host holds a key derived for its own name. `coder box-host-key` and
  `coder box-host-revoke` issue and revoke it (#398).
- A CoderOS host gets an optional desktop whose default window is Coder,
  with tiling keys and a session manager (#391).
- `Super+B` opens the Coder Browser. A stray browser command on the host
  routes to it (#426), and the browser opens dark with amber edges (#425).
- `Super+P` sets the screen up for recording and puts it back (#418).
- `Ctrl` and `+`, `-`, or `0` resize the terminal's text (#417).
- A camera view from a webcam, a recording meter docked under it, and
  `Super+C` to toggle both (#413, #428).
- A silent microphone no longer destroys a recording, and the named
  microphone stays the default at a set gain (`4bd6a048d9`, `6ed7250766`).

## Release and operations

- The staging canary mints its own credential and reads the screen, so a
  refused credential ends the wait at once (#389).
- The deploy mints one credential for the canary and the door check, and the
  soak mints its own (#427).
- `ops/release-terminal.sh --publish` reads the gate note before it builds
  (#411).
- The release conversation check accepts a chained multi-command turn
  (#412).
- The gate takes one lock per machine, sweeps a dead holder, and bounds its
  build jobs (#373).
- The Windows installer tells a missing build from a failed download (#378).
- The nightly records each family's graded count, and the release guard
  reads a family with none as missing evidence (#415).
- `coder-cli storage scan` reports what a cleanup could reclaim, by risk tier, and exits non-zero when free space is under the floor; it removes nothing (#367).
- `ops/gate.sh --pool` runs the gate in a warm checkout under `~/.openagents/gate/pool`, one slot per gate, with the slot reset to the exact commit it checks (#373).
- The soak sends its `/mcp` probes to a host the service names, so a staged image earns its soak record (#427).

## Known limits

Known limits in 0.4.1, each with its open issue:

- A Box host provisioned before this release refuses to start until it takes
  a derived key. Roll the pool with `ops/box-pool.sh template` and
  `ops/box-pool.sh update` (#398).
- A run the provider stops after the tree changed does not push what it has
  (#415).
- Windows-native execution and automatic gate coverage remain unverified
  (#378).
- The browser skill says how to cancel a subscription with you signing in.
  The flow itself is unproven (#424).
- `/tile` needs a desktop that tiles windows. Elsewhere it points at
  `/open <number>` (#408).
- Chat sync stays off. The thread follow and capability work in this release
  is behind `CODER_CHAT_SYNC` (#358, #360, #361).
- The phone lags the terminal (#352).
