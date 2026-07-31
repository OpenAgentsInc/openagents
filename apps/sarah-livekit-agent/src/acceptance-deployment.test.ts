import { describe, expect, test, vi } from "vite-plus/test";
import { resolveDeployedSarahRevision } from "./acceptance-deployment.js";

describe("Sarah LiveKit deployed revision resolver", () => {
  test("binds the source revision tag to the converged deployment digest", async () => {
    const revision = "a".repeat(40);
    const digest = `sha256:${"b".repeat(64)}`;
    const command = vi.fn(async (file: string, args: readonly string[]) => {
      if (file === "gcloud" && args[1] === "clusters") return "";
      if (file === "kubectl") {
        return JSON.stringify({
          metadata: { generation: 4 },
          spec: {
            replicas: 2,
            template: {
              spec: {
                containers: [
                  {
                    image: `us-central1-docker.pkg.dev/openagentsgemini/oa-cloud/sarah-livekit-agent@${digest}`,
                  },
                ],
              },
            },
          },
          status: { observedGeneration: 4, updatedReplicas: 2, availableReplicas: 2 },
        });
      }
      return JSON.stringify([{ version: digest, tags: [`source-${revision}`] }]);
    });

    await expect(resolveDeployedSarahRevision(command)).resolves.toBe(revision);
  });

  test("refuses an unconverged deployment or ambiguous source tags", async () => {
    const digest = `sha256:${"c".repeat(64)}`;
    const command = vi.fn(async (file: string, args: readonly string[]) => {
      if (file === "gcloud" && args[1] === "clusters") return "";
      if (file === "kubectl") {
        return JSON.stringify({
          metadata: { generation: 2 },
          spec: {
            replicas: 1,
            template: {
              spec: {
                containers: [
                  {
                    image: `us-central1-docker.pkg.dev/openagentsgemini/oa-cloud/sarah-livekit-agent@${digest}`,
                  },
                ],
              },
            },
          },
          status: { observedGeneration: 1, updatedReplicas: 1, availableReplicas: 1 },
        });
      }
      return "[]";
    });
    await expect(resolveDeployedSarahRevision(command)).rejects.toThrow("not fully converged");
  });
});
