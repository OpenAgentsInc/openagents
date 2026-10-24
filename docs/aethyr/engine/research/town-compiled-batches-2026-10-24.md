# Town compiled batches and prepared lighting experiment

Status: implemented diagnostic candidates for [#8](https://github.com/OpenAgentsInc/aethyrforge/issues/8), October 24, 2026. The default renderer retains its original batch order and material program. Neither candidate is a qualified performance improvement. Phase 1 of the [renderer redesign](../town-renderer-redesign.md) remains in progress.

## Implemented changes

- An owned batch compiler groups repeated uses of identical geometry, effective material, winding, and wind identity inside compatible opaque segments. Special draws form ordering barriers. Index and vertex bytes stay unchanged; the compiler does not bake object transforms into geometry or reduce texture quality.
- A versioned host caches plans. Unchanged frames reuse the plan; layout or material eligibility changes refresh it. Separate camera and planar gatherers preserve original model/wind bits and placement IDs. Damage masks remove every primitive use of a destroyed placement. Selection, collision, navigation, and authored commands retain their existing authoritative tables.
- Complete source-to-group and group-to-source mappings stay available. Dense indirect-command IDs are distinct from original placement IDs. Capacity limits reject the complete optimization before allocation, including the fixed zero-wind buffer limit. Unsupported and custom shader paths keep their original draws.
- The prepared BRDF candidate computes light-independent GGX terms once per nonempty local-light cluster. It preserves the per-light math, attenuation, shadows, transmission, helper signatures, and creator material insertion hook. Generated source stays under the existing 65,536-byte ceiling with at least the original edit headroom.
- Per-accepted-frame counters record encoded source draw-loop commands, separately from cached command inventory. Counts include admitted zero-instance indirect slots and exclude authored/character/effect/postprocess draws and caller setup outside the source loops. They do not claim every GPU instruction or displayed frame was observed.
- An offline comparator verifies actual RGBA bytes, camera and native actor pose metadata, stable source/asset pins, raw window files, and unchanged pacing checks. Missing observations remain unavailable. Evidence validity and performance qualification are separate outcomes.

The compiler and gatherer are TypeScript prototype components behind the trusted editor feature boundary. They are not the specified Rust artifact compiler, a durable content-addressed batch format, GPU visibility, or a complete scene-generation publisher. Existing rendering/collision/navigation activation behavior is unchanged.

## Safety and ownership

Both changes are opt-in diagnostics:

```sh
VITE_COMPILED_SOURCE_BATCHES=1 VITE_PREPARED_SURFACE_BRDF=1 npm run dev
```

Use either flag independently. Normal editor and published-player profiles keep the original renderer. Open Ground/MMORPG scenery, shadow-view compilation, and custom material programs are excluded from the new batch path. Render bundles remain the existing bounded indirect submission mechanism.

OPAQUE alone does not prove draw-order independence. Nonadjacent regrouping can change the winner at coplanar surfaces. Exact tested images are evidence for those views, not a proof for every camera or edit; this is why the candidate is not enabled by default. Transparent, masked, vegetation, wind, glass, flame, and refined-sky paths retain their ordering barriers or original path.

Plans and per-view slabs are bounded and released on replacement/close. A failed upload cannot publish a valid cache signature; retry refreshes the submission. In-place source draw/material changes must advance the source version. Host validation checks the layout on version changes while unchanged frames use a constant-time cache lookup.

## Measured source-table reduction

For the retained private Town closure, after source filtering, wind preparation, surface-sky handling, and winding splitting:

| Scenario | Original complete table | Compiled table | Reduction | Plan + one gatherer typed arrays |
| --- | ---: | ---: | ---: | ---: |
| Day | 3,911 | 2,212 | 43.44% | 1,835,548 B |
| Night | 3,885 | 2,226 | 42.70% | 1,837,812 B |

These are complete source tables, not visible draws or frame rates. Typed-array figures exclude JavaScript metadata, browser, and driver allocation. The measured candidate held four compiled camera/planar views across both scenarios: 290,176 B of distinct plan arrays and 6,766,368 B of gather arrays. Primitive memberships duplicate models where needed, so this candidate increases instance storage and CPU copying. It still runs the original CPU visibility/compaction before its gather; GPU-resident preparation remains future work. The older `preparedBufferUploadBytes` counter records preparation/allocation uploads rather than every steady-state model/wind write; it is not used here as total per-frame upload evidence.

## Browser evidence

The pinned comparison starts at `6b35a92`, with shared command-counter instrumentation. The kernel is rebuilt from that revision for both checkouts. Private assets are unchanged: 470 files, 637,679,875 B, closure SHA-256 `2a41779e89ddf88050522da8a757bacdb4af7352bae9cfb563092d2069571dd5`. Browser: Chrome 155.0.8059.85 on the same local Mac. All runs use 1280 × 720, original lighting and effects, and 4× MSAA.

The first controlled pair uses the night spawn view, moving-camera route, 5-second warmups, and 30-second observations. The 100,000 creatures retain the full 310-triangle model: 31 million submitted creature triangles. No population, resolution, texture, shadow, or effect reduction is used.

| Workload | Original submissions/s | Batches + prepared BRDF submissions/s | Original GPU envelope median | Candidate GPU envelope median |
| --- | ---: | ---: | ---: | ---: |
| Town + 100,000 creatures | 45.957 | 40.287 | 17.105 ms | 20.185 ms |
| Town alone | 47.517 | 46.149 | 14.746 ms | 14.221 ms |

In the 100,000-creature window, median encoded source commands changed from 8,136 to 5,748 draws (29.35% fewer), 1,670 to 1,638 pipeline binds, and 4,626 to 4,602 material binds. The small binding reduction explains why this is not yet the proposed material-page renderer. Render CPU median increased from 3.8 to 4.6 ms; culling/gathering is still CPU work. Both implementations failed the unchanged pacing contract.

Source-only captures before and after clearing the swarm match byte for byte across the two implementations, including camera and the canonical eight native actor poses. The comparison checks the actual 3,686,400-byte RGBA files. This establishes equality for the sampled source view only. Live swarm positions are not a cross-run pixel control.

Chronological runs do not isolate a causal GPU-speed change, especially on a shared machine. The earlier 10-second baseline also differed from the later 30-second cohort. The negative timings are retained; fewer commands do not justify claiming faster frames. Two attempted shader-only follow-ups closed their browsers before producing complete windows; both failed reports are retained and are not timing evidence. A multi-shot quality run also stopped with a command receipt timeout after one complete day window. Separate single-view checks completed; the multi-shot driver still needs investigation before full qualification.

A second pair uses committed implementation `66db899`, identical source-file pins,
the same night/spawn/sway workload with 100,000 creatures, and isolated **headless**
Chrome. Each observation is 30 seconds after a 5-second warmup:

| Variant | Submissions/s | GPU envelope median / p95 | Render CPU median |
| --- | ---: | ---: | ---: |
| Both diagnostics enabled, run first | 55.260 | 13.238 / 16.581 ms | 4.1 ms |
| Original defaults, run second | 38.889 | 20.840 / 25.474 ms | 3.7 ms |

Both pacing verdicts fail. The direction reverses the earlier headed comparison;
these chronological cohorts do not establish a portable gain or identify one
candidate's contribution. Headless and headed cohorts are separate workloads and
must not be pooled into one rate. All raw observations remain in the summary.
The final code is reproducible from its commit; earlier dirty working-copy runs
retain source hashes but are not a substitute for a committed crossover study.

The offline final comparison passes exact night source images and metadata.
Separate final single-view day controls also match exactly. Both trusted feature
flags are checked against the active renderer before those measurements. Each
completed run reports stable source/assets and zero preview workers after close.

The GPU envelope covers render/compute passes, including the swarm; it excludes uploads, copies, query readback, browser composition, and physical presentation. CPU spans overlap each other and GPU execution. Do not add them into a frame time.

## Checks and delivery limits

At `66db899`, 1,318 tests pass and two optional fixtures skip; TypeScript and whitespace checks pass. Real WebGPU lifecycle checks pass startup-failure cleanup, deliberate repeated shutdown, and unexpected device loss. The kernel rebuild produces 888,480 B.

The implementation has focused compiler, actual-renderer, arithmetic, lifecycle, and evidence-integrity tests. Renderer integration checks independent uploaded placement data and indirect words through damage, restore, moving views, versioned metadata edits, custom shaders, unsupported indirect adapters, buffer limits, failed uploads, and close. Float32 arithmetic cases are not a substitute for GPU image checks.

The existing room player byte gate already fails on the pinned main baseline: 694,163 B against 655,360 B. Final `66db899` is 697,355 B, adding 3,192 B. Gzip, Wasm, asset, and file-count checks pass; raw JavaScript still fails. No ceiling is raised and no player-adoption claim is made. The compiler host is excluded from the player graph, but shared diagnostic/shader selection still adds bytes. Restoring delivery headroom and checking every profile remains required before qualification.

Private captures and raw reports stay outside Git. The [derived evidence summary](data/boids/town-compiled-batches-2026-10-24.json) lists their identities and local scratch locations without vendor geometry, textures, or pixel bytes.

## Next work in #8

1. Keep both diagnostics disabled by default until a controlled independent comparison establishes an improvement and the complete fixed-view and edit matrix passes.
2. Compile spatial/material pages that reduce material and pipeline bindings, with explicit equal-depth ordering rules and durable source mappings. Move this into the specified artifact compiler before claiming the full Phase 1 output.
3. Replace the CPU compact-then-gather duplication with resident GPU per-view visibility and stable membership compaction. Preserve an independent CPU reference, complete overflow fallback, source IDs, and the existing custom path.
4. Reuse shadow/reflection outputs only when their recorded dependencies permit it. Do not remove an effect or skip a required update to meet the budget.
5. Implement the planned bounded latest-frame scheduling and version its clock evidence. Retain the actual failed pacing results.
6. Restore delivery headroom and run both resolutions, repeated windows, all source views, day/night, static/moving routes, editor/player, controllers, edits, spells, destruction, knights, and aiming before closing #8.
