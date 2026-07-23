import type {
  AgentHarness,
  HarnessSession,
  HarnessStartError,
  HarnessStartOptions,
} from "@openagentsinc/agent-harness-contract";
import { Effect, Schema as S } from "effect";

export const OPENAGENTS_CLOUD_CODING_SESSION_LAUNCH_PATH = "/v1/cloud-coding-sessions" as const;
export const MANAGED_SANDBOX_TURN_PATH = "/v1/managed-sandbox/runtime/turns" as const;

const HttpsBaseUrl = S.String.check(
  S.isMinLength(9),
  S.isMaxLength(2_048),
  S.isPattern(/^https:\/\/[^/?#\s]+(?::[0-9]+)?(?:\/[^?#\s]*)?$/u),
);

const StableRef = S.String.check(
  S.isMinLength(1),
  S.isMaxLength(256),
  S.isPattern(/^[A-Za-z0-9][A-Za-z0-9._:-]*$/u),
);

export const HarnessEnvironment = S.TaggedUnion({
  desktop_local: {},
  openagents_cloud: {
    controlPlaneBaseUrl: HttpsBaseUrl,
    launchPath: S.Literal(OPENAGENTS_CLOUD_CODING_SESSION_LAUNCH_PATH),
  },
  managed_sandbox: {
    controlPlaneBaseUrl: HttpsBaseUrl,
    sandboxRef: StableRef,
    turnPath: S.Literal(MANAGED_SANDBOX_TURN_PATH),
  },
}).annotate({ identifier: "HarnessEnvironment" });
export type HarnessEnvironment = typeof HarnessEnvironment.Type;

export type DesktopLocalHarnessEnvironment = Extract<
  HarnessEnvironment,
  { readonly _tag: "desktop_local" }
>;
export type OpenAgentsCloudHarnessEnvironment = Extract<
  HarnessEnvironment,
  { readonly _tag: "openagents_cloud" }
>;
export type ManagedSandboxHarnessEnvironment = Extract<
  HarnessEnvironment,
  { readonly _tag: "managed_sandbox" }
>;
export type RemoteHarnessEnvironment =
  | OpenAgentsCloudHarnessEnvironment
  | ManagedSandboxHarnessEnvironment;

export const DEFAULT_HARNESS_ENVIRONMENT: DesktopLocalHarnessEnvironment =
  HarnessEnvironment.cases.desktop_local.make({});

export const makeOpenAgentsCloudHarnessEnvironment = (
  controlPlaneBaseUrl: string,
): OpenAgentsCloudHarnessEnvironment =>
  HarnessEnvironment.cases.openagents_cloud.make({
    controlPlaneBaseUrl,
    launchPath: OPENAGENTS_CLOUD_CODING_SESSION_LAUNCH_PATH,
  });

export const makeManagedSandboxHarnessEnvironment = (input: {
  readonly controlPlaneBaseUrl: string;
  readonly sandboxRef: string;
}): ManagedSandboxHarnessEnvironment =>
  HarnessEnvironment.cases.managed_sandbox.make({
    controlPlaneBaseUrl: input.controlPlaneBaseUrl,
    sandboxRef: input.sandboxRef,
    turnPath: MANAGED_SANDBOX_TURN_PATH,
  });

export const decodeHarnessEnvironment = (
  input: unknown,
): Effect.Effect<HarnessEnvironment, S.SchemaError> =>
  input === undefined
    ? Effect.succeed(DEFAULT_HARNESS_ENVIRONMENT)
    : S.decodeUnknownEffect(HarnessEnvironment)(input, {
        onExcessProperty: "error",
      });

export class HarnessEnvironmentError extends S.TaggedErrorClass<HarnessEnvironmentError>()(
  "AgentHarness.EnvironmentError",
  {
    environment: S.Literals(["openagents_cloud", "managed_sandbox"]),
    failureClass: S.String,
    detail: S.optionalKey(S.String),
    cause: S.optionalKey(S.Defect()),
  },
) {}

export interface HarnessEnvironmentStartInput<
  Environment extends RemoteHarnessEnvironment = RemoteHarnessEnvironment,
> {
  readonly environment: Environment;
  readonly harness: AgentHarness;
  readonly options: HarnessStartOptions;
}

export interface HarnessEnvironmentRunner<
  Environment extends RemoteHarnessEnvironment = RemoteHarnessEnvironment,
> {
  readonly environment: Environment["_tag"];
  readonly start: (
    input: HarnessEnvironmentStartInput<Environment>,
  ) => Effect.Effect<HarnessSession, HarnessStartError | HarnessEnvironmentError>;
}

export interface HarnessEnvironmentRunners {
  readonly openagentsCloud?: HarnessEnvironmentRunner<OpenAgentsCloudHarnessEnvironment>;
  readonly managedSandbox?: HarnessEnvironmentRunner<ManagedSandboxHarnessEnvironment>;
}

export interface StartHarnessInEnvironmentInput {
  readonly harness: AgentHarness;
  readonly options: HarnessStartOptions;
  readonly environment?: HarnessEnvironment;
  readonly runners?: HarnessEnvironmentRunners;
}

const unavailableRunner = (
  environment: RemoteHarnessEnvironment["_tag"],
): HarnessEnvironmentError =>
  new HarnessEnvironmentError({
    environment,
    failureClass: "environment_runner_unavailable",
    detail: `No ${environment} runner is installed.`,
  });

/**
 * Start a harness in the selected environment.
 *
 * The function uses `desktop_local` when the caller does not select an
 * environment. This branch calls the current SDK adapter without a wrapper.
 * A remote runner returns the same `HarnessSession` contract. Thus, prompt
 * control and event streaming do not change between environments.
 */
export const startHarnessInEnvironment = Effect.fn("HarnessEnvironment.startHarness")(function* (
  input: StartHarnessInEnvironmentInput,
) {
  const environment = input.environment ?? DEFAULT_HARNESS_ENVIRONMENT;
  if (environment._tag === "desktop_local") {
    return yield* input.harness.start(input.options);
  }
  if (environment._tag === "openagents_cloud") {
    const runner = input.runners?.openagentsCloud;
    if (runner === undefined) {
      return yield* unavailableRunner(environment._tag);
    }
    return yield* runner.start({
      environment,
      harness: input.harness,
      options: input.options,
    });
  }
  const runner = input.runners?.managedSandbox;
  if (runner === undefined) {
    return yield* unavailableRunner(environment._tag);
  }
  return yield* runner.start({
    environment,
    harness: input.harness,
    options: input.options,
  });
});

const joinBaseAndPath = (baseUrl: string, path: string): string =>
  `${baseUrl.replace(/\/+$/u, "")}${path}`;

export const openAgentsCloudCodingSessionLaunchUrl = (
  environment: OpenAgentsCloudHarnessEnvironment,
): string => joinBaseAndPath(environment.controlPlaneBaseUrl, environment.launchPath);

export const openAgentsCloudCodingSessionLifecycleUrl = (
  environment: OpenAgentsCloudHarnessEnvironment,
  sessionId: string,
): string =>
  `${openAgentsCloudCodingSessionLaunchUrl(environment)}/${encodeURIComponent(sessionId)}`;

export const managedSandboxTurnUrl = (environment: ManagedSandboxHarnessEnvironment): string =>
  joinBaseAndPath(environment.controlPlaneBaseUrl, environment.turnPath);
