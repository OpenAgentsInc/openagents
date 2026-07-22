import {
  type ManagedSandboxPrivateIngressCapability,
  type ManagedSandboxPrivatePreviewResponse,
  decodeManagedSandboxPrivatePreviewResponse,
} from "@openagentsinc/managed-sandbox-contract";

import { parseJsonUnknown } from "./json-boundary";

const MAX_RESPONSE_BYTES = 2 * 1024 * 1024;
const forbiddenPrivateMaterial =
  /(?:Bearer|Basic)\s+[A-Za-z0-9._~+/-]+=*|(?:\/Users\/|\/home\/|[A-Za-z]:\\Users\\)|"(?:token|apiKey|authorization|refreshToken|mnemonic|password|credential|secret|localPath|hostname|processId|providerSessionId|transportHandle|socket|pid|authHome)"\s*:/iu;
const privateTopology =
  /(?:\b10(?:\.\d{1,3}){3}\b|\b192\.168(?:\.\d{1,3}){2}\b|\b172\.(?:1[6-9]|2\d|3[01])(?:\.\d{1,3}){2}\b|\.internal\b|metadata\.google\.internal|compute\.googleapis\.com)/iu;

export class ManagedSandboxPrivatePreviewTargetError extends Error {
  constructor(
    readonly status: number,
    readonly reasonRef: string,
  ) {
    super(reasonRef);
  }
}

export type ManagedSandboxPrivatePreviewTarget = Readonly<{
  use: (input: {
    requestRef: string;
    capability: ManagedSandboxPrivateIngressCapability;
    audienceRef: string;
    path: string;
    encoding: "utf8" | "base64";
  }) => Promise<ManagedSandboxPrivatePreviewResponse>;
}>;

export const makeManagedSandboxPrivatePreviewTarget = (options: {
  baseUrl: string;
  bearerToken: string;
  fetch?: typeof fetch;
}): ManagedSandboxPrivatePreviewTarget => {
  const fetchImpl = options.fetch ?? fetch;
  const configured =
    options.baseUrl.startsWith("https://") && options.bearerToken.trim().length > 0;
  const endpoint = `${options.baseUrl.replace(/\/$/u, "")}/v1/managed-sandbox/runtime/private-preview`;

  return {
    use: async (input) => {
      if (!configured) throw new ManagedSandboxPrivatePreviewTargetError(503, "target_unavailable");
      const body = JSON.stringify({
        schemaVersion: "openagents.managed_sandbox_private_preview.v1",
        requestRef: input.requestRef,
        capabilityRef: input.capability.capabilityRef,
        audienceRef: input.audienceRef,
        path: input.path,
        encoding: input.encoding,
        capability: input.capability,
      });
      if (forbiddenPrivateMaterial.test(body)) {
        throw new ManagedSandboxPrivatePreviewTargetError(400, "private_material_refused");
      }
      let response: Response;
      try {
        response = await fetchImpl(endpoint, {
          method: "POST",
          headers: {
            accept: "application/json",
            "cache-control": "no-store",
            "content-type": "application/json",
            "x-openagents-managed-sandbox-token": options.bearerToken,
          },
          body,
        });
      } catch {
        throw new ManagedSandboxPrivatePreviewTargetError(503, "target_unavailable");
      }
      const text = await response.text().catch(() => {
        throw new ManagedSandboxPrivatePreviewTargetError(503, "target_unavailable");
      });
      if (!response.ok) {
        throw new ManagedSandboxPrivatePreviewTargetError(
          response.status === 410 ? 410 : response.status >= 500 ? 503 : 403,
          response.status === 410 ? "capability_terminal" : "target_refused",
        );
      }
      if (
        text.length > MAX_RESPONSE_BYTES ||
        forbiddenPrivateMaterial.test(text) ||
        privateTopology.test(text)
      ) {
        throw new ManagedSandboxPrivatePreviewTargetError(502, "target_response_refused");
      }
      try {
        const result = decodeManagedSandboxPrivatePreviewResponse(parseJsonUnknown(text));
        if (
          result.capabilityRef !== input.capability.capabilityRef ||
          result.audienceRef !== input.audienceRef ||
          result.sandboxRef !== input.capability.sandboxRef ||
          result.resourceGeneration !== input.capability.resourceGeneration ||
          result.preview.action !== "read_file" ||
          result.preview.encoding !== input.encoding
        ) {
          throw new Error("private_preview_scope_conflict");
        }
        return result;
      } catch {
        throw new ManagedSandboxPrivatePreviewTargetError(502, "target_response_invalid");
      }
    },
  };
};
