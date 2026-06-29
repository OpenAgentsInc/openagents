// GENERATED — do not edit by hand.
//
// Source: scripts/khala-demo/artifacts/khala-crossy-road-northstar-passing.v1.html
// sha256: 885b85af3e459e9755a301310b86b10eda71cb4b6bc796a77c643116441f0b44
// Regenerate: bun apps/autopilot-desktop/scripts/generate-khala-crossy-road-game.ts
//
// The committed, 6/6-verified Khala-built crossy-road game, with the CDN
// three.js <script src> removed. The in-Verse game screen runs this inside a
// same-origin srcdoc iframe with THREE injected from the parent's bundled three.

export const KHALA_CROSSY_ROAD_ARTIFACT_SHA256 = "885b85af3e459e9755a301310b86b10eda71cb4b6bc796a77c643116441f0b44" as const

export const KHALA_CROSSY_ROAD_GAME_HTML = `<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>OpenAgents Crossy Road</title>
<style>
  html, body { margin: 0; padding: 0; overflow: hidden; background: #87CEEB; font-family: sans-serif; user-select: none; }
  #game-container { position: absolute; inset: 0; }
  #score { position: absolute; top: 20px; left: 20px; color: #fff; font-size: 28px; font-weight: bold; text-shadow: 1px 1px 3px rgba(0,0,0,0.5); pointer-events: none; z-index: 10; }
  #ui { position: absolute; inset: 0; pointer-events: none; display: flex; flex-direction: column; justify-content: center; align-items: center; z-index: 20; }
  #overlay, #game-over { pointer-events: auto; background: rgba(0,0,0,0.65); padding: 40px 60px; border-radius: 24px; text-align: center; color: #fff; box-shadow: 0 10px 30px rgba(0,0,0,0.3); }
  #overlay h1, #game-over h1 { margin: 0 0 20px; font-size: 52px; letter-spacing: 2px; }
  #overlay p, #game-over p { font-size: 20px; margin: 10px 0; }
  #start-btn, #restart-btn { font-size: 28px; padding: 16px 48px; border: none; border-radius: 14px; background: #FFD700; color: #222; cursor: pointer; font-weight: bold; transition: transform 0.1s, background 0.2s; }
  #start-btn:hover, #restart-btn:hover { background: #FFC107; transform: scale(1.05); }
  #game-over { display: none; }
</style>
</head>
<body>
<div id="game-container"></div>
<div id="score">0</div>
<div id="ui">
  <div id="overlay">
    <h1>CROSSY ROAD</h1>
    <button id="start-btn">PLAY</button>
  </div>
  <div id="game-over">
    <h1>SPLAT!</h1>
    <p>Score: <span id="final-score">0</span></p>
    <button id="restart-btn">TRY AGAIN</button>
  </div>
</div>
<script>
(function(){
  const container = document.getElementById('game-container');
  const overlay = document.getElementById('overlay');
  const gameOver = document.getElementById('game-over');
  const startBtn = document.getElementById('start-btn');
  const restartBtn = document.getElementById('restart-btn');
  const scoreEl = document.getElementById('score');
  const finalScoreEl = document.getElementById('final-score');

  let scene, camera, renderer;
  let player = { position: {x:0,y:0,z:0}, rotation: {x:0,y:0,z:0} };
  camera = { position: {x:0,y:0,z:0} };

  let rows = [];
  let maxGeneratedZ = 0;
  let started = false;
  let isDead = false;
  let loopTicks = 0;
  let progress = 0;
  let worldRowsAhead = 0;

  const TILE_SIZE = 1;
  const VIEW_AHEAD = 24;
  const VIEW_BEHIND = 8;
  const SAFE_ZONE = 15;
  const HOP_SPEED = 8;

  let hop = { active: false, t: 0, from: new THREE.Vector3(), to: new THREE.Vector3(), dir: new THREE.Vector3() };
  let pendingHop = null;

  function init() {
    if (typeof THREE === 'undefined') return;

    scene = new THREE.Scene();
    scene.background = new THREE.Color(0x87CEEB);
    scene.fog = new THREE.Fog(0x87CEEB, 25, 55);

    const width = window.innerWidth || 1;
    const height = window.innerHeight || 1;

    camera = new THREE.PerspectiveCamera(60, width / height, 0.1, 100);
    camera.position.set(8, 12, 8);

    try {
      renderer = new THREE.WebGLRenderer({ antialias: true, alpha: false , preserveDrawingBuffer: true });
      renderer.setSize(width, height);
      renderer.shadowMap.enabled = true;
      renderer.shadowMap.type = THREE.PCFSoftShadowMap;
      container.appendChild(renderer.domElement);
    } catch (e) {
      renderer = { setSize: function(){}, render: function(){} };
    }

    const hemi = new THREE.HemisphereLight(0xffffff, 0x444444, 0.85);
    scene.add(hemi);
    const dir = new THREE.DirectionalLight(0xffffff, 0.85);
    dir.position.set(20, 45, 15);
    dir.castShadow = true;
    dir.shadow.mapSize.width = 1024;
    dir.shadow.mapSize.height = 1024;
    dir.shadow.camera.left = -25;
    dir.shadow.camera.right = 25;
    dir.shadow.camera.top = 25;
    dir.shadow.camera.bottom = -25;
    scene.add(dir);

    createPlayer();
    generateInitialWorld();

    window.addEventListener('resize', onResize);
    window.addEventListener('keydown', onKeyDown);

    startBtn.addEventListener('click', function(){ window.__openagentsCrossyRoadStart(); });
    restartBtn.addEventListener('click', function(){ window.__openagentsCrossyRoadRestart(); });

    setInterval(update, 16);
    if (typeof requestAnimationFrame === 'function') {
      requestAnimationFrame(renderLoop);
    }
  }

  function createPlayer() {
    const group = new THREE.Group();
    const white = new THREE.MeshLambertMaterial({ color: 0xffffff });
    const body = new THREE.Mesh(new THREE.BoxGeometry(0.6, 0.6, 0.75), white);
    body.position.y = 0.5;
    body.castShadow = true;
    group.add(body);

    const head = new THREE.Mesh(new THREE.BoxGeometry(0.38, 0.38, 0.42), white);
    head.position.set(0, 1.0, 0.28);
    head.castShadow = true;
    group.add(head);

    const beak = new THREE.Mesh(new THREE.BoxGeometry(0.16, 0.1, 0.22), new THREE.MeshLambertMaterial({ color: 0xffa500 }));
    beak.position.set(0, 0.95, 0.58);
    group.add(beak);

    const eyeMat = new THREE.MeshLambertMaterial({ color: 0x111111 });
    const eyeL = new THREE.Mesh(new THREE.BoxGeometry(0.08, 0.08, 0.05), eyeMat);
    eyeL.position.set(-0.13, 1.05, 0.5);
    const eyeR = new THREE.Mesh(new THREE.BoxGeometry(0.08, 0.08, 0.05), eyeMat);
    eyeR.position.set(0.13, 1.05, 0.5);
    group.add(eyeL);
    group.add(eyeR);

    group.position.set(0, 0, 0);
    scene.add(group);
    player = group;
  }

  function createGroundTile(x, z, color) {
    const geo = new THREE.BoxGeometry(TILE_SIZE, 0.2, TILE_SIZE);
    const mat = new THREE.MeshLambertMaterial({ color: color });
    const mesh = new THREE.Mesh(geo, mat);
    mesh.position.set(x, -0.1, z);
    mesh.receiveShadow = true;
    scene.add(mesh);
    return mesh;
  }

  function createCar() {
    const group = new THREE.Group();
    const colors = [0xe53935, 0xfb8c00, 0xfdd835, 0x43a047, 0x1e88e5, 0x8e24aa];
    const color = colors[Math.floor(Math.random() * colors.length)];
    const body = new THREE.Mesh(new THREE.BoxGeometry(0.85, 0.42, 1.45), new THREE.MeshLambertMaterial({ color: color }));
    body.position.y = 0.22;
    body.castShadow = true;
    group.add(body);

    const wheelGeo = new THREE.BoxGeometry(0.16, 0.16, 0.16);
    const wheelMat = new THREE.MeshLambertMaterial({ color: 0x222222 });
    const positions = [[-0.34,0.06,0.48],[0.34,0.06,0.48],[-0.34,0.06,-0.48],[0.34,0.06,-0.48]];
    positions.forEach(function(p){
      const w = new THREE.Mesh(wheelGeo, wheelMat);
      w.position.set(p[0], p[1], p[2]);
      group.add(w);
    });
    group.userData = { type: 'car' };
    return group;
  }

  function createLog() {
    const geo = new THREE.BoxGeometry(2.3, 0.22, 0.72);
    const mat = new THREE.MeshLambertMaterial({ color: 0x8d6e63 });
    const log = new THREE.Mesh(geo, mat);
    log.castShadow = true;
    log.receiveShadow = true;
    log.userData = { type: 'log' };
    return log;
  }

  function createTree() {
    const group = new THREE.Group();
    const trunk = new THREE.Mesh(new THREE.BoxGeometry(0.32, 0.65, 0.32), new THREE.MeshLambertMaterial({ color: 0x5d4037 }));
    trunk.position.y = 0.32;
    trunk.castShadow = true;
    group.add(trunk);
    const leaves = new THREE.Mesh(new THREE.BoxGeometry(0.85, 0.85, 0.85), new THREE.MeshLambertMaterial({ color: 0x2e7d32 }));
    leaves.position.y = 0.9;
    leaves.castShadow = true;
    group.add(leaves);
    group.userData = { type: 'tree' };
    return group;
  }

  function generateRow(z) {
    let type = 'grass';
    if (z > SAFE_ZONE) {
      const r = Math.random();
      if (r < 0.32) type = 'road';
      else if (r < 0.58) type = 'water';
    }
    const color = type === 'grass' ? 0x4caf50 : type === 'road' ? 0x555555 : 0x2196f3;
    const row = { z: z, type: type, tiles: [], obstacles: [] };

    for (let x = -6; x <= 6; x++) {
      row.tiles.push(createGroundTile(x, z, color));
    }

    if (type === 'road') {
      const speedDir = Math.random() < 0.5 ? -1 : 1;
      const speed = speedDir * (1.4 + Math.random() * 1.6);
      const count = 1 + Math.floor(Math.random() * 2);
      for (let i = 0; i < count; i++) {
        const car = createCar();
        car.position.set(Math.random() * 12 - 6, 0.35, z);
        car.userData.speed = speed;
        row.obstacles.push(car);
        scene.add(car);
      }
    } else if (type === 'water') {
      const speedDir = Math.random() < 0.5 ? -1 : 1;
      const speed = speedDir * (0.7 + Math.random() * 0.7);
      const count = 1 + Math.floor(Math.random() * 2);
      for (let i = 0; i < count; i++) {
        const log = createLog();
        log.position.set(Math.random() * 12 - 6, 0.05, z);
        log.userData.speed = speed;
        row.obstacles.push(log);
        scene.add(log);
      }
    } else if (type === 'grass' && z !== 0 && z > SAFE_ZONE && Math.random() < 0.28) {
      const tree = createTree();
      tree.position.set(Math.floor(Math.random() * 9 - 4), 0, z);
      row.obstacles.push(tree);
      scene.add(tree);
    }

    rows.push(row);
    if (z > maxGeneratedZ) maxGeneratedZ = z;
  }

  function generateInitialWorld() {
    rows.forEach(function(r){
      r.tiles.forEach(function(t){ scene.remove(t); });
      r.obstacles.forEach(function(o){ scene.remove(o); });
    });
    rows = [];
    maxGeneratedZ = 0;
    for (let z = -VIEW_BEHIND; z <= VIEW_AHEAD; z++) {
      generateRow(z);
    }
  }

  function ensureRowsAhead() {
    const playerZ = Math.round(player.position.z);
    while (maxGeneratedZ - playerZ < VIEW_AHEAD) {
      generateRow(maxGeneratedZ + 1);
    }
    rows = rows.filter(function(r){
      if (r.z < playerZ - VIEW_BEHIND) {
        r.tiles.forEach(function(t){ scene.remove(t); });
        r.obstacles.forEach(function(o){ scene.remove(o); });
        return false;
      }
      return true;
    });
  }

  function onKeyDown(e) {
    if (!started || isDead) return;
    let dx = 0, dz = 0;
    switch (e.key) {
      case 'ArrowUp': case 'w': case 'W': dz = 1; break;
      case 'ArrowDown': case 's': case 'S': dz = -1; break;
      case 'ArrowLeft': case 'a': case 'A': dx = -1; break;
      case 'ArrowRight': case 'd': case 'D': dx = 1; break;
      default: return;
    }
    e.preventDefault();
    if (hop.active) {
      pendingHop = { dx: dx, dz: dz };
      return;
    }
    startHop(dx, dz);
  }

  function startHop(dx, dz) {
    hop.active = true;
    hop.t = 0;
    hop.from.copy(player.position);
    hop.to.set(player.position.x + dx, player.position.y, player.position.z + dz);
    hop.dir.set(dx, 0, dz);
  }

  function finishHop() {
    player.position.copy(hop.to);
    player.position.y = 0;
    hop.active = false;
    player.rotation.set(0, 0, 0);
    checkRow();
    ensureRowsAhead();
    progress = Math.round(player.position.z);
    scoreEl.textContent = progress;

    if (pendingHop && !isDead) {
      const p = pendingHop;
      pendingHop = null;
      startHop(p.dx, p.dz);
    }
  }

  function updateHop(dt) {
    if (!hop.active) return;
    hop.t += dt * HOP_SPEED;
    if (hop.t >= 1) {
      hop.t = 1;
      finishHop();
      return;
    }
    player.position.lerpVectors(hop.from, hop.to, hop.t);
    player.position.y = Math.sin(hop.t * Math.PI) * 0.42;
    player.rotation.z = -hop.dir.x * Math.sin(hop.t * Math.PI) * 0.25;
    player.rotation.x = hop.dir.z * Math.sin(hop.t * Math.PI) * 0.25;
  }

  function checkRow() {
    const z = Math.round(player.position.z);
    const row = rows.find(function(r){ return r.z === z; });
    if (!row) return;
    if (row.type === 'water') {
      let onLog = false;
      for (let i = 0; i < row.obstacles.length; i++) {
        const ob = row.obstacles[i];
        if (ob.userData.type === 'log' && Math.abs(player.position.x - ob.position.x) < 1.15) {
          onLog = true;
          break;
        }
      }
      if (!onLog) die();
    }
  }

  function updateObstacles(dt) {
    rows.forEach(function(row){
      row.obstacles.forEach(function(ob){
        if (ob.userData.speed) {
          ob.position.x += ob.userData.speed * dt;
          if (ob.position.x > 8.5) ob.position.x = -8.5;
          if (ob.position.x < -8.5) ob.position.x = 8.5;
        }
      });
    });

    const z = Math.round(player.position.z);
    const row = rows.find(function(r){ return r.z === z; });
    if (row && row.type === 'water' && !hop.active) {
      let onLog = false;
      for (let i = 0; i < row.obstacles.length; i++) {
        const ob = row.obstacles[i];
        if (ob.userData.type === 'log' && Math.abs(player.position.x - ob.position.x) < 1.15) {
          player.position.x += ob.userData.speed * dt;
          onLog = true;
          break;
        }
      }
      if (!onLog) die();
    }
  }

  function checkCollisions() {
    if (hop.active) return;
    const z = Math.round(player.position.z);
    const row = rows.find(function(r){ return r.z === z; });
    if (!row) return;
    for (let i = 0; i < row.obstacles.length; i++) {
      const ob = row.obstacles[i];
      if (ob.userData.type === 'car') {
        const dx = player.position.x - ob.position.x;
        const dz = player.position.z - ob.position.z;
        if (Math.abs(dx) < 0.7 && Math.abs(dz) < 0.8) die();
      } else if (ob.userData.type === 'tree') {
        const dx = player.position.x - ob.position.x;
        const dz = player.position.z - ob.position.z;
        if (Math.abs(dx) < 0.55 && Math.abs(dz) < 0.55) {
          player.position.copy(hop.from);
        }
      }
    }
  }

  function die() {
    if (isDead) return;
    isDead = true;
    pendingHop = null;
    finalScoreEl.textContent = progress;
    gameOver.style.display = 'block';
    overlay.style.display = 'none';
  }

  function updateCamera() {
    const targetX = player.position.x + 8;
    const targetY = player.position.y + 12;
    const targetZ = player.position.z + 8;
    camera.position.x += (targetX - camera.position.x) * 0.12;
    camera.position.y += (targetY - camera.position.y) * 0.12;
    camera.position.z += (targetZ - camera.position.z) * 0.12;
    camera.lookAt(player.position.x, player.position.y, player.position.z);
  }

  function onResize() {
    const width = window.innerWidth || 1;
    const height = window.innerHeight || 1;
    camera.aspect = width / height;
    camera.updateProjectionMatrix();
    renderer.setSize(width, height);
  }

  function update() {
    if (!started) return;
    loopTicks++;
    const dt = 0.016;
    updateObstacles(dt);
    updateHop(dt);
    checkCollisions();
    updateCamera();
    worldRowsAhead = maxGeneratedZ - Math.round(player.position.z);
  }

  function renderLoop() {
    if (typeof requestAnimationFrame === 'function') requestAnimationFrame(renderLoop);
    if (renderer && scene && camera) {
      try { renderer.render(scene, camera); } catch (e) {}
    }
  }

  window.__openagentsCrossyRoadState = function(){
    return {
      player: { x: player.position.x, y: player.position.y, z: player.position.z },
      camera: { position: { x: camera.position.x, y: camera.position.y, z: camera.position.z } },
      progress: progress,
      worldRowsAhead: worldRowsAhead,
      started: started,
      loopTicks: loopTicks
    };
  };

  window.__openagentsCrossyRoadStart = function(){
    overlay.style.display = 'none';
    gameOver.style.display = 'none';
    started = true;
    isDead = false;
  };

  window.__openagentsCrossyRoadRestart = function(){
    player.position.set(0, 0, 0);
    player.rotation.set(0, 0, 0);
    progress = 0;
    isDead = false;
    hop.active = false;
    pendingHop = null;
    scoreEl.textContent = '0';
    generateInitialWorld();
    overlay.style.display = 'none';
    gameOver.style.display = 'none';
    started = true;
  };

  function boot() {
    if (typeof THREE === 'undefined') {
      setTimeout(boot, 50);
      return;
    }
    init();
  }

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', boot);
  } else {
    boot();
  }
})();
</script>
</body>
</html>
` as const
