# Complete SDK audio reads

`audio-decode.coax` owns a synchronous decoder's exact-output/EOF boundary.
`had_open(frames,channels)` refuses an invalid shared PCM extent. For an opened
state, `had_read` returns `{kind,offset,words}`: refused (0), output read (1),
independent one-frame EOF probe (2), or complete (3). Offsets and capacities
are F32 words; `had_observe` accepts an SDK return measured in frames per
channel. Partial positive reads advance exactly that many frames. Premature
EOF, decoder errors, oversized returns and any output after the admitted
extent permanently refuse that stream. No successful prefix is published.

Callers retain the state exclusively, obey each offered capacity, reserve
the complete coded extent and the probe before opening the SDK, and retain
all SDK-borrowed input until close. Only a complete state can publish a
separately reserved retained PCM allocation. The SDK's private workspace is
separate from these observed input/output planes. Checks kill all 59 root
mutants, without equivalents.
