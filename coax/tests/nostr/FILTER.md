# Bounded NIP-01 filter profile

`coax/lib/nostr/filter.coax` builds subscription JSON and matches a parsed
Nostr event locally. This profile accepts full 64-character lowercase hex
IDs and authors, a single ASCII letter for each tag name, and full lowercase
hex values for `#e` and `#p`. It bounds each ordinary list to 64, tag filters
to 52 distinct names and each tag value to 256 UTF-8 bytes. These are profile
restrictions, not claims that all other NIP-01 deployments use these bounds.
An absent field differs from an empty list. Lists are ORed internally; fields
and tag filters are ANDed. Only the first value of each event tag is indexed;
since/until are inclusive and `limit` affects initial query results rather
than per-event matching.

The caller must supply a successful, non-forged event parser record and keep
its original JSON and all borrowed filter views valid and immutable throughout
the call. The caller must truthfully report whether serializer output overlaps
any filter view. The matcher performs no signature, freshness, relay, identity
or authorization admission. Filter JSON failure may leave partial scratch
output and returns -1.

Validation: `coax/bin/coax test coax/lib/nostr/filter.coax` and
`python3 coax/tests/nostr/filter_native.py`. The latter compiles the actual
filter and parser into checked and release native, ASan/UBSan, and WASI builds.
It compares 4,096 combinations per build to separate Boolean formulas, reads
an actual parsed fixture event, and compares one serialized result to standard
JSON decoders. A separate two-tag case refuses when the event contains `x`
but not `y`; it is not a positive two-tag fixture. The sanitizer executable
exercises the 4,096 match cases, not the serializer or that two-tag case.
The finite matrix covers chosen matching fields and one event;
it is not exhaustive protocol or target-security evidence. An independent
source review found no concrete blocker under the caller premises above.

The reviewed source SHA-256 is
`e364258615344efc147ad8a5de596893e2e75ebfdadd483c902782c3dd804cf2`;
the checks used `coax0 0.22.0`, binary SHA-256
`9857663bf2c87462d9acdd09b118c9a3e298a3a9caa43ab842dfeabedcce0cd7`.
The source and dependencies must remain unchanged for these results to apply.

The complete authority-graph mutation gate on that source killed 3,891 of
3,891 mutants, with 219,312 full checks and zero skipped checks. The focused
source test passed 199 of 199 checks; no mutation waiver was used.
