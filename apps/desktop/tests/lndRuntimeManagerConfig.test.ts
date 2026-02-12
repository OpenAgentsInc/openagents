import { describe, expect, it } from "vitest";

import { defaultLndRuntimeManagerConfig } from "../src/main/lndRuntimeManager";

const makeConfig = (env: NodeJS.ProcessEnv = {}) =>
  defaultLndRuntimeManagerConfig({
    appPath: "/tmp/openagents-desktop-app",
    resourcesPath: "/tmp/openagents-desktop-resources",
    userDataPath: "/tmp/openagents-desktop-user-data",
    isPackaged: false,
    env,
  });

describe("lnd runtime manager config defaults", () => {
  it("uses non-standard loopback p2p listen address by default", () => {
    const config = makeConfig();
    expect(config.p2pListen).toBe("127.0.0.1:19735");
  });

  it("supports OA_DESKTOP_LND_P2P_LISTEN override", () => {
    const config = makeConfig({
      OA_DESKTOP_LND_P2P_LISTEN: "127.0.0.1:29735",
    });
    expect(config.p2pListen).toBe("127.0.0.1:29735");
  });
});
