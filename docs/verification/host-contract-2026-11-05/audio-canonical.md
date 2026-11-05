# Canonical audio and actual browser playback

All 48 actual current native bank WAVs decode to **bit-identical** 48 kHz
PCM through the browser's shipped Wasm scalar bridge. The native SDK exports
the complete producer WAV span before decoding; the browser receives those
exact bytes. Every actual mono/stereo word, including odd final mono sample
counts, is compared at the unchanged absolute 2^-20 bound. There are no
nonfinite values, shape differences or omitted samples. `audio-canonical-bank.json`
retains the complete inventory. The word comparator's root gate kills all
103 mutants, with no survivors or equivalent annotations; the stereo
wrapper keeps its original frame-position behavior.

The canonical bank also passes the same eleven actual AudioWorklet/native
render ranges. Nine are bit-identical; maximum room error is 2.3634e-19 and
loud error is 3.4772e-24. All ranges have zero samples beyond 2^-20 and no
nonfinite samples. No alignment correction or mask is applied. Mixed paths
start at frame 576 on both hosts. Every OfflineAudioContext reports real
process callbacks, device frames and Coax frames for the complete requested
extent. `audio-canonical-render.json` preserves those observations.

Normal version 1 `host.js` playback now selects this worklet. It copies
generated bank bytes before asynchronous work, decodes them through the
same Coax WAV implementation and serializes complete commands into the
same Coax PCM interpreter. A suspended context retains controls/beds but
does not turn pre-gesture cues into later sounds. Engine retirement closes
the concrete node/context and fences pending decodes. Pinned pre-contract
engines keep their existing Web Audio graph.

`audio-live.json` observes a real muted running AudioContext: a control
before activation returns 128, a one-shot cue returns -2, and neither
callbacks nor frames advance before an actual dispatched key gesture.
Suspend holds both counts exactly; resume advances them. Retirement during
a blocked decode returns -1 without delivering its command. `audio-product.json`
separately exercises the actual `host.js` with the real version 1 realm
fixture and canonical WAV: automatic suspended initialization, actual key
activation, accepted cue, world cover/pause, callback hold, resume and
same-layout engine replacement with a new owner and absent old context.
Chrome remains headless and muted throughout.

The reactor includes the qualified shared WAV checkpoint `b1bd74fcf2` and
is emitted by the admitted historical tool compiler SHA256
`598ddc2358d7fdefaead3a2add7947448f54d6935238acbbf4c0254e521a5c0e`.
This is current pure-Coax host input through that historical tool, not a
current compiler release. The dev reactor is 81,352 bytes, SHA256
`970291f288211c654a996b814bc41c0c4b3e693dbdd0190ede3c9a5c63c90290`.
The ordinary web build invokes `build-audio.coax` and has an explicit
size budget; its source/output budget inventory and shell syntax pass.
No release/deploy command or production access was used.

Immutable artifacts under `~/.openagents/aethyrforge/host-contract-12869/`:
`audio-native-canonical-bank/`, `audio-native-canonical/`,
`audio-browser-canonical/`, `audio-worklet-canonical/`, `audio-product/`.
`audio-canonical-artifacts.sha256` joins exact source, inputs and outputs.
The older differing Web Audio graph and earlier fixtures remain unchanged.

Reproduction uses a blank local audio page for component receipts:

```sh
nice -n 10 coax/web/tool.sh aethyr/host/check-browser-audio-bank.coax \
  LOCAL_AUDIO_PAGE NATIVE_BANK_AUDIO.json NEW_BANK_OUTPUT
nice -n 10 coax/web/tool.sh aethyr/host/check-browser-audio.coax \
  LOCAL_AUDIO_PAGE PLAN.json AUDIO.json NEW_RENDER_OUTPUT --canonical WAV_DIR
nice -n 10 coax/web/tool.sh aethyr/host/check-browser-audio-live.coax \
  LOCAL_AUDIO_PAGE NEW_LIVE.json
nice -n 10 coax/web/tool.sh aethyr/host/check-browser-audio-host.coax \
  SITE_ROOT LOCAL_WORLD_URL FIXTURE_DIR NEW_PRODUCT.json
```

Linux playback/output, complete three-host lifecycle/GPU qualification,
version 1 clip rendering and the desktop/default decision remain separate
exits. These audio receipts do not close either issue or admit a native default.
