# Default Ogg codec failure retained

Everglade's ordinary default is the picked `NAME-CAND.ogg` set from
`aethyr/audio/everglade-bank.coax`, rather than its 48 generated synth WAVs.
The initial exact-source SDK comparison observes three available files.
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

The initial record incorrectly called all three probes picked defaults.
The current fire-hit row is `[55,-1,-1,-1]`, selecting `fire-hit-e3`;
`fire-hit-e2` is an available candidate. Its supported-codec failure is
preserved, but cannot decide default-profile conformance. The complete
122-pick comparison below supplies the actual default-bank verdict.

The complete current pick set has now been compared: **99 pass and 23
fail**, across all 122 files and 9,524,761 words. Every frame/channel shape
and 48 kHz rate matches, with zero nonfinite samples and no omitted words.
The failing files contain 964,394 words in total; their sample differences
are preserved in `audio-picked-codec/comparison.json`. Examples include
4,469 samples beyond the bound in frost-hit-e2, 1,574 in ice-e3, and a
maximum absolute error of 0.04402640461921692 in splash-e1. This is a
complete failing default-codec result, with no inferred addition of probe
counts or changes to the 2^-20 gate.

Both ambient picks were decoded in separate native reservation owners,
which returned to zero after each file. The inventory is not a claim that
all picks fit simultaneously. Ordinary demand loading and its actual
120-fetch/first-bed/room workload have separate receipts. Complete browser
and native raw `.ogg`/`.f32` files and exact SDK sources remain under the
respective `audio-picked-bank/` durable artifact directories.

`check-browser-codec.coax` is an observation harness, not ordinary decoder
admission. Its SDK glue decodes frozen files and exports every measured
word; the existing 103-mutant PCM comparator remains the acceptance
authority. Native sources, decoder artifacts and the three input files
are retained in `metal-host-12868/audio-default-first/` and the separate
`audio-default-input-second/` under `~/.openagents/aethyrforge/`.
Browser raw `.f32`/`.ogg`, exact SDK/harness source, executables, receipts
and hashes are under `host-contract-12869/audio-default-first/` there.
The committed comparison and diagnostic windows preserve the failure.

The additional 31 authored candidates have now been observed without
repeating the unchanged 122 decoder measurements. Their exact original
Ogg and native PCM bytes join the earlier snapshot before reuse. Nine
additional candidates fail, giving **121 pass and 32 fail across all 153
authored files**. The frozen extra comparison is in `audio-pcm-assets/`;
all original failures remain recorded. The versioned, source-pinned
[PCM companion component](audio-pcm-assets.md) retains those original
identities and supplies exact payloads for the 32 discrepant files.

Shared compressed preflight and post-SDK shape admission are qualified
components; their ordinary host adoption and complete default codec
workload remain open. Separate native demand-loading work removes the
unused synth-bank allocation pressure without raising the shared limits;
that memory result cannot erase this codec difference. These observations
change no audio tolerance, profile, graphics admission, desktop selection
or issue status. Chrome was headless/muted and no audio device was started
for the frozen codec comparison.
