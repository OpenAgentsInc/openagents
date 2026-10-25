// The Grid's page: keyboard, mouse (drag or pointer lock), wheel, and touch
// sticks and buttons, handed to the engine each frame through grid_input
// (grid.cpp). The bindings are three-player-controller's: W A S D or the
// arrows move, Shift runs, Space jumps, V switches view. The hotbar is ours:
// 1 (or V) first person, 2 (or R) a new look.
'use strict';

(() => {
  const $ = (id) => document.getElementById(id);
  const cv = $('canvas');
  const codes = new Set();
  let G = null, dyaw = 0, dpitch = 0, drag = null, touchRun = false, touchJump = false;
  const sticks = { move: null, look: null };  // { id, x, y, dx, dy } while held

  Module.onGridReady = () => {
    const f = (name, args, ret = null) => Module.cwrap(name, ret, args);
    const n = 'number';
    G = {
      input: f('grid_input', [n, n, n, n, n, n, n, n]), toggleView: f('grid_toggle_view', []),
      zoom: f('grid_zoom', [n]), firstPerson: f('grid_first_person', [], n),
      cameraMode:f('grid_camera_mode',[],n),cameraX:f('grid_camera_x',[],n),cameraY:f('grid_camera_y',[],n),cameraZ:f('grid_camera_z',[],n),
      anim: f('grid_anim', [], 'string'), fps: f('grid_fps', [], n),
      x: f('grid_x', [], n), y: f('grid_y', [], n), z: f('grid_z', [], n), speed: f('grid_speed', [], n),
      place: f('grid_place', [n, n, n]), reroll: f('grid_reroll', []),
      vx: f('grid_vx', [], n), vy: f('grid_vy', [], n), vz: f('grid_vz', [], n),
      yaw: f('grid_yaw', [], n), watch: f('grid_watch', [n]), pause: f('grid_pause', [n]),
      face: f('grid_face', [n,n]), look: f('grid_look', [n]),
      peer: f('grid_peer', [n,n,n,n,n,n,n]), removePeer: f('grid_peer_remove', [n]),
      project: f('grid_project', [n,n,n], n), screenX: f('grid_screen_x', [], n), screenY: f('grid_screen_y', [], n),
      reduceMotion: f('grid_reduce_motion', [n]),
      machinesClear: f('grid_machines_clear', []), machine: f('grid_machine', [n,n,n]),
    };
    const count=f('grid_metadata_count',[n],n), text=f('grid_metadata_text',[n,n],'string'), value=f('grid_metadata_value',[n,n,n],n);
    G.metadata={
      sceneId:f('grid_metadata_scene',[],'string')(),
      landmarks:Array.from({length:count(0)},(_,i)=>({id:text(i,0),label:text(i,1),interactionLabel:text(i,2),position:[value(0,i,0),value(0,i,1),value(0,i,2)],labelHeight:value(0,i,3),interaction:value(0,i,6)?{enter:value(0,i,4),leave:value(0,i,5)}:null})),
      colliders:Array.from({length:count(1)},(_,i)=>({min:[value(1,i,0),value(1,i,1),value(1,i,2)],max:[value(1,i,3),value(1,i,4),value(1,i,5)]})),
    };
    window.G = G;
    GridUI.ready(G);
    $('loading').hidden = true;
    cv.focus();
    syncButtons();
    requestAnimationFrame(tick);
  };

  function syncButtons() { $('camBtn').classList.toggle('on', !!G.firstPerson()); }
  function view() { G.toggleView(); syncButtons(); }
  // Slots hand focus back to the canvas, so Space jumps instead of pressing them.
  $('camBtn').onclick = () => { if (G) view(); cv.focus(); };
  // New look repeats while held: one now, then a fresh look every 80 ms after
  // a short pause, from the slot (mouse or touch) or the R and 2 keys.
  let rerollTimer = 0;
  function startReroll() {
    if (!G || rerollTimer) return;
    GridUI.reroll();
    rerollTimer = setTimeout(function again() { GridUI.reroll(); rerollTimer = setTimeout(again, 80); }, 300);
  }
  function stopReroll() { clearTimeout(rerollTimer); rerollTimer = 0; }
  const lookBtn = $('lookBtn');
  lookBtn.addEventListener('pointerdown', (e) => { e.preventDefault(); lookBtn.setPointerCapture(e.pointerId); startReroll(); });
  for (const ev of ['pointerup', 'pointercancel', 'lostpointercapture']) lookBtn.addEventListener(ev, () => { stopReroll(); cv.focus(); });
  lookBtn.onclick = (e) => { if (e.detail === 0 && G) GridUI.reroll(); };  // Enter or Space on the focused slot

  const MOVE = ['KeyW', 'KeyA', 'KeyS', 'KeyD', 'ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight', 'Space'];
  addEventListener('keydown', (e) => {
    if(e.metaKey||e.ctrlKey||e.altKey)return;
    if (e.target.closest?.('button') && (e.code === 'Space' || e.code === 'Enter')) return;
    if (e.target.closest?.('input, textarea, select, .panel')) return;
    if (GridUI.key(e)) { e.preventDefault(); return; }
    codes.add(e.code);
    if (MOVE.includes(e.code)) e.preventDefault();
    if (!G || e.repeat) return;
    if (e.code === 'KeyV' || e.code === 'Digit1') view();
    if (e.code === 'KeyR' || e.code === 'Digit2') startReroll();
    hideHint();
  });
  addEventListener('keyup', (e) => {
    codes.delete(e.code);
    if (e.code === 'KeyR' || e.code === 'Digit2') stopReroll();
  });
  function release() {
    codes.clear(); stopReroll(); $('runTouch').classList.remove('on'); drag = null; touchRun = touchJump = false;
    for (const key of ['move','look']) { sticks[key] = null; $(key+'Stick').firstElementChild.style.transform = ''; }
    if (G) G.input(0,0,0,0,0,0,0,0);
  }
  addEventListener('blur', release);
  addEventListener('grid-release', release);
  document.addEventListener('visibilitychange', () => { if (document.hidden) release(); });

  // Mouse: drag to orbit, or click for pointer lock. Both turn faster than the
  // library's defaults, which felt sluggish: radians per pixel.
  const DRAG_LOOK = 0.006, LOCK_LOOK = 0.0025;
  cv.addEventListener('pointerdown', (e) => {
    if (e.pointerType === 'touch') return;
    GridUI.join();
    drag = { x: e.clientX, y: e.clientY, moved: 0 };
    cv.setPointerCapture(e.pointerId);
    hideHint();
  });
  cv.addEventListener('pointermove', (e) => {
    if (document.pointerLockElement === cv) { dyaw -= e.movementX * LOCK_LOOK; dpitch -= e.movementY * LOCK_LOOK; return; }
    if (!drag) return;
    const dx = e.clientX - drag.x, dy = e.clientY - drag.y, k = DRAG_LOOK;
    drag.moved += Math.abs(dx) + Math.abs(dy); drag.x = e.clientX; drag.y = e.clientY;
    dyaw -= dx * k; dpitch -= dy * k;
  });
  cv.addEventListener('pointercancel', () => { drag = null; });
  cv.addEventListener('lostpointercapture', () => { drag = null; });
  cv.addEventListener('pointerup', () => {
    const click = drag && drag.moved < 6; drag = null;
    if (click && cv.requestPointerLock) { try { const p = cv.requestPointerLock(); if (p && p.catch) p.catch(() => {}); } catch (err) { /* drag to look instead */ } }
  });
  cv.addEventListener('wheel', (e) => {
    if (!G) return;
    e.preventDefault();
    G.zoom(e.deltaY * (e.deltaMode === 1 ? 16 : e.deltaMode === 2 ? 100 : 1));
    syncButtons();
  }, { passive: false });

  // Touch: the left stick moves (analog), the right one looks; buttons jump,
  // and hold to run.
  addEventListener('touchstart', () => document.body.classList.add('touch'), { once: true, passive: true });
  for (const [name, el] of [['move', $('moveStick')], ['look', $('lookStick')]]) {
    el.addEventListener('pointerdown', (e) => {
      GridUI.join();
      const r = el.getBoundingClientRect();
      sticks[name] = { id: e.pointerId, x: r.left + r.width / 2, y: r.top + r.height / 2, dx: 0, dy: 0 };
      el.setPointerCapture(e.pointerId);
      moveStick(name, el, e);
      hideHint();
    });
    el.addEventListener('pointermove', (e) => moveStick(name, el, e));
    const end = (e) => { if (sticks[name] && sticks[name].id === e.pointerId) { sticks[name] = null; el.firstElementChild.style.transform = ''; } };
    el.addEventListener('pointerup', end);
    el.addEventListener('pointercancel', end);
    el.addEventListener('lostpointercapture', end);
  }
  function moveStick(name, el, e) {
    const s = sticks[name]; if (!s || s.id !== e.pointerId) return;
    let dx = e.clientX - s.x, dy = e.clientY - s.y;
    const d = Math.hypot(dx, dy), max = 46;
    if (d > max) { dx *= max / d; dy *= max / d; }
    s.dx = dx / max; s.dy = dy / max;
    el.firstElementChild.style.transform = `translate(${dx}px,${dy}px)`;
  }
  const hold = (el, set) => {
    el.addEventListener('pointerdown', (e) => { e.preventDefault(); set(true); });
    for (const ev of ['pointerup', 'pointercancel', 'pointerleave']) el.addEventListener(ev, () => set(false));
  };
  hold($('jumpTouch'), (v) => { touchJump = v; });
  hold($('runTouch'), (v) => { touchRun = v; $('runTouch').classList.toggle('on', v); });
  // Look stick: nothing inside a small dead zone, then a turn rate that rises
  // with the square of the push (1.9 rad/s across, 1.15 up and down).
  const curve = (v) => { const a = Math.abs(v); return a < 0.12 ? 0 : Math.sign(v) * ((a - 0.12) / 0.88) ** 2; };

  let hintHidden = false;
  function hideHint() { if (!hintHidden) { hintHidden = true; setTimeout(() => ($('hint').style.opacity = 0), 4000); } }

  let last = performance.now(), hudAt = 0;
  const any = (...c) => c.some((k) => codes.has(k));
  function tick(now) {
    requestAnimationFrame(tick);
    const dt = Math.min(0.1, (now - last) / 1000); last = now;
    let kx = (any('KeyD', 'ArrowRight') ? 1 : 0) - (any('KeyA', 'ArrowLeft') ? 1 : 0);
    let ky = (any('KeyW', 'ArrowUp') ? 1 : 0) - (any('KeyS', 'ArrowDown') ? 1 : 0);
    const px = sticks.move ? sticks.move.dx : 0, py = sticks.move ? -sticks.move.dy : 0;
    if (sticks.look) { dyaw -= curve(sticks.look.dx) * 1.9 * dt; dpitch -= curve(sticks.look.dy) * 1.15 * dt; }
    const shift = any('ShiftLeft', 'ShiftRight') || touchRun;
    const control = GridUI.tick(now, dt, kx, ky, px, py);
    if (control) { kx=control.x; ky=control.y; }
    G.input(kx, ky, px, py, shift ? 1 : 0, codes.has('Space') || touchJump ? 1 : 0, dyaw, dpitch);
    dyaw = dpitch = 0;
    if (now - hudAt > 250) {
      hudAt = now;
      $('hud').textContent = `${G.x().toFixed(1)}, ${G.z().toFixed(1)}${G.y() > 0.05 ? ' ↑' + G.y().toFixed(1) : ''} · ${G.speed().toFixed(1)} m/s · ${G.anim()} · ${Math.round(G.fps())} fps`;
    }
  }
})();
