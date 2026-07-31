import { execFile } from "node:child_process";
import { promisify } from "node:util";

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
