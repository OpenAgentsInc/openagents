import { Config, Effect, Layer, Option, Redacted } from "effect";
import * as Context from "effect/Context";

export interface EnvironmentValues {
  readonly profile: Option.Option<string>;
  readonly apiUrl: Option.Option<string>;
  readonly token: Option.Option<Redacted.Redacted<string>>;
}

export class EnvironmentConfiguration extends Context.Service<
  EnvironmentConfiguration,
  EnvironmentValues
>()("@openagentsinc/cli/EnvironmentConfiguration") {}

export const environmentLayer = Layer.effect(
  EnvironmentConfiguration,
  Effect.gen(function* () {
    const profile = yield* Config.option(Config.string("OPENAGENTS_PROFILE"));
    const apiUrl = yield* Config.option(Config.string("OPENAGENTS_API_URL"));
    const token = yield* Config.option(Config.redacted("OPENAGENTS_TOKEN"));
    return EnvironmentConfiguration.of({ profile, apiUrl, token });
  }),
);

export const environmentLayerFromValues = (
  values: Partial<{
    readonly profile: string;
    readonly apiUrl: string;
    readonly token: string;
  }>,
) =>
  Layer.succeed(EnvironmentConfiguration, {
    profile: values.profile === undefined ? Option.none() : Option.some(values.profile),
    apiUrl: values.apiUrl === undefined ? Option.none() : Option.some(values.apiUrl),
    token: values.token === undefined ? Option.none() : Option.some(Redacted.make(values.token)),
  });
