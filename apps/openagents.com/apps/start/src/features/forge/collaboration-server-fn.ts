import {
  ForgeCollaborationRequest,
  type ForgeCollaborationResult,
} from "@/features/forge/collaboration-read";
import { createServerFn } from "@tanstack/react-start";
import { Schema as S } from "effect";

const decodeRequest = S.decodeUnknownSync(ForgeCollaborationRequest);

export const readForgeCollaboration = createServerFn({ method: "GET" })
  .validator((input: unknown) => decodeRequest(input))
  .handler(async ({ data }): Promise<ForgeCollaborationResult> => {
    const { readForgeCollaborationFromOwnedService } = await import(
      "@/server/forge/collaboration-read.server"
    );
    return readForgeCollaborationFromOwnedService(data);
  });
