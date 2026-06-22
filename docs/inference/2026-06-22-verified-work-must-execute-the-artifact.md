# Verified work must EXECUTE the artifact — intent-derived deterministic acceptance gating

*2026-06-22. The Khala coding verifier returned `verified:true` on a game with
four obvious defects. Root cause: it never runs the artifact — it pattern-matches
the source. This is the process bug to fix: turn the user's intent into a series
of deterministic tests the output must actually pass, executed in a real
programmatic environment, and gate `verified` (and the reward) on that.*

## The failure, with the receipt

A live prod `khala-code` run generated a single-file three.js crossy-road game and
returned `verification: "test_passed"`, `verified: true`, `scalar_reward: 1`. In
reality the artifact:
1. **Crashed on load** — constructor called `updateScoreUI()` before assigning the
   DOM refs → `Cannot set properties of undefined`; the `start-btn` listener never
   attached.
2. **PLAY did nothing** — a `.hidden` game-over overlay (`opacity:0`, no
   `display:none`) stayed full-screen at `z-index:20`, and `.btn{pointer-events:auto}`
   overrode `.hidden{pointer-events:none}`, so it intercepted the click.
3. **Camera flew ~100× per hop** — `updateCamera` multiplied `mesh.position.z`
   (already world units) by `TILE_SIZE` again.
4. **World stopped generating** after ~10 tiles (blue sky).

The verifier caught **none** of them — because it doesn't execute the game.
`verifyKhalaCodeCompletion` (`apps/openagents.com/workers/api/src/inference/khala-code-verifier.ts`)
is pure regex over the HTML string:
- `loads_and_runs_headless` ≔ source contains `<script>` + (`<canvas>`|`game`) +
  (`requestAnimationFrame`|`setInterval`|`loop`). **It never loads the page.**
- `sane_follow_camera` ≔ the words `camera`/`viewport`/`follow` and `lookat`/`player`
  appear somewhere.
- `direction_controls` ≔ the strings `arrowup…`, `'w'`, `keydown`, `move(` appear.

Our game contained all those tokens, so all six checks "passed." The module header
even concedes the real runner was never built — *"The hot Worker route cannot launch
a browser… a stable headless command contract for the runner that executes generated
HTML artifacts outside the Worker"* — yet the verdict stamps that headless command
ref (`KHALA_CODE_HEADLESS_COMMAND_REF`) onto the receipt **as if it ran**.

This is reward-hacking by construction (cf. TMAX §D.6): the rubric rewards *source
patterns*, not *behavior*. A model — or a coder — optimizes to include the right
strings, not to ship a working game. Any training reward derived from this is a lie.

## The principle

**`verified:true` must mean "we ran it and it did what the user asked."** Never
claim verification from static analysis. Turn intent into deterministic, executable
tests; run them in a real environment; gate `verified` and the scalar reward on the
fraction that pass.

## What the process must do

1. **Intent → acceptance spec.** From the request, derive a structured, checkable
   acceptance spec — a list of deterministic tests, not prose. For "build a crossy
   road game":
   - loads with **zero console/page errors**,
   - **PLAY starts the game** (start screen hides, an update loop runs),
   - forward input **advances the player exactly one tile**,
   - the **camera follows within a bounded delta** per move (no 100× jumps),
   - the **world keeps generating ahead** of the player for N moves (no blue sky),
   - collision **ends the game**; **restart resets**.
   Each is a program. For arbitrary prompts the coordinator/verifier generates the
   spec; deterministic, replayable, gated.

2. **Execute in a real env.** Run the artifact headless (Playwright/chromium) +
   shell, in a sandbox — the seam already named (`KHALA_CODE_HEADLESS_COMMAND_REF`)
   but never implemented. Capture console errors, page errors, DOM/state assertions,
   simulated input, and frame/state deltas. (Exactly the checks done by hand to find
   the four bugs above — load → no errors; click PLAY → started; press W ×N →
   advanced + world present + camera bounded.)

3. **Gate `verified` + reward on execution.** `verified` = **all** acceptance tests
   pass. `scalarReward` = fraction passing — a dense, honest signal that **is** the
   M6 training reward. The `verification_receipt` carries per-test results +
   artifacts (screenshots, console logs).

4. **Generation loop.** The coder worker iterates: generate → run the acceptance
   suite → read failures → fix → re-run, until pass or budget. Broken artifacts
   never reach `verified`. This is agentic coding-with-tools gated by a deterministic
   acceptance suite — the verified-work thesis done correctly.

## Infra we already have (answer to "do we have infra to run it via the API")

Yes — the pieces exist; only the execution runner + intent→spec are missing:
- **Khala gateway** — the API surface (`/v1/chat/completions`).
- **Probe runtime** (`packages/probe`) — a programmatic coding environment with
  tools (shell, files) — the natural coder-worker + acceptance-runner host.
- **Cloud sandbox compute** — `CLOUD_SANDBOX_COMPUTE_ENABLED` (set on the Worker)
  + `oa-workroomd` (cloud repo): isolated execution for running untrusted artifacts.
- **Headless browser** — Playwright/chromium, already used in `three-effect`
  (`scripts/capture-crackling-headless.ts`); runs in a sandbox / on a Pylon.
- **Verification-class registry** (Tassadar) — an `executed_acceptance_suite` class
  slots beside replay / freivalds / deterministic-recompute.
- **Pylon network** — workers run the acceptance suite for revshare (the
  verified-work flywheel pays for QC).

## Immediate honest downgrade (stop the false green now)

The hot Worker route legitimately can't launch a browser. Until the out-of-worker
acceptance runner executes the artifact, the verdict for an executable artifact
must be **`unverified`**, NOT `test_passed`. Better to say "we didn't run it" than
to falsely certify. Remove `verified:true` / `scalar_reward:1` from the regex path;
the regex checks may remain only as a cheap *pre-screen* (a gate to even attempt
execution), never as the verification verdict.

## One line

A verifier that doesn't run the artifact isn't a verifier — it's a keyword filter.
Turn intent into executable acceptance tests, run them in our sandbox, and let
`verified` and the reward mean what they say.
