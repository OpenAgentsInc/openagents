# Owned oscillator and surrogate qualification

This is chapter 109's CPU F64 plant and supervised experiment. Product physics,
training assembly, admission and scoring are Coax. The native C and Python
files are independent development oracles (Firm reason 2); the shell wrapper
only builds and runs those tools (reason 5). They are never required at runtime.

```shell
coax/bin/coax test psionic/lib/physics/oscillator-evaluate.coax
coax/bin/coax mutate psionic/lib/physics/oscillator.coax --root-only
coax/bin/coax mutate psionic/lib/physics/oscillator-data.coax --root-only
coax/bin/coax mutate psionic/lib/physics/oscillator-learn.coax --root-only
coax/bin/coax mutate psionic/lib/physics/oscillator-evaluate.coax --root-only
psionic/tests/physics/check-oscillator.sh /tmp/oscillator-native
coax/bin/coax run psionic/examples/learn-a-physical-system.coax
coax/bin/coax run psionic/examples/learn-a-physical-system.coax --release
```

The current qualification reuses the already passing per-root mutation runs;
none was repeated after passing on its final unchanged source. The native
wrapper is supplied for future reproduction; initial qualification ran its
three modes separately to avoid repeating the passing debug mode. Both
example transcripts match a golden computed from the independent Python SGD
fit and its matrix-polynomial teacher/rollout, rather than copied stdout.

The experiment parameters, whole groups, seeds and budgets were specified in
`fixtures/physics/oscillator/experiment.json` before fitting. Separate numerical
oracle budgets were specified in `verification-contract.json` before the full
256-epoch experiment and native analytic qualification. No heldout-quality
threshold was invented after observing the result; all outcomes are retained.

The independent C oracle compares 5,400 admitted RK4 steps with the analytic
constant-force underdamped solution. Maximum position/velocity absolute error
is 3.7594798385498507e-7 (budget 1e-5). At one second, halving the time step gives
error ratios 15.9863882 and 15.9959037, inside the frozen 12–20 interval. It checks
2,304 unforced steps for nonincreasing energy, all seven nonfinite scalar
positions, negative/oversize native spans, capacity, overlap, late refusal,
untouched tails and complete atomic publication. The result is 26,548 native
assertions per mode including storage/serialization assertions; the native
receipt's counter is taken just before its final close assertion.

The independent Python oracle uses the augmented matrix polynomial
`I+hA+h²A²/2+h³A³/6+h⁴A⁴/24`, not a copy of the product's four RK stages. It
checks all 8,960 feature/target words and 1,280 group IDs. It separately rebuilds
scaling, SplitMix64 initialization, persistent Fisher–Yates shuffles, mean
squared-error minibatch gradients and weight-only decay for all four trained
heads. Its 140 fitted statistics/parameters agree to 1.55e-15; it then checks
all 12 model/split metrics with independent prediction and rollout code.

Debug, optimized release and ASan+UBSan all pass. Their serialized solver
results, dataset, groups, learned parameters and metric records are bit
identical. No unmodified chapter 103 trainer gate or global Psionic gate was
rerun. The frozen quantitative receipt is
`fixtures/physics/oscillator/qualification.json`.

Every baseline has its own autoregressive state. A refused prediction ends
that rollout; its accepted prefix, refusal count and first episode/tick remain
visible. A separate one-step prediction always starts from the true previous
state. Metrics are squared sums plus explicit accepted-row denominators, so
refused rows cannot masquerade as successful predictions. The frozen full
experiment happens to have no refusals; deliberate failure checks exercise
that reporting path.

This profile uses a fixed 1 kg mass, spring [.5,4] N/m, damping [.1,1] N·s/m,
position ±2 m, velocity ±4 m/s, force ±2 N and step [2^-10,.05] s. It is an owned smooth
solver, not the articulated/contact solver needed to replace MuJoCo for Panda.
The learned discrete transition is qualified only at .05s. Compiler typed
clocks, differentiating through this solver, general function transformations,
and real-world validity are separate claims.
