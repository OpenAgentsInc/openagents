# The render fixture origin

`render-test-origin.coax` is a loopback server for browser checks. It never
builds a producer or shader. The caller supplies a JSON configuration and a
new ledger directory, then binds the result to its own source fence.

The caller imports `Basic` once. The helper loads the shared Coax CDP/socket
and qualification effects without another `Basic` import. Its entry point
must dispatch the server child before its normal arguments:

```coax
a: [1] Args;
if !args_load(a[..]) { return 2; }
child := rto_child(a[..]);
if child >= 0 { return child; }
```

The stable APIs are:

| Call | Result |
| --- | --- |
| `rto_start(port_file, config_file, ledger_dir)` | Child process handle, or -1. Refuses an existing port/readiness/request/final file before spawning. |
| `rto_child(args)` | -1 for a normal caller; otherwise runs the private server mode and returns its exit status. |
| `rto_get(port, path)` | Bounded GET; true only for HTTP 200. |
| `rto_control(port, name, enabled)` | Toggle one explicitly named source variant through a reserved loopback GET. |
| `rto_finalize(port)` | Request finalization. This acknowledges the request; the caller must also wait for server exit 0. |
| `rto_add_inputs(ledger_dir, inputs)` | Require a passed, requested finalization and unchanged original files, then add every original file to `RtqInputs`. |
| `rto_page(html, wasm_url, package_url, out)` | Produce the fixture's HTML with one `data-app` insertion and one `data-render` replacement; -1 on missing/ambiguous sites or overflow. |

`rto_serve(port_file, config_file, ledger_dir)` is the implementation called
by the private child dispatch. A normal caller starts it with `rto_start`.

The configuration schema is `aethyr.render-test-origin.v1`:

```json
{
  "schema": "aethyr.render-test-origin.v1",
  "lifetime_ms": 180000,
  "routes": [
    {"url": "/", "file": "/owned/page.html", "mime": "text/html"},
    {"url": "/api/auth/viewer", "text": ""},
    {"url": "/test.wasm", "sequence": [
      {"file": "/owned/initial.wasm"},
      {"file": "/owned/alternate.wasm"},
      {"status": 404, "text": "planted replacement fetch failure"},
      {"file": "/owned/initial.wasm"}
    ]},
    {"prefix": "/package/", "directory": "/owned/package"},
    {"prefix": "/coaxial/", "directory": "/repo/coax/web"},
    {"prefix": "/", "directory": "/repo/apps/openagents-web"}
  ],
  "variants": [{
    "name": "keep-cache", "url": "/coaxial/render.js", "kind": "replace",
    "needle": "this.mediaCache?.clear();",
    "replacement": "/* planted: retained retired cache */", "enabled": false
  }],
  "pins": [{"file": "/owned/initial.wasm", "sha256": "64 lowercase hex digits"}]
}
```

Routes are checked in declaration order. An exact `url` or a `prefix`
selects one row. A prefix with `directory` appends only a validated sibling
path. An explicit `file` serves the same file for the whole matched prefix,
which is useful for an image fixture endpoint. An inline `text` is returned
literally. A response sequence consumes one row per hit and repeats its final
row; use an explicit fourth row to restore the initial response after a
third-request failure. A row defaults to status 200 and a MIME type selected
from `.js`, `.wasm`, `.json`, or `application/octet-stream`.

Variants match an exact `url`, or a `prefix` plus `extension`. `replace`
requires exactly one literal site. `append` uses `replacement` as its suffix.
Each active variant's original and served hashes remain distinct in the
ledger. An intentional variant does not change the original source file or
its fence. `GET /__rto/control/NAME/0` and `/1` select only configured names.

The port file is written after the configuration, pins, image and ledger are
ready. `ready.json` records the port, configuration/image hashes, lifetime and
capabilities. The server appends every request's method/path/status, original
file, original/served SHA256, byte count, applied variant and send result to
`requests.jsonl`. Missing files have no claimed original SHA256.

After closing the browser, request finalization, wait for child exit 0 and
call `rto_add_inputs`. `final.json` records `reason: "requested"`, the complete
original-file inventory, counts and a passed status only if the files stayed
unchanged. Deadline expiration produces a failed final record. A missing final
record, nonzero child exit or changed inventory cannot qualify a run. The
lifetime is bounded to 840 seconds; each socket operation is also bounded.

Without `post_sinks`, the origin drains a declared, bounded POST body before
returning the legacy fixture's 404 `not found` response. It rejects malformed,
duplicate-length, chunked, truncated or surplus framing. The earlier qualified
helper epoch and its five lifecycle comparisons remain in the immutable
`render-lifecycle-tool-port` archive.

An additive `post_sinks` array declares at most 32 exact upload destinations:

```json
{
  "post_sinks": [{
    "url": "/capture/layers.bin",
    "root": "/owned/check-output",
    "file": "/owned/check-output/layers.bin",
    "max_bytes": 8388608
  }]
}
```

The root and target parent must already exist. They are canonicalized and the
parent must be inside the explicit root; `/` cannot be a sink root. The file
must be absolute, have one simple final component, and be absent or an ordinary
file whose canonical path is the declared target. A symlink that resolves
elsewhere is refused. Pinned inputs, the origin's config/image/port/ledger, and
declared GET source files or source directories cannot be sink destinations.
Duplicate URLs or destinations are refused before readiness. A URL must be a
literal path outside the reserved `/__rto` namespace; query aliases never match.
The request URL supplies no filesystem path bytes.

`max_bytes` is an integer from 0 to 67,108,864. Every POST needs one bounded
decimal Content-Length. Length, route and destination are admitted before any
body write. Received binary bytes, including NUL, are streamed in chunks of at
most 65,536 bytes into an exclusive staging directory in the target parent.
The body has a 30-second deadline. Complete framing, exact file size and SHA256,
unchanged sources and the destination are checked before one same-filesystem
atomic rename publishes it. A partial body, excess read-ahead bytes, capacity
refusal, failed write or failed rename preserves the previous output. Staging
is removed on either path. The origin closes the connection after one request;
it does not accept HTTP pipelines or treat bytes after Content-Length as part
of the declared upload. This is an origin for caller-owned fixture trees, not
a sandbox against a hostile local process racing filesystem operations.

`ready.json` reports `POST_supported` and `declared_POST_sinks`. A configured
sink returns a JSON receipt, schema `aethyr.render-test-origin.post.v1`, with
the declared/received/stored byte counts, published flag, file, exact SHA256,
reason, status and staging cleanup fact. `stored_bytes: -1` and an empty hash
mean complete storage was not observed. A refused request's received count
describes bytes actually consumed; it never implies that a refused remainder
was read. Every admitted POST attempt, including an unknown exact route, is
appended to `post-sinks.jsonl`; malformed headers are refused earlier and remain
in `requests.jsonl`. Unknown paths return 404, framing/capacity refusals 400,
storage failures 503, and complete publication 200.

After publication, a ledger failure cannot retroactively make the complete
upload an HTTP refusal: the complete-publication ACK stays truthful, while
final qualification fails. No transaction across the artifact and ledger is
claimed. `final.json` separately records `post_attempts`, `post_published`,
`sink_outputs` with the final count/hash and current equality for each published
file, `sink_outputs_unchanged`, `post_staging_removed`, and
`post_ledger_complete`. These facts, unchanged original sources and requested
finalization must all pass before exit 0. Callers wait for that exit, use
`rto_add_inputs` for original sources, then add their declared output files to
their own artifact fence. An HTTP 200 alone cannot qualify a capture.

The current controls are in `render-test-origin-tool-checks.coax`. They use a
real loopback connection to check the response sequence, variant switch,
served hashes and finalization. The pure path/framing/byte-control decisions
are mutation checked in `render-test-origin-policy.coax`.
The additive sink decisions and actual binary/refusal/storage controls are in
`render-test-sink-policy.coax` and `render-test-sink-tool-checks.coax`.
