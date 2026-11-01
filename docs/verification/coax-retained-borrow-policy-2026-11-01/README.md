# Retained-store policy foundation (B0)

[The record](record.json) binds the new unloaded policy, check-first history, original failed mutation attempt, final 128/128 killed with zero survivors, and three actual native profiles. Each O0/O2/ASan+UBSan profile observed 1,251 calls, including 112 independent policy and 16 bounds vectors, with native layout and logical field/guard checks.

[The archive](proof.tar.gz) retains source snapshots, raw outcomes, generated C/header/vector include, and original producer attribution. Native binary hashes are retained; the executables are excluded. The [manifest](manifest.json) enumerates the exact archive sources. [Final peer review](final-peer-review.json) reuses these observations without replay.

This is a standalone supplied-fact policy tested with the previously qualified 0.46 compiler. It does not change accepted source or establish a new compiler seal. B1 concrete-body traversal/caller substitution, B2 actual helper-write return composition and B3 request ownership remain open. The compiler fingerprint/bootstrap inventory must include this policy before B1 loads it.
