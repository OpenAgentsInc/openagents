import assert from "node:assert/strict";
import test from "node:test";
import { resolve } from "node:path";
import { build } from "esbuild";
import { selectedRendererFeatures as editor } from "../packages/renderer/features.ts";
import { selectedRendererFeatures as player } from "../packages/renderer/player-features.ts";
import { selectedRendererFeatures as room } from "../packages/renderer/room-features.ts";

test("the editor keeps grass support while published profiles omit its shader and implementation", async () => {
  assert.equal(typeof editor.createGrass, "function");
  assert.equal(player.createGrass, undefined);
  assert.equal(room.createGrass, undefined);
  const original = resolve("packages/renderer/features.ts");
  for (const profile of ["editor", "player", "room"] as const) {
    const bundled = await build({
      entryPoints: ["packages/renderer/renderer.ts"],
      bundle: true,
      format: "esm",
      platform: "browser",
      write: false,
      metafile: true,
      logLevel: "silent",
      plugins:
        profile === "editor"
          ? []
          : [
              {
                name: "declared-renderer-features",
                setup(builder) {
                  builder.onResolve(
                    { filter: /(?:^|\/)features\.ts$/ },
                    (args) =>
                      resolve(args.resolveDir, args.path) === original
                        ? {
                            path: resolve(
                              `packages/renderer/${profile}-features.ts`,
                            ),
                          }
                        : undefined,
                  );
                },
              },
            ],
    });
    const modules = Object.keys(bundled.metafile!.inputs);
    for (const module of ["grass.ts", "grass-shader.ts"])
      assert.equal(
        modules.some((path) => path.endsWith(`/` + module)),
        profile === "editor",
        `${profile}: ${module}`,
      );
    assert.equal(
      bundled.outputFiles!.some((file) =>
        file.text.includes("Quick_Grass procedural blades"),
      ),
      profile === "editor",
    );
  }
});
