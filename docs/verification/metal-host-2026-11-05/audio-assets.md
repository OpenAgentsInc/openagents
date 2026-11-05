# Native authored audio adoption

The native adapter consumes the complete 153-candidate catalogue introduced at
`7142ecdfc0`. It verifies each pinned Ogg source before SDK decoding, uses the
required Coax APC1 decoder for 32 assets, and preserves SDK decoding for the other
121 and unlisted safe names. Required companion failure publishes no PCM and
never calls the platform decoder. Original Ogg files remain in the package.

`audio-encoded.coax` reserves every encoded source/companion byte, rounded up to
F32 words, within the existing 8 MiB/1 MiB source profiles before the SDK read.
The ticket survives all borrowed byte views. Full coded Opus extent is reserved
before the AVAudioPCMBuffer allocation; measured input and final output must
satisfy `hpo_output`. Converter scratch and retained PCM retain their separate
tickets. The existing 128-ticket/8,388,608-word budget is unchanged. Private
AVFoundation codec workspace is not reported as measured application PCM.

## Actual observations

All paths below are relative to
`~/.openagents/aethyrforge/metal-host-12868/`.

| Gate | Actual result |
| --- | --- |
| `audio-assets-normal`, `audio-assets-asan` | 153 independently loaded/released assets; 121 SDK and 32 Coax companion decodes |
| Complete frozen browser comparison | 12,657,718 words; zero nonfinite values and zero words beyond unchanged 2^-20; all 32 companion payloads bit-identical; no shifts or masks |
| Refusal trace | Missing companion, corrupt companion and corrupt source each publish no PCM with zero SDK/companion decode increments; valid companion, known SDK and unlisted safe assets subsequently load; full budget refuses before decoding; every temporary owner releases to zero |
| `audio-assets-prefetch-network`, `audio-assets-asan-prefetch` | Frozen producer's 16 CPU frames emit 126 command rows: all 120 variant fetches, ambient bed and room ready; 122 retained assets/7,739,161 words, then zero; zero device frames |
| `audio-assets-session`, `audio-assets-asan-session` | Existing actual muted source-device/realm 32-word session gate passes: gesture, covered clocks, resumed callback plus Coax advancement, physical packet refusal and terminal retirement |

Normal and instrumented comparison JSONs are byte-identical. The complete
normal comparison is [retained here](audio-assets-comparison.json). Each PCM is
compared to `host-contract-12869/audio-picked-bank/NAME.f32`, or the corresponding
`audio-authored-extra/NAME.f32` for the additional candidates. Exact Ogg bytes
and catalogue source identity are checked for every row. The preserved raw SDK
32-failure inventory remains a separate record; no old output is relabelled.

`audio-assets-conformance.coax` kills 53/53 root mutants; encoded-byte admission
kills 12/12, with no survivors or equivalent annotations. The existing PCM,
catalogue, Opus, bank, allocation and session authorities are unchanged and their
previous sharp gates are cited rather than rerun. `tools-check` passes.
AddressSanitizer covers the changed Swift/Objective-C SDK and generated facade;
macOS leak detection is explicitly disabled. Frozen producer images remain
separate, previously qualified inputs; this does not claim new instrumentation
or current-primary admission for them.

The first bundle without companions refused the 32 required rows
(`audio-assets-build`). The first default-prefetch staging lacked
`metal/libnetwork.dylib` and stopped on the next producer frame
(`audio-assets-prefetch`); staging the previously qualified socket SDK corrected
that fixture. These failed staging observations remain retained.

## Source and executable join

The facade was emitted by the original admitted compiler SHA
`598ddc2358d7fdefaead3a2add7947448f54d6935238acbbf4c0254e521a5c0e`
in a fresh genuine `c224561457` compiler root, with pinned `7142ecdfc0` product
inputs and the exact separately retained `metal-render.coax`/`audio-encoded.coax`
overlay. Original provenance verifies before and after the product overlay and
after emission; no replacement receipt is written. The development-profile
emission takes 9,681 ms and emits 1,404,334 C bytes. Its real profile, C/header,
argv/cwd, 83-source exact join and objects are in `audio-assets-sdk/`; the join
has zero differing sources. The reusable RenderTexture object is unchanged.

`audio-assets-sources/` freezes adapter/harness sources and
`normal-artifacts.sha256`/`asan-artifacts.sha256`. Copies of those manifests are
[normal](audio-assets-normal.sha256) and [instrumented](audio-assets-asan.sha256).
The existing frozen bank producer has SHA `73e8c67c…7150440`; the packet-capacity
overlay producer used for default startup has SHA `b5a90a0e…448deec` (full values
in the manifests). Their original primary closure remains the previously
recorded historical snapshot, rather than current main. All builds are niced
and bounded. No GPU benchmark is rerun.

These are audio-adapter gates. Current primary packaging, graphics compiler
admission, ordinary integrated display parity and the complete host contract
remain separate. The desktop default stays web, experimental presentation
selection stays disabled, and #12868/#12869 remain open.

Run the actual adapter with `metal-audio-prefetch.coax --phase canonical`, then
`metal-audio-assets-compare.coax` against the two frozen browser directories.
`--phase prefetch --require-ready` checks the independently observed startup
trace. Use the same source/executable join and explicitly scoped producer.
