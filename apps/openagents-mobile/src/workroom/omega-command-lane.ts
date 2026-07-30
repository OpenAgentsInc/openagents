import { getRandomBytes, randomUUID } from "expo-crypto";
import { Effect } from "effect";

import {
  ISSUE31_COMMAND_SCHEMA_V2,
  createIssue31PrivateGiftWrap,
  decodeIssue31CommandRecordV2,
  type Issue31NostrSigner,
  type Issue31SignedNostrEvent,
} from "@openagentsinc/sarah/issue31-nostr";

import { openExpoOmegaDeviceBridgeStore } from "./omega-device-bridge-client";
import { openExpoIssue31DeviceIdentity } from "./issue31-device-key-vault";

const OMEGA_HOST_REF = "omega.host.local";
const OMEGA_RELAY_URL = "wss://relay.openagents.com";
const COMMAND_LIFETIME_SECONDS = 60;

type RelaySocket = WebSocket;

const publishWithReceipt = async (
  event: Issue31SignedNostrEvent,
  signer: Issue31NostrSigner,
): Promise<void> =>
  new Promise<void>((resolve, reject) => {
    const socket: RelaySocket = new WebSocket(OMEGA_RELAY_URL);
    let settled = false;
    let publishTimer: ReturnType<typeof setTimeout> | null = null;
    let authenticationEventId: string | null = null;

    const finish = (error?: Error): void => {
      if (settled) return;
      settled = true;
      if (publishTimer !== null) clearTimeout(publishTimer);
      clearTimeout(deadline);
      socket.close();
      if (error === undefined) resolve();
      else reject(error);
    };
    const publish = (): void => {
      if (settled || socket.readyState !== WebSocket.OPEN) return;
      socket.send(JSON.stringify(["EVENT", event]));
    };
    const deadline = setTimeout(
      () => finish(new Error("The Omega relay did not acknowledge the desktop command.")),
      10_000,
    );

    socket.addEventListener("open", () => {
      publishTimer = setTimeout(publish, 250);
    });
    socket.addEventListener("message", (message) => {
      if (typeof message.data !== "string") return;
      let frame: unknown;
      try {
        frame = JSON.parse(message.data) as unknown;
      } catch {
        return;
      }
      if (!Array.isArray(frame)) return;
      if (frame[0] === "AUTH" && typeof frame[1] === "string") {
        if (publishTimer !== null) {
          clearTimeout(publishTimer);
          publishTimer = null;
        }
        void signer
          .signEvent({
            kind: 22_242,
            created_at: Math.floor(Date.now() / 1_000),
            tags: [
              ["relay", OMEGA_RELAY_URL],
              ["challenge", frame[1]],
            ],
            content: "",
          })
          .then((authentication) => {
            authenticationEventId = authentication.id;
            socket.send(JSON.stringify(["AUTH", authentication]));
            publish();
          })
          .catch(() => finish(new Error("The Omega relay authentication could not be signed.")));
        return;
      }
      if (
        frame[0] === "OK" &&
        typeof frame[1] === "string" &&
        typeof frame[2] === "boolean"
      ) {
        if (frame[1] === authenticationEventId) {
          if (!frame[2]) finish(new Error("The Omega relay rejected device authentication."));
          return;
        }
        if (frame[1] === event.id) {
          if (frame[2]) finish();
          else finish(new Error("The Omega relay rejected the desktop command."));
        }
      }
    });
    socket.addEventListener("error", () =>
      finish(new Error("The Omega relay connection failed.")),
    );
    socket.addEventListener("close", () => {
      if (!settled) finish(new Error("The Omega relay closed before accepting the command."));
    });
  });

export const submitOmegaAgentThreadMessage = async (input: Readonly<{
  threadRef: string;
  text: string;
  disposition?: "enqueue" | "steer";
}>): Promise<Readonly<{ ok: boolean; summary: string }>> => {
  const text = input.text.trim();
  if (text === "" || input.threadRef.trim() === "") {
    return { ok: false, summary: "No paired Omega desktop thread is available." };
  }
  const stored = await Effect.runPromise(openExpoOmegaDeviceBridgeStore().load());
  const grant = stored?.grant;
  if (grant === null || grant === undefined || grant.generation === undefined) {
    return {
      ok: false,
      summary: "Pair this phone with the updated Omega desktop before sending commands.",
    };
  }
  const identity = await openExpoIssue31DeviceIdentity();
  try {
    if (identity.publicKeyHex !== grant.devicePublicKeyHex) {
      return { ok: false, summary: "The paired Omega desktop grant belongs to another device key." };
    }
    const issuedAt = Math.floor(Date.now() / 1_000);
    const record = decodeIssue31CommandRecordV2({
      schema: ISSUE31_COMMAND_SCHEMA_V2,
      recordType: "command_intent",
      hostRef: OMEGA_HOST_REF,
      hostPublicKeyHex: grant.hostPublicKeyHex,
      devicePublicKeyHex: grant.devicePublicKeyHex,
      grantRef: grant.grantRef,
      idempotencyRef: `idempotency.issue31.mobile.${randomUUID()}`,
      expectedGeneration: grant.generation,
      arguments: {
        kind: "agent_thread_message",
        actionRef: "action.issue31.omega.agent_thread_message",
        threadRef: input.threadRef,
        text,
        disposition: input.disposition ?? "enqueue",
      },
      issuedAt,
      expiresAt: issuedAt + COMMAND_LIFETIME_SECONDS,
    });
    const giftWrap = await createIssue31PrivateGiftWrap({
      signer: identity.signer,
      recipientPublicKeyHex: grant.hostPublicKeyHex,
      record,
      randomSecretKey: () => getRandomBytes(32),
      createdAt: issuedAt,
      sealCreatedAt: Math.max(0, issuedAt - 1),
      wrapCreatedAt: Math.max(0, issuedAt - 2),
    });
    await publishWithReceipt(giftWrap, identity.signer);
    return { ok: true, summary: "Submitted to the paired Omega desktop agent thread." };
  } finally {
    identity.close();
  }
};
