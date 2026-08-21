import { Effect, Option } from "effect";
import { mkdtemp, rm, stat } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, describe, expect, it } from "vitest";

import {
  PendingDeviceAuthorizationStore,
  pendingDeviceAuthorizationStoreFileLayer,
} from "../src/device-authorization-store.js";

const temporaryDirectories: Array<string> = [];

afterEach(async () => {
  await Promise.all(
    temporaryDirectories.splice(0).map((path) => rm(path, { recursive: true, force: true })),
  );
});

describe("pending device authorization store", () => {
  it("persists resumable state in a private file and removes it after completion", async () => {
    // Contract: openagents_cli.agent_device_authorization.v1
    const directory = await mkdtemp(join(tmpdir(), "openagents-cli-device-authorization-"));
    temporaryDirectories.push(directory);
    const path = join(directory, "pending.json");
    const layer = pendingDeviceAuthorizationStoreFileLayer(path);
    const pending = {
      origin: "https://openagents.com",
      device_code: "secret-device-code",
      user_code: "ABCD-EFGH",
      verification_uri: "https://openagents.com/device",
      verification_uri_complete: "https://openagents.com/device?user_code=ABCD-EFGH",
      expires_at_ms: Date.now() + 600_000,
      interval: 1,
    };

    await Effect.runPromise(
      Effect.gen(function* () {
        const store = yield* PendingDeviceAuthorizationStore;
        yield* store.set(pending);
      }).pipe(Effect.provide(layer)),
    );

    expect((await stat(path)).mode & 0o777).toBe(0o600);

    const loaded = await Effect.runPromise(
      Effect.gen(function* () {
        const store = yield* PendingDeviceAuthorizationStore;
        return yield* store.get(pending.origin);
      }).pipe(Effect.provide(pendingDeviceAuthorizationStoreFileLayer(path))),
    );
    expect(Option.getOrUndefined(loaded)).toEqual(pending);

    await Effect.runPromise(
      Effect.gen(function* () {
        const store = yield* PendingDeviceAuthorizationStore;
        yield* store.remove(pending.origin);
      }).pipe(Effect.provide(pendingDeviceAuthorizationStoreFileLayer(path))),
    );

    await expect(stat(path)).rejects.toMatchObject({ code: "ENOENT" });
  });
});
