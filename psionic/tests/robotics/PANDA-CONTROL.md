# Source-matched Panda OSC_POSE controller

`panda-control.coax` implements the pinned robosuite OSC_POSE controller for
chapter 169's LIBERO evaluation backend. The reference source is
[robosuite fbee5844](https://github.com/ARISE-Initiative/robosuite/tree/fbee5844ff5632f5b5698e204ec5357ca50be0df),
particularly `controllers/osc.py`, `utils/control_utils.py`,
`utils/transform_utils.py`, and `models/grippers/panda_gripper.py`.
The simulator supplies telemetry and applies admitted motor commands; all
controller algebra, goals, clipping and gripper state updates are Coax.
This is simulation tooling and has no physical-robot connection.

The fixed `PcSample` is exactly 144 F64 words: world grip-site position (3) and
rotation (9), arm q (7)/qd (7), mass (49), position/rotation Jacobians (21) each, world site
linear-then-angular velocity (6), bias (7), lower (7) and upper (7). Matrices are row-major.
The observation's hand BODY quaternion is a separate quantity and is never
used as the controller's SITE rotation.

```jai
home := pc_default_home();
status := pc_reset(controller[..], sample, home[..]);
output := pc_step(controller[..], sample, action[..], first_physics_substep);
```

The caller supplies one valid state object and seven action/home values.
`pc_reset` anchors goals to the current site pose, uses the explicitly supplied
nullspace home, and sets normalized gripper targets to zero. The simulation
profile uses `pc_default_home()`'s nominal source Panda setup before restoring
its chosen HDF5 state. It does not infer a hidden initial-controller home from
observed joint positions. `pc_step` stages all changes and publishes state only
on success. A refused result's zero-filled motor arrays are invalid; callers
must check `status`, and never interpret them as a zero-motor fallback.

Every new policy action clips its six pose coordinates to ±1, scales position
by .05 m and orientation by .5 rad, and reanchors position at the current site.
Nonzero world axis-angle increments left-multiply the current site rotation.
Exactly zero orientation retains the previous orientation goal. A nonzero
subnormal increment whose norm underflows still takes the source's identity
rotation/update path. The source `axisangle2quat` is F64, while `quat2mat`
explicitly stages quaternion normalization and its outer product in F32.
Coax preserves those boundaries, including NumPy 1.26's widened scalar diagonal
expressions. An unmodified Numba-JIT identity claim is not made.

Operational-space position/orientation inertia is decoupled. F64 mass inversion,
three pseudoinverses, critically damped task gains kp 150/kd 2 sqrt(150),
`Jbar = M^-1 J^T Lambda`, `N = I-Jbar J`, the source nullspace posture gain 10,
transpose of the nullspace projector, and qfrc_bias compensation all run in
Coax. Final torques clamp to ±[80, 80, 80, 80, 80, 12, 12] Nm. A focused check sets
all seven finite bias values to 500 and verifies seven clipped torques.

The normalized finger target advances by `[-1,+1]*.01*sign(command)` on
**every physics substep**, even when pose goals are retained, then clips to ±1.
The two actuator targets map to [0, .04] m and [-.04, 0] m. With twenty control ticks
per simulated second and .002 s physics, there are 25 such updates per action.
Zero gripper command holds this internal target; it does not freeze measured
finger qpos. The scheduler owns the 25-step cadence and calls `new_goal=true`
only for the first substep.

All storage is fixed and all loops have checked finite bounds. There is no
`#bounded` annotation or compiler proof claim. Telemetry admits finite position
within ±100 m, q within ±100 rad, qd/site velocity/J within ±1e4, and mass/bias/limits
within ±1e6, a proper site rotation, and exact pinned actuator limits. Home is
finite within ±100; actions are finite within ±1e308 before pose clipping.
Goal rotations allow the bounded F32 staging error. Refusal preserves every
controller-state field.

`panda-matrix.coax` uses fixed 49-word, stride 7 F64 matrices. Cholesky inversion
requires a symmetric positive-definite mass matrix; it is not an arbitrary
nonsymmetric inverse. One-sided Jacobi SVD uses at most 64 sweeps and a separate
1e-15 column-orthogonality criterion. Pseudoinverse rank uses the actual source
NumPy default: retain singular values **strictly greater than 1e-15 times the
largest**, with no 1e-5 truncation. Zero matrices return zero/rank0; nonzero
scales outside [1e-150, 1e150], nonfinite results and nonconvergence refuse.
There is no additional ambiguous-rank-gap guard. Native comparisons cover
well-conditioned matrices, exact rank deficiency, and diagonal cutoff cases
at, immediately above and immediately below 1e-15. They do not establish a
uniform forward-error guarantee for every ill-conditioned matrix.

Qualification uses seven frozen MuJoCo 3.3.1 telemetry captures from original
demo 0 rows 0, 1, 2, 3, 64, 128, 232, with other scene/state pairs left for independent
evaluation. Eight owned action perturbations and one/25-substep fixed-state
replays produce 112 controller cases. The independent NumPy 1.26.4 oracle follows
the pinned source formulas and calls `np.linalg.inv`/`np.linalg.pinv` directly;
no cloned source is imported or executed. Native debug/release/ASan+UBSan
passed 963 numeric/refusal cases each; all outputs are bit-identical across those
build modes. Against the independent oracle, torque maximum error was
3.410605131648481e-13 Nm under predeclared 2e-5 + 2e-6 |reference| limits; goal
rotation error was 1.1102230246251565e-16, all other state fields and clipping
counts matched. Eighty matrix cases had maximum relative Frobenius error
3.003715628592277e-15 and matching source-default ranks. SDK site velocity
independently matched the arm Jacobian times qd to 2.498001805406602e-16.

All 297 matrix-root and 462 controller-root mutants were killed. The 8 controller and
6 matrix entry checks include a full-field atomic refusal test, all-axis damping,
world-frame orientation, zero-rotation retention, rank-zero task/nullspace,
and per-substep gripper integration. No unchanged GR00T or earlier robotics
release gate was rerun.

```shell
PANDA_REFERENCE_PYTHON=/path/to/existing/numpy1.26/python \
  psionic/tests/robotics/check-panda-control.sh \
  FROZEN_PCSAMPLE_DIRECTORY NEW_OUTPUT_DIRECTORY
```

This qualifies the controller numerical profile and FFI. It does not by itself
qualify simulator integration, reset distribution, learned closed-loop success,
real-time cadence or physical control. Those require separate episode records.
