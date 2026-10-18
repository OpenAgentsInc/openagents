# Android Lab and visual adapters (#12089)

These sources are not connected to a Gradle app entry point yet. Do not treat
source checks as Android compilation or device evidence.

`CoreSavedHistory.lab_projection` reads only a card already shown by the
account-bound history reader or live Lab row. It uses that document's existing
space and `lab-ui::card` to produce a Rust-validated view and typed media
registrations. `CoreLabCard` freezes the card contents, checks account/card
identity on each call, accepts navigation only after mount acknowledgment, and
resolves media only within the registered card. It checks again after awaiting
media, so a replaced/disposed card cannot publish old bytes. Disposal is terminal.
Media limits reuse the reader's image/model bounds; navigation and video links
remain Rust results, not paths assembled in Kotlin.

Pass this owner to `RustNativeLabCardBridge.show`. Its production factory mounts
through the landed `SemanticViewAdapter`. Callbacks carry only instance,
revision, node, and raw field values. A failed mount retires callbacks and
closes its resource owner. Do not use the internal fixture seam for remote JSON.

A `LabSurfaceRenderer` must be registered before a media card is mounted. It
receives only Rust's typed registration; load artifacts through the issuing
`CoreLabCard.media` with the exact instance, revision, resource, and artifact.
Never resolve a resource as a URL or fetch HTTP in Kotlin. Missing resources
fail mounting; they do not display a label as a substitute. The renderer owns
its player, bitmaps, and native Rust handles, including cleanup if opening fails.
Use `OwnedVisualLabSurface.create` for a model's existing Rust visual handle
from `TranscriptView.lab_model`. Lay it out with `AndroidVisualView.layoutAt`.
`AndroidLabMediaRenderer` supplies native image, video, comparison, poster, and
Rust model-worker mounts. It takes exact projection identity, the shared
transcript, the Rust layout's media height, and a UI scope. Its only artifact read
is `CoreLabCard.media`; signed video links are passed to Android's native player.
Decode cancellation releases unclaimed bitmaps, image dimensions/bytes are
bounded, and close cancels pending work and releases playback/model/image state.
B1 still must register this renderer and connect navigation/full-screen playback;
this source slice does not claim that runnable integration.

`AndroidVisualView` detaches offscreen workers and releases bitmaps, closes
replaced native handles, and resets delivery on layout/rotation and binding
changes. Every posted display and frame drain checks its generation. An old
queued drain cannot clear or unschedule a replacement queue. Pending frames
have count and byte bounds; the cache accounts for pixels and retained frame
data. Raster allocation/copy failure releases partial bitmaps and keeps Rust's
vector/text fallback. Rust owns GPU fallback and loading/failure content.

## Checks still queued

No Cargo, Gradle, JUnit, Kotlin compiler, emulator, or device check ran for this
follow-up. The four checked-in Android JSON documents are authored test fixtures,
not captured Rust or device output. A new Rust test validates them against
`rust_native::View<lab_ui::Intent>`; that test has not run.

After fresh free space meets the 100 GiB floor and the serial slot is available,
from `apps/openagents` with a full checkout:

```sh
export CARGO_TARGET_DIR="$HOME/work/openagents-target-1-app"
export CARGO_INCREMENTAL=0 OPENAGENTS_KEEP_TARGET_DIR=1
. "$(git rev-parse --show-toplevel)/scripts/dev/cargo-build-env.sh"
cargo test -p openagents-mobile --lib client_ffi::labs::projection::tests
```

Then regenerate bindings using the existing Android/iOS build-core procedures,
compile their consumers, and run these focused Kotlin test classes after B1
registers the platform sources and app test source set:
`RustNativeLabCardBridgeTest`, `VisualFrameMailboxTest`,
`LabSurfaceResourcesTest`, and `AndroidVisualFrameBoundsTest`. The exact Gradle
command waits on the app module wiring; no runnable Gradle target exists at this
checkpoint's main base. Verify API 26/35, loading/failure, offscreen reuse,
rotation, real GPU fallback, bitmap cleanup, bounded input, TalkBack, and native
media playback with synthetic state. Do not use owner-account content.
