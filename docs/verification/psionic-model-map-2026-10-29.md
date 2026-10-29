# Psionic complete model mapping verification, 2026-10-29

`model-map.coax` stages the complete layer schedule and rebases each decoded
parameter view onto caller-owned storage before publishing all layers together.
It requires separately authenticated original files and inventory. A real-file
check bound all 32 Elephant layers and 1,057,280 decoded parameters; the same
generic mapper also bound the previously authenticated Mammoth inventory, but
that is not an end-to-end Mammoth qualification.

On the development Mac, 133 Coax checks passed in debug and release; all
3,601 mutation candidates were killed. Native debug and release checks passed
normally and with ASan/UBSan; native release checks also passed on CoderOS.
Tests include distinct parameter values, rebasing over multiple four-layer
cycles, exact capacities, late failure and alias refusal.
