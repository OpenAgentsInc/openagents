import { mkdir, stat } from "node:fs/promises";

import { Runtime } from "@openagentsinc/runtime-platform";
import { layerGcs } from "@openagentsinc/oa-infra/blob-store-gcs";
import { Config, Effect, Layer, ManagedRuntime } from "effect";

import { layerDistributedAdmission } from "./admission.js";
import { layerAuth } from "./auth.js";
import { ForgeGitConfiguration, layerConfiguration } from "./config.js";
import { layerDatabase } from "./database.js";
import { layerProjection } from "./projection.js";
import { layerRepository } from "./repository.js";
import { routeRequest } from "./routes.js";

const configurationLayer = layerConfiguration;
const databaseLayer = layerDatabase.pipe(Layer.provide(configurationLayer));
const authLayer = layerAuth.pipe(Layer.provide(databaseLayer));
const admissionLayer = layerDistributedAdmission.pipe(Layer.provide(databaseLayer));
const projectionLayer = layerProjection.pipe(Layer.provide(databaseLayer));
const repositoryLayer = layerRepository.pipe(
  Layer.provide(Layer.mergeAll(configurationLayer, layerGcs)),
);
const applicationLayer = Layer.mergeAll(
  admissionLayer,
  authLayer,
  projectionLayer,
  repositoryLayer,
);
const applicationRuntime = ManagedRuntime.make(applicationLayer);

const startup = Effect.gen(function* () {
  const port = yield* Config.number("PORT").pipe(Config.withDefault(8080));
  const configuration = yield* ForgeGitConfiguration;
  yield* Effect.promise(async () => {
    await mkdir(configuration.repositoryRoot, { recursive: true });
    const details = await stat(configuration.repositoryRoot);
    if (!details.isDirectory()) {
      throw new Error("Forge Git repository root is not a directory.");
    }
  });

  Runtime.serve({
    fetch: (request) => {
      const url = new URL(request.url);
      if (url.pathname === "/internal/healthz") {
        return Response.json({
          authority: "bare-repository",
          mirrorAuthority: false,
          ok: true,
          service: "openagents-forge-git",
        });
      }
      return applicationRuntime.runPromise(routeRequest(request));
    },
    port,
  });

  yield* Effect.logInfo("Forge Git service is listening.", {
    port,
    repositoryAuthority: "bare-repository",
  });
}).pipe(Effect.provide(configurationLayer));

void Effect.runPromise(startup).catch((error: unknown) => {
  console.error("Forge Git service failed to start.", error);
  process.exitCode = 1;
});

const shutdown = (): void => {
  void applicationRuntime.dispose().finally(() => {
    process.exit(0);
  });
};

process.once("SIGINT", shutdown);
process.once("SIGTERM", shutdown);
