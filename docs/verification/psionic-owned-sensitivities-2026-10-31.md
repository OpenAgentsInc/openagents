# Owned-plant state/action sensitivity bridge

The [scope](../../psionic/docs/physics/oscillator-functions.md) and
[qualification](../../psionic/fixtures/physics/oscillator-functions/qualification.json)
connect 107’s graph calculus to an augmented RK4 realization of 109’s plant.
The frozen 45 points and 18 batches pass 202/202 changed-root mutants and 2,592 native
assertions per host debug/release/ASan-UBSan mode. All later payloads match the
first mode exactly, so its independent mathematical calculation was reused.

The polynomial oracle checks 180 F32 primal values exactly and 1,485 formal values
with maximum error 4.44e-16, plus 540 exact batched values. Original F64 physical
states differ by at most 5.57e-8; physical Jacobians by 1.08e-8 and physical
cost derivatives by 1.91e-7, under budgets frozen before native execution.
F64 physical steps agree with the independent polynomial within 2.23e-16;
180 direct-force finite differences agree within 1.04e-12. The two example
modes match a golden whose assertions/counts come from the independent paper
point. New script/dispatcher structure is checked without rerunning these gates.

The graph’s q/v/u derivatives keep all packed context fixed. F32 staging is
not bitwise F64 solver identity, and formal calculus is not rounded-map
calculus. The raw graph admits the starting state; actual physical publication
retains os_step’s next-state admission. Read-only independent review confirmed
this boundary and the polynomial/coverage. No GR00T/robot-backend, contact,
compiler-bounded or hardware-control qualification is implied.
