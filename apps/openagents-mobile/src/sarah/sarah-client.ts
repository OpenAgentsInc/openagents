import {
  SarahPrincipalApiResponseSchema,
  type SarahPrincipalProjection,
} from "@openagentsinc/sarah";
import { Effect, Schema as S } from "effect";

export const SARAH_OWNER_MOBILE_PATH = "/api/mobile/sarah";

class SarahPrincipalUnavailable extends S.TaggedErrorClass<SarahPrincipalUnavailable>()(
  "SarahPrincipalUnavailable",
  { cause: S.Defect() },
) {}

export const fetchSarahPrincipal = (
  input: Readonly<{
    baseUrl: string;
    accessToken: string;
    fetch?: typeof fetch;
  }>,
): Promise<SarahPrincipalProjection | null> =>
  Effect.runPromise(
    Effect.tryPromise({
      try: async () => {
        const response = await (input.fetch ?? fetch)(
          new URL(SARAH_OWNER_MOBILE_PATH, input.baseUrl),
          {
            method: "POST",
            headers: {
              authorization: `Bearer ${input.accessToken}`,
              "content-type": "application/json",
            },
            body: "{}",
          },
        );
        if (!response.ok) return null;
        const decoded = S.decodeUnknownSync(SarahPrincipalApiResponseSchema)(
          await response.json(),
          { onExcessProperty: "error" },
        );
        return decoded.principal;
      },
      catch: (cause) => new SarahPrincipalUnavailable({ cause }),
    }).pipe(Effect.catch(() => Effect.succeed(null))),
  );
