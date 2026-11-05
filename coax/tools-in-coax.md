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
| Web tools in Python (`og-card.py`, `nip-pages.py`, `upload.py`) | 6,600 | Port. |
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
| `http` client (GET/POST, headers, chunked, redirects) over the existing sockets and TLS | API tests, publishers, Pylon model fetch, App Store | Done: `net/http.coax` (2026-11-01). |
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
  `coax/web/tool.sh tools-check` checks the rule below.

- 2026-11-02: the six TypeScript tools written after the rule are Coax:
  the room checks (`coax/web/room/relay-check.coax`, `load.coax`, over
  `net/relay_conn.coax`, so `wss://` too), the replay and struct-default
  checks (`coax/web/replay-hosts.coax`, `state-defaults.coax`) and Verse's
  `verse/native/wasm-run.coax`. The three that run a page's wasm do it in
  headless Chrome through `coax/web/wasm-chrome.coax`; the page's side is
  `coax/web/wasm-host.js` (reason 1), which keeps files, instantiates, calls
  exports and moves memory bytes. Every step, hash, parse and comparison is
  Coax. Each matched the old tool on the same inputs before it was deleted:
  `wasm-run.coax` wrote the same lines and the same 48 dump files, byte for
  byte, as `wasm-run.ts` on Everglade's pack; `state-defaults.coax` checked
  the same states and face globals; the room checks gave the same results
  against a local room server. `coax/web/write-changed.ts` had no callers
  and went. `tool.sh` links `tls-native.c` for a tool with a
  `// tool.sh: tls` line.

- 2026-11-04 (#12859): `coax/web/room/room-catalog.coax` replaces
  `room-catalog.py` (the same C bytes from Everglade's pack) in the room's
  `build.sh`, `worker.sh` and `deploy.sh`; `coax/web/private-guard.coax`
  replaces `private-guard.py` in `deploy.sh` (the same files refused on the
  same staged tree). Four files gained a true reason: the Cultist Raid robe
  scripts run inside Blender (6), Granite's capture runs Hugging Face and
  llama.cpp (3), and its Unicode table comes from Python's database (2).
  `coax/web/room/room-proof.coax` (`cast` and `raid`) replaces
  `cast-proof.sh` and `raid-proof.sh`: the same room, network and check,
  with the same verdicts on the same room server.

- 2026-11-05 (#12859): more ports, each checked against the old tool
  before it was deleted. `coax/web/room/local.coax` replaces `local.sh`
  (the same lines and answers from the same room and page);
  `room-proof.coax transport` replaces `transport-proof.sh` (the same
  verdicts and ACK numbers through the same proxy profiles);
  `room-math-check.coax` replaces `room-math-check.sh` (the same bits, and
  the same 19,846 failures with `sinf` scaled); `coax/tests/job_budget.coax`
  replaces `job_budget.py`; `coax/web/room/worker.coax` replaces
  `worker.sh` (the same wasm module, byte for byte, with and without the
  cache); `coder/tui/resize_check.coax` replaces `resize_check.py` (the
  same 40 verdicts, and the same 7 failures with the reflow taken out).
  `coax/web/raid-cost.ts` was a one-off measurement for #12747 with no
  callers and went. Granite's gate is Coax: `psionic/tests/check_granite.coax`
  replaces `check_granite.sh` (the same lines from the owned-only run and
  from the full run on both pinned GGUFs, timings aside),
  `granite_text.coax` replaces `granite_text.py` (the same case file, byte
  for byte, and the same verdicts on good, corrupted and short results and
  on changed vocabularies and merges), and `granite_parity.coax` replaces
  `granite_parity.py` (the same text and `--json` file, byte for byte, on
  equal, close, far and mismatched dumps). `granite_fixture.py` keeps a
  reason (2): it writes the tiny GGUF apart from the Coax reader it checks.

- 2026-11-05 (#12859): the API checks test.py ran from Python fragments
  for the rooms and the private packs are Coax, on a small harness
  (`coax/web/api/tests/api_harness.coax`: requests over `net/http.coax`,
  psql, the rooms' HMAC signature, NIP-98 from test.sh's signer).
  `tests/room_checks.coax` replaces `test_room_{admission,chat,events,
  invites,stats,vitals}.py`, with the vitals fault proxy as its `proxy`
  mode; `tests/verse_packs.coax` replaces `test_verse_packs.py`, with the
  fake bucket as its `bucket` mode. test.sh runs both after test.py. In
  full `test.sh` runs with the room key and a room server, the old and the
  new printed the same verdicts, failures included; with a broken API (a
  duplicate receipt marked applied, the pack bytes cacheable) both failed
  the same checks. The room view's empty check now allows the admission
  section #12796 added, which the old check refused.
  `verse/tools/humans/upload_packs.coax` replaces `upload_packs.sh`: against
  a stand-in gcloud and a fixed `openssl rand`, both made the same gcloud
  calls, the same manifest and the same stored objects, byte for byte, on
  new, existing, already-stored and masterless manifests. The new one checks
  every name and pack before asking gcloud anything, and `--dry-run` runs no
  gcloud at all.

## Keeping it this way

- `coax/web/release.sh` runs `coax/web/tool.sh tools-check` on the release
  tree after the compiler step, before staging changes. A failure stops the
  release. Every tracked `.ts`, `.py`, `.js`, `.sh` and `.cjs` file must
  either start one of its first 60 lines (after comment marks) with
  `Firm reason:`, or be on the port list.
- Vendored upstream vectors are data, not tools: a file under
  `coax/tests/vectors/upstream/` that `coax/tests/vectors/manifest.json`
  lists with kind `upstream` and the SHA-256 of its bytes passes as it is
  (such as Bitcoin Core's `rpc_signmessagewithprivkey.py`); an unlisted
  file there needs a reason like any other, and a listed one whose bytes
  changed fails.
- The port list is `coax/tools-port.txt`: one path a line, removed as each
  tool is ported. It only shrinks. Its header names the commit it was taken
  at (`# base: HASH`), and a listed path that wasn't tracked there fails the
  check, so a tool written since can't wait on the list.
- The list holds 1,243 paths (2026-11-02). 488 of them were added that day:
  files the first enforced run found with neither a reason nor a place on
  the list.
