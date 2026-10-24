/** Launch the standalone walking zone on its own local origins. */
import { readFile } from "node:fs/promises";
import { resolve, join } from "node:path";
import { spawn, spawnSync } from "node:child_process";
import { devOrigins } from "./dev-origins.mjs";

process.env.VITE_WORLD_PROFILE = "open-ground";
const root = process.env.AETHYRFORGE_PRIVATE_ASSETS;
if (root && !process.env.VITE_MMORPG_URL) {
  const scene = JSON.parse(
    await readFile(join(resolve(root), "scene-ground.json"), "utf8"),
  );
  if (scene.mmorpgURL) {
    if (
      !/^\/private-assets\/mmorpg-world-[0-9a-f]{64}\.json$/.test(
        scene.mmorpgURL,
      )
    )
      throw Error("Choose a prepared local game world.");
    const origin = devOrigins();
    const worldPortText = process.env.AETHYRFORGE_MMORPG_PORT ?? "55319";
    if (
      !/^[0-9]{4,5}$/.test(worldPortText) ||
      Number(worldPortText) < 1024 ||
      Number(worldPortText) > 65535
    )
      throw Error("Choose a loopback world port from 1024 to 65535.");
    const worldOrigin = `http://127.0.0.1:${worldPortText}`;
    let occupied = false;
    try {
      await fetch(`${worldOrigin}/healthz`, {
        signal: AbortSignal.timeout(500),
      });
      occupied = true;
    } catch {}
    if (occupied)
      throw Error(
        "The world port is already in use. Close its server or choose another port.",
      );
    const build = spawnSync("bash", ["tools/build-server.sh"], {
      encoding: "utf8",
      stdio: ["ignore", "pipe", "inherit"],
    });
    if (build.status !== 0) throw Error("The world server could not build.");
    const binary = build.stdout.trim();
    const server = spawn(
      binary,
      [
        "--listen",
        `127.0.0.1:${worldPortText}`,
        "--world",
        join(resolve(root), scene.mmorpgURL.slice("/private-assets/".length)),
        "--allow-origin",
        origin.editorOrigin,
        "--allow-origin",
        origin.previewOrigin,
      ],
      { stdio: "inherit" },
    );
    const stop = () => {
      if (server.exitCode === null) server.kill("SIGTERM");
    };
    process.on("SIGTERM", stop);
    process.on("SIGINT", stop);
    process.on("exit", stop);
    let failed;
    server.on("error", (error) => {
      failed = error;
    });
    server.on("exit", (code) => {
      failed = Error(`The world server stopped (${code}).`);
    });
    let ready = false;
    for (let attempt = 0; attempt < 150; attempt++) {
      if (failed) throw failed;
      try {
        const response = await fetch(`${worldOrigin}/healthz`);
        ready = response.ok;
      } catch {}
      if (ready) break;
      await new Promise((resolve) => setTimeout(resolve, 100));
    }
    if (!ready) {
      stop();
      throw Error("The world server did not open.");
    }
    process.env.VITE_MMORPG_URL = `ws://127.0.0.1:${worldPortText}/v1/worlds/open-ground/ws`;
    console.log(`Shared game world: ${worldOrigin}`);
  }
}
await import("./dev.mjs");
