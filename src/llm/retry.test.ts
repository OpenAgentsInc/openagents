import { describe, expect, test } from "bun:test";
import { HttpError, retryWithBackoff } from "./retry.js";
import { Effect } from "effect";

describe("retryWithBackoff", () => {
  test("retries retryable errors until success", async () => {
    let attempts = 0;
    const result = await Effect.runPromise(
      retryWithBackoff(
        () =>
          Effect.try({
            try: () => {
              attempts++;
              if (attempts < 3) throw new HttpError("HTTP 429", 429);
              return "ok";
            },
            catch: (e) => e as Error,
          }),
        { attempts: 3, baseDelayMs: 1, maxDelayMs: 2 },
      ),
    );

    expect(result).toBe("ok");
    expect(attempts).toBe(3);
  });

  test("does not retry non-retryable errors", async () => {
    let attempts = 0;
    await expect(
      Effect.runPromise(
        retryWithBackoff(
          () =>
            Effect.try({
              try: () => {
                attempts++;
                throw new HttpError("HTTP 401", 401);
              },
              catch: (e) => e as Error,
            }),
          { attempts: 3, baseDelayMs: 1, maxDelayMs: 2 },
        ),
      ),
    ).rejects.toThrow("HTTP 401");
    expect(attempts).toBe(1);
  });
});
