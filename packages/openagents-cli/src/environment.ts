import { Config, Effect, Layer, Option, Redacted } from "effect";
import * as Context from "effect/Context";

export interface EnvironmentValues {
  readonly profile: Option.Option<string>;
  readonly apiUrl: Option.Option<string>;
  readonly token: Option.Option<Redacted.Redacted<string>>;
  readonly noColor: boolean;
  readonly configPath: Option.Option<string>;
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
    const noColor = Option.isSome(yield* Config.option(Config.string("NO_COLOR")));
    const configPath = yield* Config.option(Config.string("OPENAGENTS_CONFIG_PATH"));
    return EnvironmentConfiguration.of({ profile, apiUrl, token, noColor, configPath });
  }),
);

export const environmentLayerFromValues = (
  values: Partial<{
    readonly profile: string;
    readonly apiUrl: string;
    readonly token: string;
    readonly configPath: string;
  }>,
) =>
  Layer.succeed(EnvironmentConfiguration, {
    profile: values.profile === undefined ? Option.none() : Option.some(values.profile),
    apiUrl: values.apiUrl === undefined ? Option.none() : Option.some(values.apiUrl),
    token: values.token === undefined ? Option.none() : Option.some(Redacted.make(values.token)),
    noColor: false,
    configPath: values.configPath === undefined ? Option.none() : Option.some(values.configPath),
  });
