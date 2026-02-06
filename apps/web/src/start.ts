import { createMiddleware, createStart } from '@tanstack/react-start';
import { authkitMiddleware } from '@workos/authkit-tanstack-react-start';
import { Effect } from 'effect';
import { getAppConfig } from './effect/config';
import { makeAppRuntime } from './effect/runtime';
import { TelemetryService } from './effect/telemetry';

export const startInstance = createStart(() => {
  const authkit = authkitMiddleware();

  // Best-effort request telemetry + context (requestId).
  const requestContext = createMiddleware({ type: 'request' }).server(async (opts) => {
    const requestId =
      typeof crypto !== 'undefined' && 'randomUUID' in crypto
        ? crypto.randomUUID()
        : Math.random().toString(36).slice(2);

    const url = new URL(opts.request.url);
    const runtime = makeAppRuntime(getAppConfig());

    try {
      await runtime.runPromise(
        Effect.gen(function* () {
          const telemetry = yield* TelemetryService;
          yield* telemetry.withNamespace('http').event('request.start', {
            requestId,
            method: opts.request.method,
            path: url.pathname,
          });
        }),
      );
    } catch {
      // Never block a request on telemetry.
    }

    return opts.next({ context: { requestId } });
  });

  // Cloudflare Workers: in some bundles TanStack Start server function IDs don't resolve.
  // If AuthKit fails with this error, continue unauthenticated (homepage still works).
  const safeAuthkit = createMiddleware({ type: 'request' }).server(async (opts) => {
    try {
      if (authkit.options.server) {
        const result = await (authkit.options.server as (o: typeof opts) => Promise<unknown>)(opts);
        return result as Awaited<ReturnType<typeof opts.next>>;
      }
      return opts.next();
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      if (!msg.includes('Invalid server function ID')) {
        const runtime = makeAppRuntime(getAppConfig());
        try {
          await runtime.runPromise(
            Effect.gen(function* () {
              const telemetry = yield* TelemetryService;
              yield* telemetry.withNamespace('auth.workos').log('error', 'authkit.middleware_error', {
                message: msg,
              });
            }),
          );
        } catch {
          // Never block a request on telemetry.
        }
      }
      return opts.next();
    }
  });

  return {
    requestMiddleware: [requestContext, safeAuthkit],
  };
});
