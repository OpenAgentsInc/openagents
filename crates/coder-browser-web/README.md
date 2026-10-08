# Coder browser workbench

This Rust/Wasm mount connects a Cloud page to an explicitly enrolled native
host. Account sign-in does not grant native Terminal or Observe access. A
native invitation creates a fresh key in page memory, proves the disclosed
host and generation, and grants host-wide Terminal access. The account
workspace remains navigation context.

`start()` waits for Cloud's admitted privacy marker, reads the public
`#cloud-workbench-config` pins, and mounts inside `#cloud-private` at
`#cloud-workbench`. Standard fields, buttons, messages, Markdown, and the
registered terminal surface use Rust Native. The surface draws the actual
`coder-browser::Workbench`, `terminal-core`, and `terminal-gfx` grid through
the shared `verse-gfx` atlas and UI pipeline, using WebGPU or WebGL2. It does
not mount a Verse world or a local shell adapter.

Saved sessions preserve native member references and Live, Closed, or Lost
state. Input and resizing require the current snapshot and typist. Exact
shell proposals display their command, OS working directory, context digest,
and revision before a decision. Native threads are read-only through the
page's separately granted Observe right; other resource kinds show labels.

Hiding, leaving, or retiring the page aborts pending work, removes callbacks,
clears controls and GPU output, and drops keys, grants, terminal state, and
received records. Changed native session or thread sources also retire the
connection. Nothing is stored in browser storage or queued for offline replay.
Detaching leaves the host terminal alive; reconnect requires fresh admission
and a snapshot. Clipboard access requires a user gesture. Missing atlas glyphs
use the shared renderer's fallback glyph.

Build assets with `scripts/build-coder-browser-web.sh OUTPUT_DIRECTORY`. The
script uses the build lease, the pinned `wasm-bindgen` version, and
`--no-typescript`. It emits `coder_browser_web.js` and
`coder_browser_web_bg.wasm`; Cloud serves both from its configured asset
directory. `terminal_receipt()` returns content-free rendering and lifecycle
evidence for acceptance checks.
