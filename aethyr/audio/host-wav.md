`host-wav.coax` owns the canonical synthesized bank's PCM16 WAV decoder and
48 kHz resampler. It accepts the exact 44-byte RIFF/fmt/data layout emitted
by `aethyr/audio/wav.coax`, one or two interleaved channels, 22,050 or 48,000 Hz,
and one through 1,000,000 source frames. Other layouts and optional compressed
assets remain outside this canonical profile.

`apw_plan(bytes)` returns validity, channels, source rate/frame count, output
frame count and interleaved sample count. Invalid input returns an empty plan.
Allocate output from that measured plan. Retain disjoint immutable WAV bytes,
output, a one-element `AudioWavFilter` view and 20,480 binary64 workspace values.
`apw_coefficients(filter, workspace)` initializes all 320 phases of the shared
64-tap Blackman-windowed sinc, with cutoff 0.95 of source Nyquist and unit DC
gain per phase. Serialize initialization and decoding.

`apw_decode(filter, bytes, workspace, output)` converts signed PCM16 steps by
32768. The 48 kHz case copies those exact values and needs no filter state.
The 22,050 Hz case uses rational source positions `frame * 147 / 320`, zero
outside the source, zero-phase alignment and `ceil(sourceFrames * 320 / 147)`
output frames. It requires the initialized filter/workspace. Refusal writes
no output; successful decoding leaves the output tail unchanged. SDKs retain
the resulting immutable 48 kHz allocation for the PCM carrier's borrowed view.

The root gate kills all 273 mutants, with zero survivors or equivalents.
Checks cover exact signed stereo decoding, RIFF/shape/rate/capacity boundaries,
finite kernel support and symmetry, per-phase normalization, DC, 1/9 kHz tones,
rational duration boundaries and a separately evaluated stereo two-sample FIR.
Actual platform adapters and decoded bank comparisons remain separate checks.
