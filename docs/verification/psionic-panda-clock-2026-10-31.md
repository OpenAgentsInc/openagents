# Paired clock consumption in the genuine Panda adapter

The [new sidecar](../../psionic/docs/physics/panda-runtime-clock.md) gates the
existing effects with paired Pl/Sc state, actual forward receipts, exact
capture/step order and explicit partial-effect halts. PTR1 keeps its original
action tick and field/version identity; its row commits after trace encoding.
The public [receipt](../../psionic/fixtures/robotics/libero-panda/panda-clock-qualification.json)
binds fifteen source files, the runtime closure, host changes and actual artifacts.

Eight checks and 251/251 root mutants pass. The new paired ABI passes 1,061
checks per debug/release/fail-fast ASan+UBSan mode. The actual changed host
header runs normal and delayed loops plus seven failure seams in each mode,
with expected abort/no completed fault ledger and no sanitizer findings.

Exactly two real SDK prefixes execute once each on one sanitized executable:
250 steps normal demo 001 and 250 steps delayed demo 000. 22,500 new SDK post-step
F64 words, 500 PTR1 records, 250 cached/fresh snapshot words and 40 request inputs
match existing/reference truth exactly. 90 reset/input words are authenticated
copies, not a new state measurement. Full original NN replies are unavailable
at later refreshes: the named recorded-command fixtures declare unused test
tails; actuator/clipping fields come from the original traces. No NN/provider
initialization, controller recomputation, RGB check or whole episode rerun is
claimed. Root reviewed acceptance-before-output atomicity, old trace tick
ordering, paired halts and original finish/reset semantics without rerunning
passing proof. Shared gates and current chapter 191 notes were added separately.

Updated provider hooks compile; fresh whole-NN use of those hooks, actual wall
timeout/cancellation, generic sensor/command age, complete snapshots and compiler
R13/R14 remain pending. Old numerical/model/SDK receipts retain their identities.
