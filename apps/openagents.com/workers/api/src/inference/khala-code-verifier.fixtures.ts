export const GOOD_CROSSY_ROAD_HTML = `<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <title>OpenAgents Khala Crossy Road</title>
  <style>
    body { margin: 0; background: #1d2830; color: white; font-family: system-ui; }
    #game { display: block; width: 100vw; height: 100vh; background: #6bbf59; }
  </style>
</head>
<body>
  <canvas id="game" width="960" height="540"></canvas>
  <button id="restart" type="button">Restart</button>
  <script>
    const canvas = document.getElementById('game');
    const context = canvas.getContext('2d');
    const startPosition = { x: 0, y: 0, z: 0 };
    const player = { x: startPosition.x, y: startPosition.y, z: startPosition.z, direction: 'forward' };
    const camera = {
      mode: 'third_person_follow',
      offset: { x: 0, y: 8, z: 12 },
      position: { x: 0, y: 8, z: 12 },
      lookAt(target) { this.target = { x: target.x, y: target.y, z: target.z }; }
    };
    let progress = 0;
    let score = 0;
    let difficulty = 1;
    let trafficSpeed = 1;

    function updateCamera() {
      camera.position = {
        x: player.x + camera.offset.x,
        y: player.y + camera.offset.y,
        z: player.z + camera.offset.z
      };
      camera.lookAt(player);
    }

    function rampDifficulty() {
      difficulty = Math.min(8, 1 + progress * 0.2);
      trafficSpeed = difficulty;
    }

    function move(direction) {
      player.direction = direction;
      if (direction === 'forward') {
        player.z += 1;
        progress += 1;
        score += 10;
      }
      if (direction === 'backward') player.z -= 1;
      if (direction === 'left') player.x -= 1;
      if (direction === 'right') player.x += 1;
      rampDifficulty();
      updateCamera();
    }

    function restartGame() {
      player.x = 0;
      player.y = 0;
      player.z = 0;
      player.direction = 'forward';
      progress = 0;
      score = 0;
      difficulty = 1;
      trafficSpeed = 1;
      updateCamera();
    }

    document.getElementById('restart').addEventListener('click', restartGame);
    window.__openagentsCrossyRoadState = () => ({
      camera: {
        mode: camera.mode,
        position: { ...camera.position },
        target: camera.target ? { ...camera.target } : null
      },
      difficulty,
      player: { ...player },
      progress,
      score,
      trafficSpeed
    });
    window.__openagentsCrossyRoadRestart = restartGame;
    window.addEventListener('keydown', event => {
      if (event.key === 'ArrowUp' || event.key === 'w') move('forward');
      if (event.key === 'ArrowDown' || event.key === 's') move('backward');
      if (event.key === 'ArrowLeft' || event.key === 'a') move('left');
      if (event.key === 'ArrowRight' || event.key === 'd') move('right');
    });

    function loop() {
      context.clearRect(0, 0, canvas.width, canvas.height);
      context.fillStyle = '#f5f2df';
      context.fillRect(470 + player.x * 24, 260 - player.z * 8, 24, 24);
      requestAnimationFrame(loop);
    }

    updateCamera();
    requestAnimationFrame(loop);
  </script>
</body>
</html>`

export const BROKEN_CONTROLS_CROSSY_ROAD_HTML = GOOD_CROSSY_ROAD_HTML.replace(
  "if (event.key === 'ArrowLeft' || event.key === 'a') move('left');\n      if (event.key === 'ArrowRight' || event.key === 'd') move('right');",
  "if (event.key === 'a') move('left');",
)

export const BROKEN_RESTART_CROSSY_ROAD_HTML = GOOD_CROSSY_ROAD_HTML.replace(
  'player.x = 0;\n      player.y = 0;\n      player.z = 0;',
  "player.direction = 'forward';",
).replace('progress = 0;\n      score = 0;', 'progress += 1;')

export const BROKEN_EXTERNAL_ASSET_CROSSY_ROAD_HTML = `<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <title>Not a single file</title>
  <script src="https://cdn.example.test/game.js"></script>
</head>
<body>
  <canvas id="game"></canvas>
</body>
</html>`

export const BROKEN_DIFFICULTY_CROSSY_ROAD_HTML = GOOD_CROSSY_ROAD_HTML.replace(
  /function rampDifficulty\(\) \{[\s\S]*?\n    \}/u,
  'function rampDifficulty() { trafficSpeed = 1; }',
)
  .replace('progress += 1;', 'progress = progress;')
  .replace('score += 10;', 'score = score;')
