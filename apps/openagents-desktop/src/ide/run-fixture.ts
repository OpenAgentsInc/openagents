import {
  IdeEnvironmentGenerationSchema,
  IdeEnvironmentManifestRefSchema,
  IdeEnvironmentManifestSchema,
  IdeExecutableAdmissionSchema,
  IdeExecutableRefSchema,
  IdeRunBindingSchema,
  IdeTaskDefinitionRefSchema,
  IdeTaskDefinitionSchema,
  IdeTaskDiscoveryGenerationSchema,
  IdeTerminalProfileRefSchema,
  IdeTerminalProfileSchema,
  IdeTestControllerRefSchema,
  IdeTestControllerSchema,
  IdeTestDiscoveryGenerationSchema,
  IdeTestItemRefSchema,
  IdeTestItemSchema,
  type IdeRunBinding,
} from "./run-contract.ts"
import {
  IdeAttachmentGenerationSchema,
  IdePlacementGenerationSchema,
  IdePlacementRefSchema,
  IdeProjectRefSchema,
  IdeRootRefSchema,
  IdeWorktreeRefSchema,
} from "./project-contract.ts"
import { emptyIdeRunSnapshot } from "./run-service.ts"

export const ideRunFixtureBinding = (): IdeRunBinding => IdeRunBindingSchema.make({
  projectRef: IdeProjectRefSchema.make("ide.project.fixture"),
  rootRef: IdeRootRefSchema.make("ide.root.fixture"),
  worktreeRef: IdeWorktreeRefSchema.make("ide.worktree.fixture"),
  attachmentGeneration: IdeAttachmentGenerationSchema.make(1),
  placementGeneration: IdePlacementGenerationSchema.make(1),
  placementRef: IdePlacementRefSchema.make("ide.placement.desktop-local"),
  cwdRef: "workspace:fixture",
  cwdLabel: "fixture",
})

export const ideRunFixtureEnvironment = () => IdeEnvironmentManifestSchema.make({
  manifestRef: IdeEnvironmentManifestRefSchema.make("ide.environment.fixture"),
  generation: IdeEnvironmentGenerationSchema.make(1),
  sources: [{ _tag: "HostSafe", precedence: 10, keys: ["PATH", "HOME"] }],
  admittedKeys: ["HOME", "PATH", "TERM"],
  redactedKeys: [],
  inheritedAllHostVariables: false,
  valuesExposedToRenderer: false,
  digest: "sha256:fixture-environment",
})

export const ideRunFixtureExecutable = () => IdeExecutableAdmissionSchema.make({
  executableRef: IdeExecutableRefSchema.make("ide.executable.fixture-node"),
  executable: process.execPath,
  argv: ["-e", "process.stdout.write('Tests 1 passed')"],
  displayLabel: "fixture node",
  source: "task_definition",
  shellInterpolation: false,
  admitted: true,
  refusalReason: null,
})

export const ideRunFixtureProfile = () => IdeTerminalProfileSchema.make({
  profileRef: IdeTerminalProfileRefSchema.make("ide.terminal-profile.fixture"),
  label: "Fixture shell",
  shellLabel: "node",
  executable: { ...ideRunFixtureExecutable(), source: "profile" },
  environmentKeys: ["HOME", "PATH", "TERM"],
  isDefault: true,
})

export const ideRunFixtureTask = () => IdeTaskDefinitionSchema.make({
  definitionRef: IdeTaskDefinitionRefSchema.make("ide.task-definition.fixture"),
  discoveryGeneration: IdeTaskDiscoveryGenerationSchema.make(1),
  version: 1,
  label: "Fixture test task",
  group: "test",
  dependencies: [],
  binding: ideRunFixtureBinding(),
  executable: ideRunFixtureExecutable(),
  environment: ideRunFixtureEnvironment(),
  problemMatchers: [{ matcherRef: "ide.problem-matcher.fixture", kind: "generic_location", severity: "error" }],
  background: { enabled: false, readinessPattern: null },
  timeoutMs: 30_000,
  maxRetries: 0,
  artifactPatterns: [],
  exactRerunLabel: "node fixture",
})

export const ideRunFixtureController = () => {
  const controllerRef = IdeTestControllerRefSchema.make("ide.test-controller.fixture")
  const generation = IdeTestDiscoveryGenerationSchema.make(1)
  const rootRef = IdeTestItemRefSchema.make("ide.test-item.fixture-root")
  const fileRef = IdeTestItemRefSchema.make("ide.test-item.fixture-file")
  return IdeTestControllerSchema.make({
    controllerRef,
    label: "Fixture tests",
    discoveryGeneration: generation,
    binding: ideRunFixtureBinding(),
    executable: { ...ideRunFixtureExecutable(), source: "test_controller" },
    environment: ideRunFixtureEnvironment(),
    items: [
      IdeTestItemSchema.make({
        itemRef: rootRef,
        controllerRef,
        discoveryGeneration: generation,
        parentRef: null,
        label: "Fixture",
        kind: "root",
        location: null,
        runnable: true,
        debugSupported: false,
      }),
      IdeTestItemSchema.make({
        itemRef: fileRef,
        controllerRef,
        discoveryGeneration: generation,
        parentRef: rootRef,
        label: "fixture.test.ts",
        kind: "file",
        location: { pathRef: "fixture.test.ts", line: 1, column: 1, label: "fixture.test.ts" },
        runnable: true,
        debugSupported: false,
      }),
    ],
    profiles: ["run", "coverage"],
    discoveryComplete: true,
    discoveryError: null,
  })
}

export const ideRunFixtureSnapshot = () => emptyIdeRunSnapshot(
  ideRunFixtureBinding(),
  [ideRunFixtureProfile()],
)
