import { Effect } from "effect";
import { Universe } from "./renderer.ts";
import { project, worlds } from "./space.ts";

const canvas = document.querySelector<HTMLCanvasElement>("#universe")!;
const stage = document.querySelector<HTMLElement>(".universe-stage")!;
const status = document.querySelector<HTMLElement>("#universe-status")!;
const controller = new AbortController();
const lifetime = new AbortController();
let runtime: Universe | undefined;
let selected = "openagents";
const select = (id: string) => {
  const world = worlds.find((w) => w.id === id);
  if (!world) return;
  selected = id;
  runtime?.focus(id);
  document.querySelector("#world-title")!.textContent = world.name;
  document.querySelector("#world-category")!.textContent = world.category;
  document.querySelector("#world-description")!.textContent = world.description;
  document
    .querySelectorAll<HTMLElement>("[data-world]")
    .forEach((el) =>
      el.setAttribute("aria-pressed", String(el.dataset.world === id)),
    );
  document
    .querySelectorAll<HTMLElement>("[data-destination]")
    .forEach((el) => (el.hidden = el.dataset.destination !== id));
  const url = new URL(location.href);
  url.searchParams.set("world", id);
  history.replaceState(null, "", url);
};
document
  .querySelectorAll<HTMLElement>("[data-world]")
  .forEach((el) =>
    el.addEventListener("click", () => select(el.dataset.world!), {
      signal: controller.signal,
    }),
  );
const initial = new URL(location.href).searchParams.get("world");
if (initial) select(initial);

let pointer: { id: number; x: number; y: number; distance: number } | undefined;
canvas.addEventListener(
  "pointerdown",
  (e) => {
    pointer = { id: e.pointerId, x: e.clientX, y: e.clientY, distance: 0 };
    canvas.setPointerCapture(e.pointerId);
  },
  { signal: controller.signal },
);
canvas.addEventListener(
  "pointermove",
  (e) => {
    if (!pointer || !runtime) return;
    const dx = e.clientX - pointer.x,
      dy = e.clientY - pointer.y;
    pointer.distance += Math.hypot(dx, dy);
    runtime.yaw -= dx * 0.004;
    runtime.pitch = Math.max(0.13, Math.min(1.15, runtime.pitch + dy * 0.004));
    pointer.x = e.clientX;
    pointer.y = e.clientY;
  },
  { signal: controller.signal },
);
canvas.addEventListener(
  "pointerup",
  (e) => {
    if (pointer && pointer.distance < 6 && runtime) {
      const rect = canvas.getBoundingClientRect();
      let best = 45,
        chosen: string | undefined;
      for (const world of worlds) {
        const p = project(world.position, runtime.matrix);
        const distance = Math.hypot(
          p.x * rect.width - (e.clientX - rect.left),
          p.y * rect.height - (e.clientY - rect.top),
        );
        if (p.visible && distance < best) {
          best = distance;
          chosen = world.id;
        }
      }
      if (chosen) select(chosen);
    }
    pointer = undefined;
  },
  { signal: controller.signal },
);
canvas.addEventListener(
  "pointercancel",
  () => {
    pointer = undefined;
  },
  { signal: controller.signal },
);
canvas.addEventListener(
  "wheel",
  (e) => {
    if (!runtime) return;
    e.preventDefault();
    runtime.distance = Math.max(
      14,
      Math.min(44, runtime.distance + e.deltaY * 0.018),
    );
  },
  { passive: false, signal: controller.signal },
);
canvas.addEventListener(
  "keydown",
  (e) => {
    if (!runtime) return;
    let used = true;
    switch (e.key) {
      case "ArrowLeft":
        runtime.yaw -= 0.08;
        break;
      case "ArrowRight":
        runtime.yaw += 0.08;
        break;
      case "ArrowUp":
        runtime.pitch = Math.min(1.15, runtime.pitch + 0.06);
        break;
      case "ArrowDown":
        runtime.pitch = Math.max(0.13, runtime.pitch - 0.06);
        break;
      case "+":
      case "=":
        runtime.distance = Math.max(14, runtime.distance - 2);
        break;
      case "-":
        runtime.distance = Math.min(44, runtime.distance + 2);
        break;
      case "Escape":
        runtime.reset();
        break;
      default:
        used = false;
    }
    if (used) e.preventDefault();
  },
  { signal: controller.signal },
);
document.querySelector("#zoom-in")!.addEventListener(
  "click",
  () => {
    if (runtime) runtime.distance = Math.max(14, runtime.distance - 3);
  },
  { signal: controller.signal },
);
document.querySelector("#zoom-out")!.addEventListener(
  "click",
  () => {
    if (runtime) runtime.distance = Math.min(44, runtime.distance + 3);
  },
  { signal: controller.signal },
);
document
  .querySelector("#reset-view")!
  .addEventListener("click", () => runtime?.reset(), {
    signal: controller.signal,
  });
const fallback = () => {
  stage.classList.remove("ready");
  document.body.dataset.rendering = "still";
  status.textContent = "Still preview · You can still choose a world.";
  document
    .querySelectorAll<HTMLButtonElement>(".camera-controls button")
    .forEach((b) => (b.disabled = true));
};
Effect.runPromise(
  Effect.scoped(
    Effect.gen(function* () {
      const renderer = yield* Effect.acquireRelease(
        Effect.tryPromise({
          try: () => Universe.create(canvas),
          catch: (error) => error,
        }),
        (r) => Effect.sync(() => r.dispose()),
      );
      runtime = renderer;
      const compilationErrors: string[] = [];
      renderer.device.addEventListener(
        "uncapturederror",
        (event) => {
          compilationErrors.push(event.error.message);
          console.error(event.error.message);
          fallback();
          lifetime.abort();
        },
        { signal: controller.signal },
      );
      renderer.device.lost.then((info) => {
        if (!renderer.disposed) {
          status.textContent =
            "The interactive view stopped. Reload to try again.";
          document.body.dataset.rendering = "still";
          lifetime.abort();
        }
      });
      renderer.render(performance.now());
      renderer.start();
      renderer.focus(selected);
      yield* Effect.tryPromise({
        try: () => renderer.device.queue.onSubmittedWorkDone(),
        catch: (error) => error,
      });
      if (compilationErrors.length)
        throw new Error(compilationErrors.join("; "));
      stage.classList.add("ready");
      document.body.dataset.rendering = "webgpu";
      status.textContent = "Drag to orbit · Scroll to zoom · Select a world";
      yield* Effect.promise(
        () =>
          new Promise<void>((resolve) => {
            if (lifetime.signal.aborted) resolve();
            else
              lifetime.signal.addEventListener("abort", () => resolve(), {
                once: true,
              });
          }),
      );
    }),
  ),
).catch((error) => {
  console.warn("Verse preview:", String(error));
  fallback();
});
addEventListener(
  "pagehide",
  () => {
    controller.abort();
    lifetime.abort();
    runtime?.dispose();
  },
  { once: true },
);
// A page restored from the back/forward cache needs a new GPU lifetime.
addEventListener("pageshow", (event) => {
  if (event.persisted) location.reload();
});
