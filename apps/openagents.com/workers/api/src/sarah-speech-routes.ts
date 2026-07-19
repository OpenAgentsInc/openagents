import type { SyncSql } from "@openagentsinc/khala-sync-server";
import { Effect, Schema as S } from "effect";

import type { HttpHeadersDecorator } from "./http/responses";
import type {
  KhalaSyncHyperdriveBinding,
  MakeKhalaSyncPushSqlClient,
} from "./khala-sync-push-routes";
import { defaultMakeKhalaSyncSqlClient } from "./khala-sync-push-routes";
import {
  hasSarahThreadAuthority,
  type SarahOwnerAuthenticatedOwner,
} from "./sarah-owner-routes";

export const SARAH_SPEECH_PATH = "/api/mobile/sarah/speech";
export const SARAH_SPEECH_SCHEMA = "openagents.sarah.speech.request.v1";
export const SARAH_SPEECH_MODEL = "gpt-4o-mini-tts";
export const SARAH_SPEECH_VOICE = "marin";
export const SARAH_SPEECH_MAX_CHARACTERS = 4_096;
const SARAH_SPEECH_MAX_REQUEST_BYTES = 24_576;
const OPENAI_SPEECH_URL = "https://api.openai.com/v1/audio/speech";

class SarahSpeechRouteError extends S.TaggedErrorClass<SarahSpeechRouteError>()(
  "SarahSpeechRouteError",
  { cause: S.Defect() },
) {}

const SarahSpeechRequest = S.Struct({
  schema: S.Literal(SARAH_SPEECH_SCHEMA),
  threadRef: S.String.check(S.isMinLength(1), S.isMaxLength(256)),
  messageRef: S.String.check(S.isMinLength(1), S.isMaxLength(256)),
  text: S.String.check(S.isMinLength(1), S.isMaxLength(SARAH_SPEECH_MAX_CHARACTERS)),
});

type SarahSpeechRouteEnv = Readonly<{
  KHALA_SYNC_DB?: KhalaSyncHyperdriveBinding | undefined;
  OPENAI_API_KEY?: string | undefined;
}>;

export type SarahSpeechRouteDependencies<Bindings extends SarahSpeechRouteEnv> = Readonly<{
  authenticateOwner: (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) => Promise<SarahOwnerAuthenticatedOwner | undefined>;
  bindingForEnv?: ((env: Bindings) => KhalaSyncHyperdriveBinding | undefined) | undefined;
  makeSqlClient?: MakeKhalaSyncPushSqlClient | undefined;
  hasAuthority?: ((sql: SyncSql, ownerUserId: string, threadRef: string) => Promise<boolean>) | undefined;
  fetch?: typeof globalThis.fetch | undefined;
}>;

const jsonResponse = (
  body: unknown,
  status: number,
  decorate?: HttpHeadersDecorator,
): Response => {
  const headers = new Headers({
    "cache-control": "no-store",
    "content-type": "application/json",
  });
  decorate?.(headers);
  return new Response(JSON.stringify(body), { status, headers });
};

const bindingConnectionString = (
  binding: KhalaSyncHyperdriveBinding | undefined,
): string | undefined => {
  const value = binding?.connectionString;
  return typeof value === "string" && value.length > 0 ? value : undefined;
};

const decodeRequest = (text: string): typeof SarahSpeechRequest.Type | null => {
  try {
    return S.decodeUnknownSync(SarahSpeechRequest)(JSON.parse(text), { onExcessProperty: "error" });
  } catch {
    return null;
  }
};

/**
 * Owner-private, delivery-only TTS. This route does not create messages,
 * mutate Sarah state, or grant tool authority. It only voices text from an
 * already-visible Sarah reply after re-admitting the exact owner/thread.
 */
export const makeSarahSpeechRoutes = <Bindings extends SarahSpeechRouteEnv>(
  dependencies: SarahSpeechRouteDependencies<Bindings>,
) => ({
  handle: (request: Request, env: Bindings, ctx: ExecutionContext): Effect.Effect<Response> =>
    Effect.gen(function* () {
      if (request.method !== "POST") {
        return jsonResponse({ error: "method_not_allowed" }, 405);
      }
      if ([...new URL(request.url).searchParams.keys()].length > 0) {
        return jsonResponse({ error: "invalid_request" }, 400);
      }
      const claimedLength = Number(request.headers.get("content-length") ?? "0");
      if (Number.isFinite(claimedLength) && claimedLength > SARAH_SPEECH_MAX_REQUEST_BYTES) {
        return jsonResponse({ error: "request_too_large" }, 413);
      }

      const authentication = yield* Effect.tryPromise({
        try: () => dependencies.authenticateOwner(request, env, ctx),
        catch: cause => new SarahSpeechRouteError({ cause }),
      }).pipe(Effect.option);
      if (authentication._tag === "None") {
        return jsonResponse({ error: "authentication_unavailable" }, 503);
      }
      const owner = authentication.value;
      if (owner === undefined) {
        return jsonResponse({ error: "unauthenticated" }, 401);
      }
      const respond = (body: unknown, status: number): Response =>
        jsonResponse(body, status, owner.decorateResponseHeaders);

      const bodyText = yield* Effect.tryPromise({
        try: () => request.text(),
        catch: cause => new SarahSpeechRouteError({ cause }),
      }).pipe(Effect.option);
      if (bodyText._tag === "None" || new TextEncoder().encode(bodyText.value).byteLength > SARAH_SPEECH_MAX_REQUEST_BYTES) {
        return respond({ error: "invalid_request" }, 400);
      }
      const body = decodeRequest(bodyText.value);
      if (body === null || body.text.trim().length === 0) {
        return respond({ error: "invalid_request" }, 400);
      }

      const connectionString = bindingConnectionString(
        (dependencies.bindingForEnv ?? (value => value.KHALA_SYNC_DB))(env),
      );
      if (connectionString === undefined) {
        return respond({ error: "storage_unavailable" }, 503);
      }
      const clientResult = yield* Effect.tryPromise({
        try: () => (dependencies.makeSqlClient ?? defaultMakeKhalaSyncSqlClient)(connectionString),
        catch: cause => new SarahSpeechRouteError({ cause }),
      }).pipe(Effect.option);
      if (clientResult._tag === "None") {
        return respond({ error: "storage_unavailable" }, 503);
      }
      const client = clientResult.value;
      const authority = yield* Effect.tryPromise({
        try: () => (dependencies.hasAuthority ?? hasSarahThreadAuthority)(
          client.sql,
          owner.userId,
          body.threadRef,
        ),
        catch: cause => new SarahSpeechRouteError({ cause }),
      }).pipe(
        Effect.option,
        Effect.ensuring(Effect.promise(() => client.end()).pipe(Effect.ignore)),
      );
      if (authority._tag === "None") {
        return respond({ error: "storage_unavailable" }, 503);
      }
      if (!authority.value) {
        return respond({ error: "sarah_speech_forbidden" }, 403);
      }

      const apiKey = env.OPENAI_API_KEY?.trim();
      if (apiKey === undefined || apiKey.length === 0) {
        return respond({ error: "sarah_speech_unavailable" }, 503);
      }
      const upstream = yield* Effect.tryPromise({
        try: () => (dependencies.fetch ?? globalThis.fetch)(OPENAI_SPEECH_URL, {
          method: "POST",
          headers: {
            authorization: `Bearer ${apiKey}`,
            "content-type": "application/json",
          },
          body: JSON.stringify({
            model: SARAH_SPEECH_MODEL,
            voice: SARAH_SPEECH_VOICE,
            input: body.text.trim(),
            instructions: "Speak naturally, warmly, and conversationally as Sarah. Keep the cadence calm and direct.",
            response_format: "mp3",
          }),
        }),
        catch: cause => new SarahSpeechRouteError({ cause }),
      }).pipe(Effect.option);
      if (upstream._tag === "None" || !upstream.value.ok || upstream.value.body === null) {
        return respond({ error: "sarah_speech_provider_unavailable" }, 502);
      }

      const headers = new Headers({
        "cache-control": "no-store",
        "content-disposition": "inline; filename=\"sarah.mp3\"",
        "content-type": "audio/mpeg",
        "x-openagents-ai-voice": "true",
      });
      owner.decorateResponseHeaders?.(headers);
      return new Response(upstream.value.body, { status: 200, headers });
    }),
});
