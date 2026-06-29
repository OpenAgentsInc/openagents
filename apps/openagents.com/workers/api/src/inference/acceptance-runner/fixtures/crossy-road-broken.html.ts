// BROKEN crossy-road artifact fixture — the 4-bug version (EPIC #6017).
//
// This reproduces the EXACT four defects the regex verifier missed and certified as
// verified:true (docs/inference/2026-06-22-verified-work-must-execute-the-artifact.md):
//   1. CRASH ON LOAD — the constructor calls updateScoreUI() BEFORE assigning the DOM
//      refs -> "Cannot read/set properties of undefined". A page error fires and the
//      start-btn click listener is never attached.
//   2. DEAD PLAY BUTTON — the game-over/start overlay uses opacity:0 with no
//      display:none, stays full-screen at z-index:20, and .btn{pointer-events:auto}
//      overrides .hidden{pointer-events:none}, so it intercepts the PLAY click.
//   3. 100x CAMERA — updateCamera multiplies player.z (already world units) by
//      TILE_SIZE again, so the camera flies ~32x per hop.
//   4. WORLD STOPS GENERATING — only ~10 rows are ever seeded; none are appended as
//      the player advances, so the world runs out (blue sky).
//
// The runner must FAIL these per-test. To make the camera/world bugs OBSERVABLE even
// though the constructor crashes, the state reader is bound to the half-built instance
// the crashing constructor leaves behind (the crash happens AFTER position/camera are
// set up but BEFORE the start listener is wired) — exactly the partial-init a real
// crash-on-load leaves. The reader is also wrapped so it never throws into the runner.

export const CROSSY_ROAD_BROKEN_HTML = `<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <title>Khala Crossy Road (broken)</title>
  <style>
    html, body { margin: 0; height: 100%; background: #1d2830; color: #fff; font-family: system-ui; }
    #game { display: block; width: 100vw; height: 100vh; background: #6bbf59; }
    /* BUG #2: overlay hides via opacity only and keeps pointer-events. */
    #overlay {
      position: fixed; inset: 0; z-index: 20;
      display: flex; align-items: center; justify-content: center;
      background: rgba(0,0,0,0.6);
    }
    #overlay.hidden { opacity: 0; pointer-events: none; }
    .btn { font-size: 20px; padding: 12px 28px; cursor: pointer; pointer-events: auto; }
  </style>
</head>
<body>
  <canvas id="game" width="1280" height="720"></canvas>
  <div id="overlay">
    <button id="start-btn" class="btn" type="button">PLAY</button>
  </div>
  <button id="restart" class="btn" type="button" style="position:fixed;right:12px;bottom:12px;">Restart</button>
  <script>
    'use strict';
    const TILE_SIZE = 32;
    var brokenGame = null;

    class Game {
      constructor() {
        this.startPosition = { x: 0, y: 0, z: 0 };
        this.player = { x: 0, y: 0, z: 0 };
        this.progress = 0;
        this.score = 0;
        this.camera = { offset: { x: 0, y: 8, z: 12 }, position: { x: 0, y: 8, z: 12 } };
        // BUG #4: seed only ~10 rows; never append more as the player advances.
        this.generatedRows = 10;
        this.started = false;
        this.loopTicks = 0;
        // expose for the runner BEFORE the crash so the camera/world bugs are observable.
        brokenGame = this;
        this.updateCamera();

        // BUG #1: call a UI update that touches DOM refs that were never assigned.
        // this.context / this.startBtn are undefined -> throws. The start listener
        // below is never reached, so PLAY is dead even ignoring the overlay bug.
        this.updateScoreUI();

        // UNREACHABLE because of BUG #1:
        this.startBtn = document.getElementById('start-btn');
        this.startBtn.addEventListener('click', () => this.start());
        window.addEventListener('keydown', (event) => this.onKey(event));
      }

      updateScoreUI() {
        // this.context was never assigned -> TypeError (the crash on load).
        this.context.fillText('Score: ' + this.score, 10, 10);
      }

      updateCamera() {
        // BUG #3: player.z is ALREADY world units; multiplying by TILE_SIZE again
        // makes the camera fly ~32x per hop.
        this.camera.position = {
          x: this.player.x + this.camera.offset.x,
          y: this.player.y + this.camera.offset.y,
          z: this.player.z * TILE_SIZE + this.camera.offset.z
        };
      }

      move(direction) {
        if (direction === 'forward') { this.player.z += 1; this.progress += 1; this.score += 10; }
        if (direction === 'backward') this.player.z -= 1;
        if (direction === 'left') this.player.x -= 1;
        if (direction === 'right') this.player.x += 1;
        // BUG #4: no row generation here -> generatedRows stays 10.
        this.updateCamera();
      }

      onKey(event) {
        if (event.key === 'ArrowUp' || event.key === 'w') this.move('forward');
        if (event.key === 'ArrowDown' || event.key === 's') this.move('backward');
        if (event.key === 'ArrowLeft' || event.key === 'a') this.move('left');
        if (event.key === 'ArrowRight' || event.key === 'd') this.move('right');
      }

      start() { this.started = true; }
      restart() {
        this.player = { x: 0, y: 0, z: 0 };
        this.progress = 0;
        this.updateCamera();
      }

      state() {
        return {
          camera: { position: { ...this.camera.position } },
          loopTicks: this.loopTicks,
          player: { ...this.player },
          progress: this.progress,
          score: this.score,
          started: this.started,
          worldRowsAhead: this.generatedRows - this.player.z
        };
      }
    }

    // State reader is resilient: even though construction throws, brokenGame was
    // assigned before the crash, so the runner can still MEASURE the camera/world bugs.
    window.__openagentsCrossyRoadState = () => {
      try { return brokenGame ? brokenGame.state() : undefined; } catch (e) { return undefined; }
    };
    // Restart hook works on the half-built instance (so the restart check is fair).
    window.__openagentsCrossyRoadRestart = () => { if (brokenGame) brokenGame.restart(); };
    // NOTE: no working __openagentsCrossyRoadStart — BUG #1 stops the constructor
    // before any start wiring, so the game can never start.

    new Game();
  </script>
</body>
</html>`
