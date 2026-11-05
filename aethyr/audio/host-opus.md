`host-opus.coax` owns preflight of the authored Ogg/Opus profile before a
platform codec allocates PCM. It does not implement Opus decoding or
establish sample parity between codec implementations.

`hpo_plan(source: []u8) -> HostOpusPlan` is exported to C. The source is
immutable, caller-owned and at most 8,388,608 bytes. No source is retained.
The result fields, in declaration order, are `valid: bool`, followed by six
Coax integers: `channels`, `pre_skip`, `frames`, `decode_frames`, `samples`,
`decode_samples`. An invalid input returns the empty/default plan.

The profile is one complete, sequential Ogg logical stream with verified
unreflected CRCs, one version-1 OpusHead on the first page, mono or stereo
mapping family zero and zero output gain. The informational source sample
rate is not a playback rate; output and all granule units are 48 kHz.
The next page contains one complete OpusTags packet. Header/comment and
audio packets do not continue across pages; lacing may span multiple
segments within a page. Additional BOS, continued pages, serial/sequence
changes, incomplete final packets, unsupported high granules, truncated
pages and bytes after EOS refuse. This deliberately bounded profile admits
all current authored picks; it is not a general-purpose Ogg demuxer.

TOC frame counts include every complete coded packet, including samples
discarded by pre-skip and EOS trimming. Nonfinal audio granules equal that
complete decoded count. EOS may trim its final page but cannot precede the
previous page's count or exceed decoded samples. `frames` is the final
granule minus pre-skip; `decode_frames` is the complete coded extent.
`samples` and `decode_samples` multiply their respective frame count by
channels. The existing `ham_extent` bounds complete decoded storage to
8,388,608 words; a short final granule cannot hide an oversized decoder
extent. Reserve actual input, decoder/output, transfer and retained planes
separately before their SDK allocations. Retain each ticket through the
final physical/sample reference, including a pending decode completion.

Metadata admission is not full packet decoding. A platform codec can still
refuse compressed payloads; that refusal publishes no PCM or successful
prefix. `hpo_output(plan, frames:int, channels:int, rate:f64) -> bool`, also
exported to C, requires a valid plan and exactly its output frame count,
channels and 48000 Hz. Apply it before copying or retaining any SDK output;
a refused observation publishes no successful prefix. Platform codec private
workspace and OS buffering remain separate observations.

The check-first root gate kills 395/395 mutants with no survivors or
equivalents. Tests include an independently recorded page CRC, every TOC
configuration/count, both channel layouts, high-byte pre-skip, exact page
endpoints, multi-segment comments/audio, zero/incomplete packets, serial,
sequence, granule and flag failures, CRC-preserving semantic corruption
and a full decoded-budget crossing despite final trim. An incomplete
comment packet was first retained as an actual failing check, then fixed.
The existing audio-memory component keeps its independent sharp gate.

`aethyr/host/check-opus-assets.coax` observes the current pick set: 122
assets, 1,096,915 compressed bytes and 9,524,761 total decoded-output words
across the entire set. This sum is an inventory, not one allowed resident
allocation. Ordinary default prefetch skips ambient variants and requests
its first bed separately. Host adoption, ownership and codec-output
comparisons retain separate gates; the known fire-hit SDK codec failure
is preserved. Frame/granule semantics follow
[RFC 7845](https://www.rfc-editor.org/rfc/rfc7845) and TOC duration follows
[RFC 6716 section 3.1](https://www.rfc-editor.org/rfc/rfc6716#section-3.1).
