import { Effect, Layer, Option } from "effect";
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { describe, expect, it } from "vitest";

import { environmentLayerFromValues } from "../src/environment.js";
import {
  PersistedConfiguration,
  persistedConfigurationLayer,
} from "../src/persisted-configuration.js";

describe("persisted CLI configuration", () => {
  it("reads a bounded nonsecret endpoint selection", async () => {
    const directory = await mkdtemp(join(tmpdir(), "openagents-cli-config-"));
    const path = join(directory, "nested", "config.json");
    try {
      await mkdir(dirname(path), { recursive: true });
      await writeFile(path, JSON.stringify({ profile: "local" }), { encoding: "utf8" });
      const layer = persistedConfigurationLayer.pipe(
        Layer.provide(environmentLayerFromValues({ configPath: path })),
      );
      const result = await Effect.runPromise(PersistedConfiguration.pipe(Effect.provide(layer)));
      expect(Option.getOrUndefined(result.profile)).toBe("local");
    } finally {
      await rm(directory, { recursive: true, force: true });
    }
  });
});
