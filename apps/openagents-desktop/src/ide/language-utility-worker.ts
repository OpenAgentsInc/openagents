import { parentPort } from "node:worker_threads";
import { lstatSync, readFileSync, readdirSync, type Dirent } from "node:fs";
import path from "node:path";

import { Schema } from "effect";
import * as ts from "typescript";

import {
  IdeLanguageResultSchema,
  IdeLanguageItemRefSchema,
  IdeLanguageResultRefSchema,
  type IdeLanguageCapability,
  type IdeLanguageItem,
  type IdeLanguagePosition,
  type IdeLanguageRange,
  type IdeLanguageRequest,
  type IdeLanguageResult,
  type IdeLanguageResultState,
} from "./language-contract.ts";
import { IdeDiagnosticRefSchema, IdeSymbolRefSchema } from "./project-contract.ts";

type ServiceBinding = Readonly<{
  serviceRef: string;
  serviceGeneration: number;
  startRef: string;
  placementRef: string;
  executable: string;
  providerVersion: string;
}>;

type WorkerRequest = Readonly<{
  kind: "request";
  root: string;
  request: IdeLanguageRequest;
  service: ServiceBinding;
}>;

type WorkerCancel = Readonly<{
  kind: "cancel";
  requestRef: string;
  reason: "user" | "superseded" | "document_replaced" | "project_stopped";
}>;

type WorkerStop = Readonly<{ kind: "stop" }>;
type WorkerInput = WorkerRequest | WorkerCancel | WorkerStop;

type WorkerOutput =
  | Readonly<{ kind: "ready"; providerVersion: string }>
  | Readonly<{ kind: "result"; result: IdeLanguageResult }>
  | Readonly<{ kind: "failed"; requestRef: string; message: string; recoverable: boolean }>;

const port = parentPort;
if (port === null) throw new Error("IDE language utility requires a worker parent port.");

const ignoredDirectories = new Set([
  ".git",
  ".next",
  ".turbo",
  ".vite",
  "coverage",
  "dist",
  "node_modules",
  "target",
]);
const sourceExtensions = new Set([
  ".cjs",
  ".cts",
  ".js",
  ".jsx",
  ".mjs",
  ".mts",
  ".ts",
  ".tsx",
]);
const maximumProjectFiles = 2_000;
const maximumProjectBytes = 32 * 1024 * 1024;

const supportedCapabilities: ReadonlySet<IdeLanguageCapability> = new Set([
  "diagnostics",
  "completion",
  "completion_resolve",
  "hover",
  "definition",
  "declaration",
  "type_definition",
  "references",
  "document_symbols",
  "workspace_symbols",
  "rename_preview",
  "format_document",
  "format_range",
  "code_actions",
  "semantic_tokens",
  "inlay_hints",
  "folding_ranges",
]);

const providerCapabilities = [...supportedCapabilities].map(capability => ({
  capability,
  available: true,
  reason: null,
}));

const cancelled = new Map<string, WorkerCancel["reason"]>();
const queued = new Map<string, WorkerRequest>();
const queueOrder: Array<string> = [];
let draining = false;
let stopped = false;

const fnv1a = (value: string): string => {
  let hash = 0x811c9dc5;
  for (let index = 0; index < value.length; index += 1) {
    hash ^= value.charCodeAt(index);
    hash = Math.imul(hash, 0x01000193);
  }
  return (hash >>> 0).toString(36);
};

const itemRef = (request: IdeLanguageRequest, index: number) =>
  IdeLanguageItemRefSchema.make(`ide.language-item.${fnv1a(`${request.requestRef}:${index}`)}.${index}`);
const resultRef = (request: IdeLanguageRequest) =>
  IdeLanguageResultRefSchema.make(`ide.language-result.${fnv1a(String(request.requestRef))}`);
const diagnosticRef = (request: IdeLanguageRequest, index: number) =>
  IdeDiagnosticRefSchema.make(`ide.diagnostic.${fnv1a(`${request.requestRef}:${index}`)}.${index}`);
const symbolRef = (request: IdeLanguageRequest, index: number) =>
  IdeSymbolRefSchema.make(`ide.symbol.${fnv1a(`${request.requestRef}:${index}`)}.${index}`);

const normalizePathRef = (root: string, absolute: string): string | null => {
  const relative = path.relative(root, absolute).split(path.sep).join("/");
  if (
    relative === "" ||
    relative === "." ||
    relative.startsWith("../") ||
    path.isAbsolute(relative)
  ) return null;
  return relative;
};

const absoluteFor = (root: string, pathRef: string): string | null => {
  const absolute = path.resolve(root, ...pathRef.split("/"));
  return normalizePathRef(root, absolute) === pathRef ? absolute : null;
};

const scanProject = (root: string): Readonly<{
  files: ReadonlyMap<string, string>;
  truncated: boolean;
}> => {
  const files = new Map<string, string>();
  let bytes = 0;
  let truncated = false;
  const visit = (directory: string): void => {
    if (truncated) return;
    let entries: Array<Dirent<string>>;
    try {
      entries = readdirSync(directory, { withFileTypes: true, encoding: "utf8" });
    } catch {
      return;
    }
    for (const entry of entries) {
      if (truncated) break;
      if (ignoredDirectories.has(entry.name)) continue;
      const absolute = path.join(directory, entry.name);
      let stat: ReturnType<typeof lstatSync>;
      try {
        stat = lstatSync(absolute);
      } catch {
        continue;
      }
      if (stat.isSymbolicLink()) continue;
      if (stat.isDirectory()) {
        visit(absolute);
        continue;
      }
      if (!stat.isFile() || !sourceExtensions.has(path.extname(entry.name).toLowerCase())) continue;
      if (files.size >= maximumProjectFiles || bytes + stat.size > maximumProjectBytes) {
        truncated = true;
        break;
      }
      try {
        const content = readFileSync(absolute, "utf8");
        files.set(absolute, content);
        bytes += Buffer.byteLength(content);
      } catch {
        // Unreadable source remains absent; provider results cannot imply it was indexed.
      }
    }
  };
  visit(root);
  return { files, truncated };
};

const scriptKind = (fileName: string): ts.ScriptKind => {
  switch (path.extname(fileName).toLowerCase()) {
    case ".js":
    case ".cjs":
    case ".mjs":
      return ts.ScriptKind.JS;
    case ".jsx":
      return ts.ScriptKind.JSX;
    case ".tsx":
      return ts.ScriptKind.TSX;
    default:
      return ts.ScriptKind.TS;
  }
};

type ProjectLanguageService = {
  service: ts.LanguageService;
  files: Map<string, string>;
  versions: Map<string, string>;
  activeFile: string;
  truncated: boolean;
  update: (request: IdeLanguageRequest) => boolean;
  dispose: () => void;
};

const projects = new Map<string, ProjectLanguageService>();

const createProjectLanguageService = (
  root: string,
  request: IdeLanguageRequest,
): ProjectLanguageService | null => {
  const activeFile = absoluteFor(root, request.pathRef);
  if (activeFile === null) return null;
  const scanned = scanProject(root);
  const files = new Map(scanned.files);
  files.set(activeFile, request.content);
  const versions = new Map<string, string>();
  for (const fileName of files.keys()) versions.set(fileName, "1");
  versions.set(activeFile, String(request.documentVersion));
  const compilerOptions: ts.CompilerOptions = {
    allowJs: true,
    allowSyntheticDefaultImports: true,
    esModuleInterop: true,
    jsx: ts.JsxEmit.ReactJSX,
    module: ts.ModuleKind.ESNext,
    moduleResolution: ts.ModuleResolutionKind.Bundler,
    noEmit: true,
    skipLibCheck: true,
    strict: true,
    target: ts.ScriptTarget.ES2022,
  };
  const host: ts.LanguageServiceHost = {
    getCompilationSettings: () => compilerOptions,
    getCurrentDirectory: () => root,
    getDefaultLibFileName: options => ts.getDefaultLibFilePath(options),
    getScriptFileNames: () => [...files.keys()],
    getScriptKind: scriptKind,
    getScriptSnapshot: fileName => {
      const value = files.get(fileName) ?? ts.sys.readFile(fileName);
      return value === undefined ? undefined : ts.ScriptSnapshot.fromString(value);
    },
    getScriptVersion: fileName => versions.get(fileName) ?? "0",
    fileExists: ts.sys.fileExists,
    readFile: ts.sys.readFile,
    readDirectory: ts.sys.readDirectory,
    directoryExists: ts.sys.directoryExists,
    getDirectories: ts.sys.getDirectories,
    useCaseSensitiveFileNames: () => ts.sys.useCaseSensitiveFileNames,
    getNewLine: () => ts.sys.newLine,
  };
  const service = ts.createLanguageService(host, ts.createDocumentRegistry());
  const project: ProjectLanguageService = {
    service,
    files,
    versions,
    activeFile,
    truncated: scanned.truncated,
    update: next => {
      const nextActiveFile = absoluteFor(root, next.pathRef);
      if (nextActiveFile === null) return false;
      files.set(nextActiveFile, next.content);
      versions.set(nextActiveFile, String(next.documentVersion));
      project.activeFile = nextActiveFile;
      return true;
    },
    dispose: () => service.dispose(),
  };
  return project;
};

const projectFor = (root: string, request: IdeLanguageRequest): ProjectLanguageService | null => {
  const existing = projects.get(root);
  if (existing !== undefined) return existing.update(request) ? existing : null;
  const created = createProjectLanguageService(root, request);
  if (created !== null) projects.set(root, created);
  return created;
};

const positionAt = (content: string, offset: number): IdeLanguagePosition => {
  const bounded = Math.max(0, Math.min(content.length, Math.trunc(offset)));
  let line = 1;
  let lineStart = 0;
  for (let index = 0; index < bounded; index += 1) {
    if (content.charCodeAt(index) !== 10) continue;
    line += 1;
    lineStart = index + 1;
  }
  return { line, column: bounded - lineStart + 1, offset: bounded };
};

const rangeAt = (
  content: string,
  start: number,
  length: number,
): IdeLanguageRange => ({
  start: positionAt(content, start),
  end: positionAt(content, start + Math.max(0, length)),
});

const locationFor = (
  root: string,
  files: ReadonlyMap<string, string>,
  fileName: string,
  start: number,
  length: number,
): Readonly<{ pathRef: string; range: IdeLanguageRange; preview: string | null }> | null => {
  const pathRef = normalizePathRef(root, fileName);
  if (pathRef === null) return null;
  const content = files.get(fileName) ?? ts.sys.readFile(fileName);
  if (content === undefined) return null;
  const line = positionAt(content, start).line;
  const preview = content.split(/\r?\n/u)[line - 1]?.trim().slice(0, 1_000) ?? null;
  return { pathRef, range: rangeAt(content, start, length), preview };
};

const commonItem = (
  request: IdeLanguageRequest,
  index: number,
  pathRef: string,
  range: IdeLanguageRange | null,
) => ({
  itemRef: itemRef(request, index),
  resultRef: resultRef(request),
  pathRef,
  range,
});

const diagnosticSeverity = (category: ts.DiagnosticCategory) => {
  switch (category) {
    case ts.DiagnosticCategory.Error:
      return "error" as const;
    case ts.DiagnosticCategory.Warning:
      return "warning" as const;
    case ts.DiagnosticCategory.Suggestion:
      return "hint" as const;
    default:
      return "information" as const;
  }
};

const diagnostics = (
  root: string,
  request: IdeLanguageRequest,
  project: ProjectLanguageService,
): ReadonlyArray<IdeLanguageItem> => {
  const values = [
    ...project.service.getSyntacticDiagnostics(project.activeFile),
    ...project.service.getSemanticDiagnostics(project.activeFile),
    ...project.service.getSuggestionDiagnostics(project.activeFile),
  ];
  return values.flatMap((diagnostic, index) => {
    const fileName = diagnostic.file?.fileName ?? project.activeFile;
    const located = locationFor(
      root,
      project.files,
      fileName,
      diagnostic.start ?? 0,
      diagnostic.length ?? 0,
    );
    if (located === null) return [];
    return [{
      _tag: "Diagnostic" as const,
      ...commonItem(request, index, located.pathRef, located.range),
      diagnosticRef: diagnosticRef(request, index),
      severity: diagnosticSeverity(diagnostic.category),
      source: diagnostic.source ?? "typescript",
      code: String(diagnostic.code),
      message: ts.flattenDiagnosticMessageText(diagnostic.messageText, "\n").slice(0, 2_000),
    }];
  });
};

const completionItems = (
  request: IdeLanguageRequest,
  project: ProjectLanguageService,
): ReadonlyArray<IdeLanguageItem> => {
  const offset = request.position?.offset ?? 0;
  if (request.capability === "completion_resolve" && request.query !== null) {
    const details = project.service.getCompletionEntryDetails(
      project.activeFile,
      offset,
      request.query,
      {},
      undefined,
      {},
      undefined,
    );
    if (details === undefined) return [];
    return [{
      _tag: "Completion",
      ...commonItem(request, 0, request.pathRef, null),
      label: details.name.slice(0, 300),
      detail: ts.displayPartsToString(details.displayParts).slice(0, 1_000) || null,
      kind: details.kind,
      insertText: details.name.slice(0, 32_000),
      sortText: null,
    }];
  }
  const info = project.service.getCompletionsAtPosition(project.activeFile, offset, {
    includeCompletionsForModuleExports: true,
    includeCompletionsWithInsertText: true,
  });
  return (info?.entries ?? []).map((entry, index) => ({
    _tag: "Completion",
    ...commonItem(request, index, request.pathRef, null),
    label: entry.name.slice(0, 300),
    detail: entry.source?.slice(0, 1_000) ?? null,
    kind: entry.kind,
    insertText: entry.insertText?.slice(0, 32_000) ?? null,
    sortText: entry.sortText.slice(0, 300),
  }));
};

const hoverItems = (
  request: IdeLanguageRequest,
  project: ProjectLanguageService,
): ReadonlyArray<IdeLanguageItem> => {
  const info = project.service.getQuickInfoAtPosition(
    project.activeFile,
    request.position?.offset ?? 0,
  );
  if (info === undefined) return [];
  const markdown = [
    ts.displayPartsToString(info.displayParts),
    ts.displayPartsToString(info.documentation),
  ].filter(Boolean).join("\n\n").slice(0, 64_000);
  return [{
    _tag: "Hover",
    ...commonItem(
      request,
      0,
      request.pathRef,
      rangeAt(request.content, info.textSpan.start, info.textSpan.length),
    ),
    markdown,
  }];
};

const locationItems = (
  root: string,
  request: IdeLanguageRequest,
  project: ProjectLanguageService,
): ReadonlyArray<IdeLanguageItem> => {
  const position = request.position?.offset ?? 0;
  const relation = request.capability === "references" ? "reference"
    : request.capability === "type_definition" ? "type_definition"
      : request.capability === "declaration" ? "declaration"
        : "definition";
  const values = request.capability === "references"
    ? project.service.getReferencesAtPosition(project.activeFile, position) ?? []
    : request.capability === "type_definition"
      ? project.service.getTypeDefinitionAtPosition(project.activeFile, position) ?? []
      : request.capability === "declaration"
        ? project.service.getDefinitionAtPosition(project.activeFile, position) ?? []
        : project.service.getDefinitionAtPosition(project.activeFile, position) ?? [];
  return values.flatMap((value, index) => {
    const located = locationFor(
      root,
      project.files,
      value.fileName,
      value.textSpan.start,
      value.textSpan.length,
    );
    return located === null ? [] : [{
      _tag: "Location" as const,
      ...commonItem(request, index, located.pathRef, located.range),
      relation,
      preview: located.preview,
    }];
  });
};

const symbolsFromTree = (
  request: IdeLanguageRequest,
  pathRef: string,
  content: string,
  tree: ts.NavigationTree,
  startIndex: number,
): ReadonlyArray<IdeLanguageItem> => {
  const values: IdeLanguageItem[] = [];
  const visit = (node: ts.NavigationTree, depth: number, containerName: string | null): void => {
    if (node.text !== "<global>") {
      const span = node.spans[0] ?? { start: 0, length: 0 };
      const index = startIndex + values.length;
      values.push({
        _tag: "Symbol",
        ...commonItem(request, index, pathRef, rangeAt(content, span.start, span.length)),
        symbolRef: symbolRef(request, index),
        name: node.text.slice(0, 300),
        kind: node.kind,
        containerName,
        depth: Math.min(32, depth),
      });
    }
    const nextContainer = node.text === "<global>" ? containerName : node.text.slice(0, 300);
    for (const child of node.childItems ?? []) visit(child, depth + 1, nextContainer);
  };
  visit(tree, 0, null);
  return values;
};

const symbolItems = (
  root: string,
  request: IdeLanguageRequest,
  project: ProjectLanguageService,
): ReadonlyArray<IdeLanguageItem> => {
  if (request.capability === "document_symbols") {
    return symbolsFromTree(
      request,
      request.pathRef,
      request.content,
      project.service.getNavigationTree(project.activeFile),
      0,
    );
  }
  const query = request.query?.toLocaleLowerCase() ?? "";
  const values: IdeLanguageItem[] = [];
  for (const [fileName, content] of project.files) {
    if (values.length >= request.limit) break;
    const pathRef = normalizePathRef(root, fileName);
    if (pathRef === null) continue;
    const next = symbolsFromTree(
      request,
      pathRef,
      content,
      project.service.getNavigationTree(fileName),
      values.length,
    ).filter(item => item._tag !== "Symbol" || query === "" || item.name.toLocaleLowerCase().includes(query));
    values.push(...next);
  }
  return values;
};

const textEdit = (
  root: string,
  request: IdeLanguageRequest,
  project: ProjectLanguageService,
  index: number,
  fileName: string,
  span: ts.TextSpan,
  newText: string,
  editKind: "rename" | "format" | "code_action",
): IdeLanguageItem | null => {
  const located = locationFor(root, project.files, fileName, span.start, span.length);
  if (located === null) return null;
  return {
    _tag: "TextEdit",
    ...commonItem(request, index, located.pathRef, located.range),
    editKind,
    newText: newText.slice(0, 1_000_000),
    expectedDocumentGeneration: request.documentGeneration,
    expectedDocumentVersion: request.documentVersion,
  };
};

const renameItems = (
  root: string,
  request: IdeLanguageRequest,
  project: ProjectLanguageService,
): ReadonlyArray<IdeLanguageItem> => {
  const position = request.position?.offset ?? 0;
  const rename = project.service.getRenameInfo(project.activeFile, position, {
    allowRenameOfImportPath: false,
  });
  if (!rename.canRename || request.query === null || request.query.trim() === "") return [];
  const locations = project.service.findRenameLocations(
    project.activeFile,
    position,
    false,
    false,
    true,
  ) ?? [];
  return locations.flatMap((location, index) => {
    const edit = textEdit(
      root,
      request,
      project,
      index,
      location.fileName,
      location.textSpan,
      request.query!,
      "rename",
    );
    return edit === null ? [] : [edit];
  });
};

const formatItems = (
  root: string,
  request: IdeLanguageRequest,
  project: ProjectLanguageService,
): ReadonlyArray<IdeLanguageItem> => {
  const settings: ts.FormatCodeSettings = {
    convertTabsToSpaces: true,
    indentSize: 2,
    newLineCharacter: "\n",
    semicolons: ts.SemicolonPreference.Insert,
    tabSize: 2,
  };
  const edits = request.capability === "format_range" && request.range !== null
    ? project.service.getFormattingEditsForRange(
        project.activeFile,
        request.range.start.offset,
        request.range.end.offset,
        settings,
      )
    : project.service.getFormattingEditsForDocument(project.activeFile, settings);
  return edits.flatMap((edit, index) => {
    const item = textEdit(
      root,
      request,
      project,
      index,
      project.activeFile,
      edit.span,
      edit.newText,
      "format",
    );
    return item === null ? [] : [item];
  });
};

const codeActionItems = (
  root: string,
  request: IdeLanguageRequest,
  project: ProjectLanguageService,
): ReadonlyArray<IdeLanguageItem> => {
  const start = request.range?.start.offset ?? request.position?.offset ?? 0;
  const end = request.range?.end.offset ?? start;
  const codes = diagnostics(root, request, project)
    .filter(item => item._tag === "Diagnostic" && item.range !== null && item.range.start.offset <= end && item.range.end.offset >= start)
    .map(item => item._tag === "Diagnostic" ? Number(item.code) : 0)
    .filter(Number.isFinite);
  const fixes = project.service.getCodeFixesAtPosition(
    project.activeFile,
    start,
    end,
    [...new Set(codes)],
    {},
    {},
  );
  const items: IdeLanguageItem[] = [];
  for (const fix of fixes) {
    const actionIndex = items.length;
    const editCount = fix.changes.reduce((count, change) => count + change.textChanges.length, 0);
    items.push({
      _tag: "CodeAction",
      ...commonItem(request, actionIndex, request.pathRef, request.range),
      title: fix.description.slice(0, 500),
      actionKind: `typescript.fix.${fix.fixName}`.slice(0, 160),
      fixId: typeof fix.fixId === "string" ? fix.fixId.slice(0, 160) : null,
      editCount,
    });
    for (const change of fix.changes) {
      for (const changeEdit of change.textChanges) {
        const edit = textEdit(
          root,
          request,
          project,
          items.length,
          change.fileName,
          changeEdit.span,
          changeEdit.newText,
          "code_action",
        );
        if (edit !== null) items.push(edit);
      }
    }
  }
  return items;
};

const semanticItems = (
  request: IdeLanguageRequest,
  project: ProjectLanguageService,
): ReadonlyArray<IdeLanguageItem> => {
  const encoded = project.service.getEncodedSemanticClassifications(
    project.activeFile,
    { start: 0, length: request.content.length },
    ts.SemanticClassificationFormat.TwentyTwenty,
  );
  const values: IdeLanguageItem[] = [];
  for (let index = 0; index + 2 < encoded.spans.length; index += 3) {
    const start = encoded.spans[index] ?? 0;
    const length = encoded.spans[index + 1] ?? 0;
    const classification = encoded.spans[index + 2] ?? 0;
    values.push({
      _tag: "SemanticToken",
      ...commonItem(request, values.length, request.pathRef, rangeAt(request.content, start, length)),
      tokenType: `typescript.${classification >> 8}`,
      modifiers: [],
    });
  }
  return values;
};

const inlayItems = (
  request: IdeLanguageRequest,
  project: ProjectLanguageService,
): ReadonlyArray<IdeLanguageItem> => {
  const span = request.range === null
    ? { start: 0, length: request.content.length }
    : {
        start: request.range.start.offset,
        length: Math.max(0, request.range.end.offset - request.range.start.offset),
      };
  const hints = project.service.provideInlayHints(project.activeFile, span, {
    includeInlayEnumMemberValueHints: true,
    includeInlayFunctionLikeReturnTypeHints: true,
    includeInlayFunctionParameterTypeHints: true,
    includeInlayParameterNameHints: "all",
    includeInlayParameterNameHintsWhenArgumentMatchesName: false,
    includeInlayPropertyDeclarationTypeHints: true,
    includeInlayVariableTypeHints: true,
    includeInlayVariableTypeHintsWhenTypeMatchesName: false,
  });
  return hints.map((hint, index) => ({
    _tag: "InlayHint",
    ...commonItem(
      request,
      index,
      request.pathRef,
      rangeAt(request.content, hint.position, 0),
    ),
    label: hint.text.slice(0, 500),
    hintKind: hint.kind === ts.InlayHintKind.Type ? "type" as const
      : hint.kind === ts.InlayHintKind.Parameter ? "parameter" as const
        : "unknown" as const,
    paddingLeft: hint.whitespaceBefore === true,
    paddingRight: hint.whitespaceAfter === true,
  }));
};

const foldingItems = (
  request: IdeLanguageRequest,
  project: ProjectLanguageService,
): ReadonlyArray<IdeLanguageItem> => project.service
  .getOutliningSpans(project.activeFile)
  .map((span, index) => ({
    _tag: "FoldingRange" as const,
    ...commonItem(
      request,
      index,
      request.pathRef,
      rangeAt(request.content, span.textSpan.start, span.textSpan.length),
    ),
    foldKind: span.kind === "comment" ? "comment" as const
      : span.kind === "imports" ? "imports" as const
        : span.kind === "region" ? "region" as const
          : "code" as const,
  }));

const itemsFor = (
  root: string,
  request: IdeLanguageRequest,
  project: ProjectLanguageService,
): ReadonlyArray<IdeLanguageItem> => {
  switch (request.capability) {
    case "diagnostics":
      return diagnostics(root, request, project);
    case "completion":
    case "completion_resolve":
      return completionItems(request, project);
    case "hover":
      return hoverItems(request, project);
    case "definition":
    case "declaration":
    case "type_definition":
    case "references":
      return locationItems(root, request, project);
    case "document_symbols":
    case "workspace_symbols":
      return symbolItems(root, request, project);
    case "rename_preview":
      return renameItems(root, request, project);
    case "format_document":
    case "format_range":
      return formatItems(root, request, project);
    case "code_actions":
      return codeActionItems(root, request, project);
    case "semantic_tokens":
      return semanticItems(request, project);
    case "inlay_hints":
      return inlayItems(request, project);
    case "folding_ranges":
      return foldingItems(request, project);
  }
};

const unavailableResult = (
  input: WorkerRequest,
  reason: "unsupported_language" | "unsupported_capability" | "invalid_path" | "provider_failed",
  message: string,
): IdeLanguageResult => resultFor(input, {
  _tag: "Unavailable",
  reason,
  message,
  retry: reason === "provider_failed" ? "bounded_backoff" : "none",
}, []);

const resultFor = (
  input: WorkerRequest,
  state: IdeLanguageResultState,
  items: ReadonlyArray<IdeLanguageItem>,
): IdeLanguageResult => {
  const observedAt = new Date();
  const requestedAt = Date.parse(input.request.requestedAt);
  return Schema.decodeUnknownSync(IdeLanguageResultSchema)({
    schemaVersion: "openagents.desktop.ide-language-result.v1",
    resultRef: resultRef(input.request),
    requestRef: input.request.requestRef,
    capability: input.request.capability,
    projectRef: input.request.projectRef,
    rootRef: input.request.rootRef,
    worktreeRef: input.request.worktreeRef,
    attachmentRef: input.request.attachmentRef,
    attachmentGeneration: input.request.attachmentGeneration,
    languageGeneration: input.request.languageGeneration,
    documentRef: input.request.documentRef,
    fileRef: input.request.fileRef,
    pathRef: input.request.pathRef,
    documentGeneration: input.request.documentGeneration,
    documentVersion: input.request.documentVersion,
    serviceRef: input.service.serviceRef,
    serviceGeneration: input.service.serviceGeneration,
    startRef: input.service.startRef,
    placementRef: input.service.placementRef,
    evidenceTier: "project_local",
    executable: input.service.executable,
    providerVersion: input.service.providerVersion,
    requestedAt: input.request.requestedAt,
    observedAt: observedAt.toISOString(),
    freshnessMs: Math.max(0, Math.min(86_400_000, observedAt.getTime() - requestedAt)),
    state,
    items,
    excerpt: input.request.content.slice(0, 64_000),
    capabilities: providerCapabilities,
  });
};

const execute = (input: WorkerRequest): IdeLanguageResult => {
  const request = input.request;
  if (!supportedCapabilities.has(request.capability)) {
    return unavailableResult(
      input,
      "unsupported_capability",
      `${request.capability} is not available from this TypeScript provider.`,
    );
  }
  if (!["typescript", "javascript"].includes(request.language)) {
    return unavailableResult(
      input,
      "unsupported_language",
      `${request.language} has document-local syntax support only; project intelligence is unavailable.`,
    );
  }
  const project = projectFor(input.root, request);
  if (project === null) {
    return unavailableResult(
      input,
      "invalid_path",
      "The language request path is outside the admitted workspace root.",
    );
  }
  const allItems = itemsFor(input.root, request, project);
  const items = allItems.slice(0, request.limit);
  const state: IdeLanguageResultState = allItems.length > request.limit
    ? { _tag: "Truncated", limit: request.limit, omitted: allItems.length - request.limit }
    : project.truncated
      ? { _tag: "Partial", reason: `Project scan reached ${maximumProjectFiles} files or ${maximumProjectBytes} bytes.` }
      : { _tag: "Complete" };
  return resultFor(input, state, items);
};

const drain = (): void => {
  if (draining || stopped) return;
  draining = true;
  setImmediate(() => {
    const requestRef = queueOrder.shift();
    if (requestRef === undefined) {
      draining = false;
      if (queueOrder.length > 0) drain();
      return;
    }
    const value = queued.get(requestRef);
    queued.delete(requestRef);
    if (value === undefined) {
      draining = false;
      if (queueOrder.length > 0) drain();
      return;
    }
    const cancellation = cancelled.get(requestRef);
    if (cancellation !== undefined) {
      port.postMessage({
        kind: "result",
        result: resultFor(value, { _tag: "Cancelled", reason: cancellation }, []),
      } satisfies WorkerOutput);
      cancelled.delete(requestRef);
    } else {
      try {
        const result = execute(value);
        const lateCancellation = cancelled.get(requestRef);
        port.postMessage({
          kind: "result",
          result: lateCancellation === undefined
            ? result
            : resultFor(value, { _tag: "Cancelled", reason: lateCancellation }, []),
        } satisfies WorkerOutput);
        cancelled.delete(requestRef);
      } catch (error) {
        port.postMessage({
          kind: "failed",
          requestRef,
          message: error instanceof Error ? error.message.slice(0, 800) : "TypeScript provider failed.",
          recoverable: true,
        } satisfies WorkerOutput);
      }
    }
    draining = false;
    if (queueOrder.length > 0) drain();
  });
};

port.on("message", (value: WorkerInput) => {
  if (value.kind === "cancel") {
    cancelled.set(value.requestRef, value.reason);
    drain();
    return;
  }
  if (value.kind === "stop") {
    stopped = true;
    queued.clear();
    queueOrder.length = 0;
    for (const project of projects.values()) project.dispose();
    projects.clear();
    port.close();
    return;
  }
  const requestRef = String(value.request.requestRef);
  queued.set(requestRef, value);
  queueOrder.push(requestRef);
  drain();
});

port.postMessage({ kind: "ready", providerVersion: ts.version } satisfies WorkerOutput);
