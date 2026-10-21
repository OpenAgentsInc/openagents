# A coding agent that runs in the browser, inside the Verse

Status: research note, 2026-10-21. Nothing here is implemented, and no
issues are filed. Wanix was read at commit `af7fd38` (2026-10-20) and
Apptron at commit `c4cb2db` (2026-07-13), both from shallow clones read as
data. None of their code was run.

Each section separates **facts**, cited to a file or a URL, from
**inference**, which is labeled. Paths such as `fs/vfs/vfs.go` are relative
to the upstream repository named in that section. Paths that start with
`crates/`, `docs/`, `nips/`, or `apps/` are in this repository.

Update, 2026-10-21: a Kiln tech demo is built ([`docs/kiln/`](../../kiln/README.md)),
and the owner's direction for a Kiln workstation in front of the Pylon Bay,
with NIP-34 patches and pull requests, is specced in
[`docs/kiln/workstation.md`](../../kiln/workstation.md).

Working name for our version: **Kiln**. "Forge" is taken
(`docs/forge/README.md`), and a repository search finds no other use of
"Kiln".

## Contents

- [Verdict](#verdict)
- [Part 1: what exists](#part-1-what-exists)
  - [Wanix](#wanix)
  - [Apptron](#apptron)
  - [fx and libfx](#fx-and-libfx)
  - [Other prior art](#other-prior-art)
- [Part 2: what we already have](#part-2-what-we-already-have)
  - [How much of our Rust already runs in the browser](#how-much-of-our-rust-already-runs-in-the-browser)
- [Part 3: agents as first-class citizens of the Verse](#part-3-agents-as-first-class-citizens-of-the-verse)
- [Part 4: Kiln, a Rust design](#part-4-kiln-a-rust-design)
- [Part 5: what a player can do with nothing installed](#part-5-what-a-player-can-do-with-nothing-installed)
- [Part 6: security](#part-6-security)
- [Part 7: how it shows up in the Verse](#part-7-how-it-shows-up-in-the-verse)
- [Part 8: phases, effort, and risks](#part-8-phases-effort-and-risks)
- [Issue-ready steps](#issue-ready-steps)
- [Sources](#sources)

## Verdict

- **Feasible, and most of the hard parts are already ours.** The Grid
  already runs our Rust in the browser: `everglade-web` draws the world,
  `verse-net` speaks to the relay over the browser's WebSocket, `nostr`
  signs and encrypts, and `spark-wallet-web` holds a Spark wallet. A
  browser agent is mostly a new arrangement of crates that already compile
  to `wasm32-unknown-unknown`.
- **The agent is the cheap part; the tools are the expensive part.** An
  agent that thinks on Pylons and acts in the Verse (moves, chats, builds,
  asks Pylons for work) needs no operating-system layer at all. An agent
  that edits and runs code needs a filesystem, a process model, and real
  tools. That's what Wanix and Apptron show how to build.
- **Wanix is the right reference, not a dependency.** It's MIT-licensed Go
  plus a TypeScript host layer. We can't ship it (no TypeScript, and a Go
  runtime in the page), but its ideas port cleanly to Rust: everything is a
  file, per-task namespaces, binds and unions, `#web/opfs` persistence,
  and synchronous syscalls from worker tasks through a shared buffer.
- **Apptron shows the ceiling and the cost.** It boots Alpine Linux in v86
  inside Wanix, syncs the project to Cloudflare R2, and publishes a folder
  as a site. It works, but every native tool runs under 32-bit x86
  emulation, and networking and public endpoints need a server. It also
  has no license file, so we read it only for ideas.
- **No vendor key.** fx's library calls models through Vercel's AI Gateway
  with `AI_GATEWAY_API_KEY`. A Kiln agent reaches models through Pylons:
  NIP-CJ jobs over `wss://relay.openagents.com`, routed by model as
  `pylon ask --model` and the website's `pylon_door` do, with the hosted
  door as the fallback. The page holds no provider key.
- **Each agent is its own Nostr key, owned by the player.** NIP-OA's `auth`
  tag already proves "authorized by this player", and we already use it
  for the browser's Verse key (`crates/openagents-web/src/verse_key.rs`).
  NIP-OA can't express spend caps, several kinds, or revocation, so the
  design adds a mandate the agent carries and three places that enforce
  it: the relay, the Pylon, and the wallet.
- **Compiling Rust in the browser exists, but it isn't a product yet.**
  Rubrc runs `rustc`, Cargo, and LLVM as WebAssembly in a worker, without
  external crates or procedural macros. Plan on remote builds for real
  Rust projects, on a Pylon, an agent computer, or `coder-worker`.
- **First demo, about two agent-weeks:** a player opens the Grid, spawns an
  agent, and within seconds that agent has its own key, answers through a
  DeepSeek V4.1 group or Flint on a Pylon, says so in Grid chat, and walks
  to a place the player names. No filesystem yet.

## Part 1: what exists

### Wanix

#### Facts

- **Identity.** Wanix describes itself as "a compute toolkit for the web
  with the depth of Plan 9 from Bell Labs" (GitHub description) and as an
  embeddable runtime that brings a Unix-like environment to the browser
  (`README.md`). It's by Jeff Lindsay (progrium) under tractor.dev, created
  2023-10-15, with about 850 stars (`gh api repos/tractordev/wanix`).
- **License.** MIT, copyright 2023 Jeff Lindsay (`LICENSE`).
- **Language.** Go is the kernel: about 75,000 lines of Go, 20,000 of
  JavaScript, and 2,400 of TypeScript, not counting the site (line counts
  over the clone). The module needs Go 1.26 and patched forks of `p9` and
  `golang.org/x/sys` (`go.mod`). Small tools build with TinyGo
  (`Dockerfile`).
- **Status.** Version `0.4.0-rc2` on npm, with active commits through
  2026-10-20 (`git log`, `README.md`).
- **Shape.** A page declares a system in HTML custom elements:
  `<wanix-namespace>`, `<wanix-bind>`, `<wanix-task>`, `<wanix-vm>`,
  `<wanix-term>` (xterm.js), and `<wanix-workbench>` (a VS Code web
  workbench) (`README.md`, `elements/`). The runtime is `wanix.min.js`
  plus `wanix.wasm` and needs no backend.

#### Filesystem, processes, and namespaces

- **Everything is a file.** Processes, terminals, VMs, browser APIs, and
  storage all appear in one namespace (`README.md`). Kernel devices use
  Plan 9's `#` names: `#task`, `#term`, `#vm`, `#ramfs`, `#pipe`,
  `#signal`, `#web`, and `#wanix` (`README.md`, "System Namespace").
- **Binds and unions.** `fs/vfs/vfs.go` implements a namespace with bind
  modes `BindAfter`, `BindReplace`, and `BindBefore`, so later binds layer
  over earlier ones. A bind can mount another namespace path, inline file
  text, a fetched URL, or a tar archive (`README.md`, "Bind types";
  `fs/tarfs`, `fs/httpfs`).
- **Filesystem kit.** `fs/` holds a copy-on-write overlay (`fs/cowfs`), an
  in-memory filesystem (`fs/memfs`), a sync layer (`fs/syncfs`), a 9P
  client and server kit (`fs/p9kit`), and an R2 object-storage filesystem
  with a written storage spec (`fs/r2fs/SPEC.md`).
- **Tasks are processes.** `task.go` defines a `Task` with a driver, a
  parent, its own namespace (`*vfs.NS`), arguments, environment, working
  directory, and file descriptors. A `TaskDriver` has two methods, `Check`
  and `Start`. Each task gets its own namespace, inheriting its parent's by
  default (`README.md`, `<wanix-task>`).
- **Plan 9 style control files.** Resources are allocated through files: a
  TCP service has `new` and per-connection `ctl` and `data` files
  (`fs/net/tcp/README.md`); Ethernet follows Plan 9's `ether(3)` with
  `clone`, `ctl`, and `data`, one frame per read or write
  (`fs/net/ether/README.md`).
- **Federation.** A namespace can be exported to another origin over
  `postMessage`, or imported over 9P on a WebSocket or an iframe
  (`README.md`, "Export and import namespaces"; `misc/ws9p`).

#### How it runs programs

- **Four task drivers:** `js` (a JavaScript worker), `gojs` (Go built with
  `GOOS=js GOARCH=wasm`), `wasi` (WASI preview 1), and `auto`, which
  detects the type (`README.md`, "Task drivers"; `wasi/driver.go`).
- **WASI through browser_wasi_shim.** The WASI worker imports
  `@bjorn3/browser_wasi_shim` and backs its files with Wanix handles
  (`wasi/fs.ts`). Each task runs in its own Web Worker
  (`web/worker/worker.go`, `wasi/driver.go`).
- **Synchronous syscalls.** A WASI program expects blocking calls, so the
  worker calls the kernel through a `SharedArrayBuffer` and blocks on
  `Atomics.wait` (`wasi/callbuffer.ts`). That requires a cross-origin
  isolated page: the development server sets `Cross-Origin-Opener-Policy:
  same-origin` and `Cross-Origin-Embedder-Policy: require-corp`
  (`cmd/wanix/serve.go`).
- **x86 Linux through v86.** `<wanix-vm>` boots Linux in the v86 emulator,
  with QEMU-style attributes, a serial console, and a guest namespace
  exported back to the host (`README.md`, `vm/`, `v86/`). The
  `wanix-linux.tgz` extra is an Alpine root filesystem built in Docker
  (`extras/Makefile`).
- **The shell.** `rc` is a small shell on `mvdan.cc/sh` that runs natively
  and in Wanix. It doesn't support pipes or redirection yet, and the plan
  is to replace it with a Plan 9 `rc` of their own (`rc/README.md`).

#### Persistence and network

- **OPFS and IndexedDB.** `#web/opfs` binds the Origin Private File System
  (`web/fsa/fsa.go`). `web/idbfs` stores files in IndexedDB with a
  metadata store and a data store, inlining files under 4 KB
  (`web/idbfs/DESIGN.txt`). Service workers and Cache Storage are also
  exposed (`web/sw`, `web/caches`).
- **No sockets in the page.** In the browser build, the TCP service's
  connections return "not supported" (`fs/net/tcp/conn_js.go`).
- **A VM's network goes through a server.** `wanix serve` and
  `wanix host netfs` run a user-space network stack (`go-netstack`'s
  `vnet`, with DHCP, DNS, and NAT) and carry raw Ethernet frames to the
  page over a WebSocket (`cmd/wanix/serve.go`, `fs/net/ether/README.md`,
  `examples/vm-network.html`). Without that server, a VM has no network.

#### Portable ideas and Go-specific parts

| Part | Portable to Rust? | Notes |
| --- | --- | --- |
| Everything-is-a-file namespace with `#` devices | **Yes** | A trait for a file service and a namespace of binds. |
| Bind modes and union directories | **Yes** | `fs/vfs/vfs.go` is the reference. |
| Copy-on-write layers over tar archives | **Yes** | Our content-addressed manifests make layers natural. |
| Per-task namespaces | **Yes** | The capability boundary for a tool. |
| Control-file APIs (`new`, `ctl`, `data`) | **Yes** | A good shape for agent-visible devices such as `/net/pylon`. |
| 9P export and import | **Yes, later** | Our transport is Nostr and NIP-REACH; 9P semantics can ride on it. |
| Synchronous syscalls through `SharedArrayBuffer` | **Yes** | Same browser constraint for any WASI host. |
| `gojs` driver and `wasm_exec.js` | **No** | Go's JavaScript bridge. |
| The TypeScript and JavaScript elements and WASI host | **No** | We'd write the host in Rust with `wasm-bindgen`. |
| `mvdan.cc/sh` shell | **No** | Pick or write a Rust shell. |
| v86 integration | **Optional** | v86 is JavaScript and WebAssembly; see [the limits](#limits-to-be-honest-about). |

### Apptron

#### Facts

- **Identity.** "Local-first development platform", homepage apptron.dev,
  from the same group (tractordev), about 1,200 stars, last commit
  2026-07-13 (`README.md`, `gh api repos/tractordev/apptron`).
- **License.** The repository has **no license file**, and GitHub's
  license API returns 404. Without a license, the default is that no one
  else may copy or reuse the code. We read it only for ideas and copy
  nothing.
- **Built on Wanix.** `boot.go` is a Go WebAssembly program that imports
  Wanix packages (`cowfs`, `syncfs`, `idbfs`, `p9kit`, `tarfs`, `vm`) and
  composes the system. `go.mod` requires `tractor.dev/wanix`.
- **What a user gets.** A "project environment": Alpine Linux with a
  custom kernel in v86, through Wanix, with a VS Code-based editor.
  Packages install with `apk`; `make`, `git`, and `esbuild` come
  preinstalled. 32-bit x86 software runs (`README.md`).
- **Persistence.** The environment resets on each page load, like a Docker
  image. The project, home, and public directories persist in browser
  storage and sync to the cloud. An `.envbuild` script in the project
  rebuilds the environment (`README.md`; `system/bin/rebuild`).
  `boot.go` layers a copy-on-write scratch over the base, stores data in
  IndexedDB (`idbfs.New("apptron-rev1")`), and syncs `home`, `project`, and
  `public` to remote storage with `syncfs` every 2 to 5 seconds.
- **The server side.** A Cloudflare Worker in TypeScript
  (`worker/src/worker.ts`) serves assets, sign-in (Hanko), and an R2
  filesystem API (`worker/src/r2fs.ts`), and forwards `/x/net` to a
  Cloudflare Container that runs the Go virtual network
  (`worker/cmd/worker/main.go`, `wrangler.toml`).
- **Virtual network with public endpoints.** Each page load gets a
  "session IP" from a virtual DHCP server. Session IPs reach each other
  across tabs and devices. A program that listens on a TCP port gets a
  public HTTPS endpoint, proxied while it runs, like ngrok (`README.md`,
  "Virtual Network").
- **Publishing.** `publish DIR` writes one line to the system's control
  file asking it to sync that folder into `/public`, which the Worker
  serves on `*.aptn.pub` (`system/bin/publish`, `worker/src/public.ts`).
- **Go is first-class, and slow.** Go installs with `apk`, but Apptron
  ships Go 1.25 with a precompiled standard library because "Go runs
  significantly slower in the browser" (`README.md`, "Using Go").

#### What Apptron shows

Inference:

- **A full Linux in the page is possible and useful,** but every tool pays
  x86 emulation on top of WebAssembly. That's fine for `git` and `make`,
  and slow for compilers.
- **Local-first still needs a server** for the network, sign-in, sync, and
  public endpoints. Apptron's server is small, but it exists.
- **"Publish a folder" is the right user verb.** It maps directly onto our
  tenant manifests.
- **Environment as a build script** (`.envbuild`) keeps the reset-on-load
  base reproducible. We'd express it as a content-addressed layer instead.

### fx and libfx

`docs/research/fx/README.md` covers fx in depth. The parts that matter
here:

- libfx is fx's Zig agent core built as WebAssembly and as a Node.js
  addon. It stores each prompt before accepting it and each step as it
  runs, so a turn survives a crash or a redeploy. Tools declare whether
  rerunning them is safe (`sdk/README.md` in vercel-labs/fx).
- **Inference goes through Vercel.** libfx's documented setup sets
  `AI_GATEWAY_API_KEY`, and model calls go through Vercel's AI Gateway; on
  Vercel it uses the deployment's token instead. It can also use a
  person's ChatGPT or Grok plan, or any Chat Completions server
  (`sdk/README.md`, "Providers").
- In a browser, libfx keeps sessions in memory only (`sdk/README.md`).

We take the durable turn and the declared tool effects. We don't take the
key: a Kiln agent pays Pylons, not a gateway account.

### Other prior art

- **WebContainers (StackBlitz).** Runs Node.js and npm in the browser on
  WebAssembly and service workers. Needs cross-origin isolation.
  Proprietary; production commercial use requires a license
  ([webcontainers.io](https://webcontainers.io/),
  [enterprise terms](https://webcontainers.io/enterprise)).
- **v86.** An x86 emulator that translates machine code to WebAssembly at
  run time and boots Linux, BSD, and Windows guests. BSD-2-Clause
  ([copy/v86](https://github.com/copy/v86)). Wanix and Apptron use it.
- **CheerpX (Leaning Technologies).** An x86 virtual machine in
  WebAssembly with a JIT, behind WebVM, which runs Debian in the browser.
  Commercial use needs a license
  ([cheerpx.io](https://cheerpx.io/), [WebVM](https://webvm.io/)).
- **Pyodide and JupyterLite.** CPython built with Emscripten (MPL-2.0),
  and a Jupyter front end that runs its kernels in the browser
  (BSD-3-Clause) ([pyodide.org](https://pyodide.org/),
  [JupyterLite](https://jupyterlite.readthedocs.io/)). Proof that a full
  language runtime with packages works in a page.
- **wasmer-js and WASIX.** Wasmer's JavaScript SDK runs WASIX programs in
  the browser. WASIX extends WASI preview 1 with threads, sockets,
  `fork`-style process calls, and more
  ([wasix.org](https://wasix.org/), [wasmer-js](https://github.com/wasmerio/wasmer-js)).
  Wasmer ran clang in the browser this way in 2024, with a package of
  about 100 MB ([Wasmer blog](https://wasmer.io/posts/clang-in-browser)).
- **browser_wasi_shim.** A small WASI preview 1 implementation for
  browsers, MIT or Apache-2.0
  ([bjorn3/browser_wasi_shim](https://github.com/bjorn3/browser_wasi_shim)).
  Wanix uses it.
- **Rust's WASI targets.** `wasm32-wasip1` is WASI preview 1;
  `wasm32-wasip1-threads` adds threads and is marked experimental;
  `wasm32-wasip2` emits components
  ([platform support](https://doc.rust-lang.org/rustc/platform-support/wasm32-wasip1-threads.html)).
  All three are installed on the owner's Mac toolchain.
- **The component model.** Components have typed WIT interfaces. Browsers
  don't run components directly; `jco` transpiles a component into core
  WebAssembly plus JavaScript glue
  ([component model book](https://component-model.bytecodealliance.org/),
  [jco](https://github.com/bytecodealliance/jco)). Inference: in the
  page, we'd run preview 1 modules, or components through a Rust host
  that implements the WIT imports.
- **OPFS.** The Origin Private File System gives each origin a private
  file tree. `FileSystemSyncAccessHandle` offers fast synchronous reads
  and writes, but only inside a dedicated worker
  ([MDN](https://developer.mozilla.org/en-US/docs/Web/API/File_System_API/Origin_private_file_system)).
  Storage is subject to the browser's quota and eviction unless the page
  is granted persistent storage.
- **isomorphic-git.** Git in JavaScript for Node.js and browsers, MIT
  ([isomorphic-git.org](https://isomorphic-git.org/)). Network operations
  from a browser need a CORS proxy.
- **gitoxide.** Its CI builds a list of plumbing crates for
  `wasm32-unknown-unknown`, `wasm32-wasip1`, and `wasm32-wasip2`:
  `gix-object`, `gix-pack`, `gix-index`, `gix-hash`, `gix-revision`,
  `gix-traverse`, and others (`.github/workflows/ci.yml` in
  GitoxideLabs/gitoxide, job `wasm`). The top-level `gix` crate and its
  network transports aren't in that list.
- **In-browser compilers.**
  - **rustc:** Rubrc runs WebAssembly builds of `rustc`, Cargo, clang, and
    LLVM in a browser worker, with rust-analyzer in the editor. It needs
    cross-origin isolation, emits `wasm32-wasip1` and
    `x86_64-unknown-linux-musl` binaries, doesn't support external crates
    or procedural macros, runs one compile at a time, and calls itself
    pre-release and not production-ready. It has no license on GitHub
    ([oligamiq/rubrc](https://github.com/oligamiq/rubrc),
    [demo](https://rubrc.pages.dev/)).
  - **clang:** Wasmer's SDK, above; and Ben Smith's earlier
    [wasm-clang](https://github.com/binji/wasm-clang).

## Part 2: what we already have

- **AGENTS.md rules.** Product code is Rust. No TypeScript. Rust Native is
  the card and sheet layer on every surface. Domain crates are linked
  into apps, never reimplemented.
- **Microcoder** (`crates/microcoder-loop`, `docs/coder/guides/microcoder.md`).
  Each step, Jev judges the state, one structured model call returns the
  next commands as JSON, and the host runs them. Commands run through the
  `Env` trait (`crates/microcoder-loop/src/env.rs`): `run` a bash script
  with a deadline, `read` a file, `stopped`, `steering`, and hooks. The
  `Env` trait is the seam a browser host plugs into.
- **coder-boundary** (`crates/coder-boundary`). A write boundary through
  `sandbox-exec` on macOS, `bwrap` on Linux, and an AppContainer on
  Windows. Everywhere else it refuses to exist (`Error::Unsupported`), by
  design. In the browser, the page's sandbox and Kiln's namespace take its
  place.
- **The Verse in the browser.** `crates/everglade-web` is a `cdylib` over
  Verse's `web` feature that draws Everglade and the shared Grid with
  WebGPU or WebGL2; the module is about 25 MB, 7.5 MB with brotli
  (`crates/everglade-web/README.md`). `crates/verse-net` speaks NIP-MV to
  the relay through `web-sys::WebSocket` in the browser
  (`crates/verse-net/Cargo.toml`, `src/net/browser.rs`).
- **The Grid at `/`.** The Grid homepage moved to `/` (#12413), and its
  terminal chat is the GPUI web client in `apps/openagents/web/lander-chat`
  and `apps/openagents/web/terminal`. A turn from it reaches
  `crates/openagents-web/src/pylon_door.rs`, which runs the Pylon router
  first and falls back to the hosted chat.
- **Pylon model routing.** A terminal turn with `X-OpenAgents-Pylon-Model:
  deepseek-v4.1-flash` goes only to a ready DeepSeek V4.1 Flash model
  group, under `pylon.deadline.v1` with a 600-second budget
  (`docs/compute/pylon.md`, "The pylon router"). Flint now has its own pick in the
  Grid chat (`apps/openagents/web/lander-chat/src/model.rs`,
  `FLINT_MODEL`; #12425). `pylon ask --model` sets `router::Settings::want`
  so only pylons serving exactly that model take the job
  (`crates/pylon/src/cli.rs`).
- **The browser Verse key.** `crates/openagents-web/src/verse_key.rs`
  (#12313): the page keeps its own key, because the Grid signs presence
  about ten times a second; the person's phone or computer signs a NIP-OA
  `auth` tag for it once, and readers show the avatar as the person's.
  This is the exact pattern a spawned agent needs.
- **Agents with their own keys.** Alice has her own key, a NIP-OA
  attestation renewed yearly, a `kind:0` profile, and engrams
  (`docs/verse/agent-identity-and-engrams.md`, "Identity"). Published
  agents follow NIP-AGENT (`nips/openagents/NIP-AGENT.md`, "Public
  agents").
- **Web deploys in seconds.** `docs/audits/2026-10-21-web-deploys-in-seconds.md`
  designs tenants with content-addressed manifests, `staging` and `live`
  channels, signed Nostr pointer events, hosted games as Grid portals, and
  live code swaps at safe points. Kiln's "publish" is this design's first
  outside tenant. (Not to be confused with `crates/live-host`, which is
  the NIP-LIVE media controller.)
- **Shared sessions.** Live sessions are Loro documents in rooms on
  `coder-sync` (`crates/coder-sync`, AGENTS.md "Sessions").

### How much of our Rust already runs in the browser

Facts come from three sources: the web image's build list
(`crates/openagents-web/Dockerfile`, the `wasm32-unknown-unknown` stages),
`Cargo.toml` files with `wasm32` targets, and `cargo check --lib --target
wasm32-unknown-unknown` run on 2026-10-21 in the shared target. Each check
was run beside `verse-net`, so getrandom's `wasm_js` feature unifies as it
does in a real browser build; alone, `nostr`, `xp-ledger`, and
`openagents-x402` stop on that feature, not on their own code.

| Crate | Status | Evidence and what blocks it |
| --- | --- | --- |
| `everglade-web` (Verse, Grid) | **Shipped** | Built in the web image; `verse-web` profile. |
| `verse`, `verse-net`, `verse-core`, zone crates | **Shipped** | Linked by `everglade-web`; `verse-net` has a browser WebSocket link. |
| `nostr` (events, NIP-19, NIP-44, `pylon`, `x402`, `cj_conversation`) | **Shipped** | Linked by `everglade-web`; `cargo check` passes. Its `pylon` module is pure: verified beacons, receipts, deadlines. |
| `spark-wallet` and `spark-wallet-web` | **Shipped, opt-in** | Breez Spark SDK in the browser (#12307, #12374); `cargo check` passes. |
| `oa-vault-web`, `coder-chat-web`, `coder-browser-web`, `coder-components-web`, `coder-cloud-web` | **Shipped** | Built in the web image. `coder-browser-web` already drives an enrolled host terminal from a page. |
| `pylons-web`, `lander5-web`, `layertree-web`, `att-web`, and others | **Shipped** | Built in the web image. |
| GPUI web chat and terminal (`apps/openagents/web/*`) | **Shipped** | Its own workspace, built in the image's app stage. |
| `world-tree`, `townsfolk`, `town-clock`, `physics`, `memory-stream` | **Builds** | `wasm32` in `Cargo.toml`; `memory-stream` checks clean. |
| `xp-ledger`, `receipts`, `atif` | **Builds** | `cargo check` passes. `atif` and `memory-stream` also pass for `wasm32-wasip2`. |
| `coder-boundary` | **Builds, refuses** | Compiles, and returns `Error::Unsupported` at run time, by design. |
| `jev`, and `openagents-x402` through `model-access` → `jev` | **Close** | About 10 errors, all in `jev`: reqwest's browser client has no `redirect` module, no `chunk()`, and futures that aren't `Send` (`crates/jev/src/config.rs:283`, `src/client.rs:863`, `:1007`). A `cfg` split for the browser client fixes it. |
| `pylon` (client and router) | **Needs a split** | 51 errors: Tokio `net`, `process`, and `rt-multi-thread` through mio, getrandom 0.2 without `js`, `reqwest`, file I/O, `coder-lease`, `stage-link`. The buyer logic (`client.rs`, `router.rs`) is the part to lift into a sans-I/O crate. |
| `microcoder-loop` | **Needs a split** | 49 errors: Tokio `process`, `acp-client`, `codex-transport`, `knowledge`, `coder-lease`, file-backed capacity book. The loop in `run.rs`, the state, the reply tap (`reply.rs`), and the `Env` trait are portable. |
| `wallet` (LDK) | **Won't go** | `ldk-node` with bundled SQLite and a node that must stay online. Use Spark in the page. |
| `coder-sync` | **Server stays native** | The Loro document side can go to the browser (`loro` builds for wasm); the room server is native. |
| `coder-pty`, `coder-ssh`, `coder-service`, `supervise`, `coder-compositor` | **Won't go** | Operating-system processes, PTYs, and services. |

Inference: the browser already holds keys, signs, encrypts with NIP-44,
talks to the relay, verifies Pylon beacons, and holds a wallet. The two
missing pieces for a browser agent are a browser Pylon buyer and a
browser Microcoder loop, and both are splits of code we have, not new
protocols.

## Part 3: agents as first-class citizens of the Verse

The target: a player opens the Grid, spawns an agent, and within seconds
that agent is talking to Pylons and acting in the world, with nothing
installed.

### 3.1 Inference with no vendor key

fx's library needs an AI Gateway key or a Vercel deployment. A Kiln agent
needs neither. It buys inference the way the website's `pylon_door` does,
from the page:

1. **Read beacons.** Subscribe on `wss://relay.openagents.com` to NIP-PYLON
   beacons (`kind 30200`) and verify them with `nostr::pylon`, which is
   pure and already compiles for the browser.
2. **Route by model.** Apply the router's rules: a request that names a
   model (`Want::Model("deepseek-v4.1-flash")` or Flint) goes only to a
   pylon or ready model group serving exactly that model; a request that
   names none ranks live pylons in code and may ask one Clef decision
   (`crates/pylon/src/router.rs`).
3. **Send one encrypted job.** A NIP-CJ request, NIP-44 encrypted to the
   pylon, signed by the agent's own key, under `pylon.deadline.v1` for a
   V4.1 group (600 seconds) or the 90-second profile otherwise.
4. **Wait, fail over, record.** Wait for the answer on the same relay
   connection, try the next candidate on refusal or timeout, and keep a
   buyer receipt (`kind 3201`).
5. **Fall back to the hosted door.** When no pylon answers, call the
   website's chat door with the player's session. The page still holds no
   provider key; the server does.

What it takes in Rust:

- **`pylon-buyer` (new crate, sans-I/O).** Lift `client.rs`'s job building,
  answer matching, and deadline rules and `router.rs`'s candidate ranking
  out of `crates/pylon`, behind two traits: a relay connection and a
  clock. Native `pylon` keeps using it with Tokio; the browser uses
  `verse-net`'s WebSocket link.
- **Getrandom and reqwest hygiene.** getrandom 0.2's `js` feature, as
  `spark-wallet` does, and no `reqwest` in the buyer path.
- **A free lane first.** Pylons serve free conversation jobs today, and the
  terminal's jobs need pylons running `--allow-any` (`docs/compute/pylon.md`).
  Paid jobs come in [3.3](#33-what-agents-can-do).

Inference: the work is a refactor of `crates/pylon` of about three to
five agent-days, because the protocol pieces already compile.

### 3.2 Each agent is its own Nostr identity

A spawned agent gets a fresh keypair in the page, and the player's key
authorizes it. The agent is the player's adjunct: it signs its own events,
and readers can verify which player stands behind it.

#### What our NIPs already say

| NIP | What it covers | Fit for a spawned agent |
| --- | --- | --- |
| **NIP-OA** Owner Attestation (`nips/block/NIP-OA.md`) | An `auth` tag `["auth", owner, conditions, sig]`. The owner signs `SHA256("nostr:agent-auth:" + agent pubkey + ":" + conditions)`. Conditions are `kind=N`, `created_at<T`, and `created_at>T`, joined by `&`. The event stays authored by the agent. | **The core.** Proves "authorized by this player". We already use it for the browser Verse key and for Alice. |
| **NIP-AA** Agent Authentication (`nips/block/NIP-AA.md`) | An agent presents its NIP-OA tag in its NIP-42 AUTH (`kind:22242`), and a relay grants "virtual membership" while the owner is a member. Rate limits SHOULD aggregate by owner. | **Relay admission.** Revocation section: removing the owner cuts every agent at its next connection. |
| **NIP-AP** Agent Personas (`nips/block/NIP-AP.md`) | `kind:30175` persona blueprints (name, avatar, prompt, model, runtime); `30177` instance state; `30178` team catalog. | **Spawn templates.** A persona is the "kind of agent" a player spawns. |
| **NIP-AE** Agent Engrams (`nips/block/NIP-AE.md`) | `kind:30174` memory, NIP-44 encrypted between agent and owner; the owner reads everything. | **Memory** for a persistent agent. |
| **NIP-AM**, **NIP-AO** | `kind:44200` per-turn usage and cost, and `kind 24200` live telemetry, encrypted to the owner. | **The player's view** of what the agent spent and did. |
| **NIP-IA** Identity Archival (`nips/block/NIP-IA.md`) | Archive requests (`kind:9035`), relay deltas, and a snapshot; owner-of-agent requests proven with NIP-OA. | **Retiring** a spent agent's key from active lists. |
| **NIP-PMA** (`nips/block/NIP-PMA.md`) | `kind 30179`, an owner-authored aggregate for one managed agent. A reservation: relays MUST reject it for now. | Not yet. |
| **NIP-26** Delegated Event Signing (`nips/official/26.md`) | A `delegation` tag that attributes events to the delegator. Marked `unrecommended`. | **Don't use.** NIP-OA explicitly refuses NIP-26's "the owner is the author" meaning. |
| **NIP-AGENT** (`nips/openagents/NIP-AGENT.md`) | Saved and public agents: an agent key of its own, NIP-OA with `created_at<T` at most a year, `kind:0` with `bot: true`, `kind 30187` definitions, `kind 3204` receipts. | **A saved agent.** A spawned agent the player keeps becomes one. |
| **NIP-SOV** (`nips/openagents/NIP-SOV.md`) | Sovereign agents: custody policies, NIP-46 remote signing, guardian quorum for exact actions, treasury policies with per-operation and aggregate ceilings. `crates/sov-custody`, `sov-lifecycle`, `sov-treasury`. Allocates no kinds. | **The long-lived tier.** Too heavy for a ten-second spawn; the vocabulary (authority, controller, custodian, allowance) is the right one. |
| **NIP-HOST** (`nips/openagents/NIP-HOST.md`) | Host-signed device grants with seven closed rights and revocation epochs. | **The model for revocable grants** if an agent reaches a player's computer. |

#### What NIP-OA can't do

- **One kind per tag.** Kind clauses are conjunctive, so `kind=1&kind=7`
  authorizes nothing; NIP-AA says to use one `kind=` per credential
  (`nips/block/NIP-AA.md`, "Kind Conditions"). A Verse agent emits many
  kinds (presence `33301`, pose `23300`, chat, CJ `25920`-family requests).
- **No spend caps.** The conditions grammar has three clause types and
  nothing else.
- **No revocation.** "The owner cannot unilaterally revoke a previously
  issued `auth` tag" (`nips/block/NIP-AA.md`, "Revocation Semantics").
  `created_at` is chosen by the agent, so expiry holds only where a relay
  checks AUTH freshness.

#### Proposal: a tag plus a mandate, enforced in three places

1. **Spawn.** The page makes a key in memory. It shows the player a
   one-screen mandate: name, persona, model, the allowed actions, a spend
   cap, and an expiry (default: one hour, renewable).
2. **Authorize.** The player's key signs two things:
   - a NIP-OA `auth` tag with conditions `created_at<EXPIRY` only, so one
     tag covers every kind the agent emits; and
   - a **mandate**, a small owner-signed document (`openagents.agent-mandate.v1`)
     that binds the agent pubkey, the persona, the allowed action set
     (closed words such as `verse.move`, `verse.chat`, `verse.build`,
     `pylon.ask`, `pylon.pay`, `wallet.receive`), the spend cap in msat,
     the expiry, and a revocation epoch. The agent publishes its digest in
     its `kind:0` and carries the mandate to anyone who asks.

   Who signs: the page never holds the player's long-term secret. As
   `verse_key.rs` does today, the player's phone or desktop app signs, or a
   NIP-07 extension or NIP-46 signer signs once per spawn. Inference: to
   make spawn take seconds, the player signs one **spawner mandate** for
   the browser's Verse key at sign-in ("this page may spawn up to 3 agents,
   each capped at 1,000 sats, for 8 hours"), and the Verse key then
   co-signs each agent's tag. That needs a NIP-OA chain rule (owner → page
   key → agent), which is a small extension we'd write down.
3. **Enforce.**
   - **Relay.** `relay.openagents.com` admits the agent under NIP-AA,
     checks the mandate's action set per event kind (NIP-AA's optional
     per-event enforcement, generalized to a set), aggregates rate limits
     by owner, and refuses after expiry or a newer revocation epoch.
   - **Pylon.** A pylon serving paid jobs verifies the tag and the mandate
     and refuses a job past the mandate's spend cap, tracked per
     agent and per owner. A free job needs only the tag.
   - **Wallet.** The hard cap is money the agent doesn't have: the player
     funds a sub-wallet with the cap and no more. See
     [3.3](#33-what-agents-can-do).
4. **Revoke.** The player publishes a revocation (an owner-signed event
   that raises the epoch, or lists agent pubkeys). The relay drops the
   agent's sessions, as NIP-AA's "Owner removal" requires for immediate
   termination, and pylons refuse it at the next job. Short expiries bound
   the damage when a reader missed the revocation. A retired key is
   archived under NIP-IA.

How others verify: a player who meets the agent in the Grid sees its name
with "authorized by `<player>`", never as the player (NIP-OA, "Client
Behavior"). A pylon or relay checks the tag signature, the expiry, the
mandate signature and digest, and the current revocation epoch, all with
`nostr`, which already runs in the browser and on the server.

### 3.3 What agents can do

| Capability | How | Today |
| --- | --- | --- |
| **Ask Pylons for inference** | `pylon-buyer` in the page, free conversation jobs first | The protocol and the website's door exist; the browser buyer is the split in 3.1. |
| **Other compute jobs** | NIP-CJ execution requests (`kind 25920`) to a worker, under the mandate | Hosted eval runners already take CJ execution jobs (`crates/eval-runner`). |
| **Move and gesture** | NIP-MV presence (`33301`) and pose frames (`23300`) from the agent's key | The Grid already signs these from a page key. |
| **Chat** | Grid chat (NIP-C7, NIP-29) as the agent | `verse-net` chat. |
| **Build** | Place objects through the zone's rules, as the player's adjunct | Zone rules decide; NIP-MV entity states carry it. |
| **Trade** | Offers between players and agents, settled in sats | Needs a market flow; NIP-MKT exists for labor, not item trades. |
| **Hold a wallet** | A Spark sub-wallet funded by the player up to the cap | `spark-wallet-web` ships today. |

#### Wallet: what's realistic

- **Spark in the page: yes.** `crates/spark-wallet` builds for
  `wasm32-unknown-unknown` with Tokio's browser-safe features and
  getrandom's `wasm_js` (`crates/spark-wallet/Cargo.toml`), and
  `crates/spark-wallet-web` ships an opt-in browser wallet (#12307). An
  agent sub-wallet is a second Spark wallet with its own seed in page
  memory, funded by a payment from the player's wallet. The cap is its
  balance.
- **LDK in the page: no.** `crates/wallet` depends on `ldk-node` and
  bundled SQLite, and a Lightning node must stay online to receive.
- **x402 to Pylons: close.** NIP-X402's native Nostr profile
  (`nostr:openagents:1`) lets a buyer pay for a job before it runs, and
  `nostr::x402` (pure, in `crates/nostr`) already compiles for the
  browser. `crates/x402` fails only through `jev` (see the table), and
  the buyer needs only the codec and verification parts. NIP-PYLON receipts
  already carry a `payment` field with `x402-exact` as a profile.
- **Seed custody.** A sub-wallet seed in page memory is lost when the tab
  closes. For a one-hour agent that's acceptable if the remainder sweeps
  back to the player on despawn; a persistent agent's seed belongs in the
  player's vault (`oa-vault-web` keeps its master key inside the module).
- **Owner steps.** Real-money checks stay owner-only (`NEEDS_OWNER.md`).

## Part 4: Kiln, a Rust design

Kiln is a browser OS layer in Rust compiled to WebAssembly, in the shape
Wanix proved, plus the agent loop that drives it.

```text
 page (main thread)                 workers
 ┌──────────────────────┐   ┌──────────────────────────────┐
 │ everglade-web (Grid) │   │ kiln-kernel (one worker)     │
 │ Rust Native cards    │◄─►│  namespace, binds, OPFS      │
 │ agent panel          │   │  task table, pipes, devices  │
 └─────────┬────────────┘   └───────┬──────────────────────┘
           │ relay (verse-net)      │ SharedArrayBuffer syscalls
           ▼                        ▼
 wss://relay.openagents.com   task workers: WASI tools
   Pylons, Grid, mandates     (shell, coreutils, rg, gix, fmt)
```

### 4.1 Namespace and files: `kiln-vfs`

- **A `FileService` trait** (open, read, write, stat, readdir, remove,
  rename) and a **namespace** of binds with `before`, `after`, and
  `replace`, following `fs/vfs/vfs.go`'s semantics. Pure Rust; it builds
  and tests natively.
- **Devices under `#`:**
  - `#ram`: in memory.
  - `#opfs`: persistent, per player.
  - `#layer/<sha256>`: a read-only, content-addressed tar layer from the
    manifest store.
  - `#cow`: a copy-on-write overlay.
  - `#task`: the task table.
  - `#pipe`.
  - `#net`: the capability proxy; see [4.4](#44-network-a-capability-proxy).
  - `#pylon`: model jobs as files; see [4.5](#45-the-agent-loop).
  - `#verse`: the agent's world actions as files.
- **Control files** in Plan 9 style: `#pylon/new` returns a job ID,
  `#pylon/<id>/ctl` takes `model deepseek-v4.1-flash`, `data` carries the
  prompt and the answer. A tool or a shell script can then use a model
  without a library, the way Wanix exposes TCP.
- **Why not plain WASI preopens only:** the agent and tools see one tree,
  and the host decides per task what's bound. A tool's namespace is its
  capability set.

### 4.2 Storage: OPFS

- The kernel runs in a dedicated worker, so it can use
  `FileSystemSyncAccessHandle` for fast synchronous I/O.
- **Per player, per workspace:** `opfs:/kiln/<npub>/<workspace>/`. A
  workspace is an overlay over content-addressed layers: base tools, a
  project template, then the player's files.
- **Persistence:** call `navigator.storage.persist()` and show what's at
  risk if the browser declines. Sync the workspace's changed files as
  content-addressed objects so it survives a cleared browser, a second
  device, or a teammate (see [Part 7](#part-7-how-it-shows-up-in-the-verse)).
  IndexedDB is the fallback where OPFS sync handles are missing.

### 4.3 Processes: WASI tools in workers

- **`kiln-wasi`, a WASI preview 1 host in Rust.** Each task runs in its own
  worker with its own `WebAssembly.Memory`. Syscalls go to the kernel
  through a `SharedArrayBuffer` and `Atomics.wait`, as Wanix does
  (`wasi/callbuffer.ts`), written in Rust with `wasm-bindgen`. This needs
  a cross-origin isolated page (COOP and COEP headers) on the Kiln route.
- **Components later.** `wasm32-wasip2` tools need a WIT host. Implement
  the `wasi:filesystem`, `wasi:cli`, and `wasi:io` imports in the same
  Rust host when a tool needs them; until then, prefer preview 1 builds.
- **The tool set, in Rust where possible:**
  - **Shell:** a small Rust shell with pipes and redirection, our own
    (inference: Nushell's engine is too large for the first set).
  - **Coreutils:** uutils coreutils built for WASI (inference: it has WASI
    builds of most utilities; to be checked).
  - **Search:** ripgrep (inference: single-threaded on `wasm32-wasip1`).
  - **Git:** gitoxide's plumbing crates (objects, packs, index,
    traversal) build for WASI per its CI. Clone and push go through the
    network proxy; checkout and commit need code of ours on top, or
    isomorphic-git's design reimplemented in Rust. Inference: start with
    "import a repository as a tar layer" and "export a patch".
  - **Formatter:** `rustfmt` and `prettyplease` for Rust, and formatters
    that already compile to WebAssembly (dprint's plugins are WebAssembly
    modules) for other languages.
  - **Runners:** the player's own program, as a WASI binary or a
    WebAssembly module with a canvas.
- **Limits per task:** memory cap, wall time, and output cap, as
  `crates/supervise` does natively, enforced by the kernel (it can
  terminate a worker).

### 4.4 Network: a capability proxy

- Tools get no network by default. A tool's namespace may bind
  `#net/fetch`, which allows only the origins the mandate names.
- **Fetch from the page** for CORS-friendly hosts. **A relay-side or
  server proxy** for the rest (git hosts, package registries), keyed to
  the agent's mandate, with per-owner quotas. This is Apptron's `/x/net`
  idea, without a VM network: HTTP requests, not raw Ethernet.
- **Nostr is native.** The agent's relay connection belongs to the
  kernel, not to tools.
- **Secrets never enter the page.** A request that needs a credential
  (a GitHub token, an API key) goes to the proxy with a reference; the
  proxy holds the secret and adds it. See [Part 6](#part-6-security).

### 4.5 The agent loop

- **Port Microcoder, don't write a new loop.** Split
  `crates/microcoder-loop` into a sans-I/O core (state, prompt building,
  the step, the reply tap, the `Env` trait, effects) and its native
  shell (Tokio processes, Codex and Claude logins, the capacity book).
  The browser build is the core plus:
  - `Env` for Kiln: `run` starts a shell task in the kernel; `read` reads
    the namespace.
  - A model door for Pylons (`pylon-buyer`), and the hosted door as
    fallback.
  - Jev through the Decisions API, after `jev`'s browser fixes; until
    then, skip the Jev step in the browser.
- **Durable turns.** Use the journal from the fx note's "Our version:
  durable turns": every step appended before its tools run, with declared
  effects (`Read`, `Idempotent`, `Write`). In the browser, the journal
  lives in OPFS, so a reload or a closed tab resumes the turn instead of
  repeating writes.
- **Where the model runs:** Pylons by default (DeepSeek V4.1 groups for
  hard steps, Flint for code, ordinary pylons for small steps), and the
  hosted door when none answers.
- **Where tools run:** in the browser sandbox. A step that needs native
  tools hands off; see [the limits](#limits-to-be-honest-about).

### 4.6 Where Apptron's ideas apply

| Apptron idea | In Kiln |
| --- | --- |
| Environment resets each load; only project, home, and public persist | The base tools are a read-only content-addressed layer; only the workspace overlay persists. |
| `.envbuild` script | A workspace manifest that lists layers by hash. |
| `syncfs` to R2 every few seconds | Changed files as content-addressed objects in the tenant store, synced on a timer and on publish. |
| `publish DIR` → `/public` → `*.aptn.pub` | `publish DIR` → a tenant manifest and a signed `staging` pointer → a Grid portal. |
| Session IPs and public endpoints for listening programs | Later. A preview served from the page's service worker covers static sites and games. |
| A Linux VM for everything | Optional, as an "advanced" tool, not the base. |

## Part 5: what a player can do with nothing installed

### What works in the page

- **Talk to an agent that acts.** Spawn, ask, watch it move, chat, build,
  and ask Pylons, with no workspace at all.
- **Edit and run code in languages whose toolchains run in WebAssembly:**
  - **Python** with Pyodide's approach. Inference: a CPython WASI build
    is the Rust-host-friendly path.
  - **Lua**, which the Voyager crate already embeds as an interpreter.
  - **JavaScript**, run by the page's own engine in a worker. This is the
    player's code, not our product code.
  - **C** with a clang WebAssembly build, at a 30 to 100 MB first
    download.
  - **Rust, small programs only,** with Rubrc-class tooling; see below.
- **Build a site or a game:** static HTML, CSS, and assets, and games whose
  logic is WebAssembly built elsewhere or written in an interpreted
  language. Preview it in a service worker or in the world.
- **Publish it as a tenant in seconds.** `publish site/` hashes the files,
  uploads changed objects, writes a manifest, and signs a `staging`
  pointer with the player's key (or the agent's, under its mandate). The
  deploy design's tiers put copy and assets at 1 to 3 seconds
  (`docs/audits/2026-10-21-web-deploys-in-seconds.md`, "Tiers").
- **See it live in the Grid.** The tenant appears as a portal whose beacon
  names the tenant and its live channel ("The Grid as the directory").

### Limits to be honest about

- **Compiling Rust in the browser.** It exists (Rubrc) and isn't ready:
  no external crates, no procedural macros, one compile at a time, needs
  cross-origin isolation, and the toolchain is a large download. Our own
  `everglade-web` takes LTO and minutes natively. **Real Rust builds go
  to a remote sandbox.**
- **Native toolchains.** No `cargo build` of a real project, no Docker, no
  `apt`, no dynamic linking. A v86 Linux runs 32-bit x86 software under
  emulation, slowly; Apptron ships a precompiled Go standard library
  because of this.
- **Threads.** WebAssembly threads need `SharedArrayBuffer` and
  cross-origin isolation, which also restricts which third-party
  resources the page can embed. `wasm32-wasip1-threads` is experimental.
- **Memory.** A 32-bit WebAssembly memory tops out at 4 GiB, and browsers
  often allow less, especially on phones. Inference: budget 1 to 2 GiB per
  page with the Grid loaded, since `everglade-web` alone is a 25 MB module
  with its own working set.
- **Mobile browsers.** iOS Safari has OPFS and cross-origin isolation, but
  tighter memory and aggressive tab eviction. Inference: on a phone, run
  the agent and the world in the page and send code work to a remote
  sandbox by default.
- **Storage eviction.** OPFS data can be evicted without persistent
  storage, so workspaces sync to the tenant store.

### Remote sandboxes, and where they run

| Need | Where | Why |
| --- | --- | --- |
| A real `cargo build`, tests, native tools | **The player's agent computer** (their own Coder host, reached through NIP-HOST and NIP-REACH; `coder-browser-web` already drives an enrolled host terminal from a page) | Their machine, their toolchains, `coder-boundary` enforcement. |
| The same, for a player with no computer | **`coder-worker`** on our hosts, taking NIP-CJ jobs | Already the other end of the relay door. |
| Heavy builds sold as compute | **A Pylon** offering execution jobs, paid by x402 from the agent's sub-wallet | Fits NIP-PYLON and NIP-CJ; needs a sandbox profile on the pylon. |

In every case the browser sends a workspace layer by hash and gets back
an artifact by hash. The agent loop doesn't change; its `Env` routes the
command.

## Part 6: security

- **The browser sandbox is the boundary.** Tools run in workers with no
  DOM, no cookies, and no network except the bound `#net`. The kernel
  decides each task's namespace. Inference: run Kiln on its own origin,
  such as `kiln.openagents.com`, so a tool bug can't read the main site's
  storage and the cross-origin isolation headers don't constrain the
  Grid page.
- **Capability-scoped network.** `#net/fetch` allows the mandate's origins
  only; the proxy enforces the same list server-side, with per-owner
  quotas (NIP-AA's owner aggregation).
- **Secrets never enter the page.** No provider keys: inference goes
  through Pylons or the hosted door. No GitHub tokens: the proxy adds
  them. The player's long-term key stays on their phone, desktop, or
  signer; the page holds only the Verse key and agent keys, each bounded
  by an expiry and a mandate.
- **Agent permissions.** The mandate's closed action set is checked by
  the kernel before a tool or world action runs, by the relay per event,
  and by pylons per job. A model's text never widens it, which matches
  Coder's `permit` rule that execution policy lives in the host
  (AGENTS.md, `crates/coder`).
- **Money.** The sub-wallet balance is the hard cap. Pylon-side caps and
  owner-aggregated limits are defense in depth.
- **Untrusted content.** Repositories, web pages, and other players'
  shared files are data. Hardened git rules from the fx note apply to
  gitoxide use: no hooks, filters, or external programs exist in the
  page, which removes that class of risk.
- **Abuse.** A spawned agent is cheap to make, so relays count agents by
  owner, and moderation can archive an agent (NIP-IA) or take down a
  tenant's pointer (deploy design, "Quotas and moderation").

## Part 7: how it shows up in the Verse

- **Spawn.** A gesture or a hotbar slot spawns an agent beside the player:
  a small figure with its own name and "authorized by `<player>`". The
  mandate sheet is a Rust Native card.
- **The workbench.** A workbench object in the world (in the player's
  house in Everglade, or a booth in the Grid) opens the workspace:
  - a file tree;
  - an editor;
  - a terminal on the Kiln shell;
  - the agent's turn as a card;
  - a preview.

  This extends the in-world terminal
  (`docs/verse/in-world-terminal.md`) and the Agent Studio's stations
  (`docs/verse/agent-studio.md`), which already put real work at desks in
  the world.
- **A persistent workspace per player.** It lives in OPFS and syncs to the
  tenant store, so it follows the player to another device.
- **Sharing.** A workspace is a Loro document in a `coder-sync` room, with
  NIP-HOST-style grants deciding who joins. Another player can watch, edit,
  or run with the owner's grant. A shared agent acts under its own
  owner's mandate, never the guest's.
- **Publishing is a place.** A published game becomes a portal in the
  Grid. Live code swaps reach players inside it at the next safe tick
  (deploy design, "Rapid game code updates").
- **Pylons are visible.** The Grid already draws online pylons
  (`crates/pylons-web`). A job from a player's agent can draw as a beam
  from the agent to the pylon that answers it.

## Part 8: phases, effort, and risks

Estimates are in agent-days and assume one agent per lane with the shared
target warm.

| Phase | Scope | Effort |
| --- | --- | --- |
| **0. First demo: spawn an agent that talks to Pylons and acts** | Agent key in the page; NIP-OA tag signed through the `verse_key` flow; `pylon-buyer` split; free jobs to a V4.1 group or Flint, hosted-door fallback; the agent posts in Grid chat and walks to a named place. No filesystem. | 8 to 10 |
| **1. Mandates and enforcement** | Mandate document and digest in `kind:0`; spawner mandate for one-tap spawns; relay checks (NIP-AA with action sets, owner aggregation, revocation epoch); NIP-IA archive on despawn. | 8 to 12 |
| **2. Kiln kernel and tools** | `kiln-vfs`, OPFS device, `kiln-wasi` with `SharedArrayBuffer` syscalls, a Rust shell, coreutils, ripgrep, a formatter; cross-origin isolated route. | 15 to 20 |
| **3. Microcoder in the page** | Split `microcoder-loop` into core and native shell; Kiln `Env`; OPFS journal for durable turns; `jev` browser fixes. | 10 to 15 |
| **4. Publish and see it in the Grid** | `publish` to a tenant manifest and `staging` pointer; Grid portal. Depends on the deploy design's phase 1 host. | 5 to 8 after that host exists |
| **5. Wallet and paid jobs** | Agent Spark sub-wallet, funding and sweep-back; x402 to Pylons; pylon-side caps. | 8 to 12, plus owner money checks |
| **6. Shared workspaces and remote sandboxes** | Loro workspace rooms; remote `Env` to an agent computer, `coder-worker`, or a Pylon. | 15 to 20 |

### Risks

- **Cross-origin isolation.** COOP and COEP break embedding of resources
  that don't send CORP headers. Mitigate with a separate Kiln origin.
- **Memory on phones.** The Grid plus a toolchain may not fit. Default to
  remote builds on phones.
- **Tool availability.** Not every tool builds for WASI. Pick a small,
  checked set, and ship each as a content-addressed layer.
- **NIP-OA's gaps.** The mandate and the chained spawner tag are our
  extensions. Write them down before shipping and keep them small.
- **Key loss.** Agent keys and sub-wallet seeds in page memory vanish with
  the tab. Keep agents short-lived by default and sweep funds back.
- **Relay load.** Many small agents multiply presence traffic. Rate-limit
  by owner and keep idle agents still.
- **Licensing.** Wanix is MIT and safe to learn from; Apptron and Rubrc
  have no license file, so copy nothing from either.

## Issue-ready steps

1. **Split the Pylon buyer into a sans-I/O crate.** Move job building,
   answer matching, deadlines, and candidate ranking from
   `crates/pylon/src/client.rs` and `router.rs` into `crates/pylon-buyer`
   behind relay and clock traits. Native `pylon` uses it unchanged.
   Acceptance: `cargo check -p pylon-buyer --target wasm32-unknown-unknown`
   passes, and `cargo test -p pylon` passes.
2. **Browser relay adapter for the buyer.** Implement the relay trait over
   `verse-net`'s browser WebSocket. Acceptance: a test page sends one free
   job to a local pylon and shows the answer.
3. **Spawn an agent in the Grid.** A page key per agent, a NIP-OA tag
   signed through the `verse_key` flow, a `kind:0` with `bot: true`, NIP-MV
   presence from the agent key. Acceptance: another browser sees the agent
   labeled "authorized by" the player.
4. **Agent asks Pylons from the Grid.** Wire issue 2 into the spawned
   agent with model choice (V4.1 group, Flint, any) and the hosted-door
   fallback. Acceptance: the first demo, on staging, with the test
   account.
5. **Write the mandate and the spawner chain.** A short spec in
   `nips/openagents/` for `openagents.agent-mandate.v1` and the
   owner → page key → agent tag chain, with fixtures in `crates/nostr`.
6. **Relay enforcement for agents.** NIP-AA admission with action sets,
   owner-aggregated limits, and revocation epochs in `crates/nostr-relay`.
7. **Fix `jev` for `wasm32-unknown-unknown`.** `cfg` around `reqwest`'s
   `redirect`, `chunk()`, and `Send` bounds. Acceptance: `cargo check -p
   jev -p openagents-x402 --target wasm32-unknown-unknown`.
8. **Split `microcoder-loop` into core and native shell.** Acceptance: the
   core checks for `wasm32-unknown-unknown`; native tests pass.
9. **`kiln-vfs` crate.** Namespace, binds, unions, `#ram`, `#layer`, `#cow`,
   tested natively.
10. **OPFS device and `kiln-wasi` host.** Kernel worker, task workers,
    `SharedArrayBuffer` syscalls; run a WASI "hello" and ripgrep from the
    shell.
11. **Publish to a tenant.** After the deploy design's phase 1 host:
    `publish DIR` from the workspace to a `staging` pointer and a portal.
12. **Agent sub-wallet.** A second Spark wallet in the page, funded from
    the player's, swept back on despawn. Owner-only money checks go in
    `NEEDS_OWNER.md`.

## Sources

Upstream repositories, read as data:

- [tractordev/wanix](https://github.com/tractordev/wanix) at `af7fd38`:
  `README.md`, `LICENSE`, `go.mod`, `task.go`, `wanix.go`, `fs/vfs/vfs.go`,
  `fs/net/tcp/README.md`, `fs/net/tcp/conn_js.go`, `fs/net/ether/README.md`,
  `fs/r2fs/SPEC.md`, `wasi/driver.go`, `wasi/fs.ts`, `wasi/callbuffer.ts`,
  `web/fsa/fsa.go`, `web/idbfs/DESIGN.txt`, `cmd/wanix/serve.go`,
  `rc/README.md`, `examples/vm-network.html`, `extras/Makefile`.
- [tractordev/apptron](https://github.com/tractordev/apptron) at `c4cb2db`:
  `README.md`, `boot.go`, `go.mod`, `wrangler.toml`, `worker/src/worker.ts`,
  `worker/src/public.ts`, `worker/cmd/worker/main.go`, `system/bin/publish`,
  `system/bin/rebuild`, `Dockerfile`.
- [vercel-labs/fx](https://github.com/vercel-labs/fx): `sdk/README.md`; and
  `docs/research/fx/README.md` in this repository.
- [oligamiq/rubrc](https://github.com/oligamiq/rubrc): `README.md`.
- [GitoxideLabs/gitoxide](https://github.com/GitoxideLabs/gitoxide):
  `.github/workflows/ci.yml`, job `wasm`.
- [Running Clang in the browser using WebAssembly](https://wasmer.io/posts/clang-in-browser),
  Wasmer, 2024-10-07.
- Prior-art links are inline in [Other prior art](#other-prior-art).
