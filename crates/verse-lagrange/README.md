# verse-lagrange

Sun–Earth L1 physics for Verse's [Lagrange 1](../../docs/verse/lagrange-1.md)
zone. No renderer, networking, or I/O; the Verse host maps input and draws.
It consumes the owned `physics` crate and serves as a regression fixture for
[Verse Engine](../../docs/verse/engine/roadmap.md).

- `orbit` — the circular restricted three-body problem (Sun and Earth–Moon
  barycenter) in the rotating frame: L1 from the collinear quintic, linear
  constants (c₂, λ, ω_p, ω_v), RK4 integration, the Jacobi integral, a
  Lissajous orbit, and station-keeping that cancels the linear unstable mode.
- `earth_moon` — the Earth–Moon CR3BP for Lagrange 2 (DE440 μ = 0.0121506):
  L1 and L2 through the shared collinear solver (`orbit::Collinear`), a
  southern L2 halo found by differential correction with continuation in z₀
  (half widths about 12,600 × 36,200 × 14,000 km, period 14.77 days, Earth
  always in view past the Moon's limb), and `HaloOrbit`, a station on it
  that evaluates a targeting burn every day (the reference halo's position
  two days downstream, through the state transition matrix) against solar
  radiation pressure (5 × 10⁻⁸ m/s²), 1 km / 1 mm/s navigation error, and
  2% burn error. It spends about 7.4 m/s a year in 362 burns; ARTEMIS flew
  Earth–Moon L2 for 7.39 m/s a year (Folta et al. 2013). `apply_burn`
  departs the halo for the ship's test burn.
- `physics::body` — shared torque-free rigid bodies: Euler's equations (RK4) and quaternion
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
