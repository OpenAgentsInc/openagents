# Shared compressed audio extent preflight

`aethyr/audio/host-opus.coax` now measures complete source framing and
decoded/output extents before an SDK Opus call. Its typed API and supported
profile are in `host-opus.md` beside the source. One bounded, CRC-verified
stream supplies every packet's TOC duration, full decoder extent, final
granule/pre-skip output extent and mono/stereo word counts. An EOS trim
cannot conceal an oversized decoded allocation. The existing shared
`ham_extent` remains the PCM word authority.

The check-first root mutation gate kills 389/389 mutants without
equivalents. The earlier 73- and 19-survivor observations are retained;
physical page parsing was separated from stream-profile decisions,
redundant branches removed and boundary/packet tests sharpened. A real
check first exposed an incomplete comment packet reaching audio admission;
the new completion check fixes it. No test or mutant is marked equivalent.

The actual pick inventory admits all 122 current files: 1,096,915 compressed
bytes and 9,524,761 total output words across the set. The inventory includes
both ambient candidates and is not one admitted resident allocation. The
ordinary prefetch program skips ambient variants and separately requests
its first bed. Full SDK decoding, per-allocation reservation, old-owner
retention and actual output parity remain separate host adoption gates.
The three earlier codec probes include the unpicked fire-hit-e2 candidate;
their failure is preserved separately and is not a whole default verdict.

Exact source, admitted-598 tool executable, actual compressed inputs,
inventory, failure/final logs and hashes are frozen under
`~/.openagents/aethyrforge/host-contract-12869/audio-opus-plan/`.
This source checkpoint changes no SDK adapter, codec, content, tolerance,
compiler admission, desktop default or issue status.
