# Current host contract qualification

This record completes the current-code qualification for #12869. The
contract is [Aethyr host v1](../../../../aethyr/host/README.md). The public
Everglade face declares version 1 on web, Vulkan and Metal. Aethyrforge's
empty host selector chooses native Metal; `--world web` and the existing
**View → Check as Players See It** action open the same web world.

## Current inputs

The original admitted Coax 0.52 executables have SHA-256 identities
`23dfc150fa0d21dd199b2da382b9c2dd0762e8183e3f576657fe1341d327117e`
(Mac) and
`efaeeae664be88349d3aea9106867fcb2a8d33dc06d2679bac89b2809aeb55b3`
(Linux). Their original provenance records are retained here unchanged.
`observed-source-digest.txt` is only a preparation observation; it does not
replace a trusted receipt or waive compiler admission.

The native primary's actual emission profile and `producer-sources.json`
identify all 197 loaded sources. After rebasing, every recorded source still
matches its SHA-256. All render-package source inputs also match. The
concurrent browser-host recovery and optional storage-limit changes are
preserved; the browser checks run against that merged host.

The actual browser primary runs 1,404 checks in 775.6 seconds. Its four
source-keyed C units are reused for the final production-face link. The final
Wasm is 2,022,499 bytes, below the unchanged 2,150,000-byte budget. The final
link takes 9.8 seconds and reuses 19 of 20 clang objects. No generated C,
source cache key, original compiler receipt or capacity limit is rewritten.

The final production face also runs the same 90-frame capture. All 54 binary
artifacts (12,668,276 bytes), text draw/layer tables and product metadata match
the qualified GPU inputs exactly, apart from the explicitly changed Wasm
identity and owned dump directory. The strict same-producer comparer correctly
refuses that new module identity; its raw-byte results remain all true.
`production/` retains both the strict refusal and the explicit byte join.
Native preprocessing before and after the public opt-in is byte-identical,
so the qualified normal and instrumented native executables remain applicable.

The current shader package has 53 products. Its 51 shader, tree and
reflection products match the earlier qualified bank byte for byte; only
the two compiler/package identity records change. The current primary runs
90 real frames through Metal, uses native addresses above 4 GiB, and retires
every GPU object and word. Earlier stopped Mac primary/room/Wasm emissions
remain explicitly inconclusive in the parent Linux-window record.

## Actual host results

| Boundary | Evidence |
| --- | --- |
| Common defaults, measured buffers, input, inbox and assets | Actual web, Vulkan and Metal receipts in [core](core/); the same Coax receipt checker accepts each host |
| Real HTTP/WebSocket effects and retired owners | Current browser transport/replacement receipts in [core](core/), plus the unchanged [native transport](../../host-contract-2026-11-05/socket-linux.md) and [Mac acceptance](../../metal-host-2026-11-05/current-85c-final.md) source joins |
| Ordinary pack ownership, reload and GPU replacement | [Pack qualification](pack-world/README.md), followed by the merged-host production receipt in `production/` |
| Real Linux input, room, hidden messages, audio, resize, JPEG and shutdown | [89-field window qualification](../linux-window/complete/README.md), normally on RTX 4080 and with full producer/SDK ASan, UBSan and leak checking on Lavapipe; validation enabled with zero errors or warnings |
| Complete GPU handoff | [Current graphics](graphics/README.md): native/Wasm Metal has zero outliers; displayed native/Wasm Vulkan has 1,915 of 230,400 channels beyond the unchanged 0.002 rule, within the unchanged 2,000 live allowance; zero nonfinite or masked values |
| Authored audio and image bytes | Unchanged complete [153-source PCM/codec](../../host-contract-2026-11-05/audio-pcm-assets.md), [browser audio](../../host-contract-2026-11-05/browser-opus.md), [Linux decode](../../host-contract-2026-11-05/linux-opus.md), [ordinary Linux demand](../../host-contract-2026-11-05/linux-audio-demand.md), and [Mac device/session](../../metal-host-2026-11-05/current-85c-final.md) component results; actual current window verifies their integration and retirement |
| Room and worker packaging | [Fresh room/worker packaging](packaging/README.md); unchanged Wasm budgets, actual private room and headless browser drift |
| Native default and web preview | `production/` retains the failing old-default test, 109 passing session checks, all six default-policy mutants killed, and current AppKit link output. The unchanged [actual menu round trip](../../metal-host-2026-11-05/web-preview.md) retains same-seat and resource-release observations; its 24/108-mutant authorities are not rerun |

The complete contract combines these section receipts. A core fixture alone
does not qualify audio, transport or interactive behavior. The historical
records retain failed iterations and their original scope; the current
record supersedes their open current-primary/default exits without relabeling
those failures as passes. No native-only HUD or gameplay is introduced.

## Supplemental worker follow-up

The final worker is 1,042,322 bytes against the unchanged 1,060,000-byte limit.
Its independent Linux run passes every gameplay drift probe in all three
profiles, with zero stalls and dropped beat. Its p95 values, 810/910/956
microseconds, pass the written host contract's 11,000-microsecond worker budget.
The command's additional `--tick-us 600` optimization gate fails and exits 1.
The shared Mac records higher timings and one 250 ms NPC-condition mismatch
while unrelated jobs consume CPU. These failures remain open in #12981;
this record does not claim the supplemental command passes. The host-contract
completion does not replace that separate optimization qualification. The
[current Mac follow-up](../../worker-12981-2026-11-07/README.md) now completes
#12981 with 471/440/420 µs p95 and all gameplay checks passing; the historical
failures remain retained below.

The [worker qualification follow-up](../../worker-qualification-2026-11-06/README.md)
retains these failures, reproduces the oversized Linux worker, and fixes
the paired NPC fixture: its two nearest-person selections named different
people. Both current release artifacts pass all gameplay profiles and
raids with an observed common target. Their separate 600-microsecond
qualification still fails; #12980 and #12981 remain open for that gate.

## Storage and replay

Small immutable observations, profiles and logs are committed alongside this
record. Large C/header units, shader products, assets, actual GPU bytes and
normal/instrumented executables are retained under
`~/.openagents/aethyrforge/host-contract-12869/current-052-final/` on the
machines that execute them. Retained portable room C uses the worker's
documented `COAX_ROOM_KEEP_C` manifest, then the ordinary native SDK link
flags. The Mac emission timeout is not represented as a compiler pass.

Use the individual linked commands with new output directories. Heavy builds
run through the shared build gate locally or at nice 15 on coderos-4080.
Browser checks use muted headless Chrome; Linux windows use an owned headless
Weston compositor. No owner display, input, speakers, process or port is used.
