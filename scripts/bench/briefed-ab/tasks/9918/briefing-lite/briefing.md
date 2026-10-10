# Briefing: #9918 Flaky under load: verse ball::tests::the_step_stays_within_lagrange_budget

## The issue

A debug-build timing budget (worst physics step ≤ 5 ms) fails whenever the machine is loaded: worst step 11 ms seen on 2026-09-28, and several agents reported it failing under load and passing alone. A wall-clock budget in a debug unit test is not a reliable gate.

Acceptance: keep the budget meaningful without flaking — e.g. measure it only in release (or behind an ignored/bench test), count work (steps, contacts, iterations) instead of time in the unit test, or compare against a calibration run; document the budget where it's enforced.

## Change plan

1. Goal: Flaky under load: verse ball::tests::the_step_stays_within_lagrange_budget
2. Change the behavior where it lives, most likely in `crates/verse/src/ball.rs`.
3. Add or update a test that pins the new behavior, beside the code's existing tests.
4. Run `check:verse`, then `test:verse`, then `fmt`; stop when they pass.

## Files to change

### `crates/verse/src/ball.rs`

Why: has `the_step_stays_within_lagrange_budget`

```
--- lines 795-825 of 869 ---
  795          body.omega = DVec3::Z * (-6.0 / RADIUS);
  796          body.wake();
  797          let mut player = PlayerController::new(Vec3::new(-30.0, 0.0, -30.0), 0.0);
  798          idle(&mut ball, &mut player, 180);
  799          let x = ball.body().pos.x;
  800          assert!(
  801              x < f64::from(wall.min[0]) - RADIUS + 0.05,
  802              "the wall stopped the ball: {x}"
  803          );
  804      }
  805  
  806      #[test]
  807      fn the_step_stays_within_lagrange_budget() {
  808          let mut ball = Ball::new();
  809          let mut player = PlayerController::new(crate::world::SPAWN, 0.0);
  810          walk(&mut ball, &mut player, 120);
  811          let mut worst = Duration::ZERO;
  812          let mut total = Duration::ZERO;
  813          let mut steps = 0;
  814          for _ in 0..240 {
  815              let from = player.pos;
  816              player.update(&InputState::default(), FRAME, &[], crate::world::HALF);
  817              ball.advance(from, &mut player, FRAME);
  818              worst = worst.max(ball.world().stats.total);
  819              total += ball.world().stats.total * ball.steps;
  820              steps += ball.steps;
  821          }
  822          eprintln!(
  823              "ball step: mean {:?}, worst {:?} over {steps} steps",
  824              total / steps.max(1),
  825              worst
```

## Similar past changes

### d93825ae36 Draw no floor disc under the bare world's ball and blocks

```diff
diff --git a/crates/verse/src/ball.rs b/crates/verse/src/ball.rs
index cada2a7340..cb88f4be0a 100644
--- a/crates/verse/src/ball.rs
+++ b/crates/verse/src/ball.rs
@@ -13,6 +13,6 @@
 //! Lagrange 1, and the ball is drawn between its last two poses. The ball is
 //! drawn with physical materials under a studio [`Key`] light: a lacquered
-//! octant pattern, so its rotation reads, and a pool of light on the floor
-//! that catches its shadow.
+//! octant pattern, so its rotation reads. Nothing is drawn on the floor
+//! under it: the grid's lines alone ground it.
 
 use std::sync::OnceLock;
@@ -64,5 +64,5 @@ const PLAYER_HEIGHT: f64 = 1.8;
 /// The fastest the player body is carried, m/s; a larger step is a teleport.
 const PLAYER_MAX_SPEED: f64 = 20.0;
-/// Radius of the floor's pool of light around the ball, m.
+/// How far around the ball its shadow region reaches, m.
 const POOL: f32 = 6.0;
 /// The largest half extent of one shadow region over the ball and the
@@ -374,6 +374,6 @@ impl Ball {
     }
 
-    /// Adds the ball, its pool of light on the floor, and the studio light
-    /// that shades them to `mesh`.
+    /// Adds the ball and the blocks, and the studio light that shades them,
+    /// to `mesh`. No floor is drawn under them.
     pub fn draw(&self, mesh: &mut Mesh) {
         let (pos, orientation) = self.pose();
@@ -384,5 +384,4 @@ impl Ball {
                 .map(|v| place(v, &transform, &Mat4::from_quat(orientation))),
         );
-        pool(&mut mesh.lit, Vec3::new(pos.x, 0.0, pos.z), POOL, -0.004);
         self.blocks
             .draw(&self.world, self.clock.alpha(), &mut mesh.lit);
@@ -514,40 +513,4 @@ fn sphere() -> &'static [LitVertex] {
 }
 
-/// A disc of stage floor under the ball whose response falls to nothing at
-/// its rim, so the key light pools around the ball and fades into the
-/// field, and the ball's shadow has a surface to fall on. It sits just below
-/// the grid, whose lines stay on top.
-pub(crate) fn pool(out: &mut Vec<LitVertex>, center: Vec3, radius: f32, depth: f32) {
-    const RINGS: usize = 14;
-    const SEGMENTS: usize = 64;
-    let (color, metallic, roughness) = Surface::Stage.parameters();
-    let code = Surface::Stage.code();
-    let at = |ring: usize, segment: usize| {
-        let r = radius * ring as f32 / RINGS as f32;
-        let angle = std::f32::consts::TAU * segment as f32 / SEGMENTS as f32;
-        let fade = (1.0 - (r / radius).powi(2)).max(0.0).powi(2);
-        LitVertex {
-            pos: (center + Vec3::new(r * angle.cos(), depth, r * angle.sin())).to_array(),
-            normal: [0.0, 1.0, 0.0],
-            tangent: [1.0, 0.0, 0.0],
-            local: [r * angle.cos(), 0.0, r * angle.sin()],
-            color,
-            params: [metallic, roughness, code, fade],
-        }
-    };
-    for ring in 0..RINGS {
-        for segment in 0..SEGMENTS {
-            let next = (segment + 1) % SEGMENTS;
-            let quad = [
-                at(ring, segment),
-                at(ring + 1, segment),
-                at(ring + 1, next),
-                at(ring, next),
-            ];
-            out.extend([quad[0], quad[1], quad[2], quad[0], quad[2], quad[3]]);
```

## Checks (run them with the `run_check` tool)

- `check:verse`: `cargo check -p verse --tests --message-format short` (compile verse and its tests)
- `test:verse`: `cargo test -p verse [FILTER]` (run verse's tests (pass a test-name filter to run fewer))
- `fmt:verse`: `cargo fmt -p verse` (format verse)

## Repo rules

- Product code is Rust. Do not add TypeScript.
- The check for a change is `cargo test -p` for the crates you edited plus `cargo fmt`. Do not run clippy, release gates, or other crates' tests.
- Never put machine talk (internal words like retained, projection, canonical, digest, lane) in text a user sees; say what happened in plain words. Each surface's tests run the `oa-copy` guard over user-visible text.
- When a test fails only because a checked-in generated file is stale, regenerate it.
- No new INVARIANTS rows, design notes, or long docs for a small change.
- Fix stale or false user-facing copy you touch in the same change.
- - `crates/verse` — the shared Verse desktop/iOS world: a Tron-style city drawn in amber lines on the terminal's near-black field, and a third-person character with WoW-style movement and mouselook. The stack follows Ruins of Atlantis (`wgpu`, `winit`, `glam`, a custom renderer); the controller is reimplemented from its `client_core`, not copied. The global plaza uses `coder_ui::theme::Intensity`; a palette test protects its amber geometry. Separately loaded zones may have their own validated colors and atmosphere. Three plaza portals lead to local zones: Ruins loads a pinned asset pack only on entry and runs the original real-time Wizard Woods combat through `verse-ruins`; Lagrange 1 is a generated Sun–Earth L1 construction station driven by `verse-lagrange`; the Physics Lab runs the `physics` crate's mechanisms live with HUD knobs (`docs/verse/physics-lab.md`). Read `docs/verse/zones.md
