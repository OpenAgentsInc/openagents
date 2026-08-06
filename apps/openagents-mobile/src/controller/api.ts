import type { QueuedCommand, TransportResult } from "@openagentsinc/client-command-outbox";

import {
  saveNativeSessionCredential,
  type NativeSessionCredential,
  type NativeSessionSecureStore,
} from "../auth/native-session-vault";
import {
  decodeControllerBootstrap,
  decodeTransportReceipt,
  MOBILE_CONTROLLER_VERSION,
  type ControllerBootstrap,
  type ControllerTarget,
} from "./contracts";

export const PRO_CONTROLLER_BASE_URL = "https://pro.openagents.com" as const;
export const PRO_CONTROLLER_TOKEN_PATH = "/api/mobile/controller/token" as const;
export const PRO_CONTROLLER_COMMAND_PATH = "/api/mobile/controller/command" as const;
export const PRO_CONTROLLER_DEVICE_PATH = "/api/mobile/controller/device" as const;
export const PRO_CONTROLLER_HARNESS_PATH = "/api/mobile/controller/harness" as const;
const REFRESH_HEADER = "x-openagents-refresh-token";

const headersFor = (credential: NativeSessionCredential): Record<string, string> => ({
  authorization: `Bearer ${credential.accessToken}`,
  [REFRESH_HEADER]: credential.refreshToken,
});

const persistRotation = async (
  store: NativeSessionSecureStore,
  credential: NativeSessionCredential,
  rotated: { access: string; refresh: string; expiresIn: number } | undefined,
): Promise<NativeSessionCredential> => {
  if (rotated === undefined) return credential;
  const next = {
    ownerUserId: credential.ownerUserId,
    accessToken: rotated.access,
    refreshToken: rotated.refresh,
  };
  await saveNativeSessionCredential(store, next);
  return next;
};

export const fetchControllerBootstrap = async (input: {
  readonly credential: NativeSessionCredential;
  readonly secureStore: NativeSessionSecureStore;
  readonly fetch?: typeof globalThis.fetch;
  readonly baseUrl?: string;
}): Promise<Readonly<{ bootstrap: ControllerBootstrap; credential: NativeSessionCredential }>> => {
  const fetcher = input.fetch ?? globalThis.fetch;
  const response = await fetcher(
    `${input.baseUrl ?? PRO_CONTROLLER_BASE_URL}${PRO_CONTROLLER_TOKEN_PATH}`,
    { method: "GET", headers: headersFor(input.credential) },
  );
  if (response.status === 401 || response.status === 403) {
    throw new ControllerApiError("signed_out", "Sign in to control your OpenAgents workspace.");
  }
  if (!response.ok)
    throw new ControllerApiError("unavailable", "The controller is temporarily unavailable.");
  const bootstrap = decodeControllerBootstrap(await response.json());
  if (bootstrap.actor.userId !== input.credential.ownerUserId) {
    throw new ControllerApiError(
      "owner_mismatch",
      "The controller account does not match this device.",
    );
  }
  return {
    bootstrap,
    credential: await persistRotation(input.secureStore, input.credential, bootstrap.rotatedTokens),
  };
};

export class ControllerApiError extends Error {
  constructor(
    readonly reason: "signed_out" | "owner_mismatch" | "unavailable" | "rejected",
    message: string,
  ) {
    super(message);
    this.name = "ControllerApiError";
  }
}

export const screenshotGrantFromUrl = (input: string): string | null => {
  try {
    const url = new URL(input);
    if (url.protocol !== "openagents:" || url.hostname !== "harness") return null;
    const grant = url.searchParams.get("grant")?.trim();
    return grant && grant.length <= 2_048 ? grant : null;
  } catch {
    return null;
  }
};

export const fetchScreenshotHarnessBootstrap = async (input: {
  readonly grant: string;
  readonly fetch?: typeof globalThis.fetch;
  readonly baseUrl?: string;
}): Promise<ControllerBootstrap> => {
  const response = await (input.fetch ?? globalThis.fetch)(
    `${input.baseUrl ?? PRO_CONTROLLER_BASE_URL}${PRO_CONTROLLER_HARNESS_PATH}`,
    {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ grant: input.grant }),
    },
  );
  if (response.status === 401 || response.status === 403) {
    throw new ControllerApiError("signed_out", "The disposable screenshot session expired.");
  }
  if (!response.ok) {
    throw new ControllerApiError("unavailable", "The screenshot harness is unavailable.");
  }
  return decodeControllerBootstrap(await response.json());
};

export const registerMobilePushDevice = async (input: {
  readonly deviceId: string;
  readonly pushToken: string;
  readonly platform: "ios" | "android";
  readonly credential: NativeSessionCredential;
  readonly secureStore: NativeSessionSecureStore;
  readonly fetch?: typeof globalThis.fetch;
  readonly baseUrl?: string;
}): Promise<Readonly<{ credential: NativeSessionCredential }>> => {
  const response = await (input.fetch ?? globalThis.fetch)(
    `${input.baseUrl ?? PRO_CONTROLLER_BASE_URL}${PRO_CONTROLLER_DEVICE_PATH}`,
    {
      method: "POST",
      headers: { ...headersFor(input.credential), "content-type": "application/json" },
      body: JSON.stringify({
        deviceId: input.deviceId,
        provider: "expo",
        pushToken: input.pushToken,
        platform: input.platform,
      }),
    },
  );
  if (response.status === 401 || response.status === 403) {
    throw new ControllerApiError("signed_out", "Sign in before registering this device.");
  }
  if (!response.ok) {
    throw new ControllerApiError("unavailable", "Push registration is temporarily unavailable.");
  }
  const body = (await response.json()) as {
    rotatedTokens?: { access: string; refresh: string; expiresIn: number };
  };
  return {
    credential: await persistRotation(input.secureStore, input.credential, body.rotatedTokens),
  };
};

export const makeControllerTransport = (input: {
  readonly credential: () => NativeSessionCredential;
  readonly updateCredential: (credential: NativeSessionCredential) => void;
  readonly secureStore: NativeSessionSecureStore;
  readonly fetch?: typeof globalThis.fetch;
  readonly baseUrl?: string;
}) => ({
  send: async (command: QueuedCommand): Promise<TransportResult> => {
    const current = input.credential();
    const response = await (input.fetch ?? globalThis.fetch)(
      `${input.baseUrl ?? PRO_CONTROLLER_BASE_URL}${PRO_CONTROLLER_COMMAND_PATH}`,
      {
        method: "POST",
        headers: { ...headersFor(current), "content-type": "application/json" },
        body: JSON.stringify(command),
      },
    );
    const body = decodeTransportReceipt(await response.json());
    const next = await persistRotation(input.secureStore, current, body.rotatedTokens);
    input.updateCredential(next);
    if (body.status === "rejected") {
      return {
        status: "rejected",
        receiptRef: body.receiptRef,
        code: body.code ?? "target_failure",
        detail: body.detail ?? "The command was rejected.",
      };
    }
    return { status: body.status, receiptRef: body.receiptRef };
  },
});

export const sendImmediateInterrupt = async (input: {
  readonly commandId: string;
  readonly target: ControllerTarget;
  readonly credential: NativeSessionCredential;
  readonly updateCredential: (credential: NativeSessionCredential) => void;
  readonly secureStore: NativeSessionSecureStore;
  readonly fetch?: typeof globalThis.fetch;
  readonly baseUrl?: string;
}): Promise<TransportResult> => {
  const response = await (input.fetch ?? globalThis.fetch)(
    `${input.baseUrl ?? PRO_CONTROLLER_BASE_URL}${PRO_CONTROLLER_COMMAND_PATH}`,
    {
      method: "POST",
      headers: { ...headersFor(input.credential), "content-type": "application/json" },
      body: JSON.stringify({
        version: MOBILE_CONTROLLER_VERSION,
        commandId: input.commandId,
        operation: "runtime.interrupt",
        payload: input.target,
      }),
    },
  );
  const body = decodeTransportReceipt(await response.json());
  const next = await persistRotation(input.secureStore, input.credential, body.rotatedTokens);
  input.updateCredential(next);
  return body.status === "rejected"
    ? {
        status: "rejected",
        receiptRef: body.receiptRef,
        code: body.code ?? "target_failure",
        detail: body.detail ?? "The interrupt was rejected.",
      }
    : { status: body.status, receiptRef: body.receiptRef };
};
