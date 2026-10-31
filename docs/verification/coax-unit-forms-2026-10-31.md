# Canonical nominal unit-form policy before source integration

The [new producer](../../coax/lib/compiler/unit-forms.coax) merges canonical
nominal atom/power lists. It cancels matching factors before the eight-atom
capacity check, admits positive signed-31-bit atom IDs and symmetric nonzero
signed-31-bit powers, and compares complete forms. Invalid input precedes
power overflow, which precedes capacity refusal. Failure payloads are zero.
This does not unify distinct declared units by dimensions or scale.

Ten authored check families cover 91 cases; focused root mutation kills 122/122
with zero survivors or equivalent waivers. An independent Python dictionary
oracle supplies 132,956 results and 2,526,164 logical words per native mode.
Debug, release and fail-fast ASan/UBSan match all words with empty stderr.
The [receipt](../../coax/tests/unit-forms/qualification.json) binds actual
sources/compiler/results and retains its initial missing-observation failures.

The tooling-only portability follow-up preserves the historical executed
script hash and records the new default of repository `coax/bin/coax`; the
four oracle function ASTs are identical. Explicit `COAX_TEST_COMPILER` still
selects the private qualified 0.43 compiler. No numerical proof was rerun for
this default-path edit. The standalone gate is registered without running the
old full compiler suite.

This is a producer, not implemented source-level UnitProduct/UnitQuotient,
compiler type interning, physical metadata, runtime quantity arithmetic or
full R12. Those integration steps are being built separately. Unchanged UA,
M1 quantity, model and SDK results are reused rather than rerun.
