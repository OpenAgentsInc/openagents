import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import path from "node:path";

import { Exit, Schema } from "effect";

import {
  IdeDebugAdapterRefSchema,
  IdeDebugConfigurationGenerationSchema,
  IdeDebugConfigurationRefSchema,
  IdeDebugConfigurationSchema,
  IdeDebugTargetRefSchema,
  type IdeDebugBinding,
} from "./debug-contract.ts";
import {
  IdeDapAdapterResolutionSchema,
  IdeDapDiscoveredConfigurationSchema,
  type IdeDapDiscoveredConfiguration,
} from "./dap-host.ts";

const nonEmpty = (maximum: number) =>
  Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(maximum));
const JsonObjectSchema = Schema.Record(Schema.String, Schema.Json);
const EnvironmentKeySchema = Schema.String.check(
  Schema.isMinLength(1),
  Schema.isMaxLength(96),
  Schema.isPattern(/^[A-Z_][A-Z0-9_]*$/u),
);
const SecretEnvironmentName =
  /(?:TOKEN|SECRET|PASSWORD|PASSWD|COOKIE|AUTHORIZATION|API_KEY|PRIVATE_KEY)/iu;

const IdeDebugManifestEntrySchema = Schema.Struct({
  ref: nonEmpty(120),
  label: nonEmpty(160),
  adapterType: nonEmpty(80),
  adapterVersion: nonEmpty(80),
  adapterExecutable: nonEmpty(4_096),
  adapterArguments: Schema.Array(Schema.String.check(Schema.isMaxLength(16_384))).check(
    Schema.isMaxLength(256),
  ),
  request: Schema.Literals(["launch", "attach"]),
  placement: Schema.optionalKey(
    Schema.TaggedUnion({
      Local: { hostLabel: nonEmpty(160) },
      Container: { containerRef: nonEmpty(192), hostLabel: nonEmpty(160) },
      Remote: { hostRef: nonEmpty(192), hostLabel: nonEmpty(160), networkRef: nonEmpty(192) },
    }),
  ),
  startArguments: JsonObjectSchema,
  cwd: nonEmpty(512),
  environmentKeys: Schema.Array(EnvironmentKeySchema).check(Schema.isMaxLength(512)),
  sourceRoots: Schema.Array(nonEmpty(512)).check(Schema.isMaxLength(64)),
  remoteRoots: Schema.Array(nonEmpty(512)).check(Schema.isMaxLength(64)),
  executableRef: Schema.optionalKey(nonEmpty(192)),
  executableLabel: Schema.optionalKey(nonEmpty(320)),
  argumentLabels: Schema.optionalKey(
    Schema.Array(Schema.String.check(Schema.isMaxLength(512))).check(Schema.isMaxLength(128)),
  ),
  prelaunchTaskRef: Schema.optionalKey(nonEmpty(192)),
  postdebugTaskRef: Schema.optionalKey(nonEmpty(192)),
  transportRef: Schema.optionalKey(nonEmpty(192)),
  targetProcessRef: Schema.optionalKey(nonEmpty(192)),
  targetProcessLabel: Schema.optionalKey(nonEmpty(240)),
  authenticationRef: Schema.optionalKey(nonEmpty(192)),
  timeoutMs: Schema.Number.check(
    Schema.isInt(),
    Schema.isBetween({ minimum: 100, maximum: 120_000 }),
  ),
});

export const IdeDebugManifestSchema = Schema.Struct({
  schemaVersion: Schema.Literal("openagents.desktop.ide-debug-manifest.v1"),
  configurations: Schema.Array(IdeDebugManifestEntrySchema).check(Schema.isMaxLength(1_000)),
}).annotate({ identifier: "IdeDebugManifest" });
export interface IdeDebugManifest extends Schema.Schema.Type<typeof IdeDebugManifestSchema> {}
const decodeIdeDebugManifest = Schema.decodeUnknownExit(IdeDebugManifestSchema);

const digest = (value: string): string => createHash("sha256").update(value).digest("hex");
const boundedId = (value: string): string =>
  value.replaceAll(/[^A-Za-z0-9._-]/gu, "-").slice(0, 120);

const confinedPath = (root: string, candidate: string, label: string): string => {
  const resolved = path.resolve(root, candidate);
  const relative = path.relative(root, resolved);
  if (relative === ".." || relative.startsWith(`..${path.sep}`) || path.isAbsolute(relative)) {
    throw new Error(`${label} must stay in the active project root.`);
  }
  return resolved;
};

const adapterExecutable = (root: string, candidate: string): string => {
  if (!candidate.includes("/") && !candidate.includes("\\")) return candidate;
  if (path.isAbsolute(candidate))
    throw new Error("An adapter executable must be a PATH command or a project-relative path.");
  return confinedPath(root, candidate, "The adapter executable");
};

const environmentFor = (
  keys: ReadonlyArray<string>,
  source: NodeJS.ProcessEnv,
): Readonly<Record<string, string>> =>
  Object.fromEntries(
    keys.flatMap((key) => {
      const value = source[key];
      return value === undefined ? [] : [[key, value] as const];
    }),
  );

export const discoverIdeDebugManifest = async (
  input: Readonly<{
    root: string;
    binding: IdeDebugBinding;
    environment?: NodeJS.ProcessEnv;
  }>,
): Promise<ReadonlyArray<IdeDapDiscoveredConfiguration>> => {
  const root = path.resolve(input.root);
  const manifestPath = path.join(root, ".openagents", "debug.json");
  let source: string;
  try {
    source = await readFile(manifestPath, "utf8");
  } catch (cause) {
    if (cause !== null && typeof cause === "object" && "code" in cause && cause.code === "ENOENT")
      return [];
    throw cause;
  }
  const decoded = decodeIdeDebugManifest(JSON.parse(source));
  if (Exit.isFailure(decoded)) throw new Error("The project debug manifest is invalid.");
  const refs = new Set<string>();
  return decoded.value.configurations.map((entry): IdeDapDiscoveredConfiguration => {
    const suffix = boundedId(entry.ref);
    if (suffix.length === 0 || refs.has(suffix))
      throw new Error("Debug configuration refs must be nonempty and unique.");
    refs.add(suffix);
    const configurationRef = IdeDebugConfigurationRefSchema.make(`ide.debug-config.${suffix}`);
    const environment = environmentFor(entry.environmentKeys, input.environment ?? process.env);
    const environmentMaterial = Object.entries(environment)
      .toSorted(([left], [right]) => left.localeCompare(right))
      .map(([key, value]) => `${key}\0${value}`)
      .join("\0");
    const sourceMaterial = [...entry.sourceRoots, ...entry.remoteRoots].join("\0");
    const cwd = confinedPath(root, entry.cwd, "The debug working directory");
    const configuration = IdeDebugConfigurationSchema.make({
      schemaVersion: "openagents.desktop.ide-debug.v1",
      configurationRef,
      configurationGeneration: IdeDebugConfigurationGenerationSchema.make(1),
      label: entry.label,
      binding: input.binding,
      intent:
        entry.request === "launch"
          ? {
              _tag: "Launch",
              executableRef: entry.executableRef ?? `ide.executable.${suffix}`,
              executableLabel: entry.executableLabel ?? entry.label,
              argumentLabels: entry.argumentLabels ?? [],
              prelaunchTaskRef: entry.prelaunchTaskRef ?? null,
              postdebugTaskRef: entry.postdebugTaskRef ?? null,
            }
          : {
              _tag: "Attach",
              transportRef: entry.transportRef ?? `ide.debug-transport.${suffix}`,
              targetProcessRef: entry.targetProcessRef ?? `ide.process.${suffix}`,
              targetProcessLabel: entry.targetProcessLabel ?? entry.label,
              authenticationRef: entry.authenticationRef ?? null,
              reusedDeadAttachment: false,
            },
      placement: entry.placement ?? { _tag: "Local", hostLabel: "Desktop local" },
      adapter: {
        adapterRef: IdeDebugAdapterRefSchema.make(`ide.debug-adapter.${suffix}`),
        adapterType: entry.adapterType,
        adapterVersion: entry.adapterVersion,
        executableRef: `ide.debug-adapter-executable.${suffix}`,
        transport: "stdio",
        admitted: true,
        capabilities: [],
      },
      targetRef: IdeDebugTargetRefSchema.make(`ide.debug-target.${suffix}`),
      cwdRef: `workspace:${path.relative(root, cwd) || "."}`,
      environment: {
        manifestRef: `ide.debug-environment.${suffix}`,
        admittedKeys: [...entry.environmentKeys].toSorted(),
        redactedKeys: entry.environmentKeys
          .filter((key) => SecretEnvironmentName.test(key))
          .toSorted(),
        sourceRefs: ["ide.environment-source.process-explicit"],
        valuesExposedToRenderer: false,
        digest: `sha256:${digest(environmentMaterial)}`,
      },
      sourceMaps: {
        manifestRef: `ide.debug-source-map.${suffix}`,
        sourceRoots: entry.sourceRoots,
        remoteRootRefs: entry.remoteRoots,
        generatedSourcesExplicit: entry.sourceRoots.length > 0,
        guessPositions: false,
        digest: `sha256:${digest(sourceMaterial)}`,
      },
      timeoutMs: entry.timeoutMs,
      admitted: entry.request === "launch" || entry.authenticationRef !== undefined,
      refusalReason:
        entry.request === "attach" && entry.authenticationRef === undefined
          ? "An attach configuration requires an authentication reference."
          : null,
    });
    const configurationDigest = digest(JSON.stringify(configuration));
    return IdeDapDiscoveredConfigurationSchema.make({
      configuration,
      resolution: IdeDapAdapterResolutionSchema.make({
        configurationRef,
        configurationDigest,
        executable: adapterExecutable(root, entry.adapterExecutable),
        argv: entry.adapterArguments,
        cwd,
        environment,
        adapterId: entry.adapterType,
        startCommand: entry.request,
        startArguments: entry.startArguments,
      }),
    });
  });
};
