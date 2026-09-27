# verse-lagrange

Sun–Earth L1 physics for Verse's [Lagrange 1](../../docs/verse/lagrange-1.md)
zone. No renderer, networking, or I/O; the Verse host maps input and draws.

- `orbit` — the circular restricted three-body problem (Sun and Earth–Moon
  barycenter) in the rotating frame: L1 from the collinear quintic, linear
  constants (c₂, λ, ω_p, ω_v), RK4 integration, the Jacobi integral, a
  Lissajous orbit, and station-keeping that cancels the linear unstable mode.
- `body` — torque-free rigid bodies: Euler's equations (RK4) and quaternion
  attitude (exponential map).
- `station` — the construction sandbox: a suited astronaut with a 40 N,
  Isp 70 s cold-gas pack under a velocity-command control law; six parts with
  real mass properties; inelastic grab, release, and latch; collisions; the
  linearized L1 field with Coriolis terms; and an orbital clock at 3,600×.

```sh
cargo test -p verse-lagrange
```

Tests check L1's distance and linear constants against the literature, Jacobi
conservation, uncontrolled divergence, two years of controlled flight, the
rocket equation, momentum-conserving capture, latch limits, collisions,
angular-momentum and energy conservation, and the intermediate-axis flip.
The [zone documentation](../../docs/verse/lagrange-1.md) lists every
approximation.
