# Experimental AEAD address admission

This Coax policy checks seven numeric spans: operation, workspace, key, nonce,
associated data, input and output. It checks lengths and target-width bounds,
rejects nonempty null and wrapping spans, validates seal/open capacity, and
requires all 21 pairs to be disjoint. Empty spans do not access their address.
The 32-bit profile rejects ranges that do not fit below its half-open address
limit; the 64-bit profile likewise conservatively rejects the last byte whose
end would equal 2^64.

The host must choose the actual target width, provide exact object sizes and
truthful extents, prove live accessible allocations including Wasm linear
memory, keep all views stable and disjoint, and serialize use. The numeric
policy cannot establish those facts or erase memory. It is not yet connected to
a public host and is not production-secret or target-security approval.

Eight loaded Coax checks passed and all 171 mutants were killed; 283 selective
checks ran with 427 skipped by the scheduler, with no mutant exclusions. An
independent integer-interval model compared 12,000 cases shared among seal/open
and 32/64-bit profiles for each checked/release native and actual Wasm build.
The generator includes null, alias, maximum, wrap and boundary cases; simulator
was compilation only. The first pass killed 158/175, and the next 170/171;
missing boundary cases and redundant conditions were addressed without waivers.

Independent source review found no concrete arithmetic or alias blocker under
the stated trusted-view and target-width assumptions. It independently checked
the model's scope and the 21 pair intersections; it did not rerun tests or
approve actual pointers or production use. Source SHA-256
`3fc762f3388f478cdba033175deb9a95388924bcfabd304bd567e4af4ebcd3ed`.
Checked generated C SHA-256 `caaf6bb08fc48d667165554a36f691a27c0031613e27e869d82d05cdb14c647d`,
Wasm `60cf8ec4499f12ba65ad0b29f8fa400c1c695661842831648db8396626d5dc38`;
release C `689a21cb18f4e17a599957d1ecfa35e45ba20798bf99d8da3ba7f713dd7ecf00`,
Wasm `60cf8ec4499f12ba65ad0b29f8fa400c1c695661842831648db8396626d5dc38`.
The compiler was coax0 0.22.0 SHA-256
`9857663bf2c87462d9acdd09b118c9a3e298a3a9caa43ab842dfeabedcce0cd7`.

```sh
coax/bin/coax test coax/lib/crypto/aead_admission.coax
coax/bin/coax mutate coax/lib/crypto/aead_admission.coax --mutation-stats
python3 coax/tests/nostr/aead_admission.py
```
