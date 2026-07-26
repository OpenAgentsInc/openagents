import "@tanstack/react-start/server-only";

import type { ForgeFileContent, ForgeRepositoryReadResult } from "@/features/forge/repository-read";
import { codeToTokens, type BundledLanguage } from "shiki";

const supportedLanguages = [
  "bash",
  "c",
  "cpp",
  "css",
  "go",
  "html",
  "java",
  "javascript",
  "json",
  "jsx",
  "markdown",
  "mdx",
  "python",
  "ruby",
  "rust",
  "shellscript",
  "sql",
  "svelte",
  "text",
  "toml",
  "tsx",
  "typescript",
  "vue",
  "xml",
  "yaml",
] as const satisfies ReadonlyArray<BundledLanguage | "text">;
type ForgeLanguage = (typeof supportedLanguages)[number];

const extensionLanguage: Readonly<Record<string, ForgeLanguage>> = {
  bash: "bash",
  c: "c",
  cc: "cpp",
  cpp: "cpp",
  css: "css",
  go: "go",
  html: "html",
  java: "java",
  js: "javascript",
  json: "json",
  jsx: "jsx",
  md: "markdown",
  mdx: "mdx",
  py: "python",
  rb: "ruby",
  rs: "rust",
  sh: "shellscript",
  sql: "sql",
  svelte: "svelte",
  toml: "toml",
  ts: "typescript",
  tsx: "tsx",
  vue: "vue",
  xml: "xml",
  yaml: "yaml",
  yml: "yaml",
  zsh: "shellscript",
};

export const forgeFileLanguage = (path: string, provided: string | undefined): ForgeLanguage => {
  const admittedProvided = supportedLanguages.find((value) => value === provided);
  if (admittedProvided !== undefined) return admittedProvided;
  const extension = path.split(".").at(-1)?.toLowerCase();
  return extension === undefined ? "text" : (extensionLanguage[extension] ?? "text");
};

const presentTextFile = async (
  file: Extract<ForgeFileContent, { readonly _tag: "text" }>,
): Promise<Extract<ForgeFileContent, { readonly _tag: "text" }>> => {
  const language = forgeFileLanguage(file.path, file.language);
  try {
    const result = await codeToTokens(file.content, {
      lang: language,
      theme: "vesper",
    });
    return {
      ...file,
      language,
      highlightedLines: result.tokens.map((line) =>
        line.map((token) => ({
          content: token.content,
          ...(token.color === undefined ? {} : { color: token.color }),
          ...(token.fontStyle === undefined ? {} : { fontStyle: token.fontStyle }),
        })),
      ),
    };
  } catch {
    return {
      ...file,
      language: "text",
      highlightedLines: file.content.split("\n").map((line) => [{ content: line }]),
    };
  }
};

export const presentForgeRepositoryRead = async (
  result: ForgeRepositoryReadResult,
): Promise<ForgeRepositoryReadResult> => {
  if (result._tag === "failed") return result;
  const file = result.projection.file;
  if (file === null || file._tag !== "text") return result;

  return {
    ...result,
    projection: {
      ...result.projection,
      file: await presentTextFile(file),
    },
  };
};
