# Psionic hybrid block and rotary checks

Verified 2026-10-29 with coax0 0.16.0, SHA-256
`5388de6c9c976ff6328fd2f4f4aa3433e28d529deaac5f78a74abaab39c52498`.
The ordered hybrid block root passed 85 Coax checks in debug and release and
killed all 1,742 mutations. The text rotary root passed 14 checks in each mode
and killed all 110 mutations. No equivalence exemptions were used.

Both native harnesses passed on the Mac (Apple arm64, Apple clang 21.0.0) in
debug and release, both plain and with ASan+UBSan. Release-generated C and the
same harnesses passed plain CPU execution on `coderos-4080` (Linux x86-64).
Both hosts used `-ffp-contract=off -fno-fast-math`; Linux additionally suppressed
unknown pragma warnings. Linux sanitizer and device execution were not tested.
The independent scalar references exercise nonzero recurrent layers, a cached
full-attention suffix and multiple hybrid layer boundaries. Native tests cover
actual alias observations and late-failure publication. Rotary tests compare
all 32 frequencies at position one against independent libm calculations.

The block stages linear and full-attention state and publishes only after the
whole block succeeds. It requires the caller to bind the correct weights,
shapes, rotary positions, model identity and prior state. The rotary generator
covers text-only Qwen positions with theta 10,000,000 and 64 rotated dimensions;
it does not implement image-position rotary semantics. These checks alone do
not qualify Elephant, Mammoth, complete requests or serving.

The focused roots and book gate ran in this increment; the full previous
Psionic suite was not repeated. The [source hashes](psionic-hybrid-executor-2026-10-29-sources.json)
record the inputs to these checks.
