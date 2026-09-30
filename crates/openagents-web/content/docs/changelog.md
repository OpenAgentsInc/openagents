# Changelog

This page lists every change in a release, one entry per change, grouped by
surface. Each entry names its GitHub issue, or its commit on `main` when no
issue exists. Each release has a page written for the person who uses
Coder: [`0.5.0.md`](release-notes-0.5.0.md), [`0.4.1.md`](release-notes-0.4.1.md), and [`0.4.0.md`](release-notes-0.4.0.md).

## 0.5.0

Range: `c8023356de..c45f393c3`, 326 commits from 2026-09-08 to 2026-09-11.
0.5.0 covers the service, the terminal, the local inference daemon, mobile,
desktop, and Autopilot orchestration.

### Terminal

- Model reasoning folds into an expandable thought drawer with step count
  and live preview (#483, `4b6780ef8`, `1c509ddbe`, `11cbce30f`).
- In-tree search tools `code_search` and `where_is` backed by a per-checkout
  trigram index (#397, `997f576e4`, `0ed5ac0bc`, `7723c6e92`).
- The terminal follows peer terminals device-to-device over a Tailnet
  without relying on the central service (#429, `6a88c2105`, `0726d494f`, `2f5d3d624`).
- Child agent delegation screens stream live with isolated worktrees, bounded
  verification slots, and fact inheritance (#541, `f076a9142`, `51f9a8dc5`, `a1925064b`).
- Key chords are unified and documented in a single bindings table rendered
  in help (#538, `fb29dc164`).
- Move command-line argument parsing out of main into a dedicated module
  (#568, `78382a699`).
- Headless execution mode drives single chat turns and reports tool invocations
  (#559, `18370f0e5`).
- Streaming layout optimizations re-render only dirty cells during delta updates
  (#572, `c3622a9a7`).

### Autopilot & Orchestration

- Autopilot orchestrator manages multi-agent missions and autonomous goal
  execution (`9315a91ef`, `1b284176e`, `6d4930d9e`).
- Headless writer acts as the Autopilot main session runtime (`9ae6f24b5`).
- Automatic mission goal inference when `/autopilot` is run without an argument
  (`5069bdf5c`).
- Structured mission rules, standing constraints, and branch tracking for
  unlanded child work (#563, #552, `4687bb2dc`, `434c9fcea`).
- Delegated child agents inherit parent facts, context artifacts, and scoped roles
  (#558, #522, `9179d07d7`, `403b405c4`, `fb9b6b33c`).
- Reader and writer execution barrier synchronizes parallel tool calls (#531, #571).

### Inference & Local Models

- Local inference daemon (`coder-inference-daemon`) serves local models over
  loopback with prefix caching and telemetry (`502c160fa`, `8e138293d`).
- In-tree GPT-OSS core with MXFP4 weight loader and Tile IR GPU decode kernels
  (#479, #480, `62b95a5ca`, `3db9d3e46`).
- Harmony prompt/completion rendering and parsing for structured model output
  (#481, `3e1f004b2`).
- Prefill prompt acceleration on GPU in blocks over per-sequence KV cache
  (`26ef96301`, `9548543ca`, `9e5566a29`).
- Sliding-window KV cache rewind eliminates re-prefilling overhead (#518, `0839bae3a`).
- Inference daemon releases GPU memory dynamically when idle (`e97407987`).

### Mobile (iOS & Android)

- Peer terminal thread sync allows direct mobile monitoring of terminal sessions
  (`b65a3ced1`, `56487f281`).
- Phone app gains offline reading and session resumption (#565, `341da3d7f`).
- Dedicated settings screen on mobile decoupling session state from environment
  (#557, `de5714fae`).
- Multi-device build and install automation for connected iPhones and emulators
  (#448, #544, `c6387e310`, `a5ead07ad`, `cebabc40f`).
- Background thread follow pauses automatically to preserve battery (#459, `2dfe91b76`).
- Machine identity tags display on each chat thread in the mobile list (`238514b17`).

### CoderOS & Box

- cuTile toolchain and CUDA 13.2 support added to CoderOS image (#478, `4653aa152`).
- Thelio power, thermal, and boost management bounds CPU wattage and adds BIOS
  update path (`b99f45ab6`, `a18d2b563`, `44f0ba983`).
- Zoom client integrated behind desktop option with sandboxed browser URL dispatch
  (`2884012cc`, `a47284ded`, `00e2da48c`).
- Hyprland configuration written as regular file with automatic reload on desktop switch
  (`9b59440bd`).
- Host auto-rebuild capability for operator accounts without sudo password
  (`6d41ab804`, `3e411dfa2`).
- Dedicated verification slots on host prevent concurrent test build interference
  (`f89d111cb`, `6203b61da`).

### Documentation & Benchmarks

- Technical specs for Coder Notes markdown editor and Coder Desktop on Mac as CoderOS
  (`c5cd9fb06`, `c45f393c3`).
- Reorganized documentation structure into topical folders (`d3ccbd237`).
- Tool contract specifications and D1 tool corpus definitions (#521, #532).
- Video production bible and HUD recording guidelines (#455, `455b95394`).
- Storage scan and garbage collection policies for stale worktrees and artifacts
  (`778ff8ac0`, `3d5be1d2d`, `f23dd1e40`).

## 0.4.1

Range: `cac3d18803..c8023356de`, 196 commits from 2026-09-07 to 2026-09-08.
`cac3d18803` is the 0.4.0 release commit: production serves it, and
terminal 0.4.0 was published from it on every platform.

The first 21 commits of the range, through `14edab2dee`, are listed under
0.4.0 and are not repeated here. 0.4.0 was published from a commit before
them, so their changes, among them #388, #390, #393, and #395, reach you
with this release.

### Terminal

- `/clear` during a turn cancels the foreground work and keeps the next
  prompt (#375).
- An entry that lands while an answer streams no longer cuts what the
  exchange keeps (#406).
- The frame loop serves every ready source within a bounded round (#376).
- A delegation report that arrives during a turn waits for the next
  request. `Session::ask_again` sends nothing when the exchange ends with
  the model's turn, and says so (#394).
- A turn lost to a tool call the door cannot read says what the round lost
  and that the prompt stays in the exchange (#401).
- The marks a Linux console font lacks draw in ASCII. `glyphs::MARKS` and
  `glyphs::MARKS_ASCII` hold the table (#396).
- `/resume` with no argument opens a session picker (#274).
- `coder pair`, `coder confirm`, and `coder claim` connect a machine from
  the released terminal. `coder pair --no-confirm` pairs one with no browser
  (#386).
- A delegated turn writes its own session file under
  `~/.openagents/sessions/`. `--export` and the headless answer name it
  (#405).
- The composer draws at the bottom of a terminal taller than 128 rows. Both
  terminal crates move to ratatui 0.30 (#420).
- The full-screen delegation view follows the tail and holds a scrolled-back
  position (`2d09bfd797`).
- `/tile <number>` opens a delegation in a window beside the parent, up to
  four at once (#408).
- The `windows` tool: `list`, `focused`, `screens`, `open`, and `describe`
  (#409, `ba33484ab4`).
- The `browser` tool: `tabs`, `page`, `find`, `open`, `go`, `click`,
  `type`, `handoff`, `wait`, and `choose` (#421, `c900b93b93`,
  `2c73e87ff6`).
- A browser click or Enter that submits something asks for the person's
  yes first (`bc8e1fe56b`).
- The `browser` tool is offered while the browser is closed and opens it on
  the first call (#426).
- The `recording` tool: `start`, `stop`, and `status` on a CoderOS host
  (#413, `5d07da5ad8`, `b8f756447a`).
- A thread follow reports caught up, bounds replay, and reports a failed
  follow. A failed follow retries without closing the shared socket (#358).
- A client hides a thread action the service does not offer (#360).
- The terminal follows a peer terminal's threads without the service, over a tailnet it joins as its own device with an auth key; `/peer` adds, removes, and lists peers, and a followed thread opens read-only with its writer named (#429).
- The model gets `code_search` and `where_is` beside `shell`, and the prompt says what each is for, so a lookup is one call instead of a run of shell commands (#397).

### Service

- A displaced generation is cancelled when its stream is replaced. The
  meter stops there, and `generation_displaced` records it. A displaced
  thread follow receives `unfollowed` (#377).
- A run the provider stalls settles as `stalled` and can continue. A guest
  waits 180 seconds for the first word and takes the turn again up to twice
  (#415, migration `0057_run_stalled.sql`).
- Every lane publishes its measured context retention (#371).
- `whoami` carries thread capabilities as data (#360).
- Admission answers `422 unsupported_profile` for a profile no pool host can
  place (#399).

### Plugins

- A catalog lock that fails to verify reports `catalog_refused` and the
  failed pin. `coder-cli plugin list` shows verified local pins (#355).
- `plugin test` refuses a stale or failed build with `artifact_stale`. A
  build writes a receipt with the source digest (#356).
- `coder-cli plugin uninstall` unpins and removes a local package (#354).

### Box and CoderOS

- A Box host holds a key derived for its own name. The pool secret is an
  issuing secret and no longer a bearer. `coder box-host-key` and
  `coder box-host-revoke` issue and revoke a host's key (#398, migration
  `0056_box_host_keys.sql`).
- `coderos.desktop.enable` gives a host a Hyprland session with Coder as
  the default window, under its session manager, with tiling keys (#391).
- `Super+B` opens the Coder Browser with the DevTools Protocol on a
  loopback port (`68e5203322`).
- Every browser command on the host routes to the Coder Browser (#426).
- The Coder Browser opens dark with an amber toolbar, and runs without
  Vulkan on the Nvidia GPU (#425, `9a48ded7da`).
- The camera view keeps its circle while it moves (#425).
- A camera view from a webcam, and a timed recording that finishes through
  `stop` (#413).
- Screen recording and dictation are declared in `os/` (`d72d45725e`).
- `Super+P` toggles presentation mode, and `screen-record --presentation`
  enters it for the recording (#418).
- `Ctrl` and `+`, `-`, or `0` resize the terminal's text. `terminal-font`
  does the same without a keyboard (#417).
- A recording meter docks under the camera view. `Super+C` toggles the
  camera and the meter (#428).
- A silent microphone no longer destroys a recording (`4bd6a048d9`).
- The host names the microphone it records with and sets its gain
  (`6ed7250766`).
- The named microphone no longer wedges after an idle suspend. `Super+T`
  opens a terminal (`8534ec76b0`).
- The camera view starts with the session (`fb42024b71`).
- The terminal emulator uses the amber palette (`e851fe313a`).
- `Super+A` opens an Android emulator (`ce100aef21`).

### Docs

- [`../roadmap.md`](../roadmap.md): every open issue by release and order,
  the claim rules, and what shipped (`cbc447aeed`, `8a79a42ab9`).
- [`../windows.md`](../windows.md): the `windows` tool (`ba33484ab4`).
- [`../browser/browser.md`](../browser/browser.md) and
  [`../browser/2026-09-07-coder-browser-coderos-mvp.md`](../browser/2026-09-07-coder-browser-coderos-mvp.md):
  the `browser` tool and its first version on CoderOS (`5c8b8c294a`,
  `c900b93b93`).
- [`../recording.md`](../recording.md): the `recording` tool and why it is
  native (`5d07da5ad8`).
- [`../delegation/delegation-record.md`](../delegation/delegation-record.md): the session file a
  delegated turn writes (#405).
- [`../delegation/delegation-tiles.md`](../delegation/delegation-tiles.md): `/tile` (#408).
- [`../session/issue-371-context-capabilities.md`](../session/issue-371-context-capabilities.md):
  measured context retention (#371).
- [`../session/issue-415-recovery.md`](../session/issue-415-recovery.md): provider-stall
  recovery and missing-evidence classification (#415).
- [`../deprecated/2026-09-07-turns-lost-to-an-unreadable-call.md`](../deprecated/2026-09-07-turns-lost-to-an-unreadable-call.md):
  the measurement behind #401.
- [`../storage-cleanup-plugin.md`](../storage-cleanup-plugin.md): the
  storage cleanup specification, not implemented (#367).
- [`../devices/2026-09-08-device-to-device-lane-decision.md`](../devices/2026-09-08-device-to-device-lane-decision.md):
  the device-to-device lane decision (#429).
- [`../qa/2026-09-08-windows-installer.md`](../qa/2026-09-08-windows-installer.md):
  Windows installer coverage and limits (#378).
- [`../qa/2026-09-08-file-viewer-readiness.md`](../qa/2026-09-08-file-viewer-readiness.md):
  the file viewer readiness repair (#410).
- Thread capability and absence policies (`673d2f669b`).
- The browser skill under `.agents/skills/` (#424).
- The 0.4.0 changelog and release notes are served on `/docs`
  (`fb792717f9`).
- `docs/deploy.md` says the deploy mints once for both checks and the soak
  mints its own (#427).

### Release and operations

- The staging canary mints its own credential, rolls the issuer key it
  verifies against, and reads the screen so a refused credential ends the
  wait at once (#389).
- The deploy mints one credential for the canary and the door check.
  `ops/soak.sh` mints its own when `--bearer` names none (#427).
- `ops/release-terminal.sh --publish` reads the gate note before it builds,
  and runs the channel's gate when there is none (#411).
- The release conversation check accepts a chained multi-command turn
  (#412).
- The gate takes one lock per machine, sweeps a dead holder, and sets
  `CARGO_BUILD_JOBS` to half the cores. `--no-admission` runs without the
  lock (#373).
- The Windows installer tells a missing build from a failed download and
  names its fallback (#378).
- The nightly writes each family's graded count and opens an issue for a
  family that graded nothing. The release guard reads it as missing
  evidence (#415).
- The gate on a CoderOS host sets an absent browser grant, so no test opens
  a browser (`c8023356de`).
- The file viewer screen check is a known-timing test, and its fixture waits
  for the submitted row (#410, `bdbe26bfd1`).
- `coder-cli storage scan` reports what a cleanup could reclaim, by risk tier, and exits non-zero when free space is under the floor; it removes nothing (#367).
- `ops/gate.sh --pool` runs the gate in a warm checkout under `~/.openagents/gate/pool`, one slot per gate, with the slot reset to the exact commit it checks (#373).
- The soak sends its `/mcp` probes to a host the service names, so a staged image earns its soak record (#427).

## 0.4.0

Range: `539bc34ccd..14edab2dee`, 592 commits from 2026-09-04 to 2026-09-07.
`539bc34ccd` published terminal 0.3.5. The repository has no version tags.

Terminal 0.3.5 was published from `origin/release/0.3`, which branched from
`main` at `7bb64fde47`. The range includes changes that were cherry-picked to
the 0.3 line and shipped in a 0.3.x terminal. Each is marked **already in
0.3.x**.

### Terminal

- Esc ends the whole round (#280).
- Stopping a turn takes two Esc presses (#353).
- Ctrl+C clears the input. A second press quits (#279).
- A stopped turn shows `turn cancelled`. A stream that ends without an
  answer shows a notice (`db55dc1a47`; already in 0.3.5).
- A turn the door ends early shows the cause. A turn that reaches the output
  limit with no output runs again once (#363).
- A projection enforces only a published context limit. The estimate uses
  four bytes per token (#337, #347).
- The composer rail shows a percentage against a published limit, or the
  estimate against a stand-in (#347).
- An interrupted turn resumes after the connection recovers (#341).
- Background results saved before an outage are delivered after it
  (`55625e0ddd`, `0fc78fae1f`).
- A delegation has no turn limit and no timeout unless the call sets one
  (`20f3f05e0c`; already in 0.3.5).
- A second writing delegation gets a fresh worktree (#277).
- A delegation rail sits under the composer, with keyboard navigation to and
  from the input bar (`4460a3772c`, `27a3a4bde6`, `4c23690386`).
- Alt or Ctrl and a digit opens a delegation full screen. `/open <number>`
  works anywhere (#393).
- The frame loop handles keyboard input before other events and lays out
  the full screen once per change, so a delegation scrolls while it streams
  (#278, `1cde4324fe`).
- Delegation resource sampling is throttled. Stale readings are hidden
  (`42893827e5`, `c780edcde1`).
- A finished delegation shows `Delegation #N finished` (`8e4949cfe3`).
- A finished delegation does not scroll the transcript (`f6642fe8d0`).
- `c` on a delegation screen lists its supervised children. `/children`
  lists every child and stops one by attempt (`5cc6fa3bf9`).
- A delegate call sets a reasoning effort from the task (#343).
- `/handoff`, and the Devin local and cloud lanes (`84a58718f9`).
- Devin messages appear in the full-screen delegation view (`2584c0333d`).
- Image attachments: paste or drop a PNG, JPEG, GIF, WebP, or HEIC file up
  to 4 MB. HEIC is converted to JPEG (#280, `f1b0882186`).
- A multiline paste keeps its line breaks (`f1b0882186`).
- A file path in the transcript opens a syntax-highlighted viewer with line
  anchors (#340).
- The composer rail shows session memory and processor use (#340,
  `4515ec7482`).
- The composer has two rows and a right-aligned rail (`0d1a665c1e`).
- `coder-terminal login --agent` signs in without a browser. `CODER_TOKEN`
  accepts a `coder_sk_` key (#326).
- The Gym screen, its commands, and Ctrl+G require `CODER_ENABLE_GYM=on`
  (`b2fa399d01`).
- The spinner uses ASCII when the terminal cannot render braille.
  `--glyphs` overrides the choice (#390).
- The screen repaints when the window regains focus or resizes
  (`3cccd69646`).
- The session log records the whole answer (#370).
- The session log carries a journal envelope and structured checkpoints
  (`65e0375ae5`, `2923c4adee`, `21c7dd7a0b`).
- The terminal reads the managed plugin catalog, downloads a release, and
  installs it locally (`58095cd6ea`).
- Bundled plugin packages are a development source and appear only with
  `CODER_PLUGIN_BUNDLES=on` (`d95ae0ee7e`).
- A delegate turn is metered to your account. A report the service refuses
  is held until a credential can send it (`8fcefb38cf`, `fe95c2c67a`,
  `5a37d25d2e`).
- The terminal registers no device and shows no sync state unless
  `CODER_CHAT_SYNC` is on (`ac7fe609d8`, `d384b42909`; already in 0.3.4).
- History that a 0.2.1 terminal wrote under `~/.config/coder` is no longer
  read (`d615b4cad7`).
- An import stops at the first refusal and waits the time the refusal
  specifies (#273, `6f03a50b25`).
- A delegation whose adapter was killed reports an error. The report names a
  replaced binary (#276).
- An adapter reports its build commit. A turn that fails records its last
  output (#272, `386170503c`, `f1c7376c0e`).
- After a delegation starts, the answer names the task and stops (#312).
- A round with two delegate calls no longer wedges the turn (#369).
- A native delegation continues when build output changes during the run
  (#345).
- A read-only native delegation searches in process under a constrained
  containment profile (#346, `957f39a9d0`, `378c432e2a`).
- The terminal builds for `windows-x86_64`. Scoped file containment is
  refused on Windows (#344).
- A command group stops after a grace period. An exited, unreaped member
  counts as stopped (`7e52794e12`, `cac3d18803`).
- `coder autopilot <goal>` runs an agent against a goal, with `--dry-run`,
  `--agent`, `--max-turns`, `--budget-cents`, and `--dir` (#395).

### Service

- A replaced websocket stream sends a terminal frame for the generation it
  replaces (#372).
- The message stream pages with the server cursor. An answered session ends
  the turn (`0645feb6c7`).
- `reason_code` reports the cause the door named. An incomplete turn no
  longer reports `upstream_error` (#363).
- The door retries a turn the upstream rejected as malformed
  (`90ed0cc6e0`, `737bc1116e`).
- Every wait on the model path has a timeout. A turn records its timing
  (`bb0f79cf31`).
- The computer list includes a claiming computer with its state, and every
  paired computer (#388).
- A machine that stops returns its runs to the queue (`a721fb64a8`).
- A follow on a free account sends no frame. A refused follow sends a
  refusal frame (`7a1f00f5b0`, `4fde9c0549`; the refusal frame is already
  in 0.3.1).
- `/mcp` returns `429` with a retry wait when a bearer exceeds its budget
  (`d678295cc8`).
- The plugin catalog is stored in Postgres and managed from an owner-only
  admin panel (#294, `80b97db983`).
- Catalog mutations are transactional, with compare-and-set versions and
  idempotent upload (#332).
- Plugin upload and publish scopes, owner identity, and reserved platform
  names (#334).
- A plugin command service answers the local plugin commands and is offered
  to native sessions as a bounded tool (#333).
- Pro and administrator accounts get Coder Cloud access with hourly metering
  (#342).
- The Box API is served behind `BOX_API_BASE`, with environments, secrets,
  repositories, snapshots, webhooks, events, prompts, and hosted ports
  (#282, #283, #284, #285).
- Effective authority is compiled and enforced across tools and subagents
  (#290).
- Containment profiles are enforced fail-closed (#291).
- Typed execution outcomes and atomic resource reservations (#292).
- Critical journal records commit before the side effect runs (#293).
- A runner supervisor with durable recovery across execution paths (#319,
  #330, #331).
- Every route, read-only tool, and pairing path records an analytics event
  (`cd0398831d`, `618667bac8`, `ffd76367fd`).
- A public `/docs` index and `/docs/{slug}` page, served from `docs/public/`
  (`c5dccd91fa`, `7d00c8b3e3`).
- The `/doc` alias is removed (`c5dccd91fa`).
- The blog is hidden in production. The GitHub sign-in links are removed
  from user-facing surfaces (`cc626eef5d`).

### Command line

- `coder-cli plugin init`, `build`, `inspect`, `test`, and `install`
  (`d89cf436ff`, #333).
- `coder-cli plugin upload`, `publish`, and `mine` (#334).
- `coder-cli evals` with eval families and expect grading (`bdb4db25d0`).
- Eval families for safety, steering, failure, and door behavior, with their
  corpora (`6d4edf8471`, `90ebb5975b`).
- A nightly live eval run and a weekly executor-parity comparison
  (`87832b3cc8`, `05155fadf7`).
- A release requires nightly and weekly eval evidence (`53c4997bd3`).
- The runner signs in and speaks MCP through `coder-auth`, with no
  dependency on the command line (`20ee0bf49c`).
- Devin and DeepWiki MCP tools are declared in the terminal (#315, #316,
  `f698e6a4b9`).

### Plugins

- `repo_search` searches for several literals in one pass (#269).
- `repo_search_bounded` ranks files by distinct query coverage
  (`2b66c45785`).
- `repo_context` returns definitions with callers, callees, and tests in one
  bounded pack, and extracts C and C++ (`745493fb3b`, `4b126e02f2`).
- `rust_outline` outlines, finds, and checks Rust with tree-sitter
  (`d2c756991e`).
- `ast_grep_bounded` searches Rust syntax by node kind, symbol, or pattern
  (#297).
- `cargo_diagnostic_filter` removes build noise from a failed Cargo command
  (#295).
- `gate_preflight` checks branch state and gate evidence (#296).
- `git_diff_summary` compresses a long diff (#318).
- `progress_filter` drops recognized progress lines (#263).
- `shell_digest` digests large successful output and keeps the original on
  disk (`c14f3992f3`).
- `env_facts` reads container facts before the first turn. Host-only
  (`d9567e46b9`).
- The default suite is empty. `plugin-suite.md` records the admission test
  and the measurements (`ad7fbf67aa`).
- A plugin compiles once per process and is shared by digest
  (`989c526afb`).
- Guest faults are caught with Unix signals on macOS (`ee988efdd7`).

### Docs

- [`../ops/linux.md`](../ops/linux.md): installing Coder on a clean Linux machine
  (#384).
- [`../plugin-catalog.md`](../plugin-catalog.md): the catalog contract
  (`82a2fc9bc2`).
- [`../plugin-suite.md`](../plugin-suite.md): suite admission and the
  measured enablement decision (`ad7fbf67aa`).
- [`../GLOSSARY.md`](../GLOSSARY.md): shared terminology, including terminal
  credential, delegation, plugin, and installer terms (`81a7353610`,
  `fc5798cef0`, `e6a6809d9b`).
- [`../data.md`](../data.md): every path that sends your content off your
  machine, with a gate test (`a9f7684e66`).
- [`../analytics.md`](../analytics.md): the analytics catalog, gated against
  routes and tools (`cd0398831d`).
- [`../raid/2026-09-05-codex-claude-code-adaptation-plan.md`](../raid/2026-09-05-codex-claude-code-adaptation-plan.md):
  the adaptation roadmap and its clean-room rules (`7bad1cdb7c`,
  `2031fa26c0`).
- [`../deprecated/2026-09-07-turns-the-door-cuts-short.md`](../deprecated/2026-09-07-turns-the-door-cuts-short.md):
  incomplete turns and the output-allowance stand-in (#363).
- [`../session/2026-09-07-local-and-synced-session-storage.md`](../session/2026-09-07-local-and-synced-session-storage.md):
  what the workspace persists and what synced mode would send (#348).
- [`../delegation/2026-09-07-shared-child-views.md`](../delegation/2026-09-07-shared-child-views.md):
  child views across the terminal and the desktop app (#338).
- [`../session/2026-09-07-structured-checkpoints.md`](../session/2026-09-07-structured-checkpoints.md):
  the checkpoint record and its evidence (#348).
- [`../box/2026-09-07-box-placement.md`](../box/2026-09-07-box-placement.md): the
  placement adapter and its status (#351).
- [`../ops/docs-gate.md`](../ops/docs-gate.md): documentation checks and their
  measured cost (#327).
- [`../box-linux-gate.md`](../box-linux-gate.md): authorized Box/Linux gate
  evidence and its limits (`1d27ca3d81`).
- The public documentation page moved to `docs/public/`, with `plugins.md`
  as its first page (`c5dccd91fa`).
- Every document is labeled current, proposed, or historical. The topology
  is documented in one place (`2485b3d12c`).
- `docs/plugins/plugin-mvp.md` documents `manifest_unreadable` and the exit code for
  a bad path (#357).

### Release and operations

- The gate has push, candidate, and release tiers (`dc70d012ff`).
- A push that changes only documentation runs the documentation gate (#327,
  `ee21ab74b6`).
- Documentation checks run without the service runtime (`20a73bcdd5`).
- `ops/gate.sh --apple` runs the macOS checks only (`f33551b81e`).
- `ops/gate.sh --remote` orders the Linux half on the pool.
  `--remote-status` reads it (`2d9ee8eb5f`).
- `ops/gate.sh --submit` lands a submission through a merge queue
  (`d96cf5b409`, `d74e2c7804`).
- A gate note holds both halves, and the pre-push hook accepts it
  (`39d082e8ff`).
- A terminal-only change scopes the `coder` package, so the end-to-end
  terminal checks run for it (#366).
- Each `ops` script has a test (`6eeece87cc`).
- Tool-contract golden corpora for `edit_file`, `run_command`, and replay
  bounding, counted on the gate note (#313).
- Authority, containment, outcome, journal, supervision, and placement
  corpora in the gate (#314, #319, #351).
- The macOS terminal is signed with the entitlements wasmtime needs
  (`0c59f641df`; already in 0.3.3).
- The release waits for Gatekeeper and refuses an artifact it rejects
  (#287).
- The release runs the built artifact and checks its first screen
  (`e7fb79685e`).
- A channel moves only to a version every supported platform can install.
  `--allow-partial` no longer covers the channel (#374).
- `--point-channel NAME` moves a channel to a published version with no
  build and no upload (#374).
- The installer distinguishes an unpublished version, a missing platform
  build, and a failed download, and names the newest version with a build
  (#374).
- The install command has a short form at `/install-terminal.sh`
  (`53f5556c4b`).
- The workspace version is `0.4.0`. A candidate gets its version at build
  time (`44475d0402`).
- The release card is generated from the approved release notes
  (`5fe2294d33`).
- A deploy sends one turn through the new revision and rolls back if the
  turn does not end (`c6ae638fc3`).
- A staging soak records an image's behavior under load. Production refuses
  an image without a soak record (`b0fb033ade`).
- Alert policies cover five previously unreported faults. A stalled turn
  sends a report (`42511bfeb1`).
- Every published surface is built from an archive of its commit
  (`5e3b2039a0`).
- A Box host pool with a baked guest image, and a gate guest that runs the
  scoped gate (`2e1a834498`, `9d0f9b353b`).
- Box snapshot archive round trips and rejection tests are repaired (#328).
- The `coderos-4080` host has a GPU driver, a tracked system configuration,
  a claim held as a service, and a documented Linux path (#379, #380, #381,
  #382, #384, #387).
- `docs/workers.md` describes the computer registration and claim path as
  built (#383).
