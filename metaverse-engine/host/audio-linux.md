# Linux audio effects

`metaverse/native/audio-host.c` executes the same Coax canonical WAV decoder,
PCM interpreter, bank admission and decoded-memory tickets as the other
version 1 hosts. C owns allocations, immutable references, locks and SDK
copies. It supplies no resampler, filter, convolution or dynamics algorithm.

`rha_open` constructs the actual Coax defaults. `rha_add_wav` admits the
name, complete canonical WAV plan and PCM ticket before allocating. Its
owned WAV remains alive with the bank registry; decoded PCM remains stable
through every Coax command/block that borrows it. `rha_bank` additionally
admits each producer-reported source span against the producer's physical
WAV allocation before reading it. Its caller must separately admit the
metadata view against the face's physical metadata capacity before calling
this API. A logical length alone is insufficient.
An invalid row reports failure while later, independently admitted rows
remain usable; it never licenses a platform decoder fallback.

`rha_command` passes the complete packet and the registered asset view to
`ap_context_command`. The `started` bit is the actual gesture state.
`rha_block` returns one 1024-frame, 48 kHz interleaved stereo block;
`rha_render` copies partial device requests while retaining that block's
cursor. A failed Coax block publishes silence and a terminal core error.
`rha_reset` is a diagnostic fresh DSP state that retains decoded assets.
Product calls use `rha_command_current`, which captures this owner's shared
[`audio-session.coax`](audio-session.md) offer. `rha_session_event` reports
actual gesture, coverage and retirement facts. `rha_render` leaves the Coax
clock and cursor held while the shared offer is inactive. Explicit
`rha_command(..., started)` remains an offline diagnostic API.

`rham_*` owns a Coax `HostAudioMemory`. `rhas_open` applies `ham_extent`
and reserves before allocating, `rhas_copy` copies into that exact measured
extent, and sample references retain their budget owner. `rhas_close`
releases the physical allocation and ticket only with the final reference.
`rha_asset_keep` retains a decoded asset across context retirement;
`rha_asset` borrows it only while its context remains alive. Writable sample
storage is filled before publication and is immutable while Coax borrows it.
`rhab_*` is the scalar SDK projection of the shared bank registry. A refused
insertion returns empty name/source views and changes no admitted row.

`audio-pipewire.c` opens an inactive actual F32, 48 kHz stereo stream. Start
follows the actual user gesture. Pause deactivates the stream without
advancing its Coax clock or partial-block cursor. Close destroys the stream
under its loop lock, then stops/joins the loop before PCM can be freed.
The callback is non-realtime and serialized; the unused realtime/RTKit
module is disabled in this context. PipeWire initialization/deinitialization
is balanced. Muted qualification writes zero to the actual SDK output
planes and counts any nonzero samples before queuing each buffer.

Component tools are `build-linux-audio.coax`, `check-linux-audio.coax` and
`check-linux-audio-memory.coax`. The builder consumes retained Coax C/header
emissions and explicit installed PipeWire SDK paths, recording exact argv,
source hashes and executable identity. `--sanitize` instruments the generated
Coax C, adapter, fixture and checker; `--memory` selects the actual allocator
fixture. Runtime sample/fixture comparison, command sequencing, receipt
acceptance and source fencing remain Coax.

`realm-host.c` now exposes an explicit optional `rh_audio_enable` effect.
It requires all v1 metadata/WAV/packet physical-capacity exports, admits
complete allocations through `htb_copy_span`, then forms borrowed views.
The bank length is read once before its pointer, and each draining packet
length is read once before its measured span. `rh_after` handles sounds
even without a navigation action. Key-down, pointer-down and wheel input
report real gestures; hidden/shown lifecycle events cover/uncover audio.
`rh_audio_retire` quiesces the concrete device while retaining the terminal
core for observation; `rh_audio_close` frees it before realm memory unmaps.
Weak SDK links preserve non-audio fixtures and legacy faces.

`build-linux-audio --product` and `check-linux-audio-product.coax` drive this
same ordinary realm dispatcher with a Coax recording face and an actual
muted device. Their receipt covers complete bank spans, one refused row
followed by a valid asset, exactly-once packets, pre-gesture refusal, covered
gesture, pause/resume, retirement, and oversize/null/wrapping packet spans.
The product observer kills 93/93 mutants. This trace contains no Vulkan
window or room and does not substitute for that remaining integration.

These are SDK component APIs and observations. Ordinary Vulkan world/audio
integration, production producer admission, complete three-host service
qualification and desktop default selection remain separate gates.
