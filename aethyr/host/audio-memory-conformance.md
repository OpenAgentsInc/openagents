# Actual PCM allocation observations

`audio-memory-conformance.coax` admits the allocator/span observations from
an actual host adapter. These are effects receipts, separate from DSP sample
parity and device output. The 96-mutant root gate kills every mutant without
equivalents. It includes the earlier 63-mutant allocation/span component.

`mamm_accept(memory,bank)` requires 20 integer memory observations and ten
flattened five-word bank observations:

| Memory positions | Actual observation |
| --- | --- |
| 0–5 | Refusal of frames/channels `(-1,1)`, `(0,1)`, `(i64max,1)`, `(1,0)`, `(1,3)`, `(4194305,2)` |
| 6 | Retained words after those refusals: zero |
| 7–10 | Full stereo allocation count/total 8,388,608; another sample refused; release returns zero |
| 11–13 | 128 one-word allocations/count/total; 129th allocation refused |
| 14–15 | Release one word then allocate stereo frame: total129; release all: zero |
| 16–19 | Copy three interleaved stereo frames: count/total6; last sample0.625; release returns zero |

The actual source allocation contains bytes37 through80. Bank rows are
`[accepted,liveRows,bytes,firstNameByte,firstSourceByte]`. Register `x BASE 44`,
then refuse its duplicate, an address before the allocation, an address at
its end with one byte, 45 bytes at its base, negative/zero lengths, `../y`
and a fourth metadata word. Those refusals return empty pointers and leave
one live row. Finally `y BASE+43 1` returns byte80 and the second live row.
The source pointer is observed at runtime; a fabricated numeric address is
never dereferenced by the fixture.

`mamm_bank(fields)` additionally admits the frozen 48-asset producer receipt:
metadata bytes1–4096; physical storage3,145,728; 48 WAV/decoded assets;
2,042,394 retained words and summed samples; the same retained total after
context close while asset references remain; zero after dropping them;
48 admitted WAVs but zero decoded assets while the word budget is full;
the known unavailable cue remains unavailable; release returns zero; a
separate optional SDK converter returns/retains24,960 samples and releases
to zero. That last converter observation is adapter-specific and does not
claim its PCM equals canonical decoding. Complete WAV/PCM byte comparison
against the independent frozen bank is a separate tool check.
