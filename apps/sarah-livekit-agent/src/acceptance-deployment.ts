import { execFile } from "node:child_process";
import { promisify } from "node:util";
import { AccessToken } from "livekit-server-sdk";

const runFile = promisify(execFile);
const IMAGE = "us-central1-docker.pkg.dev/openagentsgemini/oa-cloud/sarah-livekit-agent";

export type SarahDeploymentCommand = (file: string, args: readonly string[]) => Promise<string>;

const productionCommand: SarahDeploymentCommand = async (file, args) => {
  const result = await runFile(file, [...args], { maxBuffer: 1024 * 1024 });
  return result.stdout;
};

export const resolveDeployedSarahRevision = async (
  command: SarahDeploymentCommand = productionCommand,
): Promise<string> => {
  await command("gcloud", [
    "container",
    "clusters",
    "get-credentials",
    "oa-livekit-prod",
    "--project",
    "openagentsgemini",
    "--region",
    "us-central1",
  ]);
  const deployment = JSON.parse(
    await command("kubectl", [
      "get",
      "deployment",
      "sarah-livekit-agent",
      "--namespace",
      "livekit-system",
      "--output",
      "json",
    ]),
  ) as {
    metadata?: { generation?: number };
    spec?: { replicas?: number; template?: { spec?: { containers?: Array<{ image?: string }> } } };
    status?: { observedGeneration?: number; updatedReplicas?: number; availableReplicas?: number };
  };
  const replicas = deployment.spec?.replicas;
  const status = deployment.status;
  if (
    replicas === undefined ||
    replicas < 1 ||
    status?.observedGeneration !== deployment.metadata?.generation ||
    status?.updatedReplicas !== replicas ||
    status?.availableReplicas !== replicas
  ) {
    throw new Error("Sarah LiveKit deployment is not fully converged");
  }
  const images = deployment.spec?.template?.spec?.containers?.map((container) => container.image);
  if (images?.length !== 1) throw new Error("Sarah LiveKit deployment has an unexpected image set");
  const match = new RegExp(`^${IMAGE.replaceAll(".", "\\.")}@(sha256:[0-9a-f]{64})$`, "u").exec(
    images[0] ?? "",
  );
  const digest = match?.[1];
  if (digest === undefined) throw new Error("Sarah LiveKit deployment image is not digest pinned");

  const artifacts = JSON.parse(
    await command("gcloud", [
      "artifacts",
      "docker",
      "images",
      "list",
      IMAGE,
      "--project",
      "openagentsgemini",
      "--include-tags",
      "--filter",
      `version=${digest}`,
      "--format",
      "json",
    ]),
  ) as Array<{ version?: string; tags?: string[] }>;
  const revisions = new Set(
    artifacts
      .filter((artifact) => artifact.version === digest)
      .flatMap((artifact) => artifact.tags ?? [])
      .flatMap((tag) => /^source-([0-9a-f]{40})$/u.exec(tag)?.[1] ?? []),
  );
  if (revisions.size !== 1) {
    throw new Error("deployed Sarah image does not resolve to one source revision tag");
  }
  const revision = [...revisions][0];
  if (revision === undefined) throw new Error("deployed Sarah source revision is unavailable");
  return revision;
};

const parseProductionServerKeys = (
  value: string,
): Readonly<{ apiKey: string; apiSecret: string }> => {
  let parsed: unknown;
  try {
    parsed = JSON.parse(value);
  } catch {
    throw new Error("production LiveKit server keys are not valid JSON");
  }
  if (typeof parsed !== "object" || parsed === null || Array.isArray(parsed)) {
    throw new Error("production LiveKit server keys have an invalid shape");
  }
  const record = parsed as Record<string, unknown>;
  const keys = Object.keys(record).sort();
  if (
    JSON.stringify(keys) !== JSON.stringify(["api_key", "api_secret", "keys_yaml"]) ||
    typeof record.api_key !== "string" ||
    typeof record.api_secret !== "string" ||
    typeof record.keys_yaml !== "string" ||
    !/^[A-Za-z0-9_-]{12,128}$/u.test(record.api_key) ||
    !/^[A-Za-z0-9_-]{32,256}$/u.test(record.api_secret) ||
    ![
      `${record.api_key}: ${record.api_secret}`,
      `${record.api_key}: ${record.api_secret}\n`,
    ].includes(record.keys_yaml)
  ) {
    throw new Error("production LiveKit server keys have an invalid shape");
  }
  return { apiKey: record.api_key, apiSecret: record.api_secret };
};

export const mintProductionSubscriberGrant = async (
  roomRef: string,
  subscriberRef: string,
  command: SarahDeploymentCommand = productionCommand,
): Promise<string> => {
  if (!/^oa-sarah-[0-9a-f]{40}$/u.test(roomRef)) {
    throw new Error("acceptance subscriber room is outside the Sarah production namespace");
  }
  if (!/^acceptance-(private|community)-subscriber-[0-9a-f-]{36}$/u.test(subscriberRef)) {
    throw new Error("acceptance subscriber identity is invalid");
  }
  const keys = parseProductionServerKeys(
    await command("gcloud", [
      "secrets",
      "versions",
      "access",
      "latest",
      "--secret",
      "oa-livekit-prod-server-keys",
      "--project",
      "openagentsgemini",
    ]),
  );
  const token = new AccessToken(keys.apiKey, keys.apiSecret, {
    identity: subscriberRef,
    ttl: 60,
  });
  token.addGrant({
    room: roomRef,
    roomJoin: true,
    canPublish: false,
    canSubscribe: true,
    canPublishData: false,
    canUpdateOwnMetadata: false,
    roomAdmin: false,
    roomCreate: false,
    roomList: false,
  });
  return token.toJwt();
};
