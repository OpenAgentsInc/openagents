import { PostgresManagedSandboxPhase2Store } from "@openagentsinc/khala-sync-server";
import type { SyncSql } from "@openagentsinc/khala-sync-server";

import type { OpenAgentsWorkerEnv } from "./bindings";
import { defaultMakeKhalaSyncSqlClient } from "./khala-sync-push-routes";
import { makeManagedSandboxPrivatePreviewTarget } from "./managed-sandbox-private-preview-target";

export const readManagedSandboxPrivateIngressForAudience = async (
  env: OpenAgentsWorkerEnv,
  input: { audienceRef: string; capabilityRef: string },
) => {
  const connectionString = env.KHALA_SYNC_DB?.connectionString;
  if (typeof connectionString !== "string" || connectionString.length === 0) {
    throw new Error("private_preview_storage_not_configured");
  }
  const client = await defaultMakeKhalaSyncSqlClient(connectionString);
  try {
    return await new PostgresManagedSandboxPhase2Store(
      client.sql as unknown as SyncSql,
    ).readPrivateIngressForAudience(input);
  } finally {
    await client.end();
  }
};

export const useManagedSandboxPrivatePreview = (
  env: OpenAgentsWorkerEnv,
  input: Parameters<ReturnType<typeof makeManagedSandboxPrivatePreviewTarget>["use"]>[0],
) =>
  makeManagedSandboxPrivatePreviewTarget({
    baseUrl: env.OA_MANAGED_SANDBOX_CONTROL_URL?.trim() ?? "",
    bearerToken: env.OA_MANAGED_SANDBOX_CONTROL_TOKEN?.trim() ?? "",
  }).use(input);
