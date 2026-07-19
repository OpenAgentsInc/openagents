import { existsSync, readFileSync, readdirSync } from "node:fs";
import path from "node:path";

const appRoot = path.resolve(import.meta.dirname, "..");
const repositoryRoot = path.resolve(appRoot, "../..");
const ignoredSourceDirectories = new Set([
  ".git",
  "benchmarks",
  "dist",
  "node_modules",
  "projects",
]);

type BoundaryViolation = Readonly<{
  file: string;
  line: number;
  rule: string;
  detail: string;
}>;

const sourceFiles = (root: string): ReadonlyArray<string> => {
  if (!existsSync(root)) return [];
  return readdirSync(root, { withFileTypes: true }).flatMap((entry) => {
    const candidate = path.join(root, entry.name);
    if (entry.isDirectory()) {
      return ignoredSourceDirectories.has(entry.name) ? [] : sourceFiles(candidate);
    }
    return /\.(?:ts|tsx|rs)$/u.test(entry.name) ? [candidate] : [];
  });
};

const relative = (file: string): string => path.relative(repositoryRoot, file);

export const isHandMirroredBoundaryDeclaration = (sourceLine: string): boolean =>
  /^export\s+(?:type|interface)\s+\w+(?![^=]*=\s*typeof\s+\w+(?:Schema)?\.Type)/u.test(sourceLine);

const cursorRendererAuthorityPatterns = [
  ['from "node:', "node-host-import"],
  ["from 'node:", "node-host-import"],
  ['from "electron"', "electron-host-import"],
  ["from 'electron'", "electron-host-import"],
  ["workspace-service", "workspace-service-import"],
  ["openIdeCursorHost", "cursor-host-construction"],
  ["cursor-provider.ts", "cursor-provider-access"],
  ["IdeCursorDocumentAuthority", "document-authority-access"],
  [".executeEdits(", "direct-monaco-mutation"],
  [".pushEditOperations(", "direct-monaco-mutation"],
  [".applyEdits(", "direct-monaco-mutation"],
  [".setValue(", "direct-monaco-mutation"],
  ["writeFile", "filesystem-mutation"],
  ["renameSync", "filesystem-mutation"],
  ["unlinkSync", "filesystem-mutation"],
] as const;

export const cursorRendererAuthorityViolations = (source: string): ReadonlyArray<string> =>
  cursorRendererAuthorityPatterns
    .filter(([needle]) => source.includes(needle))
    .map(([, rule]) => rule);

const addMatches = (
  violations: BoundaryViolation[],
  file: string,
  rule: string,
  pattern: RegExp,
  detail: string,
): void => {
  const lines = readFileSync(file, "utf8").split("\n");
  lines.forEach((line, index) => {
    pattern.lastIndex = 0;
    if (pattern.test(line))
      violations.push({ file: relative(file), line: index + 1, rule, detail });
  });
};

export const inspectIdeBoundaries = (): ReadonlyArray<BoundaryViolation> => {
  const violations: BoundaryViolation[] = [];
  const ideRoot = path.join(appRoot, "src", "ide");
  const ideSources = sourceFiles(ideRoot).filter(
    (file) => file.endsWith(".ts") || file.endsWith(".tsx"),
  );
  const contracts = [
    path.join(ideRoot, "project-contract.ts"),
    path.join(ideRoot, "path-index-contract.ts"),
    path.join(ideRoot, "monaco-document-contract.ts"),
    path.join(ideRoot, "agent-code-contract.ts"),
    path.join(ideRoot, "cursor-contract.ts"),
    path.join(ideRoot, "cursor-benchmark-contract.ts"),
    path.join(appRoot, "src", "workspace-contract.ts"),
  ];
  const widgetProjectionFiles = new Set([
    path.join(ideRoot, "editor-runtime-entry.ts"),
    path.join(ideRoot, "pierre-diffs-adapter.tsx"),
    path.join(ideRoot, "spike", "entry.tsx"),
  ]);

  for (const file of contracts) {
    addMatches(
      violations,
      file,
      "schema-derived-contract-types",
      /^export\s+(?:type|interface)\s+\w+(?![^=]*=\s*typeof\s+\w+(?:Schema)?\.Type)/u,
      "Persisted/wire contract types must derive from an Effect Schema Type.",
    );
    addMatches(
      violations,
      file,
      "no-schema-class-boundaries",
      /Schema\.(?:Class|TaggedClass)\b/u,
      "Boundary data uses Struct/TaggedStruct/TaggedUnion; expected failures use TaggedErrorClass.",
    );
  }

  for (const file of ideSources) {
    addMatches(
      violations,
      file,
      "no-unsafe-casts",
      /\bas\s+(?:any|unknown)\b/u,
      "IDE authority code may not recover type safety through unchecked casts.",
    );
    if (!widgetProjectionFiles.has(file)) {
      addMatches(
        violations,
        file,
        "no-widget-authority",
        /from\s+["'](?:monaco-editor|@pierre\/)/u,
        "Monaco and Pierre imports are allowed only in the named projection adapter and contained admission fixture.",
      );
    }
  }

  const projectContract = path.join(ideRoot, "project-contract.ts");
  addMatches(
    violations,
    projectContract,
    "effect-typescript-authority",
    /from\s+["']node:/u,
    "The portable IDE project contract cannot depend on host process APIs.",
  );

  const projectService = readFileSync(path.join(ideRoot, "project-service.ts"), "utf8");
  for (const [needle, rule] of [
    ["Context.Service", "context-service"],
    ["Layer.effect", "layer-effect"],
    ["Effect.fn", "named-effect-functions"],
    ["Schema.TaggedErrorClass", "typed-expected-errors"],
    ["Effect.addFinalizer", "scoped-teardown"],
    ["Schema.decodeUnknownEffect", "boundary-decode"],
  ] as const) {
    if (!projectService.includes(needle)) {
      violations.push({
        file: relative(path.join(ideRoot, "project-service.ts")),
        line: 1,
        rule,
        detail: `Required Effect service primitive is missing: ${needle}.`,
      });
    }
  }

  const pathIndexService = readFileSync(path.join(ideRoot, "path-index-service.ts"), "utf8");
  for (const [needle, rule] of [
    ["Context.Service", "path-index-context-service"],
    ["Layer.effect", "path-index-layer-effect"],
    ["Effect.fn", "path-index-named-effect-functions"],
    ["Schema.TaggedErrorClass", "path-index-typed-expected-errors"],
    ["Effect.addFinalizer", "path-index-scoped-teardown"],
    ["Schema.decodeUnknownEffect", "path-index-boundary-decode"],
  ] as const) {
    if (!pathIndexService.includes(needle)) {
      violations.push({
        file: relative(path.join(ideRoot, "path-index-service.ts")),
        line: 1,
        rule,
        detail: `Required path-index Effect service primitive is missing: ${needle}.`,
      });
    }
  }

  const agentCodeServicePath = path.join(ideRoot, "agent-code-service.ts");
  const agentCodeService = readFileSync(agentCodeServicePath, "utf8");
  for (const [needle, rule] of [
    ["Context.Service", "agent-code-context-service"],
    ["Layer.effect", "agent-code-layer-effect"],
    ["Effect.fn", "agent-code-named-effect-functions"],
    ["Schema.TaggedErrorClass", "agent-code-typed-expected-errors"],
    ["Effect.addFinalizer", "agent-code-scoped-teardown"],
    ["Schema.decodeUnknownEffect", "agent-code-boundary-decode"],
  ] as const) {
    if (!agentCodeService.includes(needle)) {
      violations.push({
        file: relative(agentCodeServicePath),
        line: 1,
        rule,
        detail: `Required agent-code Effect service primitive is missing: ${needle}.`,
      });
    }
  }

  const agentContractSource = readFileSync(path.join(ideRoot, "agent-code-contract.ts"), "utf8");
  for (const forbidden of ["absolutePath", "rootPath", "providerPayload", "RecordEvidence:"]) {
    if (agentContractSource.includes(forbidden)) {
      violations.push({
        file: "apps/openagents-desktop/src/ide/agent-code-contract.ts",
        line: 1,
        rule: "agent-code-public-boundary",
        detail: `The public agent-code boundary may not expose ${forbidden}.`,
      });
    }
  }

  const cursorServicePath = path.join(ideRoot, "cursor-service.ts");
  const cursorService = readFileSync(cursorServicePath, "utf8");
  for (const [needle, rule] of [
    ["Context.Service", "cursor-context-service"],
    ["Layer.effect", "cursor-layer-effect"],
    ["Effect.fn", "cursor-named-effect-functions"],
    ["Schema.TaggedErrorClass", "cursor-typed-expected-errors"],
    ["Effect.addFinalizer", "cursor-scoped-teardown"],
    ["Schema.decodeUnknownEffect", "cursor-boundary-decode"],
  ] as const) {
    if (!cursorService.includes(needle)) {
      violations.push({
        file: relative(cursorServicePath),
        line: 1,
        rule,
        detail: `Required cursor Effect service primitive is missing: ${needle}.`,
      });
    }
  }

  const cursorProviderPath = path.join(ideRoot, "cursor-provider.ts");
  const cursorProvider = readFileSync(cursorProviderPath, "utf8");
  for (const [needle, rule] of [
    ["Context.Service", "cursor-provider-context-service"],
    ["Schema.TaggedErrorClass", "cursor-provider-typed-errors"],
    ["Stream.Stream<unknown", "cursor-provider-unknown-output"],
  ] as const) {
    if (!cursorProvider.includes(needle)) {
      violations.push({
        file: relative(cursorProviderPath),
        line: 1,
        rule,
        detail: `Required cursor provider boundary primitive is missing: ${needle}.`,
      });
    }
  }
  for (const forbidden of [
    'from "node:', "from 'node:", 'from "electron"', "from 'electron'",
    "workspace-service", "monaco-editor", "@pierre/",
    "writeFile", "renameSync", "unlinkSync",
  ]) {
    if (cursorProvider.includes(forbidden) || cursorService.includes(forbidden)) {
      violations.push({
        file: cursorProvider.includes(forbidden) ? relative(cursorProviderPath) : relative(cursorServicePath),
        line: 1,
        rule: "cursor-core-authority-perimeter",
        detail: `Cursor provider/service code acquired forbidden host or widget authority through ${forbidden}.`,
      });
    }
  }
  if (cursorProvider.includes("IdeCursorDocumentAuthority")) {
    violations.push({
      file: relative(cursorProviderPath),
      line: 1,
      rule: "cursor-provider-document-authority",
      detail: "The cursor provider adapter cannot acquire canonical document authority.",
    });
  }

  const cursorRendererFiles = sourceFiles(path.join(appRoot, "src", "renderer"))
    .filter((file) => /(?:^|[/\\])(?:react-)?cursor[^/\\]*\.tsx?$/u.test(file));
  for (const file of cursorRendererFiles) {
    const source = readFileSync(file, "utf8");
    for (const rule of cursorRendererAuthorityViolations(source)) {
      violations.push({
        file: relative(file),
        line: 1,
        rule: `cursor-renderer-${rule}`,
        detail: "Cursor renderer code is a decoded projection and cannot acquire document, provider, host, or direct Monaco mutation authority.",
      });
    }
  }
  const agentRendererFiles = [
    path.join(appRoot, "src", "renderer", "ide", "agent-code.ts"),
    path.join(appRoot, "src", "renderer", "ide", "agent-code-review.ts"),
    path.join(appRoot, "src", "renderer", "react-agent-code.tsx"),
    path.join(appRoot, "src", "renderer", "react-agent-context.tsx"),
  ];
  for (const file of agentRendererFiles) {
    const source = readFileSync(file, "utf8");
    for (const forbidden of ['from "node:', 'from "electron"', "workspace-service", "writeFile", "renameSync", "unlinkSync"]) {
      if (source.includes(forbidden)) {
        violations.push({
          file: relative(file),
          line: 1,
          rule: "agent-renderer-projection-only",
          detail: `Agent renderer projection acquired forbidden authority through ${forbidden}.`,
        });
      }
    }
  }

  const monacoRuntime = path.join(ideRoot, "editor-runtime-entry.ts");
  const monacoRuntimeSource = readFileSync(monacoRuntime, "utf8");
  for (const forbidden of [
    'from "node:',
    "from 'node:",
    "openWorkspaceDocument",
    "saveWorkspaceDocument",
    "workspace.grant",
    "absolutePath",
  ]) {
    if (monacoRuntimeSource.includes(forbidden)) {
      violations.push({
        file: relative(monacoRuntime),
        line: 1,
        rule: "monaco-projection-only",
        detail: `The Monaco island may not acquire host/file authority through ${forbidden}.`,
      });
    }
  }
  for (const required of [
    "inmemory://openagents/",
    "IdeMonacoDocumentEventSchema",
    "tokyoNightMonacoThemeData",
    "runtimeState === \"stopped\"",
  ]) {
    if (!monacoRuntimeSource.includes(required)) {
      violations.push({
        file: relative(monacoRuntime),
        line: 1,
        rule: "monaco-bounded-runtime",
        detail: `The Monaco island is missing its bounded projection invariant: ${required}.`,
      });
    }
  }

  const reactEditorSource = readFileSync(
    path.join(appRoot, "src", "renderer", "react-workspace-surfaces.tsx"),
    "utf8",
  );
  if (/className=["']oa-react-editor-textarea["']/u.test(reactEditorSource)) {
    violations.push({
      file: "apps/openagents-desktop/src/renderer/react-workspace-surfaces.tsx",
      line: 1,
      rule: "production-monaco-only",
      detail: "The production React editor may not regress to the legacy textarea.",
    });
  }

  const recoverySource = readFileSync(
    path.join(appRoot, "src", "renderer", "workspace-editor.ts"),
    "utf8",
  );
  if (
    !/type\s+WorkspaceEditorRecoverySnapshot\s*=\s*typeof\s+WorkspaceEditorRecoverySnapshotSchema\.Type/u.test(
      recoverySource,
    )
  ) {
    violations.push({
      file: "apps/openagents-desktop/src/renderer/workspace-editor.ts",
      line: 1,
      rule: "schema-derived-recovery",
      detail: "The persisted editor recovery shape must derive from its Effect Schema.",
    });
  }

  for (const file of sourceFiles(repositoryRoot).filter((candidate) => candidate.endsWith(".rs"))) {
    if (readFileSync(file, "utf8").includes("openagents.desktop.ide-project.v1")) {
      violations.push({
        file: relative(file),
        line: 1,
        rule: "no-rust-contract-mirror",
        detail:
          "Rust may implement bounded helpers but cannot mirror the authoritative IDE graph schema.",
      });
    }
  }

  return violations;
};

const main = (): void => {
  const violations = inspectIdeBoundaries();
  if (violations.length === 0) {
    console.log("[ide-boundaries] PASS — schema-first Effect authority is intact");
    return;
  }
  console.error(`[ide-boundaries] FAIL — ${violations.length} violation(s)`);
  for (const violation of violations) {
    console.error(`${violation.file}:${violation.line} [${violation.rule}] ${violation.detail}`);
  }
  process.exitCode = 1;
};

if (import.meta.url === `file://${process.argv[1]}`) main();
