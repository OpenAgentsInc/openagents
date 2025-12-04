import { Effect } from "effect";
import * as Duration from "effect/Duration";

export type RetryConfig = {
  attempts: number;
  baseDelayMs: number;
  maxDelayMs: number;
};

const env = typeof Bun !== "undefined" ? Bun.env : process.env;

const toNumber = (value: string | undefined, fallback: number): number => {
  const n = Number(value);
  return Number.isFinite(n) ? n : fallback;
};

export const defaultRetryConfig: RetryConfig = {
  attempts: toNumber(env.LLM_RETRY_ATTEMPTS, 3),
  baseDelayMs: toNumber(env.LLM_RETRY_BASE_MS, 500),
  maxDelayMs: toNumber(env.LLM_RETRY_MAX_MS, 4000),
};

export class HttpError extends Error {
  readonly status: number | null;
  readonly body: string | null;
  constructor(message: string, status?: number, body?: string) {
    super(message);
    this.name = "HttpError";
    this.status = status ?? null;
    this.body = body ?? null;
  }
}

const isNetworkish = (message: string) =>
  /(timeout|ECONNRESET|ENOTFOUND|ECONNREFUSED|EHOSTUNREACH|network)/i.test(message);

export const isRetryableLlmError = (error: unknown): boolean => {
  if (error instanceof HttpError) {
    if (error.status === 401 || error.status === 403 || error.status === 400) return false;
    if (error.status === 408 || error.status === 409 || error.status === 425) return true;
    if (error.status === 429) return true;
    if (typeof error.status === "number" && error.status >= 500) return true;
  }
  const msg = error instanceof Error ? error.message : String(error);
  return isNetworkish(msg);
};

const mergeConfig = (override?: Partial<RetryConfig>): RetryConfig => ({
  ...defaultRetryConfig,
  ...override,
});

/**
 * Retry an Effect-producing operation with exponential backoff.
 */
export const retryWithBackoff = <A>(
  operation: () => Effect.Effect<A, Error>,
  config?: Partial<RetryConfig>,
  shouldRetry: (error: unknown) => boolean = isRetryableLlmError,
): Effect.Effect<A, Error> => {
  const cfg = mergeConfig(config);
  const maxAttempts = Math.max(cfg.attempts, 1);

  const attempt = (remaining: number, delay: number): Effect.Effect<A, Error> =>
    operation().pipe(
      Effect.catchAll((err) =>
        shouldRetry(err) && remaining > 0
          ? Effect.sleep(Duration.millis(delay)).pipe(
              Effect.flatMap(() =>
                attempt(remaining - 1, Math.min(delay * 2, cfg.maxDelayMs)),
              ),
            )
          : Effect.fail(err),
      ),
    );

  return attempt(maxAttempts - 1, cfg.baseDelayMs);
};
