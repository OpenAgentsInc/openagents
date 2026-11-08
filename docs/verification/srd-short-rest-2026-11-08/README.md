# Short Rest recovery — partial #13040/#13044

Shared Short Rests retain an uninterrupted one-hour deadline. Completion grants
the represented class's Short Rest banks once, preserves turn/Reaction budgets
and opens sequential Hit Dice choices. Hit Dice are seeded once per creature;
restarting or rejoining does not refill them. Damage (including temporary-HP
absorption), Initiative admission, actual strikes, Dash and accepted leveled
casts interrupt the retained rest/choice window. Refused casting and cantrips
preserve it. Additional strenuous activity must be reported by its producer.

The level 1–6 matrix covers all twelve class presets. Optional Arcane/Natural
Recovery validates complete slot choices before spending its once-per-Long-Rest
use. Sorcerous Restoration enforces its chosen amount and floor-half-level cap.
Neither feature's daily history is reset by another Short Rest. No Long Rest
completion/reset API or rest UI is claimed; full activity/controller integration,
remaining recovery and the parent issues remain unfinished.

Verified on `ae1f6e3a59` plus this change: 256 core checks, 238 hotbar checks,
28 actual evaluator/checked-C O0/O2 native scenarios, ordinary checked Lab Wasm
build (81.9 s, 2,541,003 bytes). Five authority scopes kill 92/92/242/130/84
mutants: **640 killed, zero survivors, no exemptions**. Unchanged passing scopes
are reused after additional fixture-only changes. Logs and hashes are retained.
The temporary Wasm target has no registered shipping-size budget; no browser
or complete-world qualification is inferred. Compiler and budgets are unchanged.
Tools-check reuses the unchanged covered-tool result from `79a2ce92b3` at
`../srd-stabilization-2026-11-08/tools.log`; no new non-Coax tool was written.

Rebased onto `23286cef4a`, preserving incoming locomotion and actual-turn Shield
retirement. The merged tree passes 258 core and 240 hotbar checks. The five
gated policy files remain unchanged; earlier mutation/native/Wasm evidence is
retained at its stated scope, without claiming a new merged-tree Wasm run.

Source: pinned SRD 5.2.1 PDF SHA-256
`8974902d109d6e63672d7c490bde9ccf052410503d9cfa768237154fbc5e3d87`,
Short Rest p.187 and each class's recovery feature. This work includes material
from the System Reference Document 5.2.1 by Wizards of the Coast LLC, available
at https://www.dndbeyond.com/srd, licensed under Creative Commons Attribution
4.0: https://creativecommons.org/licenses/by/4.0/legalcode.
