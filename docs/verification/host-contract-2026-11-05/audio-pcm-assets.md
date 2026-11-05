# Source-pinned authored PCM component

All 153 non-synth SFXB candidates are retained, including the 31 outside
the current 122 picks. The earlier 122 source/native PCM observations join
the new native export byte for byte and are reused. Only the additional
31 were decoded again in muted headless Chrome: 22 pass and nine fail at
the unchanged 2^-20 bound. Complete authored scope is 121 passing and 32
failing files, 12,657,718 words, with matching frame/channel/rate shapes
and no nonfinite samples. The original failures are preserved; no sample
shift, mask, correction or tolerance change is applied.

`aethyr/audio/host-pcm-asset.coax` and its ABI document define APC1. It pins
the exact original Ogg bytes/SHA, output shape and exact finite interleaved
F32 payload/SHA. The companion is a lossless projection of the frozen
browser output, not an alternate synth sound. Original Ogg files remain
unchanged. Missing/corrupt required companions refuse atomically instead
of entering a platform decoder. Unsupported format versions refuse.

`everglade-pcm-assets.coax` contains all 153 identities and selects the 32
required companions. The other 121 use admitted SDK effects; unlisted
safe names are not removed. Committed companion files total 5,180,164 bytes,
including their 112-byte headers. The regeneration tool decoded every
generated companion through the shared reader and compared every output
F32 bit to the frozen browser payload. All 32 are exact. This is a format
and source component result, not ordinary audio-host parity.

Check-first root mutation kills 179/179 format/decoder, 26/26 catalogue,
395/395 compressed/post-SDK shape and 101/101 aggregate-bank mutants, with
zero survivors or equivalents. The original per-row-only bank admitted an
extra byte after the 8 MiB total; its new aggregate test first failed there.
The total counts overlapping source spans under separate names because
each can cause a copy. Neither PCM memory nor asset-ticket limits increase.

Regenerate with `aethyr/host/build-pcm-assets.coax`: arguments are the
frozen native authored manifest, original picked comparison/browser raw
directory, extra comparison/browser raw directory, repository root and
output receipt path. The tool verifies each browser original Ogg against
the native manifest source, applies complete preflight, builds companions
only for recorded failures and performs exact decode/readback comparison.
`extend-codec-inventory.coax` creates the extra input only after complete
source/native PCM/shape joins for the reused 122 observations.

Raw inputs, decoder outputs, tools, exact generated sources and logs are
under `~/.openagents/aethyrforge/host-contract-12869/audio-authored-extra/`,
with the prior browser raw data in `audio-picked-bank/`. The native authored
manifest/raw pairs/SDK snapshot are under the sibling
`metal-host-12868/audio-authored-bank/`. Committed receipts and SHA lists
are in `audio-pcm-assets/`. The generator used the unchanged admitted
598 image for Coax tool emission; this does not publish a new compiler
release or admit graphics products. Actual reactor and native adapters
must keep their own genuine compiler-source and product-source joins.

Pending exits are shared decoder/ownership adoption in browser, Linux and
Metal; actual complete default demand under the unchanged memory bounds;
muted output/lifecycle tests; current primary packaging and graphics
admission. Web remains the desktop default and both host issues remain open.
