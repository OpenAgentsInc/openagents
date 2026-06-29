// FIXED crossy-road artifact fixture (EPIC #6017).
//
// A self-contained single-file crossy-road game that PASSES every executed acceptance
// check. It exposes the runner state contract (see runner.ts header):
//   window.__openagentsCrossyRoadState(), __openagentsCrossyRoadStart(),
//   __openagentsCrossyRoadRestart().
// It is the paired counterpart to crossy-road-broken.html.ts (the 4-bug version): the
// runner must PASS this and FAIL the broken one. Kept as a TS string export so the
// test imports it without a bundler/asset step.
//
// Behavior, fixed (vs. the four documented defects):
//   1. Loads cleanly — DOM refs assigned BEFORE first UI update; no crash.
//   2. PLAY starts — the start overlay is display:none after start (no click intercept).
//   3. Camera follows by ~1 unit per hop — z is NOT multiplied by TILE_SIZE twice.
//   4. World keeps generating — rows are appended ahead as the player advances.

export const CROSSY_ROAD_FIXED_HTML = `<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <title>Khala Crossy Road (fixed)</title>
  <style>
    html, body { margin: 0; height: 100%; background: #1d2830; color: #fff; font-family: system-ui; }
    #game { display: block; width: 100vw; height: 100vh; background: #6bbf59; }
    #overlay {
      position: fixed; inset: 0; z-index: 20;
      display: flex; align-items: center; justify-content: center;
      background: rgba(0,0,0,0.6);
    }
    #overlay.hidden { display: none; }
    .btn { font-size: 20px; padding: 12px 28px; cursor: pointer; }
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

    class Game {
      constructor() {
        // FIX #1: assign DOM refs BEFORE any UI update.
        this.canvas = document.getElementById('game');
        this.context = this.canvas.getContext('2d');
        this.overlay = document.getElementById('overlay');
        this.startBtn = document.getElementById('start-btn');
        this.restartBtn = document.getElementById('restart');

        this.startPosition = { x: 0, y: 0, z: 0 };
        this.reset();
        this.started = false;
        this.loopTicks = 0;
        this.rafId = null;

        this.startBtn.addEventListener('click', () => this.start());
        this.restartBtn.addEventListener('click', () => this.restart());
        window.addEventListener('keydown', (event) => this.onKey(event));

        this.updateUI();
      }

      reset() {
        this.player = { x: 0, y: 0, z: 0 };
        this.progress = 0;
        this.score = 0;
        this.difficulty = 1;
        this.camera = { offset: { x: 0, y: 8, z: 12 }, position: { x: 0, y: 8, z: 12 } };
        // World rows generated AHEAD of the player. Seed enough to start.
        this.generatedRows = 16;
        this.updateCamera();
      }

      updateUI() {
        // Safe no-op draw target; runs only after refs exist.
        if (this.context) {
          this.context.clearRect(0, 0, this.canvas.width, this.canvas.height);
        }
      }

      updateCamera() {
        // FIX #3: follow by world units directly — do NOT multiply by TILE_SIZE.
        this.camera.position = {
          x: this.player.x + this.camera.offset.x,
          y: this.player.y + this.camera.offset.y,
          z: this.player.z + this.camera.offset.z
        };
      }

      ensureWorldAhead() {
        // FIX #4: keep generating rows ahead of the player as they advance.
        const needed = this.player.z + 16;
        if (this.generatedRows < needed) {
          this.generatedRows = needed;
        }
      }

      move(direction) {
        if (!this.started) return;
        if (direction === 'forward') { this.player.z += 1; this.progress += 1; this.score += 10; }
        if (direction === 'backward') this.player.z = Math.max(0, this.player.z - 1);
        if (direction === 'left') this.player.x -= 1;
        if (direction === 'right') this.player.x += 1;
        this.difficulty = Math.min(8, 1 + this.progress * 0.2);
        this.ensureWorldAhead();
        this.updateCamera();
      }

      onKey(event) {
        if (event.key === 'ArrowUp' || event.key === 'w') this.move('forward');
        if (event.key === 'ArrowDown' || event.key === 's') this.move('backward');
        if (event.key === 'ArrowLeft' || event.key === 'a') this.move('left');
        if (event.key === 'ArrowRight' || event.key === 'd') this.move('right');
      }

      start() {
        if (this.started) return;
        this.started = true;
        // FIX #2: fully hide the overlay (display:none) so it can't intercept input.
        this.overlay.classList.add('hidden');
        this.loop();
      }

      restart() {
        this.reset();
        this.started = true;
        this.overlay.classList.add('hidden');
      }

      loop() {
        this.loopTicks += 1;
        this.updateUI();
        if (this.started) {
          this.rafId = requestAnimationFrame(() => this.loop());
        }
      }

      state() {
        return {
          camera: { position: { ...this.camera.position } },
          difficulty: this.difficulty,
          loopTicks: this.loopTicks,
          player: { ...this.player },
          progress: this.progress,
          score: this.score,
          started: this.started,
          worldRowsAhead: this.generatedRows - this.player.z
        };
      }
    }

    const game = new Game();
    window.__openagentsCrossyRoadState = () => game.state();
    window.__openagentsCrossyRoadStart = () => game.start();
    window.__openagentsCrossyRoadRestart = () => game.restart();
  </script>
</body>
</html>`
