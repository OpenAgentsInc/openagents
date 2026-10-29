# Psionic pinned decision-model targets

Verified 2026-10-29 with coax0 0.16.0, SHA-256
`5388de6c9c976ff6328fd2f4f4aa3433e28d529deaac5f78a74abaab39c52498`.
The target root passed 88 checks in debug and release and killed all 1,799
mutations, with no exemptions. Native checks passed on the Mac in both modes,
plain and ASan+UBSan, and on `coderos-4080` (Linux x86-64) in release plain.
Linux sanitizer and GPU execution were not tested.

Coax records both pinned revisions, config/tokenizer/readout/index identities,
all shard names, lengths and digests, layer dimensions and capacities. These
are trusted metadata, not an assertion that any bytes have been authenticated
or that a complete model or service has run. Full bundle admission has its
[own record](psionic-original-bundles-2026-10-29.md). The focused target root
ran in this increment; the full earlier Psionic suite was not repeated. The
[source hashes](psionic-targets-2026-10-29-sources.json) identify the inputs.
