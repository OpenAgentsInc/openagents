import { Context, Effect, Layer } from 'effect';
import { getPageContext, getPostHog } from '../lib/posthog';

export type TelemetryLevel = "debug" | "info" | "warn" | "error";

export type TelemetryFields = Record<string, unknown>;

export type TelemetryClient = {
  readonly log: (
    level: TelemetryLevel,
    message: string,
    fields?: TelemetryFields,
  ) => Effect.Effect<void>;
  readonly event: (name: string, properties?: TelemetryFields) => Effect.Effect<void>;
  readonly identify: (distinctId: string, properties?: TelemetryFields) => Effect.Effect<void>;
  readonly withNamespace: (namespace: string) => TelemetryClient;
  readonly withFields: (fields: TelemetryFields) => TelemetryClient;
};

export class TelemetryService extends Context.Tag('@openagents/web/Telemetry')<
  TelemetryService,
  TelemetryClient
>() { }

/** PostHog sink: best-effort capture/identify with page context (client-only). */
function posthogCapture(event: string, properties?: TelemetryFields): void {
  const posthog = getPostHog();
  if (!posthog) return;
  try {
    posthog.capture(event, { ...getPageContext(), ...(properties ?? {}) });
  } catch {
    // never block on analytics
  }
}

function posthogIdentify(distinctId: string, properties?: TelemetryFields): void {
  const posthog = getPostHog();
  if (!posthog) return;
  try {
    posthog.identify(distinctId, { ...getPageContext(), ...(properties ?? {}) });
  } catch {
    // never block on analytics
  }
}

const createTelemetry = (options: {
  namespace?: string;
  fields?: TelemetryFields;
}): TelemetryClient => {
  const baseFields = options.fields ?? {};
  const namespace = options.namespace;

  const mergeFields = (fields?: TelemetryFields): TelemetryFields =>
    namespace ? { namespace, ...baseFields, ...(fields ?? {}) } : { ...baseFields, ...(fields ?? {}) };

  const emit = (level: TelemetryLevel, message: string, fields?: TelemetryFields) =>
    Effect.sync(() => {
      const payload = mergeFields(fields);
      switch (level) {
        case 'error':
          console.error(`[telemetry] ${message}`, payload);
          return;
        case 'warn':
          console.warn(`[telemetry] ${message}`, payload);
          return;
        case 'info':
          console.info(`[telemetry] ${message}`, payload);
          return;
        default:
          console.debug(`[telemetry] ${message}`, payload);
      }
    });

  return {
    log: emit,
    event: (name, properties) =>
      Effect.sync(() => {
        const payload = mergeFields(properties);
        console.log(`[telemetry:event] ${name}`, payload);
        posthogCapture(name, payload);
      }),
    identify: (distinctId, properties) =>
      Effect.sync(() => {
        const payload = mergeFields(properties);
        console.log(`[telemetry:identify] ${distinctId}`, payload);
        posthogIdentify(distinctId, payload);
      }),
    withNamespace: (nextNamespace) =>
      createTelemetry({
        namespace: nextNamespace,
        fields: baseFields,
      }),
    withFields: (fields) =>
      createTelemetry({
        namespace,
        fields: { ...baseFields, ...fields },
      }),
  };
};

export const TelemetryLive = Layer.succeed(
  TelemetryService,
  TelemetryService.of(createTelemetry({})),
);
