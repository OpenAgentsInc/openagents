# Shared Coax PCM in a real AudioWorklet

The browser SDK component now runs the same `ap_context_command`/`ap_block`
interpreter as Metal, through a real AudioWorkletProcessor and an
OfflineAudioContext. Every context observes actual process callback count,
device frames and Coax rendered frames; those counts match the complete
requested extent and no processor error is present. All 11 retained output
ranges pass the unchanged absolute 2^-20 bound against the updated native
`a464d93b03` fixture, without shifting or masking any sample.

Nine ranges are bit-identical. Maximum room error is 2.3634e-19 and loud
error is 7.6577e-24. Both mixed paths begin at frame 576, and filtered delay
begins at frame 960 on both hosts. Rate, room, bed/update, stop/tail and loud
cases now all receive the sample gate; their earlier material differences
remain retained in `audio-iteration.md`.

`audio-memory.coax` owns 128 immutable PCM allocations and a 32 MiB sample
budget, with atomic reservation and allocation-failure release. Its root
gate kills 48/48 mutants, with no equivalents. `audio-face.c` projects the
Coax API and supplies measured linear-memory allocations. The JavaScript
bridge/worklet moves bytes and copies 1024-frame blocks into the browser's
128-frame output quanta. It implements no filter, mixer, envelope or room.
The observation scheduler dispatches the exact Coax-authored command/frame
sequence through that same bridge; it does not replace audio arithmetic.

This reactor was emitted with the admitted historical tool compiler
SHA256 `598ddc2358d7fdefaead3a2add7947448f54d6935238acbbf4c0254e521a5c0e`
against current pure-Coax audio sources. It is not a current compiler
release or a claim that its historical compiler sources match current main.
The dev reactor SHA256 is
`9e13dc8094abe39ceee238ec16a2d54c0f3b1c3508e7ef2265ee362c970432ef`.
It has no graphics backend dependency.

`audio-worklet.json` retains the sample and actual callback observations;
`audio-worklet-artifacts.sha256` identifies source, native inputs and browser
outputs. Immutable full artifacts are under
`~/.openagents/aethyrforge/host-contract-12869/audio-native-delay/` and
`audio-worklet-first/`.

The component is exercised with a blank local audio page, so no world GPU
work or live speaker output competes with native measurements:

```sh
nice -n 10 coax/web/tool.sh coax/web/build-audio.coax "$PWD"
nice -n 10 coax/web/tool.sh aethyr/host/check-browser-audio.coax \
  LOCAL_AUDIO_PAGE PLAN.json AUDIO.json NEW_OUTPUT --coax
```

Product v1 host wiring, real muted running-context pause/resume, canonical
WAV decoding/resampling, Linux output and final three-host receipts remain
separate exits. The pre-contract Web Audio graph remains the independent
legacy observation path; this component result alone does not switch it.
