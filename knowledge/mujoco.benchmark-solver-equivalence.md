---
id: mujoco.benchmark-solver-equivalence
version: 1
kind: method
title: Benchmark MuJoCo solver changes against state and runtime
summary: >-
  When optimizing a MuJoCo model under a final-state tolerance and runtime
  target, change numerical solver options without changing physical
  parameters, then compare seeded trajectories and benchmark repeated runs. A
  faster solver is useful only if independent states remain within the
  specified tolerance.
tags: [mujoco, simulation, performance, numerical-validation]
applies_when: >-
  A MuJoCo XML model has a reference implementation and evaluation measures
  simulated final-state agreement plus wall-clock runtime.
status: admitted
author: microcoder kb harvest (gpt-6-luna)
provenance:
  written_from:
    - tune-mjcf
  cites:
    - "Emanuel Todorov, Tom Erez, and Yuval Tassa, “MuJoCo: A physics engine for model-based control,” 2012, sections 2–3."
    - MuJoCo official documentation, XML Reference, `option` element.
evidence:
  - "admitted 2026-09-26 by review: round2-oos-review"
---

## Details
MuJoCo solver and Jacobian choices can alter convergence cost and numerical results without changing model geometry, masses, constraints, actuators, or timestep. Treat alternatives such as PGS, CG, dense or sparse Jacobians, and solver tolerances as candidates to benchmark, not universally equivalent drop-in settings. First inspect the evaluation's state comparison, tolerance, randomization, and timing methodology. Preserve all physical model settings; test each candidate on identical seeds against the reference and reject candidates exceeding the evaluator's error threshold or producing non-finite state. Measure repeated end-to-end simulation time for both reference and candidate, since isolated solver assumptions do not establish an actual speedup. Favor a margin below the correctness threshold and validate additional independent seeds; a passing single trajectory does not establish robustness.

Source: Emanuel Todorov, Tom Erez, and Yuval Tassa, “MuJoCo: A physics engine for model-based control,” 2012, sections 2–3; MuJoCo official documentation, *XML Reference*, `option` element (solver, jacobian, tolerance, and iterations attributes).

## How to check
With the installed MuJoCo Python binding and an evaluator-provided simulation function, run paired trajectories and compare the same state representation:

```python
import time
import numpy as np
import mujoco

reference = mujoco.MjModel.from_xml_path("reference.xml")
candidate = mujoco.MjModel.from_xml_path("candidate.xml")
seeds = [11, 23, 47, 89]
errors = []
for seed in seeds:
    expected = simulate_model(reference, seed)
    actual = simulate_model(candidate, seed)
    assert np.isfinite(actual).all()
    errors.append(np.max(np.abs(actual - expected)))
assert max(errors) <= atol

for model in (reference, candidate):
    start = time.perf_counter()
    for seed in seeds:
        simulate_model(model, seed)
    print(model, (time.perf_counter() - start) / len(seeds))
```
Also inspect the XML diff to confirm only intended numerical solver options changed; rerun the official evaluator for its actual randomized correctness and timing behavior.
