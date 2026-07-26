import {
  ForgeRepositoryReadRequest,
  type ForgeRepositoryReadResult,
} from "@/features/forge/repository-read";
import { createServerFn } from "@tanstack/react-start";
import { Schema as S } from "effect";

const decodeRequest = S.decodeUnknownSync(ForgeRepositoryReadRequest);

export const readForgeRepository = createServerFn({ method: "GET" })
  .validator((input: unknown) => decodeRequest(input))
  .handler(async ({ data }): Promise<ForgeRepositoryReadResult> => {
    const [{ readForgeRepositoryFromOwnedService }, { presentForgeRepositoryRead }] =
      await Promise.all([
        import("@/server/forge/repository-read.server"),
        import("@/server/forge/repository-presenter.server"),
      ]);
    const result = await readForgeRepositoryFromOwnedService(data);
    return presentForgeRepositoryRead(result);
  });
