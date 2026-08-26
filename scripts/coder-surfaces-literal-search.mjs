// Prove no staged surface sentence survives as a literal outside its artifact.
//
// The extraction in OpenAgentsInc/openagents#122 is only worth its cost if the
// artifact is the single home of the text. A leftover copy in a `.rs` or `.ts`
// file is worse than no extraction at all: an optimizer would diff the
// artifact, the build would embed the artifact, and the second copy would go on
// being the one some path actually read.
//
// So this takes a distinctive fragment of every staged string and searches the
// whole tree for it, allowing exactly the places a copy is meant to be:
//
//   - `surfaces/coder/` — the artifacts themselves;
//   - the two generated modules, which ARE the build output;
//   - `plugins/<id>/manifest.json` — where a catalog line is edited, the
//     artifact being its mirror rather than its replacement;
//   - the two Rust golden files, which pin the composed output on purpose;
//   - `docs/`, where a document quoting a sentence is prose about the text and
//     not a copy any code path reads.
//
// Reported rather than wired into `check:fast`: the search is a `ripgrep` per
// staged string over the whole repository, which is the wrong cost for a gate
// that runs on every push. `check:coder-surfaces` is the gate.
//
// Usage: node scripts/coder-surfaces-literal-search.mjs

import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";

const ROOT = dirname(dirname(fileURLToPath(import.meta.url)));

const ALLOWED = [
  "surfaces/coder/",
  "crates/openagents-cli/src/surfaces.rs",
  "packages/openagents-cli/src/coder-surfaces.generated.ts",
  "plugins/",
  "crates/coder-lite/tests/coder-surfaces-golden.json",
  "crates/openagents-cli/tests/coder-surfaces-golden.json",
  "docs/",
];

const surfaces = ["system-prompt", "tool-descriptions", "catalog-lines"].map((id) => ({
  id,
  text: JSON.parse(readFileSync(join(ROOT, "surfaces", "coder", `${id}.v1.json`), "utf8")).text,
}));

/**
 * A fragment distinctive enough to search on: the first line of at least 40
 * characters, with the `{placeholder}` tokens removed so a template still
 * matches the prose it was cut from.
 */
const probeOf = (text) => {
  const cleaned = text.replaceAll(/\{[a-z_]+\}/gu, "").trim();
  for (const line of cleaned.split("\n")) {
    const trimmed = line.trim();
    if (trimmed.length >= 40) return trimmed.slice(0, 60);
  }
  return cleaned.length >= 20 ? cleaned.slice(0, 60) : null;
};

let searched = 0;
let skipped = 0;
const strays = [];

for (const surface of surfaces) {
  for (const [key, text] of Object.entries(surface.text)) {
    const needle = probeOf(text);
    if (needle === null) {
      skipped += 1;
      console.log(`  skipped ${surface.id}#${key}: too short to search distinctively`);
      continue;
    }
    searched += 1;
    const found = spawnSync("rg", ["--fixed-strings", "--files-with-matches", needle, "."], {
      cwd: ROOT,
      encoding: "utf8",
    });
    const files = (found.stdout ?? "")
      .split("\n")
      .map((line) => line.trim().replace(/^\.\//u, ""))
      .filter((line) => line !== "");
    const stray = files.filter((file) => !ALLOWED.some((prefix) => file.startsWith(prefix)));
    if (stray.length > 0) strays.push({ surface: surface.id, key, stray });
  }
}

for (const { surface, key, stray } of strays) {
  console.error(`STRAY LITERAL ${surface}#${key}: ${stray.join(", ")}`);
}
console.log(
  `\n${String(searched)} staged strings searched across the tree (${String(skipped)} too short to search).`,
);
console.log(`stray copies: ${String(strays.length)}`);
process.exit(strays.length > 0 ? 1 : 0);
