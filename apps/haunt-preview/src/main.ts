import { Effect } from "effect";
import { ASSET, PROJECT, sourceHash, type ShaderEdit } from "./protocol.ts";
import "./style.css";
import { createEditor } from "./editor.ts";
import type { Operation } from "./project.ts";

declare global {
  interface Window {
    kilnReady?: boolean;
    kilnSaveShader: (source: string) => Promise<boolean>;
    kilnReadShader: () => string | null;
    kilnShaderChanged?: (source: string) => void;
    kilnWrite: (path: string, source: string) => Promise<number>;
    kilnReadScene: () => string | null;
    kilnSceneChanged?: (source: string) => void;

    kilnState: () => string;

    hauntPreview: {
      save: (source: string) => Promise<Record<string, unknown>>;
      snapshot: () => unknown;
      original: () => string;
      dispose: () => void;
      command: (operation: Operation) => Promise<unknown>;
      request: (text: string) => Promise<unknown>;
      editor: () => unknown;
      capture: () => Promise<Blob>;
    };
  }
}

const el = <T extends HTMLElement>(id: string) =>
  document.getElementById(id) as T;
const shader = el<HTMLTextAreaElement>("shader"),
  saveButton = el<HTMLButtonElement>("save"),
  resetButton = el<HTMLButtonElement>("reset");
const canvas = el<HTMLCanvasElement>("house"),
  diagnostics = el<HTMLPreElement>("diagnostics");
const preview = crypto.randomUUID(),
  token = crypto.randomUUID();
let worker: Worker | undefined;
let draft = 0,
  original = "",
  sourceSent = "",
  saving = false,
  ready = false;
let snapshot: Record<string, any> = {};
let adapter: unknown;
let latestResult: Record<string, unknown> = {};
let activeRequest = Promise.resolve<Record<string, unknown>>({});
let manualSaveStart: number | undefined;
const samples: number[] = [];
const receipts: Record<string, unknown>[] = [];
const waiting = new Map<
  number,
  { resolve: (r: Record<string, unknown>) => void; started: number }
>();
const now = () => performance.timeOrigin + performance.now();
let capturePending:
  | { id: string; resolve: (b: Blob) => void; reject: (e: Error) => void }
  | undefined;

function showError(error: unknown) {
  diagnostics.hidden = false;
  diagnostics.textContent = String(error);
  el("shader-status").textContent =
    snapshot.accepted > 0
      ? "Couldn't apply this edit. The last working shader is still running."
      : "Couldn't apply the shader. Check the error below.";
}

let workspaceQueue: Promise<unknown> = Promise.resolve();
function workspace<T>(job: () => Promise<T>): Promise<T> {
  const next = workspaceQueue.catch(() => {}).then(job);
  workspaceQueue = next;
  return next;
}
const editor = createEditor({
  post: (message) => post(message),
  state: () => snapshot,
  workspace,
});
function controls() {
  saveButton.disabled = resetButton.disabled = !ready || saving;
  shader.disabled = !ready;
  editor.setReady(ready && !saving);
  el<HTMLButtonElement>("walk").disabled = !ready;
}

function post(message: object) {
  worker?.postMessage({ ...message, preview, token });
}

function forward(
  source: string,
  started = now(),
): Promise<Record<string, unknown>> {
  sourceSent = source;
  shader.value = source;
  const revision = ++draft;
  for (const [old, pending] of waiting) {
    pending.resolve({ type: "superseded", revision: old });
    waiting.delete(old);
  }
  const result = new Promise<Record<string, unknown>>((resolve) =>
    waiting.set(revision, { resolve, started }),
  );
  el("draft-state").textContent = `Draft ${revision}`;
  el("shader-status").textContent = "Checking the shader…";
  diagnostics.hidden = true;
  sourceHash(source)
    .then((hash) => {
      if (revision !== draft) return;
      const edit: ShaderEdit = {
        type: "shader",
        project: PROJECT,
        preview,
        token,
        request: crypto.randomUUID(),
        revision,
        asset: ASSET,
        hash,
        source,
        savedAt: started,
      };
      worker?.postMessage(edit);
    })
    .catch((e) => {
      showError(e);
      waiting.get(revision)?.resolve({ type: "rejected", message: String(e) });
      waiting.delete(revision);
    });
  return result;
}

async function save(source: string): Promise<Record<string, unknown>> {
  if (!ready || saving)
    throw new Error("Wait for Kiln to finish before saving another edit.");
  saving = true;
  manualSaveStart = now();
  controls();
  try {
    await Effect.runPromise(
      Effect.tryPromise({
        try: () => workspace(() => window.kilnSaveShader(source)),
        catch: (e) => new Error(String(e)),
      }),
    );
    if (sourceSent !== source) activeRequest = forward(source, manualSaveStart);
    return await activeRequest;
  } finally {
    saving = false;
    manualSaveStart = undefined;
    controls();
  }
}

const keys = new Set<string>();
function clearInput() {
  keys.clear();
  post({ type: "input", keys: [] });
}
canvas.addEventListener("keydown", (event) => {
  if (
    [
      "KeyW",
      "KeyA",
      "KeyS",
      "KeyD",
      "ArrowUp",
      "ArrowDown",
      "ArrowLeft",
      "ArrowRight",
    ].includes(event.code)
  ) {
    event.preventDefault();
    keys.add(event.code);
    post({ type: "input", keys: [...keys] });
  }
});
window.addEventListener("keyup", (event) => {
  keys.delete(event.code);
  post({ type: "input", keys: [...keys] });
});
window.addEventListener("blur", clearInput);
canvas.addEventListener("blur", clearInput);
document.addEventListener("visibilitychange", clearInput);
let dragging = false;
canvas.addEventListener("pointerdown", () => {
  canvas.focus();
  dragging = true;
});
window.addEventListener("pointerup", () => {
  dragging = false;
});
window.addEventListener("mousemove", (e) => {
  if (dragging || document.pointerLockElement === canvas)
    post({ type: "look", dx: e.movementX, dy: e.movementY });
});
el("walk").addEventListener("click", () => {
  canvas.focus();
  canvas.requestPointerLock()?.catch(() => {
    el("preview-status").textContent =
      "Drag to look around. Use W A S D to walk.";
  });
});
saveButton.addEventListener("click", () => save(shader.value).catch(showError));
resetButton.addEventListener("click", () => save(original).catch(showError));
shader.addEventListener("input", () => {
  el("draft-state").textContent = "Unsaved edit";
});
shader.addEventListener("keydown", (event) => {
  if ((event.metaKey || event.ctrlKey) && event.key === "Enter") {
    event.preventDefault();
    save(shader.value).catch(showError);
  }
});

async function start() {
  if (!navigator.gpu || !canvas.transferControlToOffscreen)
    throw new Error(
      "This preview needs a current Chrome or Edge with WebGPU and graphics acceleration. No alternate renderer is used.",
    );
  original = await (await fetch("/material.wgsl")).text();
  shader.value = original;
  worker = new Worker(new URL("./preview.worker.ts", import.meta.url), {
    type: "module",
  });
  const workerReady = new Promise<void>((resolve, reject) => {
    worker!.onerror = (event) => reject(new Error(event.message));
    worker!.onmessage = (event) => {
      const message = event.data;
      if (message?.preview !== preview || message?.token !== token) return;
      if (message.state) snapshot = message.state;
      if (
        message.type === "capture" &&
        capturePending &&
        capturePending.id === message.request
      ) {
        const job = capturePending;
        capturePending = undefined;
        if (message.blob instanceof Blob) job.resolve(message.blob);
        else job.reject(new Error(message.message));
      }
      if (message.type === "ready") {
        adapter = message.adapter;
        resolve();
      }
      if (message.type === "unsupported") {
        reject(new Error(message.message));
      }
      if (message.type === "accepted" || message.type === "rejected") {
        const pending = waiting.get(message.revision);
        if (message.type === "accepted") {
          const ms =
            message.visibleAt - (pending?.started ?? message.visibleAt);
          receipts.push({ ...message, ms });
          if (receipts.length > 100) receipts.shift();
          samples.push(ms);
          if (samples.length > 100) samples.shift();
          el("loading").hidden = true;
          if (message.revision === draft) {
            el("shader-status").textContent =
              "Saved in Kiln. Your shader is live.";
            el("preview-status").textContent = "Live from your Kiln workspace";
            el("draft-state").textContent = `Live · ${message.revision}`;
            el("timing").textContent = `${Math.round(ms)} ms`;
            diagnostics.hidden = true;
          }
        } else if (message.revision === draft) showError(message.message);
        latestResult = message;
        pending?.resolve(message);
        waiting.delete(message.revision);
      }
      if (message.type === "lost" || message.type === "error")
        showError(message.message);
      if (
        (message.type === "lost" || message.type === "disposed") &&
        capturePending
      ) {
        capturePending.reject(
          new Error("The preview closed before capture finished."),
        );
        capturePending = undefined;
      }

      if (message.type === "lost") {
        ready = false;
        controls();
        for (const pending of waiting.values())
          pending.resolve({ type: "rejected", message: message.message });
        waiting.clear();
      }
      if (message.type === "disposed") {
        ready = false;
        controls();
        el("preview-status").textContent = "Preview closed.";
      }
      editor.receive(message);
    };
  });
  const offscreen = canvas.transferControlToOffscreen();
  worker.postMessage({ type: "init", preview, token, canvas: offscreen }, [
    offscreen,
  ]);
  await workerReady;
  const resize = () => {
    const rect = canvas.getBoundingClientRect();
    post({
      type: "resize",
      width: rect.width * Math.min(2, devicePixelRatio),
      height: rect.height * Math.min(2, devicePixelRatio),
    });
  };
  const observer = new ResizeObserver(resize);
  observer.observe(canvas);
  resize();
  const kilnPath = new URL("/kiln/pkg/kiln.js", location.origin).href;
  const kiln = await import(/* @vite-ignore */ kilnPath);
  await kiln.default();
  kiln.start_headless("/kiln/");
  const until = performance.now() + 30000;
  while (!window.kilnReady) {
    if (performance.now() > until)
      throw new Error(
        "Kiln could not load. Rebuild its browser files and reopen the preview.",
      );
    await new Promise((resolve) => setTimeout(resolve, 30));
  }
  if (typeof window.kilnSaveShader !== "function")
    throw new Error(
      "Rebuild Kiln to include the Haunted House shader connection.",
    );
  window.kilnShaderChanged = (source) => {
    activeRequest = forward(source, manualSaveStart ?? now());
  };
  ready = true;
  controls();
  window.hauntPreview = {
    save,
    snapshot: () =>
      structuredClone({
        ...snapshot,
        adapter,
        draft,
        latestResult,
        samples: [...samples],
        receipts: [...receipts],
        kiln: JSON.parse(window.kilnState()),
      }),
    original: () => original,
    dispose: () => post({ type: "dispose" }),
    command: editor.submit,
    request: editor.interpret,
    editor: editor.snapshot,
    capture: () =>
      new Promise<Blob>((resolve, reject) => {
        if (capturePending || !ready) {
          reject(
            new Error("Wait for the current capture or reopen the preview."),
          );
          return;
        }
        const id = crypto.randomUUID();
        capturePending = { id, resolve, reject };
        post({ type: "capture", request: id });
      }),
  };
  const previousDraft = window.kilnReadShader();
  // A new device always starts with a working baseline, even if OPFS holds a bad draft.
  activeRequest = forward(original);
  await activeRequest;
  if (previousDraft) {
    // Upgrade this demo's v1 shader layout while preserving its authored tint/code.
    const upgraded = previousDraft.includes("LANTERN_TINT * toon")
      ? previousDraft
          .replace(
            "  light: vec4<f32>,",
            "  light: vec4<f32>,\n  tint: vec4<f32>,",
          )
          .replace(
            "LANTERN_TINT * toon",
            "LANTERN_TINT * scene.tint.xyz * toon",
          )
      : previousDraft;
    activeRequest = forward(upgraded);
    await activeRequest;
  } else await save(original);
  window.kilnSceneChanged = (source) => {
    void editor.ingest(source);
  };
  const savedScene = window.kilnReadScene?.();
  if (savedScene) await editor.ingest(savedScene, true);
  else
    document.getElementById("scene-status")!.textContent =
      "Ready. Place a pumpkin or describe one change.";
  editor.setReady(true);
  window.addEventListener(
    "pagehide",
    () => {
      editor.cancel();
      observer.disconnect();
      post({ type: "dispose" });
      worker?.terminate();
    },
    { once: true },
  );
}

Effect.runPromise(
  Effect.tryPromise({
    try: start,
    catch: (e) => (e instanceof Error ? e : new Error(String(e))),
  }),
).catch((error) => {
  ready = false;
  controls();
  post({ type: "dispose" });
  el("loading").textContent = String(error);
  el("shader-status").textContent = "The preview couldn't start.";
  showError(error);
});
