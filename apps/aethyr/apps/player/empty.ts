import "./player.css";
import { playerArtifact, playerChrome } from "./bootstrap.ts";
import {
  verifyPlayerArtifact,
  type PlayerArtifact,
} from "../../packages/runtime/player-artifact.ts";
import { EmptyWorld } from "../../packages/runtime/empty-world.ts";

const ui = playerChrome(),
  started = performance.now();
let phase = "loading",
  world: EmptyWorld | undefined,
  artifact: PlayerArtifact | undefined;
let closed = false;
let startup: Record<string, unknown> = {};
const resize = () => {
  if (!world || closed) return;
  world.resize(
    Math.max(1, Math.floor(innerWidth)),
    Math.max(1, Math.floor(innerHeight)),
  );
  world.render();
};
const close = () => {
  if (closed) return;
  closed = true;
  window.removeEventListener("resize", resize);
  world?.dispose();
  world = undefined;
  phase = "closed";
};
Object.assign(window, {
  aethyrforgePlayer: {
    status: () => ({
      phase,
      profile: "empty",
      worldId: artifact?.worldId,
      sourceIdentity: artifact?.sourceIdentity,
      startup,
      renderer: world?.benchmarkInfo(),
    }),
    validateCandidate: (value: unknown) =>
      verifyPlayerArtifact(value, {
        profile: "empty",
        runtime: artifact?.runtime,
      }),
    capture: async (value: unknown) => {
      if (!world || closed) throw new Error("Wait for the world to open.");
      if (
        !value ||
        typeof value !== "object" ||
        Array.isArray(value) ||
        Object.keys(value).some(
          (key) =>
            ![
              "width",
              "height",
              "time",
              "camera",
              "exposure",
              "transforms",
              "format",
            ].includes(key),
        )
      )
        throw new Error("Choose complete screenshot settings.");
      const { width, height, ...frameOptions } = value as Record<
        string,
        unknown
      >;
      if (frameOptions.format !== undefined && frameOptions.format !== "rgba")
        throw new Error("Choose raw RGBA for this screenshot.");
      const frameBefore = world.benchmarkInfo().frame;
      const pixels = await world.capturePixels(
        width as number,
        height as number,
      );
      const bytes = new Uint8Array(pixels);
      let binary = "";
      for (let i = 0; i < bytes.length; i += 8192)
        binary += String.fromCharCode(...bytes.subarray(i, i + 8192));
      return {
        metadata: {
          width,
          height,
          format: "rgba",
          frameBefore,
          frameAfter: world.benchmarkInfo().frame,
          ignoredOptions: Object.keys(frameOptions),
          scope:
            "Uniform clear color; no camera, time, exposure, objects, or simulation.",
        },
        base64: btoa(binary),
      };
    },
    close,
  },
});
window.addEventListener("pagehide", close, { once: true });
void (async () => {
  const loaded = await playerArtifact("empty");
  if (closed) return;
  artifact = loaded.artifact;
  const opened = await EmptyWorld.create(ui.canvas);
  if (closed) {
    opened.dispose();
    return;
  }
  world = opened;
  resize();
  await opened.device.queue.onSubmittedWorkDone();
  if (closed) return;
  startup = {
    descriptorAndRuntimeCheckMs: loaded.descriptorAndRuntimeCheckMs,
    firstReliableControlMs: performance.now() - started,
    kernel: null,
    gpuAccountedBytes: 0,
    gpuMemoryScope:
      "No owned GPU buffers or textures; browser canvas storage is not measured.",
  };
  phase = "ready";
  ui.message("An empty world");
  window.addEventListener("resize", resize);
})().catch((error) => {
  close();
  phase = "error";
  ui.message(error instanceof Error ? error.message : String(error));
});
