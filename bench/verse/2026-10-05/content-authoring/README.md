# Content authoring acceptance

V20 adds a Rust content inspector and command editor over the runtime's admitted
contracts. The sample creates a playable chamber outpost from the original
licensed ritual assets, without editing renderer code.

`acceptance.json` records the actual CLI's initial and edited content identities,
actor health, friendly quest giver, collision, navigation, stable timeline,
invalid-giver diagnostic, and immutable generation reuse. Scratch paths are
replaced with labels. `outpost.svg` is the runtime projection's spatial and
timeline visualization. `document.json` is the edited document; `generation.json`
seals the published bundle. `mips.json` and `mips.rgba` retain its actual cooked
RGBA8 chains. The complete meshes and source PNGs can be regenerated through the
original compiler and the committed sample transaction.

The host integration loads the generated bundle, authenticates a local test
principal, accepts the authored quest, advances the world, commits, and recovers
the same checkpoint after the expected controller fence. Changing quest rewards refuses recovery under the previous
content identity. Its command check runs 60 authority ticks without a display,
real engine, or owner device.

Portable engine tests validate archive hashes, complete variant coverage,
contiguous dimensions and offsets, source identity, malformed payloads, and
combined source/archive memory limits. Editor tests cover numeric key aliases and
duplicates, restart and undo, exclusive writers, bounded sessions, corruption,
interrupted builds, rejected previews, generic hostile IDs, geometry winding,
authored navigation recovery, and preservation of custom mip variants after a
material edit and workspace reopen. A native renderer test uploads the persisted
archive and reads back color, normal, scalar, and mask samples.

`verification.json` names the actual commands, outcomes, executable hashes, and
retained artifact hashes. Logs preserve the checks and observed failures. The full content check predates
the final numeric-key, retention, and workspace-reopen fixes; the final authoring
and CLI checks exercise those changes. The full world suite passes 572 tests
with one storage-resume timing failure and three ignored tests; the timing test
passes when run alone. Both results are retained as a contention limitation. `source.json` identifies the
implementation commit and source inputs; `source.patch.gz` is the implementation
patch against its recorded main parent. Documentation and evidence commits after
that source commit do not change its implementation.

The shared disk filled during a world test build. The retry keeps the warm Cargo
target directory. Cleanup receipts identify obsolete or completed Verse test executables by
retained test logs or prior executable hashes, and record each removed file's
SHA-256 value. Completed checks retain their executable hashes. No Cargo caches
or owner processes were removed.

This evidence establishes admitted content production, upload, and recovery.
It does not establish a device frame-time target, a windowed 3D editor, compressed
texture formats, arbitrary geometry import, author productivity, or production
art quality.

The main integrations change `service/net/session_pipeline.rs` and add host
texture presentation behind `imported-surface` in `verse-pbr::imported`, among
the 254 recorded source inputs. `source-before-main.json` and its patch retain the
pre-integration source. `source.json` records the rebased implementation, and
`host-main.log` passes both integration tests against that transport. The final
checks pass 141 engine tests, ten authoring tests, one CLI workflow, four host
unit tests, two host integrations, 78 renderer tests, and one native mip
readback. Six other renderer tests remain ignored. Formatting and local
documentation links pass.

`surface-main.log` checks the merged `imported-surface` feature. Its new
presentation code does not change the archive upload paths exercised by the
renderer readback.
