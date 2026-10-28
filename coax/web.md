# Coaxial: the Coax web framework for openagents.com (proposal)

Status: **the first slice runs (2026-10-28): the openagents.com splash on a
Coax engine in WebGPU, with `coax serve` hot reload. The rest is a proposal.**
Owner decisions are recorded under [Decisions](#decisions-2026-10-28).

This document specifies how openagents.com moves onto Coax and Verse: the
site takes over a full-window GPU canvas, shows the Grid and the rest of
Verse in the browser, loads fast, and is developed with edit-to-screen
times measured in milliseconds rather than seconds. It sits beside the
[implementation roadmap](ROADMAP.md): the framework is the real consumer for
the roadmap's wasm32 feasibility test (R1), its GPU and browser proof (R8)
and its reload work (R11). It does not reorder the roadmap. R0, the measured
native kernel, is separate work.

## Decisions (2026-10-28)

- **Name:** Coaxial; commands `coax serve` and `coax deploy`.
- **WebGPU only**, with a text fallback for browsers without it.
- **Upstream LLVM** installed (clang and lld 23.1.4, wasi-libc, wasi-runtimes,
  from Homebrew).
- **What openagents.com needs at the switch:** the landing page, and docs
  and blog posts written in Markdown that search engines read as text and
  people see drawn on the GPU. Nothing else: no chat, sign-in, API pages or
  payments in this site for now.
- **Voice in the browser:** after the first switch.
- **First priority:** a Coaxial version of today's "Coming soon" splash.

## Built so far

| Piece | Where | State |
| --- | --- | --- |
| The splash | `apps/openagents-web/splash.coax` (layout, stroke font, write-on animation, nebula clock; 5 checks), `face.c` (its Wasm exports), `splash.wgsl` (temporary hand-written WGSL; the nebula is the old page's), `index.html`, `index.md` | Runs in Chrome; a frame read back shows the strokes over the nebula |
| The browser host | `coax/web/host.js`: WebGPU, the frame clock, state in one `WebAssembly.Memory`, code and shader swaps, the dev error note, the `?probe` frame readback | Works for apps with one uniform block and one vertex stream |
| Coax → Wasm | `coax/web/wasm.sh`: `coax c` → clang wasm32-wasip1 → wasm-ld; dev (`-O1`, parallel) and release (`-O2`) | The splash is 24 KB (release) |
| Dev server | `coax/web/serve.c` + `serve.sh` (`coax serve`): static files, a watcher, rebuilds, `/coaxial/live` | Measured: an engine rebuild in 118–135 ms, shader and page messages, compiler errors with the `.coax` location |
| Staging deploy | `coax/web/deploy.sh APP SERVICE` (`coax deploy`, for now): release build, content-named wasm/shader/host files, a direct parallel upload of only what changed to the app's bucket (`upload.py`). `cdn.sh` (one time) puts Cloud CDN and a load balancer in front of the bucket; `deploy.sh --server` builds the static server image once for the Cloud Run fallback, which mounts the same bucket | A deploy takes about 1.2 s (0.4 s build, 0.8 s upload); staging.openagents.com |

Not built: `coax wgsl` (shaders are hand-written WGSL for now), the
production switch of openagents.com itself,
the Filament font (`font.md`), Markdown docs and blog drawn on the GPU,
compression and content-hashed URLs, and a `coax serve` subcommand inside the
compiler itself (today it's `coax/web/serve.sh`).

## Goals

1. **The Grid in the browser, fast.** A first visitor sees the live Grid
   (ground, lights, the terminal, other people) in under a second on a good
   connection and in under three on a typical phone, with the character and
   heavier assets streaming in after.
2. **One engine.** The browser runs the same Coax code the iOS app runs
   (`verse/*.coax`): scene, Grid, player, camera, cast, HUD, voice. The
   browser host does platform effects only, like Swift does on the phone.
3. **Fast iteration.** Saving a `.coax`, shader or data file updates the
   running page without losing your place in the world. Targets below.
4. **The GPU owns the screen.** The site renders through WebGPU into one
   canvas: world and HUD, in Coax. HTML is used only where the browser does
   something a canvas can't (see [HTML](#html-only-where-it-earns-its-place)).
5. **Deploys to Google Cloud** cheaply, inside the $1,500-a-month cap, with
   openagents.com always on.
6. **Agents and crawlers still read the site.** Today openagents.com serves
   `index.md`, `llms.txt`, an API catalog and OpenAPI. Those stay, served
   as text, without a GPU.

Not goals: a general-purpose web framework for other people's sites, a DOM
component system, server-side rendering of the canvas, or support for
browsers without WebGPU beyond a text fallback (open question 5).

## Where things stand

Facts this proposal starts from, checked 2026-10-28:

- **openagents.com today** is the Cloud Run service `coder` running the
  image `openagents-web`, built from sources that the reset (`0b401561a0`)
  removed from the tree. Its homepage text says the Grid runs in the
  browser. Nothing in the current tree builds or deploys it.
- **Coax** emits C11 (`coax c`, `coax lib`) and a restricted f32/bool Metal
  library (`coax metal`). It has no wasm32 target, no WGSL emitter, no
  globals and no heap. State lives in caller-allocated storage passed as
  views, the pattern `verse/platform/voice.h` uses (`oa_voice_size()`,
  caller allocates, zeroed).
- **Compile speed today, this Mac:** `coax c` on `grid.coax` (895 lines),
  `player.coax` and `cast.coax` takes about 10 ms each; `home.coax` takes
  190 ms because it imports protobuf descriptors. Clang compiles the Grid's
  generated C at `-O2` in 90 ms.
- **Toolchain:** Apple clang 21 has no wasm32 target, so the browser build
  uses upstream LLVM 23.1.4 (clang, wasm-ld) with wasi-libc, installed from
  Homebrew on 2026-10-28.
- **Precedent:** before the reset, Aethyrforge (`git show 0b401561a0^:apps/aethyr/`)
  rendered a town through WebGPU from a worker with C/Wasm kernels. Its
  measured record
  (`docs/aethyr/engine/research/c-medieval-town-results-2026-10-23.md` in
  history) shows a changed C movement module rebuilt and linked to Wasm in a
  57 ms median (Clang 23, `-O3`), modules of 5.8 KB (2.6 KB gzipped), and
  live module replacement that copies current state in one JavaScript turn
  and refuses an incompatible candidate. Same idea, now with Coax in front.
- **The playtest** (`playtest.openagents.com`, Cloud Run `aethyr-demo`) is a
  separate live service. This proposal doesn't touch it.
- **Assets:** the character cast (`Cast.bin`) is 2.4 MB uncompressed.

## Architecture

```
            dev: coax serve (Mac)                    prod: Cloud Run + CDN
            watches .coax / .wgsl / data              same server binary
                   │ WebSocket: rebuilt modules             │ HTTPS
                   ▼                                        ▼
 ┌──────────────────────────── browser tab ────────────────────────────┐
 │ host.js (hand-written, no bundler): WebGPU, input, fetch, sockets,  │
 │ audio worklet, IME text field, accessibility mirror                 │
 │   │ owns one WebAssembly.Memory (the state arena) + GPU resources   │
 │   ▼                                                                 │
 │ engine.wasm  = verse/*.coax → C11 → clang wasm32                 │
 │   grid, player, camera, cast, hud, voice … (stateless code)         │
 │ shaders.wgsl = Coax shader subset → WGSL (coax wgsl, not yet built) │
 └─────────────────────────────────────────────────────────────────────┘
                   │ WebSocket, protobuf frames (proto/)
                   ▼
   site server (C host + Coax logic): pages as text, API, Grid presence,
   voice relay (verse/voice-host pattern)
```

### Client

- **Engine code** is the existing Verse Coax, compiled to C11 by
  `coax lib`, then to wasm32 by clang with `-ffreestanding`, no libc beyond
  what the generated C needs, no fast math and no floating-point contraction.
  The browser and iOS builds compile the same Coax sources; only the host
  differs.
- **The host** is plain JavaScript, hand-written, a few hundred lines, no
  npm, no bundler, no framework. It is the browser's Swift: it creates the
  WebGPU device and canvas, owns GPU buffers and pipelines, feeds input and
  time to the engine, uploads what the engine lays out, and runs sockets and
  audio. Logic does not go here (the repository's Coax-first rule).
- **State lives in host memory.** Coax has no globals, so engine modules are
  stateless code over storage the host allocates, as on iOS. The host owns
  one `WebAssembly.Memory`; each subsystem's state is a fixed region in it,
  sized by the module's exported `*_size()` and fingerprinted by its layout
  (`coax layout`). This is what makes code reload cheap (see below).
- **Shaders** come from Coax. The Metal path already emits scalar shader math
  from `light.coax`; the browser needs the matching WGSL lowering the spec
  plans in §15 (`coax wgsl`). Until it exists, hand-written WGSL ports of
  `Stages.metal` keep the slice moving, marked as temporary.
- **Workers:** the engine runs on the main thread first (simplest, and the
  Grid is small). Moving it to a worker with `OffscreenCanvas` is a measured
  later step, not a starting assumption. Shared memory across workers needs
  cross-origin isolation headers (COOP/COEP), which the server sets from day
  one so the option stays open.

### Server

- **One binary**, the `verse/voice-host` pattern: a C host for sockets,
  TLS-terminating proxy in front, and Coax for every decision (routing,
  admission, rate limits, protocol handling). It serves:
  - the app shell (`/`): a tiny HTML page, `host.js`, `engine.wasm`, WGSL and
    asset manifests, all content-hashed and immutable;
  - text for agents and crawlers: `index.md`, `llms.txt`, the API catalog,
    OpenAPI and docs, the same URLs as today;
  - the API and live sockets: Grid presence, chat, voice (the voice host
    merges in or sits beside it).
- **Protocol:** the browser speaks the same protobuf frames the phone does
  (`proto/`), one message per WebSocket binary frame, decoded in Coax with
  `#import_proto`. No JSON on the hot path.
- **Data:** none on day one beyond what existing services hold. Storage
  choices wait for a consumer (open question 8).

### Deploy on Google Cloud

| Piece | Where | Rough monthly cost |
| --- | --- | --- |
| Site server | Cloud Run, min 1 instance, 1 vCPU (openagents.com stays always on) | about $50–100 |
| Immutable assets (wasm, WGSL, cast, textures) | Cloud Storage bucket behind Cloud CDN | a few dollars plus egress (about $0.02–0.08 per GB) |
| HTTPS load balancer | the one openagents.com already needs | about $18 per forwarding rule |

The container is the static server binary on a distroless base, a few MB,
so cold starts are short. Deploys are a local script (no GitHub Actions):
build, run the checks, push the image and assets, shift traffic, keep the
previous revision tagged for instant rollback. Content-hashed asset URLs
mean a deploy never breaks a tab that is already open.

## Fast iteration

Development runs `coax serve` on the Mac: the site server in dev mode plus a
watcher. It rebuilds only what changed and pushes it to every open tab over
a WebSocket. Four tiers, fastest first:

| Change | What happens | Target, save to new frame |
| --- | --- | --- |
| Data (world layout, HUD layout, `home.textproto`, tuning) | Re-encoded, pushed, the engine re-admits it through the same checks as at load | under 50 ms |
| Shader (`.wgsl` or Coax shader subset) | Re-emitted, `createShaderModule`, pipeline swapped at the next frame | under 100 ms |
| Engine logic (`.coax`) | `coax lib` → clang wasm32 → pushed; the host instantiates it against the same memory and swaps at a frame boundary | under 300 ms |
| Layout change (a state struct changed) | Same as logic, but the layout fingerprint differs: the subsystem migrates if a migration is declared, otherwise resets just that subsystem | under 500 ms, state kept where declared |

The budgets come from measurements, not hope: about 10 ms for `coax c`, about
90 ms for clang on the Grid, the Aethyrforge 57 ms leaf rebuild, plus
transfer and `WebAssembly.instantiate` on localhost. They are targets to
verify in phase W1, not claims.

**How a code swap works.** Because every byte of state is in the host's
memory, swapping code means:

1. Compile the candidate module while the old one keeps running.
2. Check its exported layout fingerprints against the live regions.
3. At a frame boundary, in one JavaScript turn, point the host's calls at
   the new exports. Nothing to copy for matching layouts.
4. A failed compile, a failed check or a trap in the first frame keeps the
   old module running and shows the error in the HUD with the original
   `.coax` location. A newer request always beats an older one.

Two Wasm details shape this:

- clang places a module's constant data in the same linear memory as the
  state. The build reserves a fixed low region for module data and a fixed
  state base above it (`--global-base`, a fixed initial memory and a stack
  size), so a new module's data segments never overwrite live state. To
  verify in W1.
- Wasm can't free a module's code. The host drops references and lets the
  browser collect it; long sessions are part of the W1 test.

Swapping code at a frame boundary is roadmap R11's transactional reload on
the browser, done early for development only. Production never loads code
except at page load.

**The CPU-side loop also runs natively.** The same Coax builds for the Mac,
so engine tests (`coax test`, `coax mutate`, the sanitizer harnesses) run
without a browser in milliseconds, as they do now. The browser is for
seeing it, not for testing logic.

## Loading fast

- **Order:** HTML shell (one round trip) → `host.js` and `engine.wasm`
  (streaming compile, `WebAssembly.instantiateStreaming`) → the ground and
  lights (no assets needed) → first frame → cast, then textures, in priority
  order, while you can already look around.
- **Budgets to hold, compressed:** shell + host under 30 KB, engine under
  300 KB, the first-frame set under 500 KB in total. The cast (2.4 MB raw)
  loads after the first frame, compressed, and is the first candidate for
  a smaller packed form.
- **Caching:** content-hashed, immutable URLs; Brotli precompressed at
  build time; a returning visitor's first frame needs only the HTML.
- **Measured, not assumed:** the server records time to first frame and
  bytes fetched per visit (no personal data) so the budgets are checked on
  real devices.

## HTML only where it earns its place

The page is one canvas. HTML elements appear only where a canvas can't do
the job:

- **Text input.** Typing (chat, the terminal, names) needs the browser's
  IME, autocorrect and password managers. A real `<input>`, positioned over
  the HUD field and styled to disappear, takes the keystrokes; Coax draws
  the field.
- **Accessibility.** Screen readers can't see a canvas. The HUD publishes a
  small tree (buttons, labels, live captions) that the host mirrors as
  hidden DOM with roles, as the iOS app does with accessibility identifiers.
- **Links and copying** (sharing a world, copying an address) and the text
  routes for agents and crawlers.

Everything else, including menus, panels, the hotbar and captions, is drawn
by Coax in the HUD.

## The Verse feature path in the browser

What the browser needs, in the order the Grid uses it:

| Piece | Today (iOS) | Browser |
| --- | --- | --- |
| Scene, HUD, admission | `home.coax`, `hud.coax` | Same Coax; host uploads to WebGPU |
| Ground, view, touch controls | `grid.coax` | Same; keyboard, mouse and touch input from host.js |
| Walking, collision, third-person camera | `player.coax`, `camera.coax` | Same |
| Character and robots | `cast.coax` + `Cast.bin` | Same; skinning shader in WGSL |
| Shader math | `light.coax` → Metal, `Stages.metal` | WGSL (temporary hand port, then `coax wgsl`) |
| Voice | `voice*.coax` + Swift capture | Same Coax; `getUserMedia` + AudioWorklet in the host, same voice host |
| Wallet | `wallet.coax` + Breez Spark | Later; key storage in a browser is its own design |
| Haunted House | `haunt*.coax` | Later, after the Grid |

Voice is a good early proof that the model holds: the 20 ms frame engine,
its jitter buffer and frame decisions run unchanged, and the browser joins
the same voice host the phone uses.

## Releasing

One command ships openagents.com, and it always goes through staging:

```sh
coax/web/release.sh                  # origin/main → staging → smoke → production → smoke
coax/web/release.sh --staging-only   # stop after staging
coax/web/rollback.sh [<time>]        # list kept releases, or put one back
```

1. **Build once, from committed code.** A clean worktree of `origin/main` (or `--ref`), never a working directory other agents are editing.
2. **Staging.** `deploy.sh` uploads to the staging bucket; `smoke.sh` checks the page, every file it names (status, type, caching, wasm magic), the hidden text, HTTP to HTTPS and the 404.
3. **Promote the same bytes.** The staging bucket is copied into production's, files first and pages last, so a page never names a file that isn't there yet. Production's previous pages are kept under `releases/<time>/`.
4. **Production smoke.** The same checks, plus the routes the old `coder` service still owns. A failure puts the previous pages back on its own.

Skipping staging takes `--skip-staging --yes-skip-staging`, for emergencies.

**Routing.** One global load balancer (`coaxial-openagents-web-staging`, IP 136.69.24.60) serves both hosts:
- staging.openagents.com goes to the staging bucket;
- openagents.com sends `/`, `/favicon.svg`, `/build/*` and `/coaxial/*` to the production bucket, and everything else to the Cloud Run `coder` service until it is retired ([deprecation plan](../docs/2026-10-28-coder-service-deprecation.md)).

Moving a route off `coder` is one path rule. DNS for openagents.com is a single A record at Cloudflare. The records it replaced (Cloud Run's four A and four AAAA) are kept in `~/work/.secrets/openagents-com-dns-before-coaxial.json`.

## Phases

Each phase ends with something running and measured. Phase numbers are this
document's; the roadmap milestones they feed are named.

- **W0. Toolchain and a triangle.** Upstream LLVM (clang + lld) pinned on
  this Mac; one Coax function compiled to wasm32 and called from host.js;
  WebGPU clears and draws. Feeds roadmap R1's wasm32 feasibility test
  (types, layout, a host import, memory, a source location).
- **W1. Dev loop.** `coax serve`, the four reload tiers, the module data /
  state split, and errors shown with `.coax` locations. Exit: the targets
  above measured on this Mac.
- **W2. The Grid.** `grid.coax`, `player.coax`, `camera.coax` and the
  ground and light shaders in WebGPU; walking with keyboard, mouse and touch.
  Exit: time to first frame and frame time measured on a Mac and a phone.
- **W3. Cast and HUD.** Character and robots, the hotbar, captions, the
  text input and accessibility mirror.
- **W4. The site server.** The C host + Coax server serving the shell, the
  text routes (parity with today's `index.md`, `llms.txt`, API catalog,
  OpenAPI), presence and voice. Deployed to a preview address next to the
  live site.
- **W5. Switch openagents.com.** After parity on the routes that matter
  (open question 7), point openagents.com at it with the old revision kept
  for rollback.
- **W6. More Verse.** Voice in the browser, then the Haunted House, the
  wallet and the rest, each as its own slice.

Roadmap R8's full gate (the same pinned workload on native Metal and in a
browser, with output comparisons) is met by W2–W3 plus the comparisons R8
asks for. This framework doesn't claim it earlier.

## Naming

The engine is Verse and the language is Coax. The framework is the part
that puts Verse on the web and runs the site around it. A separate name
is worth having only if it's a thing people build with (the site, and later
other people's worlds). If not, call it "Verse for the web" and skip the
name.

The name is **Coaxial** (owner's direction, 2026-10-28): the cable's full
name, Coax carried all the way to the screen and the server. The commands
are Coax subcommands, not a second tool: `coax serve` (dev server and
reload) and `coax deploy`.

Considered and set aside:

| Name | Why not |
| --- | --- |
| Coaxium | The starship fuel in Star Wars (a Lucasfilm term); reads as a reference and risks a trademark problem on a public product |
| Triax | Triaxial cable is coax with one more shield, which fits, but it's less obvious |
| Braid, Feedline, Ferrule, Splice, Jack | Cable parts; less direct than the cable itself |

## Open questions

Decisions for the owner. Each has a recommendation.

1. **Name.** Decided: Coaxial, with `coax serve` and `coax deploy`.
2. **WebGPU only?** Decided: yes, with a text fallback.
3. **Install upstream LLVM?** Decided: yes; installed.
4. **Plain JavaScript for the host**, hand-written, no npm, no TypeScript,
   no bundler? Keeps the toolchain to Coax + clang and keeps the host honest
   about doing effects only. TypeScript would add a compiler and type
   checking for a few hundred lines. *Recommend plain JavaScript.*
5. **What a visitor without WebGPU sees:** the text site (chat still works,
   with HTML forms), or only a notice? *Recommend the text site.*
6. **Chat in the canvas or HTML?** Today chat is a page (`/chat`). In the
   canvas it's drawn by Coax with an overlaid input; in HTML it's a separate
   page that isn't the Grid. *Recommend the canvas, with the text version
   kept for agents and no-WebGPU visitors.*
7. **What must keep working at the switch (W5).** Which of today's routes
   and features are required: `/chat`, Coder sign-in, the API and pricing,
   docs, blog, the agent catalog, payments? This decides how big W4 is.
   Decided: the landing page plus Markdown docs and blog, drawn on the GPU
   and readable as text.
8. **Server data.** The new server needs accounts, sessions and chat
   history only when those features move over. Keep them on today's
   services and call them from the new server until then? *Recommend yes.*
9. **Preview address.** A second hostname (for example `next.openagents.com`)
   during W4 costs about $18 a month for its forwarding rule, or it can share
   the existing load balancer with a path or host rule at no extra cost.
   *Recommend sharing the existing load balancer.*
10. **Voice in the browser:** decided: after the first switch.

## What I need from you

- Nothing blocking. Questions 4, 5, 6, 8 and 9 use their recommendations
  unless you say otherwise.
