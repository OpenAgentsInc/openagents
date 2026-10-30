# Fleet numeric boundary correction

Scoped mutation testing found that answer tolerance could admit a value
outside its semantic range. A confidence of `1.000000001` differs from a
maximum probability of `1` by less than the serialization tolerance.
Likewise, a two-level score of `1.000000001` is close to the expected score,
yet exceeds that question's maximum of `1`.

The independent HTTP harness reproduced both bugs on the previous frozen
sanitized fleet, SHA-256
`6b7049b1e469cb6930f3ea166e02e55138380bcb4064e90b2ba22c6f17d29e54`.
Both signed and encrypted malformed replies became HTTP 200 results. Those
red results are retained in
[`data/oct30-fleet-numeric/old-confidence.json`](data/oct30-fleet-numeric/old-confidence.json)
and [`old-score.json`](data/oct30-fleet-numeric/old-score.json).

The runtime now enforces the confidence interval `[0,1]` and the score
interval `[0, level_count-1]` independently of serialization tolerance.
The successor sanitized executable, SHA-256
`b406aa3483d08308214fb07ddf90252c61addf7fe767e6da1b8aad725a031bfb`,
passed the following affected checks:

- **10 numeric cases:** negative and excessive confidence/score refusal;
  a three-level score just above `2` refused; exact inclusive upper bounds
  accepted; exact `0.000001` confidence/score tolerance accepted; and a
  valid three-level score of `1.5` accepted. Score validation uses its
  question's range. The peer sends the negative nanounit as decimal
  `-0.000000001`, avoiding an exponent-format refusal. Every case requires
  an actual dispatched job and independently signed/encrypted reply.
- **One native path:** HTTP to the compiled fleet, through the actual
  sanitized Coax relay and a real Coax Pylon/toy engine, returns valid
  choice, noul and score answers with the exact model identity.
- **Six affected fallback framing cases:** status 2000, duplicate/conflicting
  lengths, transfer encoding, no length, truncated body and trailing body
  are all refused after the service parser cleanup.

There were no sanitizer reports. The current admitted score profile is
2..10 levels (`sv_nip`), so its valid score domain ends at 9. The numeric
wire profile covers that admitted range.

Exact source hashes, native hashes, raw reply JSON and per-case outcomes
are in
[`data/oct30-fleet-numeric/qualification.json`](data/oct30-fleet-numeric/qualification.json).
The earlier 37-case and public-relay evidence remains unchanged at its
original source and binary hashes. The successor was checked on these 17
affected cases; unrelated cases were not rerun.

Reproduce the numeric slice with `FLEET_SCOPE=peers FLEET_MATCH=numeric-`
and an explicit frozen `FLEET` path, following
[`psionic/tests/fleet/README.md`](../../psionic/tests/fleet/README.md).
