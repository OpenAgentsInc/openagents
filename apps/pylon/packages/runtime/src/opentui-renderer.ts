import {
  createCliRenderer,
  MarkdownRenderable,
  CodeRenderable,
  DiffRenderable,
  BoxRenderable,
  TextRenderable,
  ScrollBoxRenderable,
  LineNumberRenderable,
  SyntaxStyle,
  parseColor,
  type CliRenderer,
} from "@opentui/core";

export interface ProbeRenderer {
  readonly renderer: CliRenderer;
  readonly syntaxStyle: SyntaxStyle;
  readonly session: ScrollBoxRenderable;
  readonly text: MarkdownRenderable;
}

export { parseColor, TextRenderable, ScrollBoxRenderable, BoxRenderable, DiffRenderable, LineNumberRenderable } from "@opentui/core";

export function createDefaultSyntaxStyle(): SyntaxStyle {
  return SyntaxStyle.fromStyles({
    keyword: { fg: parseColor("#FF7B72"), bold: true },
    string: { fg: parseColor("#A5D6FF") },
    comment: { fg: parseColor("#8B949E"), italic: true },
    number: { fg: parseColor("#79C0FF") },
    function: { fg: parseColor("#D2A8FF") },
    type: { fg: parseColor("#FFA657") },
    operator: { fg: parseColor("#FF7B72") },
    variable: { fg: parseColor("#E6EDF3") },
    property: { fg: parseColor("#79C0FF") },
    bracket: { fg: parseColor("#F0F6FC") },
    delimiter: { fg: parseColor("#C9D1D9") },
    "markup.heading": { fg: parseColor("#00D7FF"), bold: true },
    "markup.bold": { fg: parseColor("#F0F6FC"), bold: true },
    "markup.italic": { fg: parseColor("#F0F6FC"), italic: true },
    "markup.list": { fg: parseColor("#FF7B72") },
    "markup.quote": { fg: parseColor("#8B949E"), italic: true },
    "markup.raw": { fg: parseColor("#A5D6FF"), bg: parseColor("#161B22") },
    "markup.link": { fg: parseColor("#58A6FF"), underline: true },
    "markup.link.url": { fg: parseColor("#58A6FF"), underline: true },
    conceal: { fg: parseColor("#6E7681") },
    default: { fg: parseColor("#E6EDF3") },
  });
}

export function createProbeRenderer(): Promise<CliRenderer> {
  return createCliRenderer({
    exitOnCtrlC: true,
    targetFps: 30,
    screenMode: "main-screen",
  });
}

export function createAssistantText(renderer: CliRenderer): MarkdownRenderable {
  return new MarkdownRenderable(renderer, {
    content: "",
    syntaxStyle: createDefaultSyntaxStyle(),
    conceal: true,
    internalBlockMode: "top-level",
    streaming: true,
    width: "100%",
  });
}

export function createCodeBlock(
  renderer: CliRenderer,
  content: string,
  filetype?: string,
): CodeRenderable {
  return new CodeRenderable(renderer, {
    content,
    filetype: filetype ?? "plaintext",
    syntaxStyle: createDefaultSyntaxStyle(),
    width: "100%",
  });
}

export function createCodeWithLineNumbers(
  renderer: CliRenderer,
  content: string,
  filetype?: string,
): LineNumberRenderable {
  const code = createCodeBlock(renderer, content, filetype);
  return new LineNumberRenderable(renderer, {
    target: code,
    minWidth: 3,
    paddingRight: 1,
    width: "100%",
  });
}

const EXTENSION_MAP: Record<string, string> = {
  ts: "typescript",
  tsx: "tsx",
  js: "javascript",
  jsx: "jsx",
  rs: "rust",
  py: "python",
  go: "go",
  rb: "ruby",
  java: "java",
  kt: "kotlin",
  scala: "scala",
  swift: "swift",
  c: "c",
  h: "c",
  cpp: "cpp",
  hpp: "cpp",
  cc: "cpp",
  cs: "csharp",
  fs: "fsharp",
  zig: "zig",
  toml: "toml",
  yaml: "yaml",
  yml: "yaml",
  json: "json",
  md: "markdown",
  markdown: "markdown",
  html: "html",
  css: "css",
  scss: "scss",
  less: "less",
  sql: "sql",
  sh: "shellscript",
  bash: "shellscript",
  zsh: "shellscript",
  fish: "shellscript",
  dockerfile: "dockerfile",
  tf: "terraform",
  rs: "rust",
  ron: "rust",
  nix: "nix",
  lua: "lua",
  dart: "dart",
  php: "php",
  r: "r",
  clj: "clojure",
  cljs: "clojure",
  elm: "elm",
  erl: "erlang",
  ex: "elixir",
  exs: "elixir",
};

export function detectFiletype(filePath: string): string | undefined {
  const name = filePath.toLowerCase();
  if (name.endsWith(".dockerfile") || name === "dockerfile") return "dockerfile";
  const dot = name.lastIndexOf(".");
  if (dot < 0) return undefined;
  const ext = name.slice(dot + 1);
  return EXTENSION_MAP[ext];
}
