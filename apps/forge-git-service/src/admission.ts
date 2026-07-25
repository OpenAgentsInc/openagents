import { createHash } from "node:crypto";

import { Context, Effect, Layer, Semaphore } from "effect";

import { ForgeGitDatabase } from "./database.js";

export interface ForgeGitAdmissionShape {
  readonly withReceiveLease: <A, E, R>(
    repositoryKey: string,
    effect: Effect.Effect<A, E, R>,
  ) => Effect.Effect<A, E, R>;
}

export class ForgeGitAdmission extends Context.Service<ForgeGitAdmission, ForgeGitAdmissionShape>()(
  "@openagentsinc/forge-git-service/Admission",
) {}

export const layerAdmission = Layer.sync(ForgeGitAdmission, () => {
  const leases = new Map<string, Semaphore.Semaphore>();

  return ForgeGitAdmission.of({
    withReceiveLease: (repositoryKey, effect) => {
      const semaphore = leases.get(repositoryKey) ?? Semaphore.makeUnsafe(1);
      leases.set(repositoryKey, semaphore);
      return semaphore.withPermit(effect);
    },
  });
});

const advisoryLockKey = (repositoryKey: string): string => {
  const digest = createHash("sha256").update(repositoryKey).digest();
  return BigInt.asIntN(64, digest.readBigUInt64BE(0)).toString();
};

export const layerDistributedAdmission = Layer.effect(
  ForgeGitAdmission,
  Effect.gen(function* () {
    const database = yield* ForgeGitDatabase;
    const leases = new Map<string, Semaphore.Semaphore>();

    return ForgeGitAdmission.of({
      withReceiveLease: (repositoryKey, effect) => {
        const semaphore = leases.get(repositoryKey) ?? Semaphore.makeUnsafe(1);
        leases.set(repositoryKey, semaphore);
        const lockKey = advisoryLockKey(repositoryKey);

        return semaphore.withPermit(
          Effect.acquireUseRelease(
            Effect.tryPromise(async () => {
              const sql = await database.sql.reserve();
              try {
                await sql`SELECT pg_advisory_lock(${lockKey}::bigint)`;
                return sql;
              } catch (error) {
                sql.release();
                throw error;
              }
            }),
            () => effect,
            (sql) =>
              Effect.promise(async () => {
                try {
                  await sql`SELECT pg_advisory_unlock(${lockKey}::bigint)`;
                } finally {
                  sql.release();
                }
              }),
          ),
        );
      },
    });
  }),
);
