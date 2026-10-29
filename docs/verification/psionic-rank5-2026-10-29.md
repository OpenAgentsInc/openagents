# Psionic rank-five inventory and tensor access

Verified 2026-10-29 with coax0 0.16.0, SHA-256
`5388de6c9c976ff6328fd2f4f4aa3433e28d529deaac5f78a74abaab39c52498`.
The rank-five inventory root passed 52 Coax checks in debug and release and
killed all 1,554 mutations. The tensor-access root passed 74 checks in both
modes and killed all 1,924 mutations. There were no equivalence exemptions.
Both native harnesses passed on Apple arm64 in debug and release, plain and
with ASan+UBSan. Release-generated C and the same tests passed plain CPU on
`coderos-4080` (Linux x86-64); Linux sanitizer and GPU execution were not tested.

On the Mac, the inspected original Elephant file had 760 tensors and exactly
18,819,627,488 covered payload bytes. Its sole rank-five tensor is
`model.visual.patch_embed.proj.weight`, shape `[1152, 3, 2, 16, 16]`.
The complete 18,819,722,392-byte file independently matched its pinned
SHA-256 `682f02348dfe4617dd21bba2915c12e805de8cf16327a61e895a831ea681d0ba`.
Inspection alone does not verify hashes or bind expected model metadata. The
original bytes remain external; the [native inspection harness](../../psionic/tests/elephant_inventory.c)
requires the user to supply them. The existing rank-four `original-weights-v1`
profile remains unchanged.

The focused roots ran in this increment; the full earlier Psionic suite was not
repeated. The [source hashes](psionic-rank5-2026-10-29-sources.json) identify
these checks and the compiler binary.
