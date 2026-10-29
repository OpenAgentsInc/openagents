# Experimental fused field candidate

**Source-audit security correction:** the historical aggregate-source acceptance
or conditional decision in this record is superseded and untrusted until
the exact graph and premises are revalidated with the corrected analyzer.
See the [checked-cast advisory](2026-10-29-secret-audit-checked-cast.md).

This partial snapshot provides `ef_field_add`, `ef_field_mul` and `ef_inverse` on 8-by-32-bit field values, plus its fixed-schedule group helpers. Inverse maps inputs congruent to zero to zero and otherwise computes the inverse modulo secp256k1's field prime. These helpers are private by convention. The source and input hashes and [test evidence](2026-10-29-ecdh-fast-field.json) identify the exact graph.

The independent public fixture oracle checked 101,310 full-width input pairs for each of addition and multiplication and 518 inverse inputs. Each checked and release profile passed 203,138 cases on native and Wasm. The complete authority mutation gate killed all 1,416 mutants, with none surviving. The tested compiler and artifact hashes are recorded in the evidence.

An independent source review found the fused carry/canonical selection sound under its reviewed residual and word-bound premises, and checked the inverse schedule against the existing field chain. The recorded conditional integer model additionally depends on the previously reviewed schoolbook induction and unrolled-fold word bounds; model execution alone does not establish source correspondence. Canonical group and finite-point assumptions remain caller obligations.

A source audit of a separate, not-yet-published ECDH private arithmetic root accepted with public peer and secret scalar on the recorded source hash. That historical result does not cover the ECDH byte-view entry or any signing caller; the whole-entry audit remains refused. No object or JIT timing, implicit-copy erasure, ECDH authorization or production-secret approval is claimed. A caller converting projective coordinates must separately establish its finite-point and infinity handling and its own classified source boundary.
