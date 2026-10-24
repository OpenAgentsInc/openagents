# OpenAgents host adapter

The editor can be embedded by the Rust host SDK in `crates/aethyrforge-host`.
The engine and renderer now live in this repository too (`apps/aethyr`,
`crates/aethyr`). No purchased assets are copied into the repository.

Build-time configuration is optional. All three values must be set to enable an
embed; standalone editor builds need none of them:

- `VITE_OPENAGENTS_HOST_ORIGIN`: exact approved host origin, without a trailing slash.
- `VITE_OPENAGENTS_PROTOCOL_URL`: operator-pinned URL of the public SDK's generated
  `aethyrforge_host.js`, with its matching Wasm beside it. Use an immutable release
  path in production. Messages and query strings cannot choose this URL.
- `VITE_OPENAGENTS_ENGINE_RELEASE`: release identity the host must pin independently.

`VITE_PREVIEW_URL` selects the isolated preview. `VITE_EDITOR_ORIGIN` fixes the
preview's expected editor origin. These must agree with the deployed origins.
Production uses separate registrable domains for the host, editor, and untrusted
project preview; localhost ports exercise only message routing, not that deployment
security boundary. Configure response headers and immutable artifact delivery as
OpenAgents #12546 describes. Disable source maps and keep authoring assets private.

The public SDK owns schema validation, the exact-origin/parent-window bootstrap,
the fresh session, scoped MessagePort, sequence checks, queue/byte bounds, request
results, and status rate limit. The private adapter only maps accepted requests
to this editor service. The earlier selection slice requires public SDK wire version 2. Prior
pinned v1 releases remain unchanged. Inspection, selection, and observation work
as guests. The house additionally accepts one bounded move after the Rust parent
checks its trusted service and sends a short-lived grant on the scoped channel.
The local guard binds the exact action hash, release, project, expiry, and one
use; it is not the durable authorization boundary. Revocation preserves guest
exploration. Embeds hide the standalone editor controls. Publish, share, money,
and durable project saving remain separate service capabilities. House candidates
are never written into shared browser storage.

A single `EditorService` survives panel changes and HMR. Status includes the
complete eye/target/mode checkpoint. Replacing the frame starts a new service and
worker, then restores the checkpoint after readiness. It does not preserve GPU
objects or promise recovery into a physically incompatible world. Closing removes
the preview frame, stops observation, and closes the service and port.

For local artifact conformance, run `python3 tools/host-fixture.py OUTPUT` and
serve that generated directory as `/private-assets/` beside a built editor and
preview on their configured origins. This creates an original triangle and empty
catalog, with no vendor files. Run the public `scripts/check-aethyrforge-host.mjs`
through `openagents browser run`, pointing `AETHYRFORGE_TEST_ENGINE` to this built
editor. This proves transport and real engine lifecycle behavior, not Haunted
House visuals, authorization, production isolation, or persistence services.


The editable-project adapter uses public wire version 6. Frozen v1–v5 releases
keep their pinned SDKs. Export returns bounded creator content, never fixed
collision, compiled geometry, renderer source, credentials, or player presence.
Restore requires exact parent-confirmed authoring permission, matching world,
asset-set and scene identities, and a complete kernel/GPU candidate before
adoption. It clears local undo history; the public account project service owns
accepted saved versions and actor-aware compensating undo. The personal camera
checkpoint is checked against the new collision; a blocked place uses the
validated deployment spawn or refuses the load without changing the world.

Set `VITE_OPENAGENTS_ASSET_SET` to the immutable runtime asset-set SHA-256 for
editable releases. The adapter refuses export when either that identity or the
prepared source scene identity is absent. Device recovery, existing account
project membership/storage, original OPFS export/import, and save labels belong
to the public Rust host. A private preview receives no account token. Material
layer PNG storage is a public contract; rendering and editing those layers are
qualified with the material-authoring implementation, not by saving metadata.

The wire v6 face-material adapter keeps generated and handmade 256 × 256 PNG
layers separate, with a 16 KiB limit per image. A temporary preview changes only
the displayed face; confirmed Apply stores source references through the same
kernel prepare/commit history as other edits. Each face override uses one object
and material binding, reusing the existing color texture array rather than adding
a sampled-texture binding. Failed image decoding, GPU staging, or stale work
keeps the accepted document. Source text keeps its exact bytes and hash up to
24 KiB, within the public 48 KiB project bound. JSON member order has no meaning
for checked material values.

The material mechanics check (`tools/check-material.mjs`) uses real PNG decoding,
the private kernel and WebGPU with canvas-drawn test images. It checks a localized
face color change, read-only preview/cancel, Apply/undo/redo/restore, pretty source
round-trip, and size/image refusal. It does not qualify the public painting UI,
local scar generator, account/provider sign-in, speech, or the full creator journey.
