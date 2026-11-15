# RB-01b — canonical robotics experiment record

Source base: `183866be3b`. This completes RB-01's bounded CPU teaching profile
with the [RXE1 contract](../protocol/robotics-experiment-v1.md), its registry
row, one Coax codec and independently frozen golden bytes. It does not qualify
a robot importer, asset authentication, simulation, training or hardware.

`rxw_write` validates the decoded contract, builds only active records in a
private bounded buffer and publishes the exact byte span and tagged identity.
`rxw_read` validates framing/counts/length, matches the caller's expected
artifact digest, preflights IEEE words before conversion and invokes the
existing expected-embodiment admission before publishing a fresh descriptor.
Inactive array slots retain the type defaults. Signed zero survives exactly.

The independent stdlib oracle was written before the codec. Python `struct`
encodes all declared fields and `hashlib` computes the tagged identity; it
checks both the complete JSON golden and generated Coax constant vectors.
The baseline is **472 bytes**; record lengths range from **324 to 2,096 bytes**.

## Observed gates

```shell
coax/remote/remote.sh run -- bash psionic/check.sh --only robotics/experiment-wire
```

- **70 checks passed** in debug and release, including retained dependency
  checks. Coverage includes every single-byte change in the golden, all 32
  expected-digest bytes, every cut and one extra byte, every encoded numerical
  role, malformed conventions, minimum/maximum records and full refusal output.
- Root mutation: **230 mutants, 228 killed, zero survivors, two reviewed
  equivalents**. Widening the identity copy's destination to 33 bytes still
  copies the same 32 source bytes. The zero-size clause is redundant after
  positive input length and exact size equality. Reasons remain at the sites.
- The independent oracle matched every golden field, byte and tagged hash.
- The actual public writer/reader passed native debug/release at `-O0`/`-O2`,
  each plain and with **ASan/UBSan**. These runs include 30 hostile IEEE
  field/word cases on each side of the codec, complete descriptor preservation,
  signed zero, all cuts and the extra-tail case.
- The book example now reconstructs the record before using the decoded
  description for its action map. Its exact debug/release transcript is checked
  through the existing robotics qualifier's `example` phase.

Framing diagnostics were tested at the complete-header and maximum-span
boundaries. Separately rehashed malformed records test semantic admission;
they cannot pass merely because their new bytes have a matching digest.

The digest binds an artifact selected by the caller. It is neither a signature
nor proof of a genuine assembly/calibration, source rights, motor permission
or a completed experiment. The 61-observation vendor policy remains a future
profile. No struct dump, alternate codec or new layer exception was added.
