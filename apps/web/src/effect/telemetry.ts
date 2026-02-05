import { Context, Effect, Layer } from 'effect';

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
>() {}

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
        console.log(`[telemetry:event] ${name}`, mergeFields(properties));
      }),
    identify: (distinctId, properties) =>
      Effect.sync(() => {
        console.log(`[telemetry:identify] ${distinctId}`, mergeFields(properties));
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
