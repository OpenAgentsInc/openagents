# Browser v1 clip audio

The ordinary v1 `host.js` clip now uses the same Coax PCM interpreter,
canonical asset decoder and real AudioWorklet as live audio. The previous
[ordinary-host failure](browser-clip-audio/product-red.log) returned a WAV
through the pre-contract Web Audio graph, with no v1 receipt and a live
suspended AudioContext. The corrected adapter starts no live audio device.

The [merged ordinary-host run](browser-clip-audio/merged-product.json) passes
all 32 measured fields and independently checks every header byte and all
9,600 PCM16 words of the actual [19,244-byte WAV](browser-clip-audio/clip.wav).
It observes two distinct command times/pans, a 3,200-frame pre-roll crop,
4,800 output frames, 8,192 actual Worklet frames/64 callbacks, two accepted
commands, no refusal and one eight-word asset. Full history is rendered
from clock zero before cropping. All timeline, asset, Worklet and output
owners reach zero after retirement. No live AudioContext exists or starts.
This is a bounded recording-face dispatch/PCM receipt, not a new full-game
performance or complete three-host/default result.

`audio-timeline.coax` admits at most 8,192 complete commands/8 MiB, each
1..1,024 encoded bytes. Bad input poisons the whole recording; a successful
seal freezes its frontier. Microseconds round to 48 kHz frames and commands
enter at the next 1,024-frame mixer boundary. The complete offline output
and crop reserve existing shared PCM tickets before allocation, under a
separate 32 MiB owner from the ordinary decoded-source bank. Both complete
output and crop coexist within that allowance; no existing limit is raised.
SDK hidden workspace remains separate from measured application copies.

The actual ordinary-host failure receipts cover [8,193 commands](browser-clip-audio/final-count.json),
a [1,025-byte line](browser-clip-audio/final-line.json), a [120-second output](browser-clip-audio/final-pcm.json),
and a [missing required sound](browser-clip-audio/final-missing.json).
Each rejects the complete WAV, preserves bounded accounting and returns both
PCM owners and timeline storage to zero; the checker requires all 17 fields.
No successful or silent prefix is published.

The real [held-descriptor retirement](browser-clip-audio/final-retire.json)
checks 15 fields. While the descriptor is held, the inactive old timeline
still owns its one command/13 bytes and the output owns 19,840 words;
retirement remains unfinished. Releasing the descriptor rejects that
recording and releases every old owner. An independent new timeline keeps
its own one-command frontier. The [earlier failing trace](browser-clip-audio/retire-red.json)
is retained: it prematurely cleared the timeline while output was still
owned. The corrected live adapter tracks descriptor resolution as well as
installation, and recording retirement waits for those concrete effects.

The timeline [root mutation gate](browser-clip-audio/timeline-mutants.log)
kills 103/103 mutants. The independent PCM/ownership observer
[kills 202/202](browser-clip-audio/observer-mutants.log), with zero survivors
or equivalent exclusions. Its WAV oracle checks every silent frame and both
pulses rather than accepting only output length or peaks.

The normal small audio reactor was rebuilt through `wasm.sh`, not relabelled
from the earlier 105,010-byte artifact. The actual emitted
[24-source join](browser-clip-audio/current-audio-sources.json) has zero
mismatches; [the old artifact audit](browser-clip-audio/audio-source-join.json)
has two changed dependencies (SHA secure-zero defers and JSON module marking).
The compiler is the genuinely qualified 0.50 image at
`~/.cache/coax-release/c82201e2cfff7c937fc0a29bf7010ae1ec75e7ab746ad29548bac7c2db61409e/coax`,
SHA256 `b5872254e02de4b2fb84f039e81f602090f5526301c036c077004294a7e580eb`.
Its original qualification remains scoped to that original compiler-input
closure, not later compiler edits. The successful normal emission's E0400
90%-step note is retained in [the build log](browser-clip-audio/current-audio-build.log);
no check limit or source was changed to make that build pass.

The browser independently hashes its [actually fetched reactors](browser-clip-audio/artifacts.json):

| Reactor | Release bytes | SHA256 |
| --- | ---: | --- |
| Audio | 96,229 | `4c71171b5025a5ebde14b318ab58d6d05dfe04ebe3870c7c8d8181925da13eb1` |
| Clip timeline | 16,649 | `7f60f0167a89a19972d3c2164693e7e4a9c51682492500500a8693b2189e976d` |

The unchanged audio budget is 140,000 bytes; the independent timeline budget
is 20,000. Web and desktop build hooks build/package both. The merged desktop
copy list also preserves the separately landed socket reactor. The
[prepared fixture/services/socket and SDK source hashes](browser-clip-audio/sources.json)
keep these qualified components distinct from a newly rebuilt full realm.

The freshly rebuilt ordinary audio adapter also passes the existing authored
cue/voice/bed/room/tail [sample comparison](browser-clip-audio/default-render/comparison.json):
all 262,144 actual samples, unchanged 2^-20, zero nonfinite or over-bound
values. The four observed F32 files are retained alongside it. The unchanged
plan, seven pinned original/companion inputs and native reference samples
remain under `~/.openagents/aethyrforge/host-contract-12869/browser-default-render/`
and `~/.openagents/aethyrforge/metal-host-12868/audio-default-render/`.
No 153-codec gate is relabelled or repeated by this regression check.

Rerun the changed slice with a prepared directory containing the recorded
v1 fixture Wasm/WGSL and `build/host-{audio,audio-timeline,services,sockets}.wasm`:

```sh
nice -n 15 env COAX="$QUALIFIED_COAX" COAX_JOBS=1 COAX_CHECK_JOBS=2 \
  coax/web/tool.sh coax/web/build-audio.coax "$PWD" \
  "$PREPARED/build/host-audio.wasm" "$PREPARED/build/host-audio-timeline.wasm"
nice -n 15 env COAX="$QUALIFIED_COAX" \
  coax/web/serve.sh "$PREPARED" --static --listen 127.0.0.1:61282
# In a second shell; use a new output name for each optional mode.
nice -n 15 env COAX="$QUALIFIED_COAX" COAX_JOBS=1 COAX_CHECK_JOBS=2 \
  coax/web/tool.sh aethyr/host/check-browser-audio-timeline.coax \
  "$PREPARED" http://127.0.0.1:61282/world.html "$OUTPUT.json" [count|line|pcm|missing|retire]
```

Each check launches and closes its own headless, muted Chrome. The 82-check
desktop builder gate, JavaScript syntax, release reactor budgets and Wasm
budget manifest pass. Tools-check passes after staging the new files: 1,803 tracked tools, zero
new tools without a reason; its [raw result](browser-clip-audio/tools-check.log)
is retained.
The remaining ordinary Linux/window/input/service integration and final
three-host/native-default decision remain #12869 work; this component does
not close the issue or opt the public web face into v1.
