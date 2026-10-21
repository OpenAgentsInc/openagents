# livehost

A prototype of web deploys in seconds, phase 1 of
[Web deploys in seconds](../../docs/audits/2026-10-21-web-deploys-in-seconds.md).
It is standalone: nothing in `openagents-web` uses it.

A stable host serves an immutable, content-addressed **manifest**. A deploy
uploads only the objects that changed, writes a new manifest, and publishes a
signed Nostr **pointer** to it. Hosts follow the pointer over a relay and swap
the manifest in memory, and connected browsers update.

Every site or game is a **tenant**: a name, the key that signs its pointers,
and its own manifest history and channels (`staging`, `live`). Our own site is
just one tenant. Objects live in one store that every tenant shares, so an
object two tenants use is stored once.

## Concepts

| Piece | What it is |
| --- | --- |
| Object | Any file, named by the SHA-256 of its bytes. Stored at `objects/<sha256>` in a directory or `gs://BUCKET/PREFIX`. |
| Manifest | JSON (`openagents.livehost.manifest.v1`) listing a tenant's objects by path, routes, browser modules, handler components, compressed copies, an optional `meta` object, and the `parent` manifest. Itself an object. |
| Pointer | A Nostr event of kind `30440`, signed by the tenant's key, with tags `d` = `TENANT/CHANNEL`, `tenant`, `channel`, `m` = manifest hash, and `prev`. The relay keeps the newest one per key and `d`. |
| Host | `livehost serve`: follows each tenant's pointer, refuses pointers from any other key, loads the manifest, and swaps it in. Each request keeps the manifest it started with. |
| Handler | A WebAssembly component (`wasm32-wasip2`) implementing [`wit/livehost.wit`](wit/livehost.wit). Compiled once per hash with wasmtime and instantiated per request, with no files, network, or environment. |
| Browser module | A `wasm32-unknown-unknown` module referenced by hash from a template. |

## Routes the host serves

| Route | What it returns |
| --- | --- |
| `/t/TENANT/PATH` | The tenant's page or file at `PATH`. Without the prefix, the `Host` header picks the tenant (the first tenant answers unknown names). |
| `/_o/SHA256.EXT` | An object, `Cache-Control: immutable`, as its `br` or `gzip` copy when the client accepts it, with CORS. |
| `/t/TENANT/_live` | Server-sent events, one JSON message per pointer change: `{"tenant", "channel", "manifest", "seq", "from", "css", ...}`. The first message is the current manifest. CORS. |
| `/t/TENANT/_manifest/SHA256` | That tenant's manifest document. CORS. |
| `/_livehost/state` | Each tenant's current manifest. |
| `/_livehost/ack`, `/_livehost/acks` | Browsers report each update they show; benchmarks read the stream. |

A path resolves in this order: a manifest route (template or handler), then a
manifest object at that path, then `PATH/index.html`. HTML responses get the
live-reload client ([`src/client.js`](src/client.js)), which swaps changed
stylesheets in place and reloads the page for anything else.
A page that applies updates itself, such as a game that swaps its logic
between ticks, opts out of the reload with `<html data-livehost-self>`:
the client then only tracks the current manifest and never reloads, and the
page follows `/t/TENANT/_live` and posts its own acks. An ack body is at
most 16 KiB; its fields reach `/_livehost/acks` as sent, with `tenant` and
`at_ms` added.

## Templates

The host interprets templates at run time ([`src/template.rs`](src/template.rs)):
`{{ name }}` (escaped), `{{{ name }}}` (raw), `{{#each items}}...{{/each}}`
with `this`, `{{#if name}}...{{else}}...{{/if}}`, `{{> other.html }}`,
`{{asset static/style.css}}`, and `{{module NAME}}`. Every template gets
`livehost.manifest`, `livehost.tenant`, and `livehost.base` (`/t/TENANT` under
a prefix, else empty).

## Deploy

```sh
livehost deploy DIR --tenant NAME --channel staging \
  --store STORE --relay ws://127.0.0.1:7448 --key TENANT.key [--meta meta.json] [--force]
livehost rollback --tenant NAME --channel staging --store STORE --relay URL --key TENANT.key [--to SHA256]
```

Every file under `DIR` becomes an object by its relative path, whatever its
type. Unchanged files keep their hash and aren't uploaded again. Files of
text and WebAssembly types over 1 KiB also get `br` and `gzip` copies. An
optional `DIR/site.json` adds:

```json
{
  "tenant": "site",
  "routes": [
    {"path": "/", "template": "index.html", "data": "content/index.json"},
    {"path": "/hello", "handler": "hello"}
  ],
  "meta": {},
  "guests": "../../guests",
  "guest_inputs": ["../../wit"],
  "modules": {"greet": "livehost-module"},
  "handlers": {"hello": "livehost-handler"}
}
```

A deploy digests each guest package's sources, rebuilds only a package whose
digest changed (`cargo build --release` in the guest workspace, incremental),
and prints each phase's time. Set `LIVEHOST_GUEST_TARGET` to give guest builds
their own target directory, so a deploy never waits on another build's lock.

`livehost rollback` re-points the channel at the current manifest's parent.

### From Rust

```rust
use livehost::deploy::{self, CheckInput, Options, Target};

let check = |input: &CheckInput<'_>| -> anyhow::Result<()> {
    // For example, a zone's first-play budget over input.objects and input.files.
    Ok(())
};
let report = deploy::deploy(dir, &Target { store: &store, relay, signer: &key, channel: "live" },
    &Options { tenant: Some("icecream".into()), check: Some(&check), ..Default::default() }).await?;
```

A failing check refuses a `live` deploy unless `override_check` is set, and
only adds a warning on any other channel. `livehost::host::Host` is the
embeddable host: `Host::new(config, store)`, `host.router()`, and
`host.receive(event)` for each pointer event.

## Demo

From the repository root, with the shared target:

```sh
export CARGO_TARGET_DIR=~/work/openagents-target-1
. scripts/dev/cargo-build-env.sh
cargo build --release -p livehost -p nostr-relay
rustup target add wasm32-wasip2
crates/livehost/scripts/relay.sh &                        # Postgres and a relay on :7448
export LIVEHOST_GUEST_TARGET=~/work/openagents-target-1-livehost
crates/livehost/scripts/demo.sh "$(openagents scratch)/livehost-demo"   # prints deploy commands, serves :8090
```

Open `http://127.0.0.1:8090/` and `http://game.localhost:8090/`, edit a file
under the work directory's `src/demo`, and run the printed deploy command.

## Benchmark

```sh
openagents browser run --timeout 60 -- "$CARGO_TARGET_DIR/release/livehost" bench \
  --host 127.0.0.1:8090 --store WORK/store --relay ws://127.0.0.1:7448 \
  --site WORK/src/demo/site --site-key WORK/site.key \
  --game WORK/src/demo/game --game-key WORK/game.key \
  --runs 5 --load 64,20 --out WORK/bench.json
```

It edits the copied sources (a stylesheet, copy, a template, the module, the
handler, the game's sprite), deploys each, and times edit to host serving and
edit to the browser showing it, from the browser's own report. It then
measures rollback and runs load with and without manifest swaps. Results are
in the design doc's "Prototype measurements" section.

## Limits

- The object cache in the host has no eviction, and any loaded object is
  readable by hash from any tenant's origin.
- A handler instance is created per request; fuel bounds its run time. There
  are no host imports, so handlers can only render from request data.
- Pointer `created_at` is in whole seconds; a deploy bumps it past the
  previous pointer, so rapid deploys may stamp a few seconds ahead.
- No quotas, moderation, promotion between channels, or provenance checks.
