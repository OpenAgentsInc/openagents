import { afterEach, describe, expect, test } from "bun:test";
import { Effect } from "effect";
import { detectBackend } from "./detect.js";

const runDetect = () => Effect.runPromise(detectBackend);

const originalPlatform = process.env.OA_SANDBOX_PLATFORM;
const originalDockerEnv = process.env.OPENAGENTS_DOCKER_AVAILABLE;

afterEach(() => {
  if (originalPlatform === undefined) {
    delete process.env.OA_SANDBOX_PLATFORM;
  } else {
    process.env.OA_SANDBOX_PLATFORM = originalPlatform;
  }

  if (originalDockerEnv === undefined) {
    delete process.env.OPENAGENTS_DOCKER_AVAILABLE;
  } else {
    process.env.OPENAGENTS_DOCKER_AVAILABLE = originalDockerEnv;
  }
});

describe("detectBackend (non-macOS)", () => {
  test("prefers docker when available on linux override", async () => {
    process.env.OA_SANDBOX_PLATFORM = "linux";
    process.env.OPENAGENTS_DOCKER_AVAILABLE = "1";

    const backend = await runDetect();
    expect(backend.name).toBe("docker");
  });

  test("falls back to none when docker unavailable on linux override", async () => {
    process.env.OA_SANDBOX_PLATFORM = "linux";
    process.env.OPENAGENTS_DOCKER_AVAILABLE = "0";

    const backend = await runDetect();
    expect(backend.name).toBe("none");
  });
});
