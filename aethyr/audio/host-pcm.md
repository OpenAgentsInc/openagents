The host PCM carrier interprets the existing mixer packets in Coax. SDKs own
decoded immutable interleaved PCM and the audio device. This foundation does
not establish full host conformance or change desktop/backend selection.

Emit `host-pcm.coax` as a library with a generated C header. Construct one
`AudioPcmHost` using its generated default constructor and retain a one-element
view. Allocate `ac_words(1024,128)` binary64 workspace elements and 2048
binary32 output elements. Serialize command delivery and rendering; keep the
state, workspace, output and all borrowed PCM allocations disjoint.

`ap_context_command(state, workspace, line, samples, channels, started)` takes
the exact mixer line and a stable immutable decoded PCM view at 48,000 Hz,
one or two interleaved channels. The SDK retains that allocation until engine
retirement. Name and ID bytes are copied. A repeated bed can pass an empty
PCM view. The result is a voice slot 0–127, 128 for an accepted control, -1
for refusal, or -2 for a transient cue ignored before the first gesture.
Startup room, bus, fetch and bed controls remain admissible before playback.
`ap_command` is the same interpreter for an already started context.

`ap_block(state, workspace, output)` writes exactly 1024 interleaved stereo
frames on success. A failed block must not be published; retire that invalid
engine, whose internal sample state may already have advanced. The SDK copies these
blocks into its measured device buffers and must not run the interpreter
concurrently or call it after freeing borrowed PCM. PCM sample magnitudes
through 2^30 are admitted, including finite decoder overshoots above one.
The authored room impulse requires finite samples within full scale.

The carrier preserves independent limits of 16 legacy voices, 48 mixed
voices and 64 looping beds. The latest repeated voice ID owns a stop command;
an older voice can finish without inheriting that ID. Beds ease gain over
0.5 seconds initially, then gain/pan over 0.25 seconds. Bus gain eases over
30 milliseconds. Stops fade the dry branch over 15 milliseconds and end the
source after 80 milliseconds, including its raw room send. Delay, rate,
one-pole filtering, mono equal-power pan and the stereo cross-channel matrix
are shared arithmetic. Automation observes the current value at sample zero.

The first accepted `!room NAME` installs the supplied stereo/mono impulse.
It uses the Web Audio RMS normalization at 48 kHz and partitioned convolution
with 1024-frame blocks; four-channel true-stereo impulses are unsupported.
Everglade's generated `room` is 67,200 frames × two channels, requiring 66
partitions. Its raw source send precedes dry filtering and panning.

The bus compressor retains -18 dB threshold, 12 dB knee, ratio 3, 10 ms
attack and 250 ms release. The wall retains -1 dB threshold, ratio 20, 1 ms
attack and 80 ms release, followed by a linked stereo -1 dB ceiling. Each
stage delays its audio by 288 frames while applying the current detector
gain, matching the observed 6 ms per-stage lookahead of the existing browser
graph. The total bus delay is 576 frames; legacy voices and beds bypass it.

Web Audio's old compressor includes implementation-specific detector and
filter behavior. These explicit portable dynamics are not a claim of sample
equivalence with that old graph. Version-1 browser/native adapters must use
the same carrier and qualify actual output. SDK decoding/resampling remains
a separate observation, including the synthesized bank's 22,050 Hz WAVs.

Root authority mutation kills all 134 packet-decoder, 206 voice, 240
convolution, 57 dynamics, 4 level-selection and 226 carrier mutants, with
zero survivors and no equivalents. See the
[foundation record](../../docs/verification/host-audio-2026-11-05/README.md).
