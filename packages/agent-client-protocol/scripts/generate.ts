import { execFileSync } from "node:child_process";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { resolve } from "node:path";

import { SCHEMA_RELEASE, SDK_AUTHORITY, UPSTREAM_ASSETS, WIRE_VERSION } from "./source.ts";

type Meta = Readonly<{
  version: number;
  agentMethods: Readonly<Record<string, string>>;
  clientMethods: Readonly<Record<string, string>>;
  protocolMethods: Readonly<Record<string, string>>;
}>;

type SchemaNode = boolean | Readonly<Record<string, unknown>>;
type JsonSchema = Readonly<{ $defs: Readonly<Record<string, SchemaNode>> }>;
type Direction = "client-to-agent" | "agent-to-client" | "protocol";
type Kind = "request" | "notification" | "request-or-notification";

type MethodPolicy = Readonly<{
  kind: Kind;
  capability: string;
  paramsSchema: string | null;
  responseSchema: string | null;
}>;

const stablePolicy: Readonly<Record<string, MethodPolicy>> = {
  "client-to-agent:initialize": {
    kind: "request",
    capability: "baseline",
    paramsSchema: "InitializeRequest",
    responseSchema: "InitializeResponse",
  },
  "client-to-agent:authenticate": {
    kind: "request",
    capability: "initialize.authMethods contains methodId",
    paramsSchema: "AuthenticateRequest",
    responseSchema: "AuthenticateResponse",
  },
  "client-to-agent:session/new": {
    kind: "request",
    capability: "baseline",
    paramsSchema: "NewSessionRequest",
    responseSchema: "NewSessionResponse",
  },
  "client-to-agent:session/load": {
    kind: "request",
    capability: "agentCapabilities.loadSession",
    paramsSchema: "LoadSessionRequest",
    responseSchema: "LoadSessionResponse",
  },
  "client-to-agent:session/set_mode": {
    kind: "request",
    capability: "session advertises mode",
    paramsSchema: "SetSessionModeRequest",
    responseSchema: "SetSessionModeResponse",
  },
  "client-to-agent:session/set_config_option": {
    kind: "request",
    capability: "session advertises config option",
    paramsSchema: "SetSessionConfigOptionRequest",
    responseSchema: "SetSessionConfigOptionResponse",
  },
  "client-to-agent:session/prompt": {
    kind: "request",
    capability: "baseline",
    paramsSchema: "PromptRequest",
    responseSchema: "PromptResponse",
  },
  "client-to-agent:session/cancel": {
    kind: "notification",
    capability: "baseline",
    paramsSchema: "CancelNotification",
    responseSchema: null,
  },
  "client-to-agent:session/list": {
    kind: "request",
    capability: "agentCapabilities.sessionCapabilities.list",
    paramsSchema: "ListSessionsRequest",
    responseSchema: "ListSessionsResponse",
  },
  "client-to-agent:session/delete": {
    kind: "request",
    capability: "agentCapabilities.sessionCapabilities.delete",
    paramsSchema: "DeleteSessionRequest",
    responseSchema: "DeleteSessionResponse",
  },
  "client-to-agent:session/resume": {
    kind: "request",
    capability: "agentCapabilities.sessionCapabilities.resume",
    paramsSchema: "ResumeSessionRequest",
    responseSchema: "ResumeSessionResponse",
  },
  "client-to-agent:session/close": {
    kind: "request",
    capability: "agentCapabilities.sessionCapabilities.close",
    paramsSchema: "CloseSessionRequest",
    responseSchema: "CloseSessionResponse",
  },
  "client-to-agent:logout": {
    kind: "request",
    capability: "agentCapabilities.auth.logout",
    paramsSchema: "LogoutRequest",
    responseSchema: "LogoutResponse",
  },
  "agent-to-client:session/request_permission": {
    kind: "request",
    capability: "active session authority",
    paramsSchema: "RequestPermissionRequest",
    responseSchema: "RequestPermissionResponse",
  },
  "agent-to-client:session/update": {
    kind: "notification",
    capability: "baseline",
    paramsSchema: "SessionNotification",
    responseSchema: null,
  },
  "agent-to-client:fs/write_text_file": {
    kind: "request",
    capability: "clientCapabilities.fs.writeTextFile",
    paramsSchema: "WriteTextFileRequest",
    responseSchema: "WriteTextFileResponse",
  },
  "agent-to-client:fs/read_text_file": {
    kind: "request",
    capability: "clientCapabilities.fs.readTextFile",
    paramsSchema: "ReadTextFileRequest",
    responseSchema: "ReadTextFileResponse",
  },
  "agent-to-client:terminal/create": {
    kind: "request",
    capability: "clientCapabilities.terminal",
    paramsSchema: "CreateTerminalRequest",
    responseSchema: "CreateTerminalResponse",
  },
  "agent-to-client:terminal/output": {
    kind: "request",
    capability: "clientCapabilities.terminal",
    paramsSchema: "TerminalOutputRequest",
    responseSchema: "TerminalOutputResponse",
  },
  "agent-to-client:terminal/release": {
    kind: "request",
    capability: "clientCapabilities.terminal",
    paramsSchema: "ReleaseTerminalRequest",
    responseSchema: "ReleaseTerminalResponse",
  },
  "agent-to-client:terminal/wait_for_exit": {
    kind: "request",
    capability: "clientCapabilities.terminal",
    paramsSchema: "WaitForTerminalExitRequest",
    responseSchema: "WaitForTerminalExitResponse",
  },
  "agent-to-client:terminal/kill": {
    kind: "request",
    capability: "clientCapabilities.terminal",
    paramsSchema: "KillTerminalRequest",
    responseSchema: "KillTerminalResponse",
  },
  "protocol:$/cancel_request": {
    kind: "notification",
    capability: "in-flight request ownership",
    paramsSchema: "CancelRequestNotification",
    responseSchema: null,
  },
};

const unstablePolicy: Readonly<Record<string, Partial<MethodPolicy>>> = {
  "client-to-agent:providers/list": { kind: "request", capability: "agentCapabilities.providers" },
  "client-to-agent:providers/set": { kind: "request", capability: "agentCapabilities.providers" },
  "client-to-agent:providers/disable": {
    kind: "request",
    capability: "agentCapabilities.providers",
  },
  "client-to-agent:session/fork": {
    kind: "request",
    capability: "agentCapabilities.sessionCapabilities.fork",
  },
  "client-to-agent:mcp/message": {
    kind: "request-or-notification",
    capability: "agentCapabilities.mcpCapabilities.acp",
  },
  "agent-to-client:mcp/message": {
    kind: "request-or-notification",
    capability: "agentCapabilities.mcpCapabilities.acp",
  },
  "agent-to-client:mcp/connect": {
    kind: "request",
    capability: "agentCapabilities.mcpCapabilities.acp",
  },
  "agent-to-client:mcp/disconnect": {
    kind: "request",
    capability: "agentCapabilities.mcpCapabilities.acp",
  },
  "agent-to-client:elicitation/create": {
    kind: "request",
    capability: "clientCapabilities.elicitation",
  },
  "agent-to-client:elicitation/complete": {
    kind: "notification",
    capability: "clientCapabilities.elicitation.url",
  },
};

const packageRoot = resolve(import.meta.dirname, "..");
const outputFlag = process.argv.indexOf("--output-root");
const outputRoot = outputFlag >= 0 ? resolve(process.argv[outputFlag + 1] ?? "") : packageRoot;
const upstreamRoot = resolve(packageRoot, "upstream", SCHEMA_RELEASE);

const readJson = async <A>(name: string): Promise<A> =>
  JSON.parse(await readFile(resolve(upstreamRoot, name), "utf8")) as A;
const [stableMeta, unstableMeta, stableSchema, unstableSchema] = await Promise.all([
  readJson<Meta>("meta.json"),
  readJson<Meta>("meta.unstable.json"),
  readJson<JsonSchema>("schema.json"),
  readJson<JsonSchema>("schema.unstable.json"),
]);

if (stableMeta.version !== WIRE_VERSION || unstableMeta.version !== WIRE_VERSION) {
  throw new Error(`wire version drift: ${stableMeta.version}/${unstableMeta.version}`);
}

const entries = (meta: Meta): Array<Readonly<{ direction: Direction; method: string }>> => [
  ...Object.values(meta.agentMethods).map((method) => ({
    direction: "client-to-agent" as const,
    method,
  })),
  ...Object.values(meta.clientMethods).map((method) => ({
    direction: "agent-to-client" as const,
    method,
  })),
  ...Object.values(meta.protocolMethods).map((method) => ({
    direction: "protocol" as const,
    method,
  })),
];

const stableKeys = new Set(
  entries(stableMeta).map((entry) => `${entry.direction}:${entry.method}`),
);
const makeMembers = (meta: Meta, lane: "stable" | "unstable") =>
  entries(meta).map((entry) => {
    const key = `${entry.direction}:${entry.method}`;
    const policy = stablePolicy[key];
    if (lane === "stable" && policy === undefined)
      throw new Error(`stable method lacks reviewed policy: ${key}`);
    const override = unstablePolicy[key];
    return {
      direction: entry.direction,
      method: entry.method,
      kind:
        policy?.kind ??
        override?.kind ??
        (entry.method.startsWith("document/") ||
        entry.method.startsWith("nes/accept") ||
        entry.method.startsWith("nes/reject")
          ? "notification"
          : "request"),
      stability: stableKeys.has(key) ? "stable" : "unstable",
      requiredCapability:
        policy?.capability ?? override?.capability ?? "peer-profile capability required",
      paramsSchema: policy?.paramsSchema ?? null,
      responseSchema: policy?.responseSchema ?? null,
      supportState:
        lane === "stable" ? "codec-ready" : stableKeys.has(key) ? "codec-ready" : "profile-gated",
    };
  });

const manifest = (lane: "stable" | "unstable", meta: Meta) => ({
  protocol: "Agent Client Protocol",
  schemaRelease: SCHEMA_RELEASE,
  wireVersion: WIRE_VERSION,
  lane,
  sourceSha256:
    lane === "stable"
      ? UPSTREAM_ASSETS["schema.json"].sha256
      : UPSTREAM_ASSETS["schema.unstable.json"].sha256,
  sdk: SDK_AUTHORITY,
  members: makeMembers(meta, lane),
});

const stableManifest = manifest("stable", stableMeta);
const unstableManifest = manifest("unstable", unstableMeta);
const stableTypes = Object.keys(stableSchema.$defs).toSorted();
const unstableTypes = Object.keys(unstableSchema.$defs)
  .filter((name) => !(name in stableSchema.$defs))
  .toSorted();

const banner = (lane: "stable" | "unstable") => {
  const asset =
    lane === "stable" ? UPSTREAM_ASSETS["schema.json"] : UPSTREAM_ASSETS["schema.unstable.json"];
  return [
    `Generated from Agent Client Protocol ${SCHEMA_RELEASE} ${lane} schema.`,
    `Source: ${asset.url}`,
    `SHA-256: ${asset.sha256}`,
    "Generate: pnpm --dir packages/agent-client-protocol generate",
    "License: Apache-2.0; see THIRD_PARTY_NOTICES.md and upstream/schema-v1.19.0/LICENSE.",
    "Do not edit.",
  ].join("\n");
};

const identifier = (reference: string): string => {
  const segment = reference.split("/").at(-1);
  if (segment === undefined) throw new Error(`invalid schema reference: ${reference}`);
  return segment.replaceAll("~1", "/").replaceAll("~0", "~");
};

const literal = (value: unknown): string =>
  value === null || ["string", "number", "boolean"].includes(typeof value)
    ? JSON.stringify(value)
    : "unknown";

const parenthesize = (value: string): string =>
  value.includes(" | ") || value.includes(" & ") ? `(${value})` : value;

const emitStructuralType = (schema: Readonly<Record<string, unknown>>): string => {
  switch (schema.type) {
    case "null":
      return "null";
    case "boolean":
      return "boolean";
    case "integer":
    case "number":
      return "number";
    case "string":
      return "string";
    case "array": {
      const item = (schema.items ?? true) as SchemaNode;
      return `Array<${emitSchemaType(item)}>`;
    }
    case "object": {
      const properties =
        schema.properties !== null && typeof schema.properties === "object"
          ? (schema.properties as Readonly<Record<string, SchemaNode>>)
          : {};
      const required = new Set(
        Array.isArray(schema.required)
          ? schema.required.filter((name): name is string => typeof name === "string")
          : [],
      );
      const members = Object.entries(properties).map(
        ([name, property]) =>
          `${JSON.stringify(name)}${required.has(name) ? "" : "?"}: ${emitSchemaType(property)};`,
      );
      if (schema.additionalProperties !== false) {
        const additional =
          schema.additionalProperties === undefined || schema.additionalProperties === true
            ? "unknown"
            : emitSchemaType(schema.additionalProperties as SchemaNode);
        members.push(`[key: string]: ${parenthesize(additional)};`);
      }
      return members.length === 0 ? "Record<string, unknown>" : `{ ${members.join(" ")} }`;
    }
    default:
      return "unknown";
  }
};

const emitSchemaType = (schema: SchemaNode): string => {
  if (schema === true) return "unknown";
  if (schema === false) return "never";

  const parts: Array<string> = [];
  if (typeof schema.$ref === "string") parts.push(identifier(schema.$ref));
  if ("const" in schema) parts.push(literal(schema.const));
  if (Array.isArray(schema.enum)) parts.push(schema.enum.map(literal).join(" | ") || "never");

  const alternatives = schema.anyOf ?? schema.oneOf;
  if (Array.isArray(alternatives)) {
    parts.push(
      parenthesize(alternatives.map((member) => emitSchemaType(member as SchemaNode)).join(" | ")),
    );
  }
  if (Array.isArray(schema.allOf)) {
    parts.push(...schema.allOf.map((member) => emitSchemaType(member as SchemaNode)));
  }

  const hasStructuralKeywords =
    "type" in schema ||
    "properties" in schema ||
    "required" in schema ||
    "additionalProperties" in schema;
  if (hasStructuralKeywords) {
    if (Array.isArray(schema.type)) {
      parts.push(
        parenthesize(
          schema.type.map((type) => emitStructuralType({ ...schema, type })).join(" | "),
        ),
      );
    } else {
      parts.push(emitStructuralType(schema));
    }
  }

  return parts.length === 0 ? "unknown" : parts.map(parenthesize).join(" & ");
};

const stableTypeModule = `/*\n${banner("stable")}\nThese structural types are generated directly from the stable artifact; no unstable SDK types enter ./stable.\n*/\n${stableTypes
  .map((name) => `export type ${name} = ${emitSchemaType(stableSchema.$defs[name] ?? false)}`)
  .join("\n")}\n`;
const unstableTypeModule = `/*\n${banner("unstable")}\nThe official SDK 1.2.1 is generated from this exact unstable artifact; these aliases never enter ./stable.\n*/\n${unstableTypes.map((name) => `export type ${name} = import("@agentclientprotocol/sdk").${name}`).join("\n")}\n`;
const methodsModule = `/*\n${banner("stable")}\nMethod metadata also compares the separately pinned unstable meta artifact.\n*/\nexport const STABLE_METHOD_MANIFEST = ${JSON.stringify(stableManifest, null, 2)} as const\n\nexport const UNSTABLE_METHOD_MANIFEST = ${JSON.stringify(unstableManifest, null, 2)} as const\n`;
const definitionsModule = `/*\n${banner("stable")}\n*/\nexport const STABLE_DEFINITION_NAMES = ${JSON.stringify(stableTypes, null, 2)} as const\nexport const UNSTABLE_ONLY_DEFINITION_NAMES = ${JSON.stringify(unstableTypes, null, 2)} as const\n`;

await Promise.all([
  mkdir(resolve(outputRoot, "src", "generated"), { recursive: true }),
  mkdir(resolve(outputRoot, "manifests"), { recursive: true }),
]);
const generatedFiles = [
  "src/generated/stable-types.ts",
  "src/generated/unstable-types.ts",
  "src/generated/methods.ts",
  "src/generated/definitions.ts",
  "manifests/stable.json",
  "manifests/unstable.json",
] as const;
const contents = [
  stableTypeModule,
  unstableTypeModule,
  methodsModule,
  definitionsModule,
  `${JSON.stringify(stableManifest, null, 2)}\n`,
  `${JSON.stringify(unstableManifest, null, 2)}\n`,
] as const;
await Promise.all(
  generatedFiles.map((file, index) => writeFile(resolve(outputRoot, file), contents[index])),
);

const repositoryRoot = resolve(packageRoot, "..", "..");
const formatter = resolve(repositoryRoot, "node_modules", ".bin", "vp");
execFileSync(formatter, ["fmt", ...generatedFiles.map((file) => resolve(outputRoot, file))], {
  cwd: repositoryRoot,
  stdio: "pipe",
});

console.log(
  `Generated ${stableTypes.length} stable types, ${unstableTypes.length} unstable-only types, and ${stableManifest.members.length}/${unstableManifest.members.length} method entries.`,
);
