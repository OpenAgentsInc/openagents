import "./ui.css";
import { Universe } from "../../packages/universe/renderer";
import {
  camera,
  project,
  worlds,
  type Vec3,
} from "../../packages/universe/space";
const params = new URLSearchParams(location.hash.slice(1));
const session = params.get("session") ?? "";
const editor = params.get("editorOrigin") ?? "";
if (!/^[a-z0-9-]{1,64}$/.test(session) || new URL(editor).origin !== editor)
  throw Error("The planets need a compatible host.");
const canvas = document.querySelector<HTMLCanvasElement>("#universe")!;
let runtime: Universe | undefined,
  selected = "openagents",
  timer = 0;
const lifetime = new AbortController();
const send = (value: object) =>
  parent.postMessage({ session, ...value }, editor);
const checkpoint = () => {
  const r = runtime!;
  return {
    eye: camera(
      r.yaw,
      r.pitch,
      r.distance,
      r.target,
      Math.max(1, r.width) / Math.max(1, r.height),
    ).eye,
    target: r.target,
    mode: "fly",
  };
};
const status = () => ({
  project: { id: "universe", branch: "main", revision: 0 },
  generation: session,
  frame: runtime?.submittedFrames ?? 0,
  save: "unsaved",
  checkpoint: runtime ? checkpoint() : null,
  selection: {
    id: selected,
    asset: "planet:" + selected,
    position: worlds.find((w) => w.id === selected)!.position,
  },
});
const publish = () => send({ type: "status", status: status() });
const select = (id: string) => {
  if (!worlds.some((w) => w.id === id))
    throw Error("Choose one of the planets on this map.");
  selected = id;
  runtime!.focus(id);
  runtime!.ambient = false;
  publish();
};
const stop = () => {
  lifetime.abort();
  clearInterval(timer);
  runtime?.dispose();
  runtime = undefined;
};
window.addEventListener(
  "message",
  (event) => {
    if (
      event.source !== parent ||
      event.origin !== editor ||
      event.data?.session !== session ||
      typeof event.data?.request !== "string"
    )
      return;
    const message = event.data;
    try {
      if (!runtime) throw Error("The planets are still opening.");
      if (message.type === "select") select(message.target);
      else if (message.type === "restore") {
        const c = message.checkpoint;
        if (
          !c ||
          ![c.eye, c.target].every(
            (p) =>
              Array.isArray(p) &&
              p.length === 3 &&
              p.every(
                (n: unknown) =>
                  typeof n === "number" &&
                  Number.isFinite(n) &&
                  Math.abs(n) <= 100,
              ),
          )
        )
          throw Error("Choose a view on this map.");
        const offset = c.eye.map((n: number, i: number) => n - c.target[i]),
          distance = Math.hypot(...offset);
        if (distance < 12 || distance > 60)
          throw Error("Choose a closer view of the planets.");
        runtime.yaw = Math.atan2(offset[0], offset[2]);
        runtime.pitch = Math.max(
          0.12,
          Math.min(1.3, Math.asin(offset[1] / distance)),
        );
        runtime.distance = distance;
        runtime.target = [...c.target] as Vec3;
        runtime.desired = [...c.target] as Vec3;
      } else if (message.type === "close") {
        stop();
        send({ type: "reply", request: message.request, status: null });
        return;
      } else throw Error("That control is not available here.");
      send({ type: "reply", request: message.request, status: status() });
    } catch (error) {
      send({
        type: "reply",
        request: message.request,
        error:
          error instanceof Error ? error.message : "The view could not change.",
      });
    }
  },
  { signal: lifetime.signal },
);
window.addEventListener("pagehide", stop, { once: true });
try {
  runtime = await Universe.create(canvas);
  if (lifetime.signal.aborted) {
    runtime.dispose();
    throw Error("The map closed.");
  }
  document.querySelector("#notice")!.setAttribute("hidden", "");
  const markers = document.querySelector("#markers")!;
  for (const world of worlds) {
    const b = document.createElement("button");
    b.className = "marker";
    b.id = "marker-" + world.id;
    b.textContent = world.name;
    b.onclick = () => select(world.id);
    markers.append(b);
  }
  let drag: { x: number; y: number; distance: number } | undefined;
  canvas.addEventListener(
    "pointerdown",
    (e) => {
      canvas.focus();
      canvas.setPointerCapture(e.pointerId);
      drag = { x: e.clientX, y: e.clientY, distance: 0 };
      runtime!.ambient = false;
    },
    { signal: lifetime.signal },
  );
  canvas.addEventListener(
    "pointermove",
    (e) => {
      if (!drag || !runtime) return;
      const dx = e.clientX - drag.x,
        dy = e.clientY - drag.y;
      drag.distance += Math.abs(dx) + Math.abs(dy);
      runtime.yaw -= dx * 0.006;
      runtime.pitch = Math.max(0.12, Math.min(1.3, runtime.pitch + dy * 0.004));
      drag.x = e.clientX;
      drag.y = e.clientY;
    },
    { signal: lifetime.signal },
  );
  canvas.addEventListener(
    "pointerup",
    (e) => {
      if (drag && drag.distance < 5 && runtime) {
        const box = canvas.getBoundingClientRect();
        let nearest: string | undefined,
          distance = 45;
        for (const w of worlds) {
          const p = project(w.position, runtime.matrix);
          const d = Math.hypot(
            p.x * box.width - (e.clientX - box.left),
            p.y * box.height - (e.clientY - box.top),
          );
          if (p.visible && d < distance) {
            nearest = w.id;
            distance = d;
          }
        }
        if (nearest) select(nearest);
      }
      drag = undefined;
    },
    { signal: lifetime.signal },
  );
  const zoom = (n: number) => {
    if (runtime) {
      runtime.distance = Math.max(16, Math.min(48, runtime.distance + n));
      runtime.ambient = false;
    }
  };
  canvas.addEventListener(
    "wheel",
    (e) => {
      e.preventDefault();
      zoom(Math.sign(e.deltaY) * 1.3);
    },
    { passive: false, signal: lifetime.signal },
  );
  canvas.addEventListener(
    "keydown",
    (e) => {
      if (!runtime) return;
      const r = runtime;
      switch (e.key) {
        case "ArrowLeft":
          r.yaw -= 0.08;
          break;
        case "ArrowRight":
          r.yaw += 0.08;
          break;
        case "ArrowUp":
          r.pitch = Math.min(1.3, r.pitch + 0.06);
          break;
        case "ArrowDown":
          r.pitch = Math.max(0.12, r.pitch - 0.06);
          break;
        case "+":
        case "=":
          zoom(-2);
          break;
        case "-":
          zoom(2);
          break;
        case "Escape":
          r.reset();
          break;
        default:
          return;
      }
      r.ambient = false;
      e.preventDefault();
    },
    { signal: lifetime.signal },
  );
  document
    .querySelector("#zoom-in")!
    .addEventListener("click", () => zoom(-2), { signal: lifetime.signal });
  document
    .querySelector("#zoom-out")!
    .addEventListener("click", () => zoom(2), { signal: lifetime.signal });
  document
    .querySelector("#reset")!
    .addEventListener("click", () => runtime?.reset(), {
      signal: lifetime.signal,
    });
  runtime.device.addEventListener(
    "uncapturederror",
    () =>
      send({
        type: "error",
        message: "The planets could not be drawn. Reload to try again.",
      }),
    { signal: lifetime.signal },
  );
  runtime.start();
  publish();
  timer = window.setInterval(publish, 200);
} catch {
  send({
    type: "error",
    message: "The 3D view is unavailable. You can still choose a destination.",
  });
}
