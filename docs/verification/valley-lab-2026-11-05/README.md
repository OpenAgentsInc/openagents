# Initial Valley Lab

Story: direct `/worlds/valley-lab` route loads the procedural Coax realm,
browser input drives Aethyr's walker, and its follow camera draws the actual
scene. No API, credentials, downloads or Unreal assets are needed.

The unlisted entry is in `apps/openagents-web/realms.json`; its generated
page is noindex and its discovery-directory projection is in `discovery.txt`.
The world contains a tiled flat basin, sandstone boxes, an arch, climbable
steps and a block traveller. Collision and drawn obstacles share one box
table. WASD moves, Shift runs, Space jumps, arrows look, the wheel zooms and
R/RESET returns to spawn. Left/right touch drags move/look and a right tap
jumps. Blur and Escape release held input.

Validation on the Apple M5 Max, headless muted Chrome using hardware Metal:

- `realm-checks.log`: all 358 checks pass, including movement, jump/land,
  collision, low-step climbing, keyboard/touch equivalence and desktop/phone
  frame bounds. The other logs retain the collision/adoption dependency checks.
- Collision profile authority: 27/27 root mutants killed; renderer adoption
  authority: 44/44 killed. Zero survivors and no equivalent markings.
- `browser.log`: real desktop keyboard and phone-viewport touch dispatches
  through the served page, jump/land, reset, scene readiness, noindex and no
  console errors. This is not physical phone qualification.
- `desktop.png` and `phone.png`: actual rendered views, not mockups.
- Release Wasm: 64,818 bytes under its declared 160,000-byte cap. Shader
  generation comparison, realm generation comparison, build budget inventory,
  whitespace and visual-source checks pass.

The pre-existing tools-check failure remains
`coax/tests/capacity_refusals.py` missing a Firm reason header; no new
non-Coax test/build tool was introduced. Browser verification is itself Coax:
`coax/web/tool.sh valley-lab-check http://127.0.0.1:PORT`.

This is the initial graybox/controller slice of the
[plan](../../verse/2026-11-05-valley-lab.md). The renderer uses the existing
indexed scene/HUD path with explicit adoption tracking; it has no qualified
shared renderer profile or native rendering receipt yet. No virtualized
geometry, asset import, streaming, saved replay, articulated character or
encounter is claimed. Those remain the next milestones. The shared walking
clock and capsule/follow-camera primitives are used directly.
