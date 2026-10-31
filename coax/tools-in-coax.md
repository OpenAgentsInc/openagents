# Tools in Coax

Owner rule, 2026-10-30: every tool we write is Coax unless there is a firm
reason it can't be. A tool is anything we run that isn't the product itself:
build and release scripts, recorders, checks, packers, publishers, CLIs, and
tests of our own code.

## What counts as a firm reason

A non-Coax tool must say which of these it is, in its header, as a line
starting `Firm reason:`. Anything else gets ported.

1. **It runs in the browser and talks to the DOM, WebGPU or audio.** The
   page host (`coax/web/host.js`, `loader.js`, `gpui.js`, `voice.js`,
   `drop.js`, `uploads.js`, `entropy*.js`) is the glue between the browser
   and Coax's wasm. Keep it thin; logic moves into the wasm.
2. **It is an independent oracle.** A check that recomputes our answer in
   another language must not share our implementation, or it checks
   nothing. Psionic's `*_oracle.py` files and the Nostr vector generators are
   oracles on purpose.
3. **It runs a reference implementation we test against.** The `.cjs`
   files run upstream Nostr libraries to produce interop vectors.
4. **It exists before Coax does.** The compiler bootstrap
   (`coax/bootstrap*`, `coax/build.sh`, `coax/provenance.sh`).
5. **It is a thin wrapper around another program and holds no logic.** A
   shell line that calls `xcodebuild`, `gcloud`, `docker` or a Coax binary.
   A wrapper that grows branches, loops or parsing is no longer thin.
6. **A platform SDK exists only in another language** and has no C API we
   can call through `#foreign`. Name the SDK.

"Faster to write in TypeScript", "the last tool was TypeScript" and "it's
only a script" are not reasons.

## Where we are (2026-10-30)

About 70,000 lines of non-Coax code in tools and tests.

| Group | Lines | Verdict |
| --- | ---: | --- |
| Psionic tests (`psionic/**/*.py`, 140 files, 70 of them oracles) | 16,900 | Oracles stay (2). The rest (device drivers, service checks) port. |
| Nostr tests and vectors (`coax/tests/nostr`, `.py` and `.cjs`) | 12,800 | Vector generators and reference runners stay (2, 3); harnesses port. |
| Web tools in TypeScript (`coax/web/*.ts`: shot, clip, goldens, perf, frametime, realms, strip, style-check, golden-*, place-reload, connect-pylon-page) | 8,500 | Port. |
| Web tools in Python (`og-card.py`, `nip-pages.py`, `nip-shader.py`, `upload.py`) | 6,600 | Port. |
| `tools/coax-nostr/*` (audits, fuzz, timing, guards) | 6,400 | Mostly port; oracle pieces stay (2). |
| API tests (`coax/web/api/*.py`, 35 files) and deploy checks | 5,500 | Port: tests of our own C API, not oracles. |
| Project CLI (`coax/web/project/*.ts`: publish, baked, tasks) | 3,500 | Port. |
| Verse tools (`verse/tools/pack_*.py`, oracles, benches) | 3,400 | Packers port; `haunt_oracle.py` stays (2). |
| Browser host (`coax/web/*.js`) | 3,000 | Stays (1), kept thin. |
| Shell (`coax/web/*.sh`, `release.sh`, API deploy scripts) | 3,600 | Thin wrappers stay (5). `release.sh`, `deploy.sh`, `golden-check.sh` hold real logic: port. |
| `scripts/` (nips, book publishing, Pylon model fetch) | 1,300 | Port. |
| iOS tooling (`app_store.py`, test scripts) | small | Port `app_store.py` (HTTP and an ES256 JWT); `xcodebuild` wrappers stay (5). |

## What Coax needs first

Most ports are blocked on a handful of libraries, not on the language. What
exists today: sockets and TLS (`coax/platform/socket-native.c`,
`tls-native.c`), a WebSocket client and server (`coax/lib/net/ws.coax`,
`ws_server.coax`), processes (`process-native.c`, `cxproc_spawn`), files
(`store-file-native.c`), crypto, Nostr.

| Library | For | Notes |
| --- | --- | --- |
| `json` (parse into a tree, read paths, write) | DevTools, every API test, the project CLI, GitHub and GitBook APIs | The agent libraries parse JSON ad hoc; make one shared module with tests. |
| `http` client (GET/POST, headers, chunked, redirects) over the existing sockets and TLS | API tests, publishers, Pylon model fetch, App Store | |
| `args` (flags, usage text) | every CLI | |
| `fs` (read, write, list, walk, mkdir, temp dirs, remove) | packers, strip, realms, release | Extend `store-file-native.c`. |
| `proc` (spawn with args, pipes, wait, kill, timeouts) | release, clip (Chrome, ffmpeg), deploy | Build on `cxproc_spawn`; the timeout is the lesson from the hung live-home check. |
| `base64`, `sha256` (and hex) | DevTools frames, cache keys | Crypto may already have sha256. |
| `cdp` (Chrome DevTools over `net/ws`) | shot, clip, goldens, perf, frametime, style-check | One module replaces `chrome.ts` and the per-tool copies. |
| `image` (PNG decode and encode, compare) | goldens, style measures, OG cards | Today goldens compare in TypeScript. |
| `sql` client (Postgres wire protocol) | API tests that seed data | Or keep seeding inside the C API's own test mode. |

## Order

1. **The libraries above**, each with `coax test` cases, in `coax/lib/`.
2. **Browser tools on `cdp`:** `clip`, `shot`, then `goldens`, `perf`,
   `frametime`, `style-check`. These are the most used by agents and the
   most copied. `clip` first: the Coax version must make the same clip
   (same frames, byte for byte) before `clip.ts` is deleted.
3. **Build tools:** `strip`, `realms` (the realm page generator), `wgsl.py` (done: `coax wgsl`),
   `og-card.py`, `nip-pages.py`, the `verse/tools` packers.
4. **The project CLI** (`coax/web/project`).
5. **API tests** into Coax test programs that call the API over `http`.
6. **`release.sh`, `deploy.sh`, `golden-check.sh`** into one Coax release
   tool, once `proc`, `fs` and `http` have carried the others.
7. **The rest** of `scripts/`, `tools/` and Psionic's non-oracle tests.

Each port keeps the old tool until the new one produces the same output on
the same inputs, then deletes it in the same commit that switches callers.

## Done so far

- 2026-10-30: libraries `json/json.coax`, `text/base64.coax`, `sys/args.coax`,
  `sys/fs.coax`, `sys/proc.coax`, `net/cdp.coax` (see `lib/sys/README.md`);
  `coax/web/tool.sh NAME` builds and runs a Coax tool. `clip` and `shot` are
  Coax (`coax/web/clip.coax`, `shot.coax`); `clip.coax` made the same frames,
  byte for byte, as `clip.ts` before it was deleted. `Firm reason:` headers
  are on the browser host, the oracles and vector generators, and the seed
  scripts; everything else is on `coax/tools-port.txt`.
  `coax/web/tool.sh tools-check` checks the rule below (not yet in release.sh).

## Keeping it this way (to build)

- A check in `coax/web/release.sh` (and later the Coax release tool) lists
  every tracked `.ts`, `.py`, `.js`, `.sh` and `.cjs` file. A new one that has
  no `Firm reason:` line and isn't on the port list fails the check.
- The port list lives in `coax/tools-port.txt`: one path a line, removed as
  each tool is ported. It only shrinks.
