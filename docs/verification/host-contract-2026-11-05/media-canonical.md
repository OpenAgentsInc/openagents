# Canonical authored still media

Contract v1 browser media now uses the same Coax natural-image decoder as
native Metal. Pre-v1 browser engines retain their SDK decoder. Ordinary
presentation precision, the Everglade web opt-in and the desktop default
remain unchanged. These are component receipts; #12869 is still incomplete.

`aethyr/render/host-image.coax` admits complete PNG/baseline-JPEG input,
natural rectangular dimensions up to 2048 per axis, 8 MiB compressed source,
32 MiB workspace and 16 MiB straight RGBA8 output. The measured plan precedes
allocation; success publishes the complete image after exactly one alpha
round trip. There are no mips, SDK resizing, second premultiplication or
platform-decoder fallback. Its precise caller-owned ABI is
[`host-image.md`](../../../aethyr/render/host-image.md).

The separate browser reactor is built by `coax/web/build-media.coax ROOT OUT`.
Its C/JavaScript adapters project measured storage and copy successful output;
Coax owns codec and allocation admission. The browser uploads that output
with `writeTexture` into the ordinary Coax media descriptor. Async completion
checks the concrete media object/engine before allocation or readiness.
Malformed decoding refuses without a partial plan. Media failure is an
explicit failed/not-ready observation, rather than an SDK fallback.

## Complete image bytes

| Actual comparison | Logical bytes | Different bytes |
| --- | ---: | ---: |
| Old native SDK GPU–canonical Coax | 64,512 | **3,830: retained failure** |
| New browser Wasm decoder–new native GPU | 64,512 | 0 |
| New browser GPU–new native GPU | 64,512 | 0 |
| Historical browser GPU–new native GPU | 64,512 | 0 |

These are all seven authored 48×48 hotbar JPEGs, with exact compressed-file
and URL identities. The GPU comparison visits every byte of all 336 logical
rows, including alpha. WebGPU's 86,016 physical transfer bytes include only
recorded row padding; no logical texel is masked. Every new browser texture
hash also equals its retained historical browser counterpart. This preserves
the browser's authored image bytes while correcting native SDK decoding;
it does not imply that two independently timed complete frames are identical.

The native actual-GPU receipt also compares every byte after recovery, with
zero differences, three completed frames, pending zero and a closed ledger
of zero live resources/words. API/GPU validation is explicitly enabled and
clean. Browser source fencing passes. An additional actual browser malformed
input probe returns `[1,0,0,0,0,0]`: observed refusal and all five plan fields
zero, with no successful output object. Its source fence passes.

## Separate complete-frame measurements

The new production browser host capture retains its complete CPU buffers,
eight textures, seven media observations, actual display bytes and source
fence. It explicitly requests the disabled quantized presentation iteration;
no ordinary host selector is changed. The same captured RFI bytes replay
through the existing Vulkan/Metal operation interpreters.

| Matched complete workload | Channels past unchanged 0.002 rule | Nonfinite / masked | Existing live allowance |
| --- | ---: | ---: | ---: |
| New browser frame–Metal, same frozen RFI | 4 / 230,400 | 0 / 0 | 2,000 |
| New browser frame–Vulkan, same frozen RFI | 1,885 / 230,400 | 0 / 0 | 2,000 |
| Metal–Vulkan, same new browser RFI | 1,885 / 230,400 | 0 / 0 | 2,000 |
| Actual native 90–Metal replay of matched Wasm 90 | 27 / 230,400 | 0 / 0 | 2,000 |
| Actual native 90–Vulkan replay of native RFI | 1,697 / 230,400 | 0 / 0 | 2,000 |
| Actual native 90–Vulkan replay of matched Wasm 90 | 1,723 / 230,400 | 0 / 0 | 2,000 |

Every nonzero count exceeds the strict zero-outlier rule; all complete counts
are within the inherited live allowance. The last row measures actual composition
directly, without adding separate CPU/asset/backend counts. Its matched
90-frame workload is distinct from the normal browser capture; comparing
native 90 directly to that different browser frame would be inadmissible.
The native RFI replays byte-exactly to its actual native graph and display,
establishing the handoff. The earlier 2,213-channel native composition failure
with SDK media remains retained alongside this corrected result.

All three Vulkan measurements retain validation errors/warnings zero,
unchanged local/remote/SDK fences and successful owned cleanup. Metal
preserves both FP16 graph and actual presented bytes exactly after recovery.
The actual browser held-completion/recovery workload passes all 13 existing
observations: refused producer advancement while held, a fresh device/ledger,
late old completion unable to release the new pending frame or upload prefix,
old handles retired and resumed producer progress. The checks do not claim
ordinary Linux window/audio/session qualification.

## Provenance and gates

CPU emission uses the original admitted compiler image SHA
`598ddc2358d7fdefaead3a2add7947448f54d6935238acbbf4c0254e521a5c0e`.
Its genuine c224561457 compiler root passes the original receipt before and
after the separately pinned product overlay. The image source is public
dbd3934983 plus the retained browser carrier. Profile-report emission has
six exact source hashes and is byte-identical, C and header, to the C/header
retained from the actual Wasm build. The reactor is 51,577 bytes, within its
explicit 70,000-byte app budget. No compiler policy or release receipt changes.

The native 90-frame producer keeps its separately frozen a0f source/binary
identity. Private graphics products retain their existing iteration-only
status described in [the display experiment](display-quantized-iteration.md).
This component publishes no replacement shader package or compiler release.
The canonical codec's 47-mutant gate is unchanged. New observer checks were
written first; final root mutation kills all 87 mutants, with no survivors
or equivalents. Sharpening retained the earlier layout boundary failures.
The source-qualified browser probe retains its actual observer executable
identity and exact observer/reactor/effects source copies. JavaScript syntax,
the explicit image-reactor size gate and the required repository tools check
pass (1,786 tracked tools, 35 vendored vectors, no new missing reason headers).

Full sources, generated C/header/profile, original verification outputs,
raw browser images/readbacks, source fences, failed attempts and remote
payloads/logs remain in
`~/.openagents/aethyrforge/host-contract-12869/media-bridge/`.
Native old/new/recovery GPU bytes, exact compressed files, SDK/source snapshots
and matched handoffs remain in
`~/.openagents/aethyrforge/metal-host-12868/media-canonical/`,
`cpu-media-v1/` and `handoff-media-v1/`.
