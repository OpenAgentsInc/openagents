# Canonical nominal unit forms

[`unit-forms.coax`](../../lib/compiler/unit-forms.coax) is a pure producer for
future derived-unit interning. It does not inspect physical dimensions/scales,
resolve declarations, change operators or grant ownership of an atom ID.

`UF_Form` contains `count:int`, `atoms:[8]int` and `powers:[8]int`.
Active atoms must be strictly increasing IDs in **1..2,147,483,647**. Active
powers are nonzero integers in **−2,147,483,647..2,147,483,647**. Every inactive
atom and power slot is exactly zero. The empty form is formal `UnitOne`.
IDs are opaque and program-local; the caller must bind them to admitted nominal
declarations. Equal physical metadata never merges different IDs here.

| API | Result |
| --- | --- |
| `UF_one()` | Canonical empty `UF_Form` |
| `UF_atom(id)` | One atom with power one, or metadata refusal |
| `UF_valid(form)` | Admission of the complete canonical representation |
| `UF_equal(a,b)` | False for either invalid input; otherwise exact count/atom/power equality |
| `UF_product(a,b)` | Sorted sum of powers, removing zeros |
| `UF_quotient(a,b)` | Sorted difference of powers, removing zeros |

Constructors and arithmetic return `UF_Result { status, form }` except
`UF_one`, which directly returns the empty form. Status values are `ok=0`,
`metadata=1`, `power=2`, `capacity=3`. Both inputs are admitted first. With valid
inputs, a result power outside the symmetric range wins over excess capacity,
even if that power is encountered after the ninth surviving atom. Only then is
the eight-atom result limit checked. Every refused form is entirely zero.

Both input powers are bounded before addition, subtraction or negation, so
intermediate powers fit signed 64-bit arithmetic. Merge consumes at least one
input atom per iteration. Cancellation happens before result counting; two
eight-atom forms may cancel completely or leave an admitted smaller form.
Private staged data can change, but no partial form escapes on refusal.

For example, `(atom1 / atom2) * atom2` reduces to the single `atom1`. Two
different declared dimensionless tags remain distinct atoms. The first is not
automatically raw scalar data, and a form such as `HalfMetre / Metre` does not
become `UnitOne` merely because both physical dimensions cancel.

This is a partial algebra. An intermediate power/capacity refusal is not
rescued by a later expression. The compiler must separately admit each physical
unit through the existing metadata algebra, then apply this form profile, and
intern using the complete canonical form. Reassociating types must not reorder
runtime effects or floating-point operations. Source integration, physical
metadata, registry membership, F32/F64 arithmetic, device ABI, frames and full
R12 remain separate qualifications.

## Evidence

Ten check families cover **91** cases. Changed-root mutation kills **122/122**
mutants with zero waivers. The first pass exposed three missing observations:
invalid-versus-valid equality and the exact eight/nine-atom boundary. Adding
those checks closed all three without changing product arithmetic.

The independent Python oracle accumulates signed powers in an arbitrary-
precision dictionary and sorts only the final entries. It does not reproduce
the producer's two-pointer merge. **132,956** cases include all small sparse
forms over three IDs, all their pairs, exponent boundaries, distinct high IDs,
invalid counts/order/tails, cancellation, power-before-capacity precedence and
fixed random cases (seed `0x55464f524d533032`). Counts and full transcript lengths
are asserted before comparison.

Debug, optimized release and ASan/UBSan each match **2,526,164** independently
expected logical words exactly, including all failure payloads and Boolean
observations. Sanitizers fail fast with `-fno-sanitize-recover=all`; native runs
require empty stderr. The C harness only transports values, calls Coax and
serializes results. No dictionary implementation or unit decision ships in C
or Python. Large generated cases/transcripts stay outside the repository.

The [receipt](qualification.json) binds the source, oracle, transport, private
qualified Coax 0.43 compiler and exact output hashes on Darwin arm64. It does
not establish a cross-architecture or whole-compiler guarantee. Reuse this
proof when these files are unchanged.

```shell
/tmp/coax-quantity-bootstrap-043/coax test coax/lib/compiler/unit-forms.coax
/tmp/coax-quantity-bootstrap-043/coax mutate coax/lib/compiler/unit-forms.coax --root-only --jobs 8
COAX_TEST_COMPILER=/tmp/coax-quantity-bootstrap-043/coax python3 coax/tests/unit_forms.py
```

The tool accepts `COAX_TEST_COMPILER` and `COAX_UNIT_FORMS_RECORD_DIR`. Its
portable fallback is the repository's `coax/bin/coax`; the recorded run used
the explicit private 0.43 compiler above. A tooling-only follow-up changes this
fallback, preserving the executed driver hash and compiler identity in the
receipt. Its oracle/generator function bodies are unchanged, and no numerical
gate was repeated. These checks do not install a compiler or update its cache.
