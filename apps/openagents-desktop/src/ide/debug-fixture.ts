import {
  IdeDebugAdapterRefSchema,
  IdeDebugConfigurationGenerationSchema,
  IdeDebugConfigurationRefSchema,
  IdeDebugConfigurationSchema,
  IdeDebugSnapshotSchema,
  IdeDebugTargetRefSchema,
  type IdeDebugCapability,
  type IdeDebugConfiguration,
  type IdeDebugSnapshot,
} from "./debug-contract.ts";
import {
  IdeAttachmentGenerationSchema,
  IdeLanguageGenerationSchema,
  IdePlacementGenerationSchema,
  IdePlacementRefSchema,
  IdeProjectRefSchema,
  IdeRootRefSchema,
  IdeServiceGenerationSchema,
  IdeWorktreeRefSchema,
} from "./project-contract.ts";

const capabilityNames = [
  "configuration_done",
  "conditional_breakpoints",
  "hit_conditional_breakpoints",
  "log_points",
  "function_breakpoints",
  "data_breakpoints",
  "set_variable",
  "evaluate",
  "pause",
  "step_in",
  "step_over",
  "step_out",
  "step_back",
  "run_to_cursor",
  "restart_frame",
  "restart_session",
  "continue",
  "disconnect",
  "terminate",
  "modules",
  "loaded_sources",
  "source_request",
  "cancel_request",
] as const;

export const ideDebugFixtureCapabilities = (
  unsupported: ReadonlyArray<(typeof capabilityNames)[number]> = [],
): ReadonlyArray<IdeDebugCapability> =>
  capabilityNames.map((capability) => ({
    capability,
    supported: !unsupported.includes(capability),
    reason: unsupported.includes(capability)
      ? "The fixture adapter does not support this capability."
      : null,
  }));

export const ideDebugFixtureBinding = () => ({
  projectRef: IdeProjectRefSchema.make("ide.project.fixture"),
  rootRef: IdeRootRefSchema.make("ide.root.fixture"),
  worktreeRef: IdeWorktreeRefSchema.make("ide.worktree.fixture"),
  attachmentGeneration: IdeAttachmentGenerationSchema.make(1),
  languageGeneration: IdeLanguageGenerationSchema.make(1),
  placementGeneration: IdePlacementGenerationSchema.make(1),
  serviceGeneration: IdeServiceGenerationSchema.make(1),
  placementRef: IdePlacementRefSchema.make("ide.placement.fixture"),
  language: "typescript",
});

export const ideDebugFixtureConfiguration = (
  intent: "launch" | "attach" = "launch",
  unsupported: ReadonlyArray<(typeof capabilityNames)[number]> = [],
): IdeDebugConfiguration =>
  IdeDebugConfigurationSchema.make({
    schemaVersion: "openagents.desktop.ide-debug.v1",
    configurationRef: IdeDebugConfigurationRefSchema.make(`ide.debug-config.fixture-${intent}`),
    configurationGeneration: IdeDebugConfigurationGenerationSchema.make(1),
    label: intent === "launch" ? "Launch fixture" : "Attach fixture",
    binding: ideDebugFixtureBinding(),
    intent:
      intent === "launch"
        ? {
            _tag: "Launch",
            executableRef: "ide.executable.fixture",
            executableLabel: "Fixture executable",
            argumentLabels: ["--inspect"],
            prelaunchTaskRef: "ide.task-definition.fixture-build",
            postdebugTaskRef: null,
          }
        : {
            _tag: "Attach",
            transportRef: "ide.debug-transport.fixture",
            targetProcessRef: "ide.process.fixture",
            targetProcessLabel: "Fixture process",
            authenticationRef: "ide.authentication.fixture",
            reusedDeadAttachment: false,
          },
    placement:
      intent === "launch"
        ? { _tag: "Local", hostLabel: "Local fixture" }
        : {
            _tag: "Remote",
            hostRef: "ide.host.fixture",
            hostLabel: "Remote fixture",
            networkRef: "ide.network.fixture",
          },
    adapter: {
      adapterRef: IdeDebugAdapterRefSchema.make("ide.debug-adapter.fixture"),
      adapterType: "node",
      adapterVersion: "1.0.0",
      executableRef: "ide.executable.debug-adapter.fixture",
      transport: "stdio",
      admitted: true,
      capabilities: ideDebugFixtureCapabilities(unsupported),
    },
    targetRef: IdeDebugTargetRefSchema.make(`ide.debug-target.fixture-${intent}`),
    cwdRef: "ide.cwd.fixture",
    environment: {
      manifestRef: "ide.debug-environment.fixture",
      admittedKeys: ["NODE_ENV", "API_TOKEN"],
      redactedKeys: ["API_TOKEN"],
      sourceRefs: ["ide.environment-source.project"],
      valuesExposedToRenderer: false,
      digest: "environment-digest-fixture",
    },
    sourceMaps: {
      manifestRef: "ide.debug-source-map.fixture",
      sourceRoots: ["ide.source-root.fixture"],
      remoteRootRefs: intent === "attach" ? ["ide.remote-root.fixture"] : [],
      generatedSourcesExplicit: true,
      guessPositions: false,
      digest: "source-map-digest-fixture",
    },
    timeoutMs: 30_000,
    admitted: true,
    refusalReason: null,
  });

export const ideDebugFixtureSnapshot = (): IdeDebugSnapshot =>
  IdeDebugSnapshotSchema.make({
    schemaVersion: "openagents.desktop.ide-debug.v1",
    binding: ideDebugFixtureBinding(),
    capabilityState: { _tag: "Unconfigured" },
    configurations: [],
    breakpointSets: [],
    sessions: [],
    receipts: [],
    stopped: false,
  });
