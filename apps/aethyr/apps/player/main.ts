import "./player.css";
import { playerArtifact, playerChrome } from "./bootstrap.ts";
import { preparePublishedWorld } from "../../packages/runtime/published-world.ts";
import { verifyPlayerArtifact } from "../../packages/runtime/player-artifact.ts";
import { WorldSession } from "../../packages/runtime/world-session.ts";
import { liveViewport } from "../../packages/runtime/live-viewport.ts";
import {
  cameraForCapture,
  environmentForCapture,
  validateFrameCaptureOptions,
  validateFilmCaptureSize,
} from "../../packages/renderer/film-frame.ts";
import { DRUID_ABILITIES } from "../../packages/runtime/druid-abilities.ts";
import type { V3 } from "../../packages/renderer/math.ts";
import {
  browserEnvironment,
  LIVE_QUERIES,
  probeDeviceProfile,
  probeLive,
} from "../../packages/runtime/device-profile.ts";
import { DetailDriver } from "../../packages/runtime/detail-driver.ts";
import { defaultProfile } from "../../packages/runtime/detail.ts";

const ui = playerChrome(),
  startup = performance.now();
const lifetime = new AbortController();
let session: WorldSession | undefined;
let disposed = false,
  frame = 0,
  paused = false,
  lastStatus = 0;
let capturing = false;
let startupMetrics: Record<string, unknown> = {};
let phase: "loading" | "ready" | "error" | "closed" = "loading";
let selectedSpell: (typeof DRUID_ABILITIES)[number]["id"] | undefined;
let detail: DetailDriver | undefined;
let failure: { message: string; detail: unknown } | undefined;
// Probe while the world downloads; a probe failure only loses information.
const probing = probeDeviceProfile().catch(() => defaultProfile());
const keys = new Set<string>();
const close = () => {
  if (disposed) return;
  disposed = true;
  lifetime.abort();
  cancelAnimationFrame(frame);
  keys.clear();
  session?.dispose();
  session = undefined;
  phase = "closed";
};
const release = () => {
  keys.clear();
  session?.clearInput();
};
const updateInput = () =>
  session?.input({
    forward:
      Number(keys.has("KeyW") || keys.has("ArrowUp")) -
      Number(keys.has("KeyS") || keys.has("ArrowDown")),
    right:
      Number(keys.has("KeyD") || keys.has("ArrowRight")) -
      Number(keys.has("KeyA") || keys.has("ArrowLeft")),
    sprint: keys.has("ShiftLeft") || keys.has("ShiftRight"),
    jump: keys.has("Space"),
  });
const objective = () => {
  if (!session?.game) return "WASD to move · drag to look · V to switch view";
  const state = session.game.snapshot();
  return state.completed
    ? "You found the exit! Restart to play again."
    : state.keyCollected
      ? "Find the exit · E to interact"
      : "Find the key · E to interact";
};
const message = (error: unknown) =>
  ui.message(error instanceof Error ? error.message : String(error));
/** Failure reports carry the profile and the Detail the world was running at. */
const fail = (error: unknown) => {
  failure = {
    message: error instanceof Error ? error.message : String(error),
    detail: detail?.report() ?? null,
  };
};
const applyInterfaceDetail = () =>
  detail?.applyInterface(document.documentElement);
const resize = () => {
  if (!session || capturing) return;
  const size = liveViewport(Math.floor(innerWidth), Math.floor(innerHeight));
  if (size) session.resize(size.width, size.height);
};
function draw(now: number) {
  if (disposed) return;
  try {
    if (session && !paused && !document.hidden) session.tick(now);
    if (now - lastStatus > 250) {
      lastStatus = now;
      ui.message(objective());
    }
    frame = requestAnimationFrame(draw);
    if (detail && detail.detail !== interfaceDetail) {
      interfaceDetail = detail.detail;
      applyInterfaceDetail();
    }
  } catch (error) {
    fail(error);
    close();
    phase = "error";
    message(error);
  }
}
let interfaceDetail: number | undefined;
window.addEventListener("keydown", (event) => {
  if (!session || event.target instanceof HTMLButtonElement) return;
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
      "Space",
      "ShiftLeft",
      "ShiftRight",
    ].includes(event.code)
  ) {
    event.preventDefault();
    keys.add(event.code);
    updateInput();
  }
  if (event.repeat) return;
  if (event.code === "Escape") {
    selectedSpell = undefined;
    session.cancelCast();
  }
  if (event.code === "KeyE") {
    try {
      session.interact();
    } catch (error) {
      message(error);
    }
  }
  if (event.code === "KeyR") session.reset();
  if (event.code === "KeyV" && session.source.artifact.profile === "character")
    session.perspective(
      session.characterCamera.perspective === "first" ? "third" : "first",
    );
  const spell = DRUID_ABILITIES.find((entry) => entry.key === event.key);
  if (
    spell &&
    (spell.id !== "destroy_castle" ||
      session.source.prepared?.dayGameplay?.manifest.targetGroups?.castle
        .length)
  ) {
    selectedSpell = spell.id;
    ui.message(spell.name + " · click to cast");
  }
});
window.addEventListener("keyup", (event) => {
  keys.delete(event.code);
  updateInput();
});
window.addEventListener("blur", release);
document.addEventListener("visibilitychange", () => {
  if (document.hidden) release();
});
window.addEventListener("resize", resize);
window.addEventListener("pagehide", close, { once: true });
let dragging = false;
ui.canvas.addEventListener("pointerdown", (event) => {
  ui.canvas.focus();
  dragging = true;
  ui.canvas.setPointerCapture(event.pointerId);
  if (selectedSpell && session) void session.cast(selectedSpell).catch(message);
});
ui.canvas.addEventListener("pointerup", () => {
  dragging = false;
});
ui.canvas.addEventListener("pointercancel", () => {
  dragging = false;
  release();
});
ui.canvas.addEventListener("pointermove", (event) => {
  if (dragging || document.pointerLockElement === ui.canvas)
    session?.look(event.movementX, event.movementY);
});
ui.canvas.addEventListener("dblclick", () => {
  void ui.canvas.requestPointerLock();
});

async function start() {
  const profile =
    import.meta.env.VITE_PLAYER_PROFILE === "room" ? "room" : "character";
  const { artifact, descriptorAndRuntimeCheckMs } =
    await playerArtifact(profile);
  const canvas = ui.canvas.transferControlToOffscreen();
  const source = await preparePublishedWorld(artifact, canvas, {
    report: ui.message,
    compatibility: { profile },
    signal: lifetime.signal,
    rendererOptions: {
      diagnostics: import.meta.env.VITE_PLAYER_DIAGNOSTICS === "1",
    },
  });
  if (disposed) {
    source.prepared?.renderer.dispose();
    return;
  }
  session = await WorldSession.create(source);
  detail = new DetailDriver(await probing);
  session.attachDetail(detail);
  applyInterfaceDetail();
  const env = browserEnvironment();
  for (const query of LIVE_QUERIES)
    matchMedia(query).addEventListener(
      "change",
      () => {
        detail?.updateProfile(probeLive(env, detail.profile));
        session?.attachDetail(detail);
        applyInterfaceDetail();
      },
      { signal: lifetime.signal },
    );
  resize();
  session.render(0);
  await session.renderer.device.queue.onSubmittedWorkDone();
  if (disposed) {
    session.dispose();
    return;
  }
  const now = performance.now();
  startupMetrics = {
    descriptorAndRuntimeCheckMs,
    ...source.stageTimings,
    ...session.kernel.benchmarkInfo(),
    renderer: session.renderer.benchmarkInfo(),
    firstReliableControlMs: now - startup,
    stream: source.streamMetrics,
  };
  phase = "ready";
  session.start(now);
  ui.button("Interact", "Interact (E)", () => {
    session!.interact();
  });
  ui.button("Restart", "Restart (R)", () => {
    session!.reset();
  });
  if (profile === "character")
    ui.button("View", "First or third person (V)", () => {
      session!.perspective(
        session!.characterCamera.perspective === "first" ? "third" : "first",
      );
    });
  if (source.prepared?.dayGameplay?.objects.length) {
    for (const spell of DRUID_ABILITIES.filter(
      (entry) =>
        entry.id !== "destroy_castle" ||
        source.prepared?.dayGameplay?.manifest.targetGroups?.castle.length,
    ))
      ui.button(spell.key, spell.name, async () => {
        await session!.cast(spell.id);
      });
    ui.button("Stop", "Stop spell (Escape)", () => {
      selectedSpell = undefined;
      session!.cancelCast();
    });
  }
  ui.button("Fullscreen", "Fullscreen", async () => {
    if (document.fullscreenElement) await document.exitFullscreen();
    else await ui.canvas.requestFullscreen();
  });
  ui.message(objective());
  frame = requestAnimationFrame(draw);
}

/** Local diagnostics grant no editor or source-code mutation capability. */
Object.assign(window, {
  aethyrforgePlayer: {
    status: () => ({
      phase,
      startup: startupMetrics,
      ...(session ? session.inspect() : {}),
      detail: detail?.report() ?? null,
      ...(failure ? { failure } : {}),
    }),
    input: (value: Parameters<WorldSession["input"]>[0]) =>
      session?.input(value),
    clearInput: release,
    benchmark: async (options: Parameters<WorldSession["benchmark"]>[0]) => {
      if (!session || capturing)
        throw new Error("Wait for the live world before measuring frames.");
      if (!options.canonical) return session.benchmark(options);
      const priorPaused = paused;
      paused = true;
      capturing = true;
      release();
      try { return await session.benchmark(options, import.meta.env.VITE_PLAYER_DIAGNOSTICS === "1"); }
      finally { capturing = false; paused = priorPaused; }
    },
    scenario: (mode: "day" | "night") => session?.scenario(mode),
    referenceShot: (id: string) => session?.referenceShot(id),
    restoreGameplay: () => session?.restoreGameplay(),
    aim: () => session?.aim(),
    cancelCast: () => session?.cancelCast(),
    cast: (ability: Parameters<WorldSession["cast"]>[0]) =>
      session?.cast(ability),
    interact: () => session?.interact(),
    reset: () => session?.reset(),
    perspective: (value: "first" | "third") => session?.perspective(value),
    setCamera: (eye: V3, target: V3, mode?: "walk" | "fly") =>
      session?.setCamera(eye, target, mode),
    pause: (value = true) => {
      paused = value;
      release();
    },
    validateCandidate: (value: unknown) =>
      verifyPlayerArtifact(value, {
        profile: session?.source.artifact.profile,
        runtime: session?.source.artifact.runtime,
        kernel: session?.source.artifact.kernel,
      }),
    capture: async (value: unknown) => {
      if (!session) throw new Error("Wait for the world to open.");
      if (capturing)
        throw new Error("Let the current screenshot finish first.");
      if (!value || typeof value !== "object")
        throw new Error("Choose complete screenshot settings.");
      const { width, height, ...frameOptions } = value as Record<
        string,
        unknown
      >;
      const size = validateFilmCaptureSize({ width, height });
      const options = validateFrameCaptureOptions(frameOptions),
        priorPaused = paused;
      paused = true;
      capturing = true;
      release();
      const renderer = session.renderer,
        old = { width: renderer.width, height: renderer.height };
      // Screenshots are taken at full detail, whatever the device was doing.
      session.holdDetail(1);
      try {
        renderer.resize(size.width, size.height);
        const result = await renderer.capturePixelsDetailed(options.time ?? 0, {
          camera: cameraForCapture(session.camera, options.camera),
          env: environmentForCapture(
            session.environment,
            options.exposure ?? 1,
          ),
          actorReference: options.actorReference,
        });
        const bytes = new Uint8Array(result.pixels);
        let binary = "";
        for (let i = 0; i < bytes.length; i += 8192)
          binary += String.fromCharCode(...bytes.subarray(i, i + 8192));
        const { pixels, ...metadata } = result;
        return { metadata, base64: btoa(binary) };
      } finally {
        capturing = false;
        if (!disposed) {
          session.holdDetail(undefined);
          renderer.resize(old.width, old.height);
          paused = priorPaused;
          resize();
        }
      }
    },
    close,
  },
});
void start().catch((error) => {
  if (disposed) return;
  fail(error);
  session?.dispose();
  session = undefined;
  phase = "error";
  message(error);
});
