import { describe, expect, test, vi } from "vite-plus/test";
import {
  mintProductionSubscriberGrant,
  resolveDeployedSarahRevision,
} from "./acceptance-deployment.js";

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

describe("Sarah LiveKit acceptance subscriber grant", () => {
  test("mints a short-lived subscribe-only token after the exact room is known", async () => {
    const apiKey = "APIacceptance123";
    const apiSecret = "s".repeat(48);
    const command = vi.fn(async () =>
      JSON.stringify({
        api_key: apiKey,
        api_secret: apiSecret,
        keys_yaml: `${apiKey}: ${apiSecret}`,
      }),
    );
    const roomRef = `oa-sarah-${"a".repeat(40)}`;
    const subscriberRef = "acceptance-private-subscriber-123e4567-e89b-42d3-a456-426614174000";

    const grant = await mintProductionSubscriberGrant(roomRef, subscriberRef, command);
    const payloadPart = grant.split(".")[1];
    expect(payloadPart).toBeDefined();
    const payload = JSON.parse(Buffer.from(payloadPart ?? "", "base64url").toString("utf8"));
    expect(payload.sub).toBe(subscriberRef);
    expect(payload.video).toEqual({
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
    expect(command).toHaveBeenCalledWith("gcloud", [
      "secrets",
      "versions",
      "access",
      "latest",
      "--secret",
      "oa-livekit-prod-server-keys",
      "--project",
      "openagentsgemini",
    ]);
  });

  test("rejects a subscriber token outside the exact production room namespace", async () => {
    await expect(
      mintProductionSubscriberGrant("other-room", "subscriber", vi.fn()),
    ).rejects.toThrow("outside the Sarah production namespace");
  });
});

describe("production subscriber grant identity bound", () => {
  const roomRef = `oa-sarah-${"a".repeat(40)}`;
  const uuid = "123e4567-e89b-42d3-a456-426614174000";
  const reject = () => Promise.reject(new Error("no secret read should be reached"));

  test("admits the drill subscriber beside the acceptance one", async () => {
    for (const ref of [
      `acceptance-private-subscriber-${uuid}`,
      `acceptance-community-subscriber-${uuid}`,
      `drill-private-subscriber-${uuid}`,
      `drill-community-subscriber-${uuid}`,
    ]) {
      // Reaching the secret read proves the identity passed the bound; the
      // rejection after it is this test refusing to touch Secret Manager.
      // eslint-disable-next-line no-await-in-loop
      await expect(mintProductionSubscriberGrant(roomRef, ref, reject)).rejects.toThrow(
        "no secret read should be reached",
      );
    }
  });

  test("still refuses an identity outside the two bounded prefixes", async () => {
    for (const ref of [
      `operator-private-subscriber-${uuid}`,
      `drill-private-subscriber-not-a-uuid`,
      `drill-owner-subscriber-${uuid}`,
      "principal.sarah",
    ]) {
      // eslint-disable-next-line no-await-in-loop
      await expect(mintProductionSubscriberGrant(roomRef, ref, reject)).rejects.toThrow(
        "acceptance subscriber identity is invalid",
      );
    }
  });
});
