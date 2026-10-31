# Psionic hybrid primitives: intermediate verification

Verified 2026-10-29 on Apple arm64 LP64, Apple clang 21.0.0 and the existing
coax0 0.16.0 binary with SHA-256
`5388de6c9c976ff6328fd2f4f4aa3433e28d529deaac5f78a74abaab39c52498`.
The [full focused test output](psionic-hybrid-2026-10-29.log) records the
Coax checks, mutation gates and native harnesses for each new primitive. The
[source identity manifest](psionic-hybrid-2026-10-29-sources.json) binds 116
Psionic and imported-source/fixture hashes to that run.

| Authority root | Debug / release checks | Mutants killed |
| --- | --- | --- |
| `delta.coax` | 34 / 34 | 533 / 533 |
| `qwen-attention.coax` | 38 / 38 | 556 / 556 |
| `qwen-matvec.coax` | 31 / 31 | 369 / 369 |

Each root's generated C and native adapter passed debug `-O0` and release `-O2`
both plain and with ASan+UBSan, twelve native harness runs. Compilation used
`-ffp-contract=off -fno-fast-math`; LeakSanitizer was disabled. Independent
owned scalar fixtures cover nonzero recurrent/history state over three steps,
query/key normalization and rotary coordinates, grouped masked attention and
output gates. Native tests include actual unaligned BF16 bytes, alias, capacity
and non-finite refusals. The model-sized shape metadata was checked without
allocating or executing full models.

These are intermediate gates. The whole `psionic/check.sh` suite was not
repeated in this increment; it now includes the new roots and native tests.
No upstream model runtime was executed for this evidence. The
[hybrid progress note](../../psionic/docs/hybrid-kernels.md) defines supported layouts,
limits and the remaining work to complete chapter 11 and model serving.
