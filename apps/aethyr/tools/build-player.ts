#!/usr/bin/env -S npx tsx
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { createHash, randomUUID } from "node:crypto";
import { mkdir, readFile, realpath, rm, writeFile } from "node:fs/promises";
import { dirname, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { build, type Plugin } from "vite";
import {
  checkByteBudgets,
  type PlayerByteBudget,
} from "../experiments/c-town/measurement.mjs";
import {
  PLAYER_ARTIFACT_SCHEMA,
  playerProfileFeatures,
  verifyPlayerArtifact,
  type PlayerArtifact,
  type PlayerFeature,
  type PlayerProfile,
} from "../packages/runtime/player-artifact.ts";
import {
  auditPlayerBuild,
  type PlayerBuildRecord,
} from "./audit-player-build.ts";
import {
  auditPlayerPrivateProperties,
  playerPrivatePropertyMinify,
  type PropertyModule,
} from "./player-private-properties.ts";

const root = await realpath(fileURLToPath(new URL("..", import.meta.url)));
const hash = (bytes: Uint8Array | string) =>
  createHash("sha256").update(bytes).digest("hex");
const encode = (value: unknown) =>
  new TextEncoder().encode(JSON.stringify(value, null, 2) + "\n");
interface Preparation {
  profile: "room" | "character";
  worldId: string;
  sourceIdentity: string;
  worldURL: string;
  packagePath: string;
  cookedRoot: string;
  resources: Partial<Record<PlayerFeature, string[]>>;
}
interface Emitted {
  type: "asset" | "chunk";
  fileName: string;
  source?: string | Uint8Array;
  code?: string;
  modules?: Record<string, { renderedLength?: number }>;
  imports?: string[];
  dynamicImports?: string[];
  referencedFiles?: string[];
  isEntry?: boolean;
}
function emittedRecord(value: Emitted): PlayerBuildRecord {
  return {
    fileName: value.fileName,
    type: value.type,
    ...(value.modules
      ? {
          modules: Object.entries(value.modules)
            .filter(([, info]) => (info.renderedLength ?? 1) > 0)
            .map(([id]) => id),
        }
      : {}),
    imports: value.imports ?? [],
    dynamicImports: value.dynamicImports ?? [],
    referencedFiles: value.referencedFiles ?? [],
    ...(value.isEntry ? { isEntry: true } : {}),
  };
}
function graphPlugin(
  records: Map<string, PlayerBuildRecord>,
  included: Map<string, PropertyModule>,
): Plugin {
  return {
    name: "aethyrforge-player-output-graph",
    generateBundle(_options, bundle) {
      for (const value of Object.values(bundle)) {
        records.set(value.fileName, emittedRecord(value as Emitted));
        if (value.type !== "chunk") continue;
        for (const [id, module] of Object.entries(value.modules)) {
          if ((module.renderedLength ?? 1) <= 0) continue;
          const code = this.getModuleInfo(id)?.code;
          assert(
            typeof code === "string",
            `Missing included player module: ${id}.`,
          );
          included.set(id, { id, code });
        }
      }
    },
  };
}
function featurePlugin(profile: PlayerProfile): Plugin {
  const original = resolve(root, "packages/renderer/features.ts");
  const replacement = resolve(
    root,
    `packages/renderer/${profile === "room" ? "room-features" : "player-features"}.ts`,
  );
  return {
    name: "aethyrforge-player-features",
    enforce: "pre",
    async resolveId(source, importer, options) {
      const found = await this.resolve(source, importer, {
        ...options,
        skipSelf: true,
      });
      return found?.id.split("?")[0] === original ? replacement : null;
    },
    ...(profile === "empty"
      ? {
          transformIndexHtml: {
            order: "pre" as const,
            handler: (html: string) =>
              html.replace("/apps/player/main.ts", "/apps/player/empty.ts"),
          },
        }
      : {}),
  };
}
function htmlReferences(html: string): string[] {
  return [...html.matchAll(/(?:src|href)=["']([^"']+)["']/g)]
    .map((match) => match[1])
    .filter((value) => !value.startsWith("#") && !value.startsWith("data:"))
    .map((value) => value.replace(/^\//, ""));
}
const args = process.argv.slice(2);
if (args.includes("--help")) {
  console.log(
    "Build minimal static players: npx tsx tools/build-player.ts [--out DIR] [--profiles empty,room,character] [--town PRIVATE_SOURCE] [--scratch PRIVATE_DIR]",
  );
  process.exit(0);
}
const options = new Map<string, string>();
for (let index = 0; index < args.length; index += 2) {
  assert(
    ["--out", "--profiles", "--town", "--scratch"].includes(args[index]) &&
      args[index + 1] &&
      !args[index + 1].startsWith("--"),
    "Choose named player build options with values.",
  );
  assert(!options.has(args[index]), "Choose each player build option once.");
  options.set(args[index], args[index + 1]);
}
const profiles = (options.get("--profiles") ?? "empty,room,character").split(
  ",",
) as PlayerProfile[];
assert(
  profiles.length > 0 &&
    new Set(profiles).size === profiles.length &&
    profiles.every((value) => ["empty", "room", "character"].includes(value)),
  "Choose distinct empty, room, and character profiles.",
);
const output = resolve(options.get("--out") ?? resolve(root, "dist/player"));
assert(
  output !== root && !root.startsWith(output + sep),
  "Choose a dedicated player build output directory.",
);
assert(
  !output.startsWith(root + sep) ||
    output.startsWith(resolve(root, "dist") + sep),
  "Keep generated player builds under dist or outside the repository.",
);
const scratch = resolve(
  options.get("--scratch") ??
    process.env.OPENAGENTS_SCRATCH ??
    execFileSync("openagents", ["scratch"], {
      cwd: root,
      encoding: "utf8",
    }).trim(),
);
assert(
  scratch !== root && !scratch.startsWith(root + sep),
  "Keep player preparation outside the repository.",
);
await mkdir(output, { recursive: true });
await mkdir(scratch, { recursive: true });
const privatePreparation = resolve(scratch, `player-build-${randomUUID()}`);
await mkdir(privatePreparation);
const summaries: Record<string, unknown>[] = [];
const budgetBytes = await readFile(
  resolve(root, "experiments/kaigen/player-budgets.json"),
);
const budgets = JSON.parse(budgetBytes.toString("utf8")) as {
  schema: string;
  profiles: Record<string, PlayerByteBudget>;
};
assert.equal(budgets.schema, "aethyrforge.player-byte-budgets.v1");
const selections = profiles.map((profile) => ({
  name: profile as string,
  profile,
  town: undefined as string | undefined,
}));
if (options.has("--town"))
  selections.push({
    name: "town",
    profile: "character",
    town: resolve(options.get("--town")!),
  });

for (const selection of selections) {
  const { name, profile, town } = selection;
  const started = performance.now();
  const target = resolve(output, name);
  await rm(target, { recursive: true, force: true });
  const generated = new Map<string, PlayerBuildRecord>(),
    workerRecords = new Map<string, PlayerBuildRecord>();
  const included = new Map<string, PropertyModule>();
  let preparation: Preparation | undefined;
  if (profile !== "empty") {
    const prepared = execFileSync(
      "python3",
      [
        resolve(root, "tools/assets/prepare-player.py"),
        "--profile",
        profile,
        "--out",
        resolve(privatePreparation, name),
        ...(town ? ["--town", town] : []),
      ],
      { cwd: root, encoding: "utf8", maxBuffer: 4 * 1024 * 1024 },
    );
    preparation = JSON.parse(prepared) as Preparation;
  }
  const built = await build({
    root,
    configFile: false,
    publicDir: false,
    base: "/",
    define: { "import.meta.env.VITE_PLAYER_PROFILE": JSON.stringify(profile) },
    plugins: [featurePlugin(profile), graphPlugin(generated, included)],
    worker: {
      format: "es",
      plugins: () => [
        featurePlugin(profile),
        graphPlugin(workerRecords, included),
      ],
    },
    build: {
      outDir: target,
      emptyOutDir: true,
      sourcemap: false,
      assetsInlineLimit: 0,
      rolldownOptions: {
        input: resolve(root, "player.html"),
        output: { minify: { mangleProps: playerPrivatePropertyMinify() } },
      },
    },
  });
  const emitted: PropertyModule[] = [];
  let mappings: Record<string, string | false> | undefined;
  for (const result of Array.isArray(built) ? built : [built]) {
    assert("output" in result, "The player build did not return output.");
    assert(
      result.mangleCache,
      "The player minifier did not return its private-property map.",
    );
    assert(
      !mappings,
      "Private-property player builds require one output group.",
    );
    mappings = result.mangleCache;
    for (const value of result.output)
      if (value.type === "chunk")
        emitted.push({ id: value.fileName, code: value.code });
  }
  const rendererPath = resolve(root, "packages/renderer/renderer.ts");
  const privateProperties = auditPlayerPrivateProperties({
    renderer: { id: rendererPath, code: await readFile(rendererPath, "utf8") },
    modules: [...included.values()],
    emitted,
    mappings: mappings!,
  });
  await writeFile(
    resolve(output, `${name}.private-properties.json`),
    encode(privateProperties),
  );
  for (const result of Array.isArray(built) ? built : [built])
    if ("output" in result)
      for (const value of result.output)
        generated.set(value.fileName, emittedRecord(value as Emitted));
  for (const [fileName, record] of workerRecords)
    generated.set(fileName, record);
  const files = new Map<string, Uint8Array>();
  for (const fileName of generated.keys())
    files.set(
      fileName,
      new Uint8Array(await readFile(resolve(target, fileName))),
    );
  const html = new TextDecoder().decode(files.get("player.html"));
  assert(html, "The player build did not emit player.html.");
  const references = htmlReferences(html);
  generated.set("player.html", {
    fileName: "player.html",
    type: "asset",
    referencedFiles: references,
  });
  const runtimeName = [...html.matchAll(/<script\b[^>]*src=["']([^"']+)["']/g)]
    .map((match) => match[1].replace(/^\//, ""))
    .find((fileName) => generated.get(fileName)?.type === "chunk");
  assert(
    runtimeName && files.has(runtimeName),
    "The player HTML needs its emitted runtime entrypoint.",
  );
  const runtime = files.get(runtimeName)!;
  const artifact: PlayerArtifact = {
    schema: PLAYER_ARTIFACT_SCHEMA,
    abi: 1,
    profile,
    worldId: preparation?.worldId ?? "empty-player",
    sourceIdentity:
      preparation?.sourceIdentity ?? hash("aethyrforge.empty-player.v1"),
    runtime: {
      abi: 1,
      url: "/" + runtimeName,
      sha256: hash(runtime),
      bytes: runtime.byteLength,
    },
    features: playerProfileFeatures(profile),
    resources: preparation?.resources ?? {},
  };
  const artifactReferences = [runtimeName];
  if (preparation) {
    const cooked = JSON.parse(await readFile(preparation.packagePath, "utf8"));
    artifact.content = {
      package: cooked,
      entryCell: cooked.entryCell,
      worldURL: preparation.worldURL,
    };
    for (const resource of cooked.files) {
      const fileName = resource.url.replace(/^\//, "");
      const source = resolve(preparation.cookedRoot, fileName);
      assert(
        source.startsWith(resolve(preparation.cookedRoot) + sep),
        "Cooked resources must remain in their output root.",
      );
      const bytes = new Uint8Array(await readFile(source));
      assert(
        bytes.byteLength === resource.bytes && hash(bytes) === resource.sha256,
        `The cooked resource changed: ${resource.url}.`,
      );
      const destination = resolve(target, fileName);
      await mkdir(dirname(destination), { recursive: true });
      await writeFile(destination, bytes);
      files.set(fileName, bytes);
      generated.set(fileName, { fileName, type: "asset" });
      artifactReferences.push(fileName);
    }
    const bytes = new Uint8Array(
      await readFile(resolve(root, "public/kernel.wasm")),
    );
    artifact.kernel = {
      abi: 1,
      url: "/kernel.wasm",
      sha256: hash(bytes),
      bytes: bytes.byteLength,
    };
    await writeFile(resolve(target, "kernel.wasm"), bytes);
    files.set("kernel.wasm", bytes);
    generated.set("kernel.wasm", { fileName: "kernel.wasm", type: "asset" });
    artifactReferences.push("kernel.wasm");
  }
  const checked = await verifyPlayerArtifact(artifact);
  const artifactBytes = encode(checked);
  await writeFile(resolve(target, "player-artifact.json"), artifactBytes);
  files.set("player-artifact.json", artifactBytes);
  generated.set("player-artifact.json", {
    fileName: "player-artifact.json",
    type: "asset",
    referencedFiles: artifactReferences,
  });
  const forbiddenModules: RegExp[] = [
    /\/experiments\//,
    /\/renderer-experiment\.ts$/,
  ];
  if (profile === "empty")
    forbiddenModules.push(
      /\/packages\/renderer\//,
      /\/packages\/runtime\/(?:kernel|world-session|starter-game)\.ts$/,
    );
  if (profile === "room")
    forbiddenModules.push(
      /\/packages\/renderer\/(?:character-renderer|character|sky|sky-shader)\.ts$/,
    );
  const graph = [...generated.values()];
  const audit = auditPlayerBuild({
    roots: ["player.html", "player-artifact.json"],
    records: graph,
    files,
    forbiddenModules,
  });
  await writeFile(resolve(output, `${name}.graph.json`), encode(graph));
  await writeFile(resolve(output, `${name}.audit.json`), encode(audit));
  assert(audit.ok, audit.errors.join("\n"));
  const budget = checkByteBudgets(audit, budgets.profiles[name]);
  await writeFile(resolve(output, `${name}.budget.json`), encode(budget));
  assert(budget.ok, budget.violations.join("\n"));
  const summary = {
    name,
    profile,
    worldId: checked.worldId,
    output: target,
    sourceIdentity: checked.sourceIdentity,
    runtime: checked.runtime,
    kernel: checked.kernel ?? null,
    features: checked.features,
    cookedDescriptor: checked.content?.package.sha256 ?? null,
    resourceCount: checked.content?.package.files.length ?? 0,
    audit,
    budget,
    privateProperties,
    elapsedMs: performance.now() - started,
  };
  summaries.push(summary);
  console.log(
    JSON.stringify({
      name,
      ok: true,
      files: audit.requests,
      js: audit.totals.js,
      wasm: audit.totals.wasm,
      assets: audit.totals.assets,
      elapsedMs: summary.elapsedMs,
    }),
  );
  // Release the large town buffers before building another profile.
  files.clear();
}
const room = summaries.find((item) => item.name === "room") as
  { audit: ReturnType<typeof auditPlayerBuild> } | undefined;
const character = summaries.find((item) => item.name === "character") as
  { audit: ReturnType<typeof auditPlayerBuild> } | undefined;
if (room && character) {
  assert(
    character.audit.modules.some((value) =>
      value.endsWith("/character-renderer.ts"),
    ),
    "The character profile did not include its character feature.",
  );
  assert(
    character.audit.modules.some((value) => value.endsWith("/sky-shader.ts")),
    "The character profile did not include its sky feature.",
  );
  assert(
    room.audit.totals.js.rawBytes < character.audit.totals.js.rawBytes,
    "Removing character and sky did not remove delivered JavaScript.",
  );
  assert(
    room.audit.totals.assets.rawBytes < character.audit.totals.assets.rawBytes,
    "Removing character and sky did not remove delivered data.",
  );
}
await writeFile(
  resolve(output, "summary.json"),
  encode({
    schema: "aethyrforge.player-builds.v1",
    compression: { gzipLevel: 9, brotliQuality: 6 },
    requestScope:
      "Static complete delivered dependency closure; browser requests and wire bytes are measured separately.",
    preparation: privatePreparation,
    profiles: summaries,
    byteBudgetSHA256: hash(budgetBytes),
  }),
);
