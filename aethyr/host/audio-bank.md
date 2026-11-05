# Synthesized audio bank admission

This component admits metadata before a host copies the producer's WAV
bytes. It does not establish complete audio-device or host conformance.

`coaxial_sound_bank_storage(state)` and `_storage_bytes(state)` name the
producer's actual complete immutable WAV allocation. Everglade exposes its
existing 3 MiB `sound_bank`; no bytes, synthesis or layout change. The
metadata list remains `NAME ADDRESS BYTES` lines. Pointer exports use the
face's established address width. Legacy faces without a bank remain valid;
a v1 bank consumer must not infer physical capacity from its reported count.

`audio-bank.coax` exports:

| API | Semantics |
| --- | --- |
| `hab_span(line, base:u64, capacity:u64) -> HostAudioBankSpan` | Complete metadata admission, no mutation |
| `hab_insert(state, line, base, capacity) -> int` | Copy an admitted unique identity into a free row; return its 0..127 ID or -1 |
| `hab_find(state, name) -> int` | Exact registered name lookup or -1 |
| `hab_source(state, id) -> HostAudioBankSpan` | Original admitted span and a name borrowed from the live registry, or an empty refusal |
| `hab_reset(state)` | Construct all registry defaults and release its identities |

State is a caller-owned one-element `HostAudioBank` view, disjoint from
input metadata. A span has `valid`, borrowed `name`, unsigned 64-bit
`address` and signed 64-bit `bytes`. Invalid spans are entirely empty.
`hab_span` borrows the metadata name during that call; insertion copies all
name bytes into fixed storage. The producer allocation must remain alive
while an admitted source is borrowed. A registry name remains borrowed only
until that registry is reset or destroyed.

Names use the shared `apv_asset_name` profile: 1..48 lowercase ASCII
letters, digits or hyphens. Exactly three whitespace-delimited words are
required, within the existing 1024-byte packet limit. Address and byte count
are complete unsigned decimal words; leading zeroes are accepted, signs,
suffixes, non-digits and overflow are refused. Null address and empty data
are refused. A compressed span is at most 8 MiB and lies wholly in the
measured physical allocation, including a final exact byte. The complete
base/capacity range itself must fit in unsigned 64 bits. No address-plus-
length overflow or partial decimal prefix is accepted.

There are 128 stable identity rows. Invalid, duplicate and exhausted offers
change no existing row. This identity registry is separate from decoded PCM
memory: consumers must additionally apply `apw_plan` before allocating and
use `ham_reserve`/`ham_words`/`ham_release` for the shared 128-ticket,
8,388,608-float PCM budget. A bank name alone does not admit decoded storage
or playback. Original source spans, canonical WAV decoding, immutable PCM
lifetime and the SDK device stream remain distinct responsibilities.

Checks were written first. Final entry-source mutation kills all 92 mutants,
with no survivors or equivalents. Tests cover every unsigned boundary,
physical base/end/capacity, malformed metadata, duplicate atomicity, copied
name ownership, all 128 entries, 48-byte names and complete reset. The
original admitted 598 compiler emits the native carrier with exact source
hashes; no compiler build/load policy or release changes. Native/browser
adapters and actual allocation-refusal measurements are subsequent work.
