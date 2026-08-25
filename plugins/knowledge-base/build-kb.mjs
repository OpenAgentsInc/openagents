#!/usr/bin/env node
// Regenerate kb.json from the approved public docs plus the curated stances.
//
// The corpus is content and is reviewed like content: this script harvests
// the served docs site (openagents.com priv/docs) into compact entries —
// title, summary, headings, tags — merges kb/stances.json, and writes
// kb.json beside this script. The plugin embeds kb.json at build, so a
// regeneration is: node build-kb.mjs && cargo build --release --target
// wasm32-unknown-unknown -p knowledge-base, then re-pin the digest.
import { readFileSync, readdirSync, writeFileSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const docsRoot = process.argv[2] ?? join(here, "../../../openagents.com/priv/docs");

const entries = [];
for (const name of readdirSync(docsRoot).sort()) {
  if (!name.endsWith(".md")) continue;
  const text = readFileSync(join(docsRoot, name), "utf8");
  const lines = text.split("\n");
  const title = (lines.find((line) => line.startsWith("# ")) ?? `# ${name}`).slice(2).trim();
  const headings = lines
    .filter((line) => line.startsWith("## "))
    .map((line) => line.slice(3).trim())
    .slice(0, 12);
  const body = lines.filter((line) => !line.startsWith("#"));
  const first = body.join("\n").trim().split("\n\n")[0]?.replace(/\s+/g, " ").trim() ?? "";
  const tags = name.replace(/\.md$/, "").split(/[-_]/).filter((word) => word.length >= 3);
  entries.push({
    kind: "doc",
    title,
    source: `priv/docs/${name}`,
    headings,
    summary: first.slice(0, 400),
    tags,
  });
}

const stances = JSON.parse(readFileSync(join(here, "kb/stances.json"), "utf8")).map(
  (stance) => ({ kind: "stance", ...stance }),
);

writeFileSync(
  join(here, "kb.json"),
  JSON.stringify({ version: 1, entries: [...stances, ...entries] }, null, 1) + "\n",
);
console.log(`kb.json: ${stances.length} stances + ${entries.length} docs`);
