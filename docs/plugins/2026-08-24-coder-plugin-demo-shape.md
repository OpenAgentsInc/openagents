# The coder plugin demo, and the shape it recommends

2026-08-24. Branch `coder-plugin-demo`. This is the one-off working demo
ahead of the plugin walking skeleton (#26): a WASM plugin loads into
`openagents coder`, its digest is verified before load, and a live model
calls it as a tool in the chat. The contract it follows is
`OpenAgentsInc/openagents.com` docs
`2026-08-24-triage-and-plugin-model-assessment.md` sections 4.2–4.4.

## What was built

- `plugins/word-stats/` — a guest plugin in dependency-free Rust, compiled
  to `wasm32-unknown-unknown` (rustc 1.94.1), with the 52 KB artifact and
  its SHA-256 checked in beside the source so the demo runs without a Rust
  toolchain. It computes text statistics: bytes, chars, words, lines,
  longest word, most frequent word. Pure computation — the module imports
  nothing.
- `plugins/word-stats/manifest.json` — identity (name, version, author),
  the artifact path and `sha256:` digest pin, the ABI export names, typed
  input and output JSON Schemas, capability declarations (empty mounts,
  empty hosts, a 2000 ms timeout, a memory figure), and reserved
  `price_msats` / `license` fields per the assessment.
- `packages/openagents-cli/src/coder-plugins.ts` — the host. Plain
  `WebAssembly` API, no new dependencies. Load: parse and validate the
  manifest, refuse non-empty capability declarations, read the artifact,
  compare SHA-256 to the pin, compile, refuse any module whose import list
  is not empty, require the declared exports. Invoke: one
  `node:worker_threads` worker per call, terminated at the manifest's
  timeout. Every failure is a typed refusal `{code, reason}` returned as a
  value, never thrown.
- `/plugin load <manifest>` in both the interface and `--plain`. A loaded
  plugin materializes one session-scoped `CoderTool`: the manifest's name
  is the tool name, its description is the tool description (suffixed
  "experimental, session-only, sandboxed"), its input schema is the tool
  parameters, and `run` marshals the JSON arguments to a packet and the
  output packet back to text.
- `scripts/plugin-demo.mjs` — the happy path and all three refusal paths
  from the shell, and `test/coder-plugins.test.ts` — nine tests including a
  `--plain` transcript driven through `CoderSession` by a scripted source.

Proved live: `printf '/plugin load ../../plugins/word-stats/manifest.json\n
<prompt>\n' | node dist/main.js coder --plain` — the thread model called
`word_stats`, the call and its `→ ok` rendered in the transcript, and the
reply was built from the plugin's output.

## ABI and memory mechanics

The packet contract is `handle_packet(bytes) -> bytes`, and bytes cross the
boundary through guest linear memory:

1. The host calls the guest's exported allocator `packet_alloc(len) -> ptr`
   and writes the input packet — the UTF-8 JSON encoding of the tool
   arguments — into guest memory at `ptr`.
2. The host calls `handle_packet(ptr, len)`. The guest allocates its output
   inside its own memory and returns one `u64` packing the location:
   `(out_ptr << 32) | out_len`. (WASM `i64` returns surface as `BigInt` in
   Node; the worker unpacks with shifts.)
3. The host re-reads `memory.buffer` after the call — the guest may have
   grown memory, which detaches earlier views — bounds-checks
   `out_ptr + out_len`, and copies the output packet out.

The output buffer is deliberately leaked by the guest: the instance lives
for exactly one call, so a `packet_free` export would be ceremony. That
choice is coupled to one-worker-per-invocation, which also buys the two
properties the contract cares about: the timeout is enforceable (a WASM
call is synchronous and cannot be preempted in-process, so the host
terminates the worker), and no state survives between calls.

Refusals are typed on both sides. The host refuses with a closed code set
(`manifest_invalid`, `digest_mismatch`, `capabilities_unsupported`,
`imports_declared`, `exports_missing`, `not_wasm`, `timeout`, `trap`,
`bad_packet`); the guest returns `{"refusal": {code, reason}}` as its
output packet. Both reach the model as text it can act on.

## What the skeleton (#26) should keep

- **The manifest as the whole declaration.** Digest pin checked before
  compile; import list checked before instantiate; capabilities are
  declared-and-enforced or refused, never ignored. The demo's
  `capabilities_unsupported` refusal for any non-empty mounts/hosts is the
  right default until host imports exist.
- **Refusals as values.** `LoadedPlugin | PluginRefusal` and
  `Uint8Array | PluginRefusal` compose; exceptions do not. The tool layer
  turning a refusal into one sentence is what let the live model handle the
  demo gracefully.
- **Termination as the timeout mechanism.** Whatever engine the skeleton
  adopts, the bound must survive a guest that never returns. Worker
  isolation (or engine epochs/fuel) is load-bearing; an in-process `await`
  with a timer is not enforcement.
- **The tool-materialization seam.** `pluginTool(loaded): CoderTool` is
  exactly the `CoderTool` shape the session already declares, so plugins,
  skills, shell, and delegate all ride one declaration path and the
  `/plugin load` wiring is ~20 lines in `cli.ts`. Tier-3 session loading
  per assessment 4.4 falls out of this for free.
- **Session-scoped registration with replace-by-name**, so iterating on a
  plugin re-declares rather than duplicates.

## What the skeleton should replace

- **The hand-rolled guest.** The demo guest parses JSON with a scanner
  because it ships dependency-free. The skeleton's owned Rust PDK should
  own the ABI: serde for packets, a `Refusal` enum, a `#[plugin_fn]`-style
  macro over `packet_alloc`/`handle_packet`, and the pack/unpack of the
  return word. Guest authors should never see a pointer.
- **The raw `WebAssembly` host, behind the engine abstraction.** The
  bare API has no fuel metering, no memory ceiling at instantiation (the
  manifest's `memory_max_mib` is declared but unenforced here), and no
  WASI. The issue's engine-abstraction constraint is right: keep the
  demo's *contract* (load → verify → instantiate → invoke-with-bound →
  bytes-or-refusal) as the interface and let wasmtime/Extism-derived code
  implement it. The worker-per-call model can stay as the Node fallback
  engine.
- **Per-call worker spawn, eventually.** ~10–30 ms per invocation is fine
  for a demo; a pooled worker holding a compiled `WebAssembly.Module`
  (modules are transferable) is the obvious next step if plugins get hot.
- **Schema validation.** The manifest carries typed input/output schemas
  and the demo forwards the input schema to the model but validates
  neither side. The skeleton should validate both (Effect Schema in the
  CLI per the assessment), and reject non-conforming output packets as a
  host-side `bad_packet`.
- **No receipt.** The contract wants a `tool.ran` thread event carrying
  the digest per invocation. The demo's tool run is visible in the
  transcript but writes no durable event; the skeleton must, since the
  economy lane projects over those records.
- **`/plugin` surface.** Only `load` exists. `list`, `unload`, the
  digest-pinned local catalog, and the `capability` discovery tool
  (tiers 1–2 of assessment 4.4) are all skeleton scope.

## Open questions

- **WASI.** The demo proves pure compute needs none. The pilot (foreign
  session resume) needs read-only mounts, which means WASI preview 1
  filesystem imports or Extism-style host functions — the first real host
  import surface, and the first ask-once approval per assessment 4.4.
- **Fuel/CPU metering.** Wall-clock termination bounds time but not spend;
  fuel or epoch interruption needs an engine (wasmtime) the plain Node API
  does not expose. Decide whether the Node host ever needs it or whether
  wall-clock is the Node engine's honest ceiling.
- **Memory ceiling.** Enforceable today by instantiating with a
  host-provided bounded `WebAssembly.Memory` only if the guest imports its
  memory; rustc's default is to export it. The PDK could flip guests to
  imported memory, or the engine abstraction owns the limit.
- **Effect integration.** The host is promise-based to match `CoderTool`.
  Whether the skeleton wraps it as an Effect service with typed errors
  (per the workspace's Effect conventions) or stays at the tool seam is a
  skeleton decision; nothing in the demo blocks either.
- **Packet encoding.** JSON-in-JSON (arguments → UTF-8 JSON packet) is
  legible and matches the manifest schemas, but binary payloads (the
  session-resume pilot moves transcripts) may want a length-prefixed or
  CBOR packet kind — the manifest's `abi.kind: "packet-v0"` field exists
  so this can version.
- **Where guest crates live.** `plugins/word-stats` is a standalone crate
  (`[workspace]` empty table) so the monorepo's cargo workspace does not
  build it. The skeleton should decide whether guests join the workspace,
  get a `plugins/` workspace of their own, or live out-of-repo entirely
  once the registry exists.
