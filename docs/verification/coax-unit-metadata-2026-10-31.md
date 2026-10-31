# Exact unit metadata producers, before language integration

The first [quantity policy](../../coax/lib/compiler/quantity.coax) is frozen at
SHA 5c072cfaf6cf9cab834d24f0234f0900cd97ec4a0d7812e98227932589d2f441
and 96/96 root mutants killed. It validates four bounded signed dimensions,
positive scales, matching-unit scalar operator admission and binary-ratio
conversion metadata. Its parser/checker/carrier integration is a separate
in-progress milestone; this commit alone enables no new language syntax.

The [unit-algebra producer](../../coax/tests/unit-algebra/README.md) adds exact
canonical rational normalization, product, quotient and equal-dimension ratio
metadata. Nine authored families cover 1,344 cases; all 69 changed-root mutants
die, with six unchanged quantity-policy checks loaded. The independent Python
Fraction oracle compares 114,192 results/799,344 words per debug/release/ASan-UBSan
mode, all exact including refusal payloads. Its [source-bound receipt](../../coax/tests/unit-algebra/qualification.json)
retains the 114,192-row input and output digests and source/compiler identities.

The sanitizer wrapper was hardened to fail fast and require empty stderr.
Original sanitized stderr was empty; only the changed sanitized phase was
rebuilt/rerun from cached generated C, input and Fraction answers. It still
matches every word, with empty stderr. Debug/release, mathematical oracle and
unchanged mutation results were reused, with numerical-function AST identity
recorded. Test registration was checked structurally; the full compiler gate
was not run.

Q_Unit result bounds remain ±8 dimensions and 2^20 scale fields after reduction.
A transient conversion exponent 40 is not a representable metadata scale 2^40;
the different result contracts remain explicit. Nominal declaration identity,
derived-unit type interning, source operator lowering, frames and full R12
consumer qualification remain separate integration work.
