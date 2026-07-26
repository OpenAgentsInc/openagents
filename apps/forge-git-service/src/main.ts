import { mkdir, stat } from "node:fs/promises";

import { Runtime } from "@openagentsinc/runtime-platform";
import { layerGcs } from "@openagentsinc/oa-infra/blob-store-gcs";
import { Config, Effect, Layer, ManagedRuntime } from "effect";

import { ForgeGitAdmission, layerDistributedAdmission } from "./admission.js";
import { layerAuth } from "./auth.js";
import { ForgeGitConfiguration, layerConfiguration } from "./config.js";
import { layerDatabase } from "./database.js";
import { layerProjection } from "./projection.js";
import { ForgeGitProjector, layerProjector } from "./projector.js";
import { layerRepository } from "./repository.js";
import { ForgeGitRepository } from "./repository.js";
import { ForgeGitRelayOutbox, layerRelayOutbox } from "./outbox.js";
import { routeRequest } from "./routes.js";
import { layerForgeWebRead } from "./web-read.js";
import { layerForgeWebReadPolicy } from "./web-read-policy.js";

const configurationLayer = layerConfiguration;
const databaseLayer = layerDatabase.pipe(Layer.provide(configurationLayer));
const authLayer = layerAuth.pipe(Layer.provide(configurationLayer));
const admissionLayer = layerDistributedAdmission.pipe(
  Layer.provide(Layer.mergeAll(databaseLayer, configurationLayer)),
);
const projectionLayer = layerProjection.pipe(Layer.provide(databaseLayer));
const outboxLayer = layerRelayOutbox.pipe(Layer.provide(Layer.mergeAll(configurationLayer, databaseLayer)));
const repositoryLayer = layerRepository.pipe(
  Layer.provide(Layer.mergeAll(configurationLayer, layerGcs)),
);
const projectorLayer = layerProjector.pipe(Layer.provide(Layer.mergeAll(admissionLayer, repositoryLayer)));
const webReadLayer = layerForgeWebRead.pipe(Layer.provide(configurationLayer));
const webReadPolicyLayer = layerForgeWebReadPolicy.pipe(Layer.provide(configurationLayer));
const applicationLayer = Layer.mergeAll(
  configurationLayer,
  admissionLayer,
  authLayer,
  projectionLayer,
  outboxLayer,
  projectorLayer,
  repositoryLayer,
  webReadLayer,
  webReadPolicyLayer,
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

  // Relay visibility is derived after the durable Git mutation. This loop is
  // deliberately fail-soft: pending rows remain retryable when the relay is
  // down and no transport success is rewritten as a relay success.
  const outboxTimer = setInterval(() => {
    void applicationRuntime
      .runPromise(Effect.gen(function* () {
        const outbox = yield* ForgeGitRelayOutbox;
        yield* outbox.drain();
      }))
      .catch((error) => console.error("Forge relay outbox drain failed.", error));
  }, 60_000);
  outboxTimer.unref();

  const purgatoryTimer = setInterval(() => {
    void applicationRuntime
      .runPromise(Effect.gen(function* () {
        const admission = yield* ForgeGitAdmission;
        const projector = yield* ForgeGitProjector;
        const nowIso = new Date().toISOString();
        const repositories = yield* admission.listAdmittedRepositories();
        yield* Effect.forEach(
          repositories,
          (repository) => Effect.gen(function* () {
            yield* projector.reconcile({ ...repository, nowIso });
            const git = yield* ForgeGitRepository;
            const refs = yield* admission.dueNostrRefGc({ ...repository, nowIso });
            if (refs.length === 0) return;
            yield* git.deleteRefs({ ...repository, refNames: refs });
            yield* admission.markNostrRefsDeleted({ ...repository, refNames: refs });
          }),
          { concurrency: 4, discard: true },
        );
      }))
      .catch((error) => console.error("Forge purgatory reconciliation failed.", error));
  }, 60_000);
  purgatoryTimer.unref();

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
