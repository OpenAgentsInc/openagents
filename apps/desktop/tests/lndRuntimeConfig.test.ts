import fs from "node:fs";
import os from "node:os";
import path from "node:path";

import { describe, expect, it } from "vitest";

import {
  buildLndConfText,
  buildLndLaunchArgs,
  materializeLndRuntimeConfig,
  resolveLndRuntimePaths,
} from "../src/main/lndRuntimeConfig";

describe("lnd runtime config", () => {
  it("builds deterministic config text and launch args for neutrino mode", () => {
    const paths = resolveLndRuntimePaths({
      userDataPath: "/tmp/openagents-desktop",
      network: "testnet",
    });

    const confText = buildLndConfText(
      {
        userDataPath: "/tmp/openagents-desktop",
        network: "testnet",
        alias: "OpenAgentsDesktop",
        rpcListen: "127.0.0.1:10009",
        restListen: "127.0.0.1:8080",
        p2pListen: "0.0.0.0:9735",
        debugLevel: "info",
        neutrinoPeers: ["a.example:18333", "b.example:18333"],
      },
      paths,
    );

    expect(confText).toContain("[Application Options]");
    expect(confText).toContain("bitcoin.node=neutrino");
    expect(confText).toContain("bitcoin.testnet=true");

    const lines = confText.trim().split(/\r?\n/);
    const peerLines = lines.filter((line) => line.startsWith("neutrino.addpeer="));
    expect(peerLines).toEqual([
      "neutrino.addpeer=a.example:18333",
      "neutrino.addpeer=b.example:18333",
    ]);

    const args = buildLndLaunchArgs({
      configPath: paths.configPath,
    });

    expect(args).toEqual([`--configfile=${paths.configPath}`]);
  });

  it("materializes config into app-scoped runtime directories", () => {
    const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), "oa-lnd-config-"));
    try {
      const result = materializeLndRuntimeConfig({
        userDataPath: tempRoot,
        network: "testnet",
        alias: "OpenAgentsDesktop",
        rpcListen: "127.0.0.1:10009",
        restListen: "127.0.0.1:8080",
        p2pListen: "0.0.0.0:9735",
        debugLevel: "info",
        neutrinoPeers: [],
      });

      expect(result.paths.runtimeDir.startsWith(path.resolve(tempRoot))).toBe(true);
      expect(fs.existsSync(result.paths.configPath)).toBe(true);
      expect(fs.readFileSync(result.paths.configPath, "utf8")).toBe(result.configText);
      expect(result.launchArgs[0]).toBe(`--configfile=${result.paths.configPath}`);
    } finally {
      fs.rmSync(tempRoot, { recursive: true, force: true });
    }
  });
});
