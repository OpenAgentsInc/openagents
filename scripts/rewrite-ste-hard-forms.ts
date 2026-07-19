#!/usr/bin/env node

import { readFileSync, writeFileSync } from "node:fs";

import { inspectStructure, type SteMode, type SteProfile } from "./ste-core";

const contractionForms: Readonly<Record<string, string>> = {
  "aren't": "are not",
  "can't": "cannot",
  "couldn't": "could not",
  "didn't": "did not",
  "doesn't": "does not",
  "don't": "do not",
  "hadn't": "had not",
  "hasn't": "has not",
  "haven't": "have not",
  "he's": "he is",
  "isn't": "is not",
  "it's": "it is",
  "mustn't": "must not",
  "shouldn't": "should not",
  "that's": "that is",
  "there's": "there is",
  "they're": "they are",
  "wasn't": "was not",
  "we're": "we are",
  "weren't": "were not",
  "won't": "will not",
  "wouldn't": "would not",
  "you're": "you are",
};
const spellingForms: Readonly<Record<string, string>> = {
  analyse: "analyze",
  analysed: "analyzed",
  authorise: "authorize",
  authorised: "authorized",
  behaviour: "behavior",
  colour: "color",
  favour: "favor",
  labour: "labor",
  licence: "license",
  optimise: "optimize",
  organise: "organize",
  recognise: "recognize",
};

const withCase = (source: string, replacement: string): string =>
  /^[A-Z]/.test(source) ? replacement[0]!.toUpperCase() + replacement.slice(1) : replacement;

const replaceOutsideInlineCode = (line: string, rewrite: (text: string) => string): string =>
  line.split(/(`[^`]*`)/g).map((part) => part.startsWith("`") ? part : rewrite(part)).join("");

export const rewriteSteHardForms = (input: string, mode: SteMode): string => {
  const lines = input.split(/\r?\n/);
  const diagnostics = inspectStructure("document.md", input, mode);
  const byLine = new Map<number, Set<string>>();
  for (const item of diagnostics) {
    if (!["STE-8.1", "STE-9.1", "STE-1.4"].includes(item.rule)) continue;
    const rules = byLine.get(item.line) ?? new Set<string>();
    rules.add(item.rule);
    byLine.set(item.line, rules);
  }
  for (const [lineNumber, rules] of byLine) {
    let line = lines[lineNumber - 1] ?? "";
    line = replaceOutsideInlineCode(line, (text) => {
      let result = text;
      if (rules.has("STE-8.1")) {
        result = result
          .replace(/;\s*([a-z])/g, (_value, letter: string) => `. ${letter.toUpperCase()}`)
          .replace(/;/g, ".");
      }
      if (rules.has("STE-9.1")) {
        result = result.replace(
          /\b(?:aren't|can't|couldn't|didn't|doesn't|don't|hadn't|hasn't|haven't|he's|isn't|it's|mustn't|shouldn't|that's|there's|they're|wasn't|we're|weren't|won't|wouldn't|you're)\b/gi,
          (value) => withCase(value, contractionForms[value.toLowerCase()] ?? value),
        );
      }
      if (rules.has("STE-1.4")) {
        result = result.replace(
          /\b(?:analyse|analysed|authorise|authorised|behaviour|colour|favour|labour|licence|optimise|organise|recognise)\b/gi,
          (value) => withCase(value, spellingForms[value.toLowerCase()] ?? value),
        );
      }
      return result;
    });
    lines[lineNumber - 1] = line;
  }
  return lines.join("\n");
};

const args = process.argv.slice(2);
if (args[0] === "--write") {
  const ledger = JSON.parse(readFileSync("docs/ste/migration-ledger.v1.json", "utf8")) as {
    profiles: SteProfile[];
  };
  const profiles = new Map(ledger.profiles.map((profile) => [profile.path, profile]));
  for (const path of args.slice(1)) {
    const profile = profiles.get(path);
    if (!profile) throw new Error(`Missing STE profile for ${path}`);
    const input = readFileSync(path, "utf8");
    writeFileSync(path, rewriteSteHardForms(input, profile.ste_mode));
  }
}
