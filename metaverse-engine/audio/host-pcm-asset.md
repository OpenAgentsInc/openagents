`host-pcm-asset.coax` defines the version-1 `APC1` lossless PCM companion
format. This format preserves an authored Ogg asset's identity and exact
48 kHz finite interleaved F32 output where the recorded platform codecs
differ. It adds no codec approximation, resampling, alignment or sample
tolerance. Original Ogg files remain part of the identity and package.

`HostAudioAsset` fields, in declaration order, are `valid:bool`,
`name:string`, `source_url:string`, `source_bytes:int`, `source_sha:[32]u8`,
`frames:int`, `channels:int`, `canonical:bool`, `companion_url:string`,
`companion_bytes:int`, `pcm_sha:[32]u8`. SHA arrays use the conventional
32-byte digest order. Strings are borrowed immutable catalogue data. All
other fields are values. The catalogue selects a companion; SDK glue does
not construct a second selection policy.

`everglade-pcm-assets.coax` exports `hpc_everglade(name)` and
`hpc_everglade_at(id)`. Its complete 153-candidate inventory pins original
compressed bytes, shape, source digest and frozen browser PCM digest.
Exactly 32 rows require companions; the other 121 retain metadata-admitted
SDK decoding. An unknown lookup returns the empty descriptor. This does
not remove existing safe-name SDK paths: an unlisted safe name can still
use `hpo_plan` and `hpo_output`. A listed canonical row must refuse a
missing/corrupt companion; it cannot fall back to the differing SDK codec.

| API | Result and obligations |
| --- | --- |
| `hpc_extent(frames, channels) -> int` | Complete companion bytes, or zero; positive mono/stereo, at most 262116 PCM words |
| `hpc_source_plan(asset, source) -> HostOpusPlan` | Exact source count, SHA, complete Ogg preflight and pinned output shape, or empty plan |
| `hpc_plan(asset, source, companion) -> HostPcmAssetPlan` | Complete canonical format/identity/finite admission or empty plan |
| `hpc_decode(asset, source, companion, output:[]f32) -> bool` | Copy exact admitted F32 bits; any refusal leaves all output untouched |
| `hpc_pack(source, pcm:[]u8, output:[]u8) -> int` | Regeneration only: complete byte count or -1, no output writes on refusal |

These five functions are exported to C.
`HostPcmAssetPlan` contains `valid:bool`, then `channels`, `frames`,
`samples` and `offset` Coax integers; a valid offset is 112. Source and
companion inputs are caller-owned immutable views, disjoint from writable
output, and no view is retained. Source preflight retains the existing
8 MiB compressed bound. Hashing uses the shared fixed-workspace SHA kernel;
it allocates no source-sized padding buffer. Companion bytes are at most
1 MiB; a larger profile requires a new format version.

The 112-byte header is little-endian except raw SHA bytes:

| Offset | Field |
| --- | --- |
| 0 | Four bytes `APC1` |
| 4 | U32 header size 112 |
| 8 | U32 output rate 48000 |
| 12 | U32 channels, 1 or 2 |
| 16 | U64 output frames |
| 24 | U64 original Ogg byte count |
| 32 | 32-byte original Ogg SHA-256 |
| 64 | 32-byte PCM payload SHA-256 |
| 96 | Sixteen reserved zero bytes |
| 112 | Exact little-endian F32 interleaved payload, with no padding |

The count must equal `112 + frames * channels * 4` and the pinned
companion count. Both header digests must match actual bytes and catalogue
identity. NaN and infinities refuse; signed zero, subnormals and finite
extremes retain their bits. There is no successful prefix. Hosts reserve
source, companion transfer and retained PCM storage before their physical
allocations and keep ownership through the last SDK/Worklet reference.
`ham_*` limits remain 128 tickets and 8,388,608 F32 words.

Check-first entry-source mutation kills 179/179 mutants with no survivors
or equivalents. Checks include every header/payload byte, source identity,
both layouts, every output word, full-width endian values, short/missing
buffers, exact capacity, nonfinite payloads and an admitted Ogg stream
beyond the version-1 companion extent. `hpo_output` separately kills
395/395; aggregate bank admission kills 101/101. Adapter adoption, complete
default demand, device lifecycle and compiler admission remain separate
gates.
