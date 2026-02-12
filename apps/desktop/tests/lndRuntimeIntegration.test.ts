import fs from "node:fs";

import { Effect } from "effect";
import { describe, expect, it } from "@effect/vitest";

import { LndRuntimeManagerService } from "../src/main/lndRuntimeManager";
import { makeLndRuntimeHarness } from "./support/lndRuntimeHarness";

describe("lnd runtime integration with fake transport", () => {
  it.effect("spawns with deterministic args and keeps renderer-safe logs", () => {
    const harness = makeLndRuntimeHarness({
      network: "testnet",
      alias: "OpenAgentsDesktop",
      neutrinoPeers: ["peer-b.test:18333", "peer-a.test:18333"],
    });

    return Effect.gen(function* () {
      const manager = yield* LndRuntimeManagerService;

      yield* manager.start();

      expect(harness.spawnCalls.length).toBe(1);
      const firstSpawn = harness.spawnCalls[0];
      expect(firstSpawn?.args[0]?.startsWith("--configfile=")).toBe(true);
      expect(firstSpawn?.args.length).toBe(1);
      expect(firstSpawn?.cwd.endsWith("/lnd/testnet")).toBe(true);

      const configPath = firstSpawn?.args[0]?.replace("--configfile=", "");
      expect(configPath).toBeDefined();
      const configText = fs.readFileSync(configPath as string, "utf8");
      expect(configText).toContain("bitcoin.node=neutrino");
      expect(configText).toContain("neutrino.addpeer=peer-a.test:18333");
      expect(configText).toContain("neutrino.addpeer=peer-b.test:18333");

      harness.controllers[0]?.emitStdout("wallet password=super-secret");
      harness.controllers[0]?.emitStderr("macaroon=super-secret");

      const logs = yield* manager.logs();
      const serialized = JSON.stringify(logs);
      expect(serialized.includes("super-secret")).toBe(false);

      yield* manager.stop();
    }).pipe(
      Effect.provide(harness.layer),
      Effect.ensuring(Effect.sync(harness.cleanup)),
    );
  });
});
