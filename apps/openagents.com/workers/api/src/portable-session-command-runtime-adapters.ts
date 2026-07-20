import {
  PortableCommittedCheckpointArtifactResolver,
  PostgresPortablePhaseOperationStore,
  createPortableCheckpointGoogleKmsCustodyDecryptor,
  createPortableCommandInstallationPortResolver,
  createPostgresManagedPortableCommandInstallationPortResolver,
  createPostgresOwnerLocalPortableCommandInstallationPortResolver,
  createPostgresPortableCommandDestinationRunnerSessionResolver,
  createProductionPortableCommandBrokerFactory,
  type SyncSql,
} from "@openagentsinc/khala-sync-server";

import type { ArtifactsEnv } from "./artifacts-binding";
import type { OpenAgentsWorkerConfigEnv } from "./config";
import {
  makeGoogleCloudKmsDekClient,
  makeGoogleCloudWorkloadIdentityAccessTokenProvider,
} from "./google-cloud-kms";
import { makePortableCheckpointArtifactService } from "./portable-checkpoint-artifact-service";
import {
  portableCheckpointArtifactBucketForEnv,
  readPortableCheckpointArtifactAuthority,
} from "./portable-checkpoint-artifact-routes";
import { makePortablePrivateCheckpointArtifactReader } from "./portable-private-checkpoint-artifact-reader";
import type { PortableSessionCommandRuntimeAdapters } from "./portable-session-command-dispatch-scheduled";

const SAFE_REF = /^[A-Za-z0-9][A-Za-z0-9._:-]{2,255}$/u;
const GCS_BUCKET = /^[a-z0-9][a-z0-9._-]{1,220}[a-z0-9]$/u;

export type PortableSessionCommandRuntimeAdapterEnv = OpenAgentsWorkerConfigEnv &
  ArtifactsEnv;

const workloadIdentityTokenProvider =
  makeGoogleCloudWorkloadIdentityAccessTokenProvider();

const exactHttpsBaseUrl = (value: string | undefined): string | undefined => {
  if (value === undefined || value !== value.trim()) return undefined;
  try {
    const url = new URL(value);
    if (
      url.protocol !== "https:" ||
      url.username !== "" ||
      url.password !== "" ||
      url.pathname !== "/" ||
      url.search !== "" ||
      url.hash !== "" ||
      value !== url.origin
    ) {
      return undefined;
    }
    return value;
  } catch {
    return undefined;
  }
};

const privateValue = (value: string | undefined, minimum: number): string | undefined =>
  value !== undefined && value === value.trim() && value.length >= minimum
    ? value
    : undefined;

/**
 * Builds the complete production command runtime. Partial configuration does
 * not create a broker, storage fallback, custody fallback, or target adapter.
 */
export const makePortableSessionCommandRuntimeAdapters = (
  env: PortableSessionCommandRuntimeAdapterEnv,
  sql: SyncSql,
): PortableSessionCommandRuntimeAdapters | undefined => {
  const grantAuthorityBaseUrl = exactHttpsBaseUrl(
    env.PORTABLE_SESSION_COMMAND_GRANT_AUTHORITY_BASE_URL,
  );
  const managedInstallationBaseUrl = exactHttpsBaseUrl(
    env.PORTABLE_SESSION_COMMAND_MANAGED_INSTALLATION_BASE_URL,
  );
  const serviceBearer = privateValue(
    env.PORTABLE_SESSION_COMMAND_SERVICE_BEARER,
    16,
  );
  const bucket = env.ARTIFACTS_GCS_BUCKET;
  const gcsAccessKeyId = privateValue(env.ARTIFACTS_GCS_HMAC_ACCESS_KEY_ID, 8);
  const gcsSecret = privateValue(env.ARTIFACTS_GCS_HMAC_SECRET, 16);
  const gcsEndpoint =
    env.ARTIFACTS_GCS_ENDPOINT === undefined
      ? "https://storage.googleapis.com"
      : exactHttpsBaseUrl(env.ARTIFACTS_GCS_ENDPOINT);
  const kmsResource = env.PORTABLE_CHECKPOINT_KMS_KEY_RESOURCE;
  const kmsKeyRef = env.PORTABLE_CHECKPOINT_KMS_KEY_REF;
  if (
    grantAuthorityBaseUrl === undefined ||
    managedInstallationBaseUrl === undefined ||
    serviceBearer === undefined ||
    bucket === undefined ||
    !GCS_BUCKET.test(bucket) ||
    gcsAccessKeyId === undefined ||
    gcsSecret === undefined ||
    gcsEndpoint === undefined ||
    kmsResource === undefined ||
    kmsResource !== kmsResource.trim() ||
    kmsKeyRef === undefined ||
    !SAFE_REF.test(kmsKeyRef)
  ) {
    return undefined;
  }

  try {
    const installationPorts = createPortableCommandInstallationPortResolver({
      ownerLocal: createPostgresOwnerLocalPortableCommandInstallationPortResolver({ sql }),
      openAgentsManaged: createPostgresManagedPortableCommandInstallationPortResolver({
        sql,
        baseUrl: managedInstallationBaseUrl,
        bearerToken: serviceBearer,
      }),
    });
    const brokerFactory = createProductionPortableCommandBrokerFactory({
      grantAuthority: {
        baseUrl: grantAuthorityBaseUrl,
        serviceBearer,
      },
      installationPorts,
      destinationRunnerSessions:
        createPostgresPortableCommandDestinationRunnerSessionResolver(sql),
    });

    const artifactService = makePortableCheckpointArtifactService({
      bucket: portableCheckpointArtifactBucketForEnv(env),
      readAuthority: (pylonRef, targetRef, operationRef) =>
        readPortableCheckpointArtifactAuthority(
          sql,
          new PostgresPortablePhaseOperationStore(sql),
          { pylonRef, targetRef, operationRef },
        ),
    });
    const objects = makePortablePrivateCheckpointArtifactReader({
      sql,
      artifacts: artifactService,
    });
    const kms = makeGoogleCloudKmsDekClient({
      cryptoKeyResource: kmsResource,
      tokenProvider: workloadIdentityTokenProvider,
    });
    const custody = createPortableCheckpointGoogleKmsCustodyDecryptor({
      authority: {
        unwrapDek: async (input) => {
          if (input.keyRef !== kmsKeyRef) throw new Error("kms_key_ref_mismatch");
          return kms.unwrapDek(
            input.wrappedDek,
            input.additionalAuthenticatedData,
          );
        },
      },
    });
    const checkpointArtifacts = new PortableCommittedCheckpointArtifactResolver({
      sql,
      objects,
      custody,
    });
    return { brokerFactory, checkpointArtifacts };
  } catch {
    return undefined;
  }
};
