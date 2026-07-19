#!/usr/bin/env node

import { readFileSync, writeFileSync } from "node:fs";

const words = (text: string): readonly string[] =>
  [...text.matchAll(/[A-Za-z0-9]+(?:[-'][A-Za-z0-9]+)*/g)].map((match) => match[0].toLowerCase());

export const rewriteSteSemicolons = (input: string): string => {
  let inFence = false;
  let inCodeSpan = false;
  const output = input.split(/\r?\n/).map((line) => {
    if (/^\s*(```|~~~)/.test(line)) {
      inFence = !inFence;
      return line;
    }
    if (inFence) return line;
    const semanticCodeRanges = [...line.matchAll(/`[^`\n]+`/g)].map(
      (match) => [match.index ?? 0, (match.index ?? 0) + match[0].length] as const,
    );
    let result = "";
    for (let index = 0; index < line.length; index += 1) {
      const rest = line.slice(index);
      const url = /^https?:\/\/\S+/.exec(rest)?.[0];
      if (!inCodeSpan && url) {
        result += url;
        index += url.length - 1;
        continue;
      }
      const character = line[index] ?? "";
      if (character === "`" && line[index - 1] !== "\\") {
        inCodeSpan = !inCodeSpan;
        result += character;
      } else if (
        !inCodeSpan &&
        character === ";" &&
        !semanticCodeRanges.some(([start, end]) => index >= start && index < end)
      ) {
        result += ",";
      } else result += character;
    }
    return result;
  });
  const result = output.join("\n");
  if (JSON.stringify(words(result)) !== JSON.stringify(words(input)))
    throw new Error("STE structural rewrite changed the normalized word sequence");
  return result;
};

const args = process.argv.slice(2);
if (args[0] === "--write") {
  const paths = args.slice(1);
  if (paths.length === 0) throw new Error("Specify at least one path after --write");
  for (const path of paths) {
    const input = readFileSync(path, "utf8");
    const output = rewriteSteSemicolons(input);
    writeFileSync(path, output);
    console.log(`rewrote ${path}`);
  }
}
