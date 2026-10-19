// /lander3 (#12180): the Grid from overhead, drawn with three.js. The server
// builds the scene from the verse crate (src/pages/lander3.rs) and names its
// address in #lander3's data-scene; this script holds no Grid data. Every
// surface is unlit, its texture times its tint, as the Grid's engine draws
// it. The robots walk their patrols from the sampled rounds and play the
// skin matrices the engine sampled for each clip.
import * as THREE from "/lander3/three/three.module.min.js";

const root = document.getElementById("lander3");
const canvas = document.getElementById("lander3-canvas");
const status = document.getElementById("lander3-status");
const buttons = {
  zoomIn: document.getElementById("lander3-in"),
  zoomOut: document.getElementById("lander3-out"),
  fit: document.getElementById("lander3-fit"),
  robots: document.getElementById("lander3-robots"),
};

function say(text) {
  if (status) {
    status.textContent = text;
  }
}

// Little-endian arrays the server sends as base64.
function decode(text, Type) {
  const binary = atob(text);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) {
    bytes[i] = binary.charCodeAt(i);
  }
  return new Type(bytes.buffer);
}

// Everything made here, released when the page goes away.
const owned = [];
function own(resource) {
  owned.push(resource);
  return resource;
}

function linear(rgb) {
  return new THREE.Color().setRGB(rgb[0], rgb[1], rgb[2], THREE.LinearSRGBColorSpace);
}

// One model's surfaces as geometries and materials, shared by every
// placement of it.
function prepare(model, textures) {
  return model.surfaces.map((surface) => {
    const geometry = own(new THREE.BufferGeometry());
    geometry.setAttribute(
      "position",
      new THREE.BufferAttribute(decode(surface.positions, Float32Array), 3)
    );
    geometry.setAttribute("uv", new THREE.BufferAttribute(decode(surface.uvs, Float32Array), 2));
    geometry.setIndex(new THREE.BufferAttribute(decode(surface.indices, Uint32Array), 1));
    if (surface.joints && surface.weights) {
      geometry.setAttribute(
        "skinIndex",
        new THREE.Uint16BufferAttribute(decode(surface.joints, Uint16Array), 4)
      );
      geometry.setAttribute(
        "skinWeight",
        new THREE.BufferAttribute(decode(surface.weights, Float32Array), 4)
      );
    }
    geometry.computeBoundingBox();
    geometry.computeBoundingSphere();
    const color = linear(surface.tint);
    const material = own(
      surface.lines
        ? new THREE.LineBasicMaterial({ color })
        : new THREE.MeshBasicMaterial({
            color,
            map: textures[surface.texture] || null,
            side: THREE.DoubleSide,
          })
    );
    return { geometry, material, lines: surface.lines };
  });
}

// A rigid placement of `parts` at a column-major matrix.
function place(parts, matrix) {
  const group = new THREE.Group();
  group.matrixAutoUpdate = false;
  group.matrix.fromArray(matrix);
  for (const part of parts) {
    group.add(
      part.lines
        ? new THREE.LineSegments(part.geometry, part.material)
        : new THREE.Mesh(part.geometry, part.material)
    );
  }
  return group;
}

// A skinned copy of `parts` whose skeleton plays the matrices the server
// sampled: each frame copies the current sample into the bone matrices
// instead of computing them from bones.
function skinned(parts, bones) {
  const skeleton = new THREE.Skeleton(Array.from({ length: bones }, () => new THREE.Bone()));
  skeleton.sample = null;
  skeleton.update = function () {
    if (this.sample) {
      this.boneMatrices.set(this.sample);
      if (this.boneTexture) {
        this.boneTexture.needsUpdate = true;
      }
    }
  };
  own(skeleton);
  const group = new THREE.Group();
  const identity = new THREE.Matrix4();
  for (const part of parts) {
    if (part.lines) {
      const lines = new THREE.LineSegments(part.geometry, part.material);
      group.add(lines);
      continue;
    }
    const mesh = new THREE.SkinnedMesh(part.geometry, part.material);
    mesh.bind(skeleton, identity);
    mesh.frustumCulled = false;
    group.add(mesh);
  }
  return { group, skeleton };
}

function start(scene3) {
  let renderer;
  try {
    renderer = own(new THREE.WebGLRenderer({ canvas, antialias: true }));
  } catch (error) {
    say("This browser can't draw the Grid: WebGL is turned off or not available.");
    return;
  }
  renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 2));

  const world = new THREE.Scene();
  world.background = linear(scene3.background);

  const loader = new THREE.TextureLoader();
  const textures = scene3.textures.map((url) => {
    const texture = own(loader.load(url));
    texture.colorSpace = THREE.SRGBColorSpace;
    return texture;
  });

  const models = {};
  for (const [name, model] of Object.entries(scene3.models)) {
    models[name] = { parts: prepare(model, textures), bones: model.bones };
  }

  // The ground spans the zoom-out; everything standing on it is what the
  // view frames.
  const ground = new THREE.Box3();
  const standing = new THREE.Box3();
  for (const placed of scene3.statics.concat(scene3.gates)) {
    const model = models[placed.model];
    if (!model) {
      continue;
    }
    const object = place(model.parts, placed.matrix);
    world.add(object);
    object.updateMatrixWorld(true);
    const box = new THREE.Box3().setFromObject(object);
    if (box.max.y - box.min.y < 0.01) {
      ground.union(box);
    } else {
      standing.union(box);
    }
  }

  // The robots: one per patrol, each with its near and far models.
  const info = scene3.robots;
  const poses = {};
  for (const [name, clips] of Object.entries(info.poses)) {
    poses[name] = clips.map((clip) => ({
      duration: clip.duration,
      frames: decode(clip.frames, Float32Array),
    }));
  }
  const robots = info.patrols.map((patrol) => {
    const samples = decode(patrol.samples, Float32Array);
    for (let i = 0; i < samples.length; i += 5) {
      standing.expandByPoint(new THREE.Vector3(samples[i], 0, samples[i + 1]));
    }
    const body = new THREE.Group();
    body.scale.setScalar(info.scale);
    const near = skinned(models[info.near].parts, models[info.near].bones);
    const far = skinned(models[info.far].parts, models[info.far].bones);
    body.add(near.group, far.group);
    world.add(body);
    return { round: patrol.round, samples, body, near, far };
  });

  function sample(skeleton, name, bones, clip, time) {
    const pose = poses[name][clip];
    const size = bones * 16;
    const count = pose.frames.length / size;
    const at = Math.min(count - 1, Math.max(0, Math.floor((time % pose.duration) * info.rate)));
    skeleton.sample = pose.frames.subarray(at * size, (at + 1) * size);
  }

  const camera = new THREE.PerspectiveCamera(40, 1, 0.5, 6000);
  const halfTan = Math.tan(THREE.MathUtils.degToRad(camera.fov / 2));
  // The view: the ground point under the middle of the canvas, and the
  // camera's height over it. The spawn's forward (+Z) points up the screen.
  const view = { x: 0, z: 0, height: 60 };
  let lowest = 6;
  let highest = 1500;

  function frame(box, height) {
    const size = box.getSize(new THREE.Vector3());
    const center = box.getCenter(new THREE.Vector3());
    const fitZ = size.z / 2 / halfTan;
    const fitX = size.x / 2 / (halfTan * camera.aspect);
    return { x: center.x, z: center.z, height: Math.max(fitZ, fitX) * 1.15 + height };
  }

  function fit() {
    const target = standing.isEmpty() ? ground : standing;
    if (target.isEmpty()) {
      return;
    }
    Object.assign(view, frame(target, target.max.y));
  }

  function limits() {
    if (!ground.isEmpty()) {
      highest = frame(ground, 0).height;
    }
    lowest = Math.max(4, (standing.isEmpty() ? 0 : standing.max.y) + 3);
    view.height = THREE.MathUtils.clamp(view.height, lowest, highest);
  }

  function aim() {
    camera.position.set(view.x, view.height, view.z);
    camera.up.set(0, 0, 1);
    camera.lookAt(view.x, 0, view.z);
    camera.updateMatrixWorld();
  }

  // Meters on the ground per CSS pixel at the current height.
  function metersPerPixel() {
    return (2 * view.height * halfTan) / Math.max(1, canvas.clientHeight);
  }

  // Moves the view so the content follows the pointer by (dx, dy) pixels.
  // Screen right is world -X and screen up is world +Z.
  function pan(dx, dy) {
    const m = metersPerPixel();
    view.x += dx * m;
    view.z += dy * m;
  }

  // Zooms by `factor` keeping the ground point under (px, py) still, given
  // in pixels from the canvas's middle.
  function zoom(factor, px, py) {
    const before = metersPerPixel();
    const height = THREE.MathUtils.clamp(view.height * factor, lowest, highest);
    const after = (before * height) / view.height;
    const pointX = view.x - px * before;
    const pointZ = view.z - py * before;
    view.x = pointX + px * after;
    view.z = pointZ + py * after;
    view.height = height;
  }

  function resize() {
    const width = Math.max(1, canvas.clientWidth);
    const height = Math.max(1, canvas.clientHeight);
    renderer.setSize(width, height, false);
    camera.aspect = width / height;
    camera.updateProjectionMatrix();
    limits();
  }

  const observer = new ResizeObserver(resize);
  observer.observe(root);
  resize();
  fit();
  limits();

  // Pointer drags pan; two pointers pinch to zoom.
  const pointers = new Map();
  let pinch = null;
  function offset(x, y) {
    const rect = canvas.getBoundingClientRect();
    return [x - rect.left - rect.width / 2, y - rect.top - rect.height / 2];
  }
  function pair() {
    const [a, b] = Array.from(pointers.values());
    return {
      distance: Math.hypot(a.x - b.x, a.y - b.y),
      x: (a.x + b.x) / 2,
      y: (a.y + b.y) / 2,
    };
  }
  canvas.addEventListener("pointerdown", (event) => {
    canvas.setPointerCapture(event.pointerId);
    pointers.set(event.pointerId, { x: event.clientX, y: event.clientY });
    canvas.classList.add("dragging");
    pinch = pointers.size === 2 ? pair() : null;
  });
  canvas.addEventListener("pointermove", (event) => {
    const last = pointers.get(event.pointerId);
    if (!last) {
      return;
    }
    if (pointers.size === 1) {
      pan(event.clientX - last.x, event.clientY - last.y);
      pointers.set(event.pointerId, { x: event.clientX, y: event.clientY });
      return;
    }
    pointers.set(event.pointerId, { x: event.clientX, y: event.clientY });
    if (pointers.size === 2 && pinch) {
      const now = pair();
      pan(now.x - pinch.x, now.y - pinch.y);
      if (now.distance > 0 && pinch.distance > 0) {
        const [px, py] = offset(now.x, now.y);
        zoom(pinch.distance / now.distance, px, py);
      }
      pinch = now;
    }
  });
  function release(event) {
    pointers.delete(event.pointerId);
    pinch = pointers.size === 2 ? pair() : null;
    if (pointers.size === 0) {
      canvas.classList.remove("dragging");
    }
  }
  canvas.addEventListener("pointerup", release);
  canvas.addEventListener("pointercancel", release);
  canvas.addEventListener(
    "wheel",
    (event) => {
      event.preventDefault();
      const lines = event.deltaMode === 1 ? 16 : event.deltaMode === 2 ? 400 : 1;
      const [px, py] = offset(event.clientX, event.clientY);
      zoom(Math.exp(event.deltaY * lines * 0.0015), px, py);
    },
    { passive: false }
  );
  canvas.addEventListener("keydown", (event) => {
    const step = canvas.clientHeight * 0.1;
    const keys = {
      ArrowLeft: () => pan(step, 0),
      ArrowRight: () => pan(-step, 0),
      ArrowUp: () => pan(0, step),
      ArrowDown: () => pan(0, -step),
      "+": () => zoom(0.8, 0, 0),
      "=": () => zoom(0.8, 0, 0),
      "-": () => zoom(1.25, 0, 0),
    };
    const act = keys[event.key];
    if (act) {
      event.preventDefault();
      act();
    }
  });

  buttons.zoomIn.addEventListener("click", () => zoom(0.7, 0, 0));
  buttons.zoomOut.addEventListener("click", () => zoom(1 / 0.7, 0, 0));
  buttons.fit.addEventListener("click", () => {
    fit();
    limits();
  });
  let paused = false;
  buttons.robots.addEventListener("click", () => {
    paused = !paused;
    buttons.robots.textContent = paused ? "Play robots" : "Pause robots";
    buttons.robots.setAttribute("aria-pressed", paused ? "true" : "false");
  });
  for (const button of Object.values(buttons)) {
    button.disabled = false;
  }

  let clock = 0;
  let last = performance.now();
  let running = true;
  function draw(now) {
    if (!running) {
      return;
    }
    // A frame's timestamp can come before the clock read at start.
    const dt = Math.max(0, Math.min(0.1, (now - last) / 1000));
    last = now;
    if (!paused) {
      clock += dt;
    }
    aim();
    for (const robot of robots) {
      const count = robot.samples.length / 5;
      const i = Math.min(count - 1, Math.max(0, Math.floor((clock % robot.round) * info.rate))) * 5;
      const s = robot.samples;
      robot.body.position.set(s[i], 0, s[i + 1]);
      robot.body.rotation.set(0, s[i + 2], 0);
      const far = camera.position.distanceTo(robot.body.position) > info.far_meters;
      robot.near.group.visible = !far;
      robot.far.group.visible = far;
      const clip = s[i + 3];
      sample(robot.near.skeleton, info.near, models[info.near].bones, clip, s[i + 4]);
      sample(robot.far.skeleton, info.far, models[info.far].bones, clip, s[i + 4]);
    }
    renderer.render(world, camera);
    requestAnimationFrame(draw);
  }
  requestAnimationFrame(draw);
  say("");

  window.addEventListener("pagehide", () => {
    running = false;
    observer.disconnect();
    for (const resource of owned) {
      resource.dispose();
    }
  });
}

if (root && canvas) {
  const address = root.getAttribute("data-scene");
  fetch(address, { credentials: "same-origin" })
    .then((response) => {
      if (!response.ok) {
        throw new Error("the scene answered " + response.status);
      }
      return response.json();
    })
    .then(start)
    .catch(() => {
      say("The Grid didn't load. Reload the page to try again.");
    });
}
