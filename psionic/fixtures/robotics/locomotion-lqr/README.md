# Scalar-LQR projected PPO actor experiment

This is a normalized scalar actor experiment, not physical robotics, a learned critic or a complete gait learner. All old tape, AdamW, GAE, Gaussian and counter-RNG bodies remain unchanged. The four new authority roots separate actor VJP, rollout, update and quality scoring; the learner never imports or calls the scorer/optimum.

The model uses x_next=x+u, reward=−(x²+u²), actor mean=−K*x, fixed log-standard-deviation −1.3862943611198906 and nominal sigma=.25. The declared nominal continuous-Gaussian critic is a quadratic analytic aid, not an exact finite-top53 value. K_old and its critic, sampled actions/states, old logp and GAE targets are detached for all four epochs. The identical critic coefficients are recomputed from immutable K per scalar transition and also retained in the batch header; no one-call-per-rollout performance claim is made.

`rlp_block` admits 1..64 rows and builds one one-parameter tape with exactly 7+13*N nodes (839 at 64). It validates current logp against the carried Gaussian query before backward. The tape scalar is a local-VJP proxy, not the actual PPO loss. Epochs reduce raw block sums in fixed order, divide once by actual count, call old binary64/refuse AdamW in disjoint scratch and project only the proposed K into the global box intersected with rounded K_old±1/256. The same K_old anchors all four full-batch updates. Proposed moments and step survive projection; there is only one persistent K/master.

`rle_transition` is a genuine checked numeric sampled-observation boundary, including observed z and candidate next-state bounds; it is not proof of RNG provenance. `rle_rollout` supports 1..16 worlds and 1..32 horizon for small fixtures. Named learning always uses 16×32, eight 64-row blocks and four full-batch epochs. World states and explicit disjoint counter ranges persist. Initialization and state admission bind seed labels, counters, batch and exact optimizer step. Diagnostic seed 13 is used only for one source wiring fixture; canonical run labels are 0,1,7.

Malformed ordinary inputs, ratios and optimizer operations refuse atomically. An impossible constructed-tape failure or logp-word mismatch is a fatal internal invariant before publication, not a new policy fallback. Helpers/PODs have numeric, shape and lineage preconditions and are unsealed. Fixed arrays are not compiler #bounded, R14 or a no-blocking guarantee. The four type witnesses demonstrate explicit raw normalized boundaries and record roles, not a physical dimensional LQR interface.

## Frozen evidence

`source-qualification.json` binds 554 actual new product mutation sites (158 actor,111 rollout,188 update,97 score),21 groups and zero survivors. Separate test-only coefficient transport closes 23 sites in two groups; the load-only aggregate has zero body sites and no invented proof. First parse, redundant/default and observation failures are retained; no equivalent waivers or old standalone gate repeats. Four compiler-only type/role refusals are in `unit-refusals.json`.

The authorized source-VM diagnostic wiring witness runs one 512-row batch/four updates for seed 13. Its source/mutant evaluations are separate from canonical native workload. Mathematical fixtures and the pure example are also separate.

The first debug, release and fail-fast ASan+UBSan mathematical profiles each pass 229,063 assertions, 177 refusals, 12 overlaps and 83 reader poisons; complete output streams are byte-identical. The corpus contains 112 actor blocks and 24 epochs. Gradient absolute error is at most 2.911e−10 under the FROZEN per-block conditioning bounds; loss/proxy errors are at most 2.138e−11/5.821e−11. Epoch loss/gradient and raw-candidate errors are numerically zero in this corpus. The coefficient layer is independently validated before analytic local-VJP comparison; observed score/gradient fields are not used as their own oracle.

Each mathematical host run executes 597 corpus rollout steps plus 13 guard rollout steps, 798 short deterministic evaluation steps and other declared scalar/epoch guard queries. It executes ZERO successful full training batches. Those fixtures are separate from canonical learning, checkpoint replay and the later 3,072-step named evaluation. Ordinary critic/transition/projection comparison error is zero; short evaluation error is at most 3.553e−15 and rollout error 1.777e−15. This is the qualified Darwin arm64 binary64 scalar-libm/no-FMA profile, not cross-target determinism or an owned-libm proof.

The fixed experiment completed all 128 batches and 512 AdamW updates per seed. All 31 execution/audit actions passed, including byte-exact next-batch checkpoint replay. **All three seeds miss both prospective convergence goals.** No seed is promoted and no setting is retuned.

| Seed | Final K | Absolute gain error | Worst nominal held-out cost gap | Result |
| --- | ---: | ---: | ---: | --- |
| 0 | 0.5184256094 | 0.085306525 | 1.404964% | Both goals missed |
| 1 | 0.5227558679 | 0.080976266 | 1.259577% | Both goals missed |
| 7 | 0.5214162090 | 0.082315925 | 1.303622% | Both goals missed |

The nominal optimum is K*=0.6037321344. The frozen limits are gain error at most .05 and cost gap at most 1%. The initial K=.25 has gain error .3537321344 and worst nominal gap 40.451147%; the learned gains improve those measures substantially, but improvement does not satisfy the acceptance rule. The independent real-arithmetic margins also reject all three seeds. Rounded table values are explanatory; the original receipts retain full precision.

Canonical learning executes 196,608 plant steps, 1,536 AdamW updates, 12,288 tape forward/backward pairs and 786,432 sample-gradient evaluations. Declared checkpoint replay adds 1,536 steps/12 updates/96 tape pairs; four mean-policy evaluations add 3,072 deterministic steps. The summed action time is about 208 seconds on this named host, not a GPU throughput or end-to-end engineering-time measurement. The [LQR fixture](README.md) retains the full outcomes and compact curves.

This closes the mathematical and experiment-integrity slice. The LQR convergence criterion remains **open**. A future separately frozen experiment may study optimizer/update budget; extending this profile would also require revisiting its state/counter bounds. The current result is retained without extra training or in-place tuning.

Canonical planned workload is 196,608 new plant steps, 1,536 AdamW updates, 786,432 sample-gradient evaluations and 12,288 tape block pairs. Same-target next-batch checkpoint replay adds 1,536 plant steps/12 updates/6,144 sample-gradient evaluations/96 block pairs. Baseline plus three final deterministic mean policies at six held-out starts add 3,072 transitions. The nominal analytic noisy-policy costs are not executed held-out stochastic episodes. Every fixed seed/outcome is reported; failures are retained without tuning, seed selection or a best-checkpoint promotion.

The default gate is fixture-only. Canonical learning is an explicit release-only one-shot command, chunked with persisted completed checkpoints; it is not automatically retrained in all host modes. Quadruped gait, sensors/contacts, terrain, learned critic, device batching, repeatable whole learning curves and teacher/student transfer remain open.
