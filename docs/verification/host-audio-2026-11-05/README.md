This checkpoint publishes shared Coax audio authorities for #12868/#12869.
It does not qualify either complete host, switch the desktop default, or
claim that the old Web Audio compressor matches the new portable dynamics.

The command was `COAX_JOBS=2 COAX_CHECK_JOBS=2 coax/bin/coax mutate FILE
--library --root-only --jobs 2`. Every root site gates; no equivalent
annotations were added. The component logs retain the final passing runs:

| Source | Killed / total | Log |
| --- | ---: | --- |
| host-effects.coax | 134 / 134 | audio-mutation.log |
| host-voice.coax | 206 / 206 | voice-mutation-automation-final.log |
| host-convolution.coax | 240 / 240 | convolution-mutation-modules.log |
| host-dynamics.coax | 57 / 57 | dynamics-mutation-ceiling.log |
| host-level.coax | 4 / 4 | direct tool output; recorded here |
| host-pcm.coax | 226 / 226 | pcm-mutation-ceiling.log |

Checks include direct stereo time-domain convolution across multiple blocks
and partitions, normalized impulse scale, workspace/view admission, scratch
sanitization, all PCM playback fields, independent voice exhaustion, repeated
IDs and bed reuse, first-sample automation, delayed stereo dynamics and its
linked output ceiling. Previously passing unchanged gates were retained.

`coax/web/tool.sh tools-check` was run in this clean foundation worktree.
It reports the same five existing missing Firm reason headers in
`verse/tools/humans/{face_rig,metahuman,outfits,pack_human,shape}.py`.
This checkpoint adds only Coax and documentation; those unrelated files
were not changed. `git diff --check` passes.

The trusted tool-runner image is the existing cached compiler at
`~/.cache/coax-release/b9ab17ba5e4b6eb808fa4a93ac7e2d3db3c91cf2053519dce5a7b62923447e00/coax`,
SHA-256 `598ddc2358d7fdefaead3a2add7947448f54d6935238acbbf4c0254e521a5c0e`.
It was not overwritten or promoted as a new graphics compiler release.

Earlier actual SDK observations remain under
`~/.openagents/aethyrforge/metal-host-12868/audio-current/`: 48 generated
bank assets decoded, authored 66-part room, finite rendered samples, and real
muted CoreAudio callbacks through pause/resume. This earlier carrier revision
preceded the dynamics lookahead/ceiling correction; it is not evidence for
that correction's device integration. The updated component checks above
are the qualification published here.

The independent old Web Audio graph compared against frozen Apple-decoded
48 kHz PCM under `host-contract-12869/audio-browser-first/` passes the existing
2^-20 simple voice bound: left max error 0, right 2.08e-10, filter with
960-sample delay 7.45e-9, stereo pan 3.73e-9. Bed cases also remain below that
bound. Mixed bus differences exceed it and are preserved; source and rendered
browser/native conformance, canonical WAV resampling, running worklet output
and complete host exits remain subsequent work. The ABI is documented in
[host-pcm.md](../../../aethyr/audio/host-pcm.md).
