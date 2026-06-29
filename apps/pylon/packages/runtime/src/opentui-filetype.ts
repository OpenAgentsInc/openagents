// Pure filetype detection used by the interactive Probe CLI rendering.
//
// This lives separately from `opentui-renderer.ts` so it can be imported
// statically without pulling in `@opentui/core`. `@opentui/core` resolves
// platform-native optional dependencies through cross-platform dynamic
// `import()` calls that a bundler cannot statically resolve, which would
// break the headless Pylon node bundle (#5037). Keeping this map free of
// any `@opentui` import lets the static module graph stay headless-safe.

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
