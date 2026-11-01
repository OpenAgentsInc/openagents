# Locomotion foundation: return targets and a clipped scalar objective

This CPU F64 precursor constructs GAE targets and evaluates one clipped PPO surrogate. It does not run an environment, learn a policy, update an optimizer or make a quadruped walk. Chapter 171's LQR convergence, held-out locomotion, learning replay and teacher/student criteria remain open.

## Types, shapes and admission

`RgaReward{value:f64}` is a nominal normalized-reward wrapper. Gamma/lambda and PPO ratio/epsilon use `Quantity(UnitOne,f64)`. Physical-time discounts, physical-length ratios and substituting a dimensionless Quantity for nominal Reward are refused by three new compiler witnesses. These wrappers do not attest where a caller obtained a reward or value.

A GAE input row is 40 B: reward, current value, next value (three F64), then terminated/truncated (two I64). Its 40 B output is delta, advantage, lambda-return, then bootstrap/trace masks. Inputs are world-major, 1..16 fragments × 1..128 rows, at most 2048 rows. Invalid dimensions refuse before multiplication. Shapes, both finite 0..1 hyperparameters, ALL numeric fields (including masked ones), ±1e6 numeric caps and exact 0/1 flags are admitted before computation. An internal fixed stage publishes all results only after success, including overlapping views. It is not a compiler R14 stack/scratch/no-blocking guarantee.

PPO outputs 40 B: surrogate, negative loss, formal derivative with respect to new log-probability, clipped ratio and active-clip integer. Ratio is in [2^-20,16], epsilon in [0,1] and |advantage|≤1e9. The wider advantage cap admits every bounded GAE advantage (≤3.84e8) without implicit normalization or clipping. Every scalar must be finite before range admission, even when advantage is zero. Complete outputs remain unchanged on refusal.

## Episode semantics and rounding

Termination removes bootstrap and trace. Truncation removes only trace and uses the FINAL observation's value before reset. A nonterminal collector tail also bootstraps but ends recurrence. Both flags may be one; termination wins. A genuine finite-horizon task deadline must be represented as termination, not blindly treated as an external continuing-task cutoff. The API cannot verify final-observation provenance.

The frozen recipe computes gamma×lambda once, conditional gamma×nextV, `(reward+next)-V`, conditional `delta+discount*next_advantage`, and then advantage+V. Reverse recurrence resets per world. All listed operations round to binary64 with no contraction/reassociation; masked numeric fields still require full admission. Targets and old-policy data are detached for future learning use.

PPO uses rounded endpoints 1±epsilon and the sign-aware minimum. Positive advantage clips only above the upper endpoint; negative advantage clips only below the lower. Exact rounded knots choose the UNCLIPPED formal derivative. Active clipping is determined from sign/rho, never equality of rounded products. Only zero advantage canonicalizes surrogate/loss/derivative to +0; other signed-zero behavior follows the frozen recipe. The derivative is of the real-formal objective under the declared branch, not a quantized-output derivative. There is no exp/log-probability adapter, batch mean, KL, entropy, advantage whitening, value loss or optimizer hidden in this query.

## First qualification

- 16 new source groups and 156 actual emitted root sites killed: 100 GAE + 56 PPO, zero survivors or equivalent waivers. The public aggregate has zero new body sites and no separate mutation run. Initial GAE observational survivors are retained in source history.
- 96 frozen GAE cases / 5,023 rows and 1,108 PPO cases. Independent GAE expectations use explicit future residual sums/products, Fraction rounding and Decimal80 rather than the production reverse recurrence.
- All 5,023 delta words and 10,046 mask integers exact; 565 one-row segments also have exact advantage/target (6,153 exact GAE F64 word comparisons total). Longer-segment errors are budgeted using local segment length and weighted residual magnitude. Maximum advantage/target errors 1.283e-8 / 1.281e-8 use at most 3.761% / 4.279% of their frozen per-row bounds.
- All 4,432 PPO F64 words and 1,108 active flags exact against the Fraction-RN oracle, including 218 zero-advantage and 152 exact rounded-knot cases.
- Debug, release and fail-fast ASan+UBSan each pass 22,545 assertions, 374 refusals, 22 overlaps and 85 IEEE patterns. Complete output streams match byte for byte across modes. 28 new reader poisons are refused; all actual/expected values must be finite before error maxima.
- The pure `locomotion-targets.coax` example matches its independently prewritten dyadic transcript in its first debug/release executions. It has no environment or training calls.

The first native target is Darwin arm64, binary64 round-to-nearest with gradual underflow and no contraction/fast math. Other targets refuse this native qualification rather than count as a skipped pass. Quantity values are used at API boundaries; general Quantity-operator provenance and compiler R14 remain separate work.

The primary papers motivate GAE, clipped PPO and time-limit bootstrapping. Domain caps, mask transport, exact rounded knots, chosen derivative, atomicity and numerical budgets are explicit owned contracts. See `experiment.json`, `source-qualification.json`, `unit-refusals.json`, `reference.json` and `native-qualification.json` for fixed inputs/evidence. Low-level helpers and PODs are unsealed numerical values, not provenance certificates.

The focused `check-locomotion.sh` gate consumes committed inputs/expectations and runs only this new numerical/ABI context. It needs no private assets, NN, SDK, simulator or external RL runtime. No old tape, optimizer or model gate is repeated. Distribution/log-density adapters, policy/critic updates and a frozen known-optimum LQR learner come before any quadruped/contact/device-batching claim. Hardware and compiler R14/R20/R21/R23 remain separate.
