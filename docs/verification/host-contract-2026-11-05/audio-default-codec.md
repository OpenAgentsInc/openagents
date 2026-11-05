# Default Ogg codec failure retained

Everglade's ordinary default is the picked `NAME-CAND.ogg` set from
`aethyr/audio/everglade-bank.coax`, rather than its 48 generated synth WAVs.
The initial exact-source SDK comparison observes three real picked files.
Chrome's actual 48 kHz `decodeAudioData` and the native AVAudioFile/converter
path decode the same complete compressed bytes. Every interleaved binary32
word is compared without shifts, masks or omission at the unchanged 2^-20
sample bound.

| Asset | Words | Past bound | Maximum absolute error |
| --- | ---: | ---: | ---: |
| fire-cast-e2 | 32,652 | 0 | 1.7881393432617188e-7 |
| fire-hit-e2 | 42,240 | 745 | 0.025290057063102722 |
| ambient-e1 | 1,920,000 | 0 | 2.384185791015625e-7 |

The complete shapes match: mono effects and a 960,000-frame stereo bed,
all 48 kHz, with no nonfinite samples. The fire-hit failure is confined to
sample indices 2,604–3,648. It is an actual codec mismatch, not a passed
audio result. A separate native export immediately after AVAudioFile.read,
before AVAudioConverter, is byte-identical to its final PCM. Conversion
and interleaving therefore do not explain this failure. No corrective
shift, alternate asset or synth alias has been applied.

`check-browser-codec.coax` is an observation harness, not ordinary decoder
admission. Its SDK glue decodes frozen files and exports every measured
word; the existing 103-mutant PCM comparator remains the acceptance
authority. Native sources, decoder artifacts and the three input files
are retained in `metal-host-12868/audio-default-first/` and the separate
`audio-default-input-second/` under `~/.openagents/aethyrforge/`.
Browser raw `.f32`/`.ogg`, exact SDK/harness source, executables, receipts
and hashes are under `host-contract-12869/audio-default-first/` there.
The committed comparison and diagnostic windows preserve the failure.

The shared compressed-input/PCM preflight and complete default codec
workload remain open. Separate native demand-loading work removes the
unused synth-bank allocation pressure without raising the shared limits;
that memory result cannot erase this codec difference. These observations
change no audio tolerance, profile, graphics admission, desktop selection
or issue status. Chrome was headless/muted and no audio device was started
for the frozen codec comparison.
