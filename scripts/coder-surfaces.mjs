// The staged coder text surfaces: build the artifacts, or prove the build ran.
//
// `surfaces/coder/` holds the coder's optimizable text as data
// (OpenAgentsInc/openagents#122): the system prompt, the tool descriptions,
// and the capability catalog lines. Each artifact carries a schema id and each
// gets a content digest in `surfaces/coder/index.json`. The native CLI embeds
// the text at build time in `crates/openagents-cli/src/surfaces.rs`, so it does
// not keep a second copy of a sentence the artifact owns.
//
// THE FAILURE THIS SCRIPT EXISTS TO PREVENT is the knowledge base's: a corpus
// edit that is not followed by the rebuild ships nothing, and says nothing
// while it does. Here the same edit has two ways to go quiet:
//
//   1. The generated modules are the copies the CLIs actually compile. Edit an
//      artifact and skip the rebuild and the CLIs keep shipping the old
//      sentence.
//   2. The digests in `index.json` are what a bench row records as the text
//      that produced it. Edit an artifact and skip the re-pin and every later
//      row names text that was never run.
//
// So `--check` (the default, wired into `check:fast` as `check:coder-surfaces`)
// regenerates everything in memory and refuses when any of it differs from
// what is on disk, naming the artifact and the reason. `--write` does the
// rebuild.
//
// The catalog-lines artifact is deliberately the odd one out and the doc says
// so: a plugin's catalog line is already data — the top-level `description` of
// its `plugins/<id>/manifest.json`, discovered at runtime by
// `discover_catalog()` — so staging it is not a move. The artifact mirrors
// those descriptions into one diffable object with a digest, and the check
// fails when a manifest and the mirror disagree. The manifest stays the place
// the text is edited.
//
// Usage:
//   node scripts/coder-surfaces.mjs            # check
//   node scripts/coder-surfaces.mjs --write    # rebuild and re-pin

import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
import { readdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = dirname(dirname(fileURLToPath(import.meta.url)));
const SURFACES_DIR = join(ROOT, "surfaces", "coder");
const PLUGINS_DIR = join(ROOT, "plugins");
const INDEX_PATH = join(SURFACES_DIR, "index.json");
const RUST_MODULE = join(ROOT, "crates", "openagents-cli", "src", "surfaces.rs");

/** The artifacts, in the order the index lists them. */
const SURFACES = [
  { id: "system-prompt", file: "system-prompt.v1.json", authored: true },
  { id: "tool-descriptions", file: "tool-descriptions.v1.json", authored: true },
  { id: "catalog-lines", file: "catalog-lines.v1.json", authored: false },
];

const INDEX_SCHEMA = "openagents.coder_surface_index.v1";

/** Exactly how an artifact is serialised, so a digest is a fact about bytes. */
const canonical = (doc) => `${JSON.stringify(doc, null, 2)}\n`;

const digestOf = (text) => `sha256:${createHash("sha256").update(text).digest("hex")}`;

const readJson = (path) => JSON.parse(readFileSync(path, "utf8"));

/**
 * The catalog-lines artifact, mirrored from the installed plugin manifests.
 *
 * Keyed by the manifest's own `name`, which is the id the `capability` tool
 * loads by, so a candidate diff over this surface names the same plugin the
 * catalog does.
 */
const buildCatalogLines = () => {
  const text = {};
  for (const entry of readdirSync(PLUGINS_DIR, { withFileTypes: true }).sort((left, right) =>
    left.name.localeCompare(right.name),
  )) {
    if (!entry.isDirectory()) continue;
    const manifestPath = join(PLUGINS_DIR, entry.name, "manifest.json");
    let manifest;
    try {
      manifest = readJson(manifestPath);
    } catch {
      continue; // Not a plugin directory. Discovery skips these too.
    }
    if (typeof manifest.name !== "string" || typeof manifest.description !== "string") continue;
    text[manifest.name] = manifest.description;
  }
  return {
    schema: "openagents.coder_surface.catalog_lines.v1",
    surface: "catalog-lines",
    // Named here because it is the one surface this script does not own: the
    // manifest is where the text is edited, and this artifact is its mirror.
    source: "plugins/<id>/manifest.json#description",
    text,
  };
};

/**
 * Which staged values the native CLI embeds.
 *
 * A `rust` segment marks native-only text. Keys without a consumer segment are
 * shared data and also reach the native CLI.
 */
const consumersOf = (key) => {
  const segments = key.split(".");
  if (segments.includes("rust") || segments[0] === "coder_lite") return ["rust"];
  return ["rust"];
};

const entriesFor = (doc, consumer) =>
  Object.entries(doc.text).filter(([key]) => consumersOf(key).includes(consumer));

const rustConst = (key) => key.replaceAll(/[.-]/gu, "_").toUpperCase();

const rustLiteral = (value) => JSON.stringify(value);

const formatRust = (source) => {
  const result = spawnSync("rustfmt", ["--emit", "stdout"], {
    input: source,
    encoding: "utf8",
  });
  if (result.status !== 0) {
    throw new Error(`rustfmt failed while building coder surfaces: ${result.stderr.trim()}`);
  }
  return result.stdout;
};

const rustModule = (docs, digests) => {
  const lines = [
    "// @generated by scripts/coder-surfaces.mjs — do not edit.",
    "//",
    "// The staged coder text surfaces (`surfaces/coder/`), embedded at build",
    "// time. Editing a sentence here is editing a build output: the artifact",
    "// is the source, `pnpm run build:coder-surfaces` is the build, and",
    "// `pnpm run check:coder-surfaces` refuses a tree where the two disagree.",
    "//",
    "//! Staged coder text surfaces, embedded from `surfaces/coder/`.",
    "",
    "/// The system prompt surface: `surfaces/coder/system-prompt.v1.json`.",
    "pub mod system_prompt {",
  ];
  for (const [key, value] of entriesFor(docs["system-prompt"], "rust")) {
    lines.push(`    /// \`${key}\``);
    lines.push(`    pub const ${rustConst(key)}: &str = ${rustLiteral(value)};`);
  }
  lines.push("}", "");
  lines.push("/// The tool-description surface: `surfaces/coder/tool-descriptions.v1.json`.");
  lines.push("pub mod tool_descriptions {");
  for (const [key, value] of entriesFor(docs["tool-descriptions"], "rust")) {
    lines.push(`    /// \`${key}\``);
    lines.push(`    pub const ${rustConst(key)}: &str = ${rustLiteral(value)};`);
  }
  lines.push("}", "");
  lines.push("/// Every staged surface and the digest of the artifact this was built from.");
  lines.push("///");
  lines.push("/// A run records these so a bench row names exactly which text produced it.");
  lines.push(`pub const SURFACE_DIGESTS: [(&str, &str); ${String(digests.length)}] = [`);
  for (const [id, digest] of digests) {
    lines.push(`    (${rustLiteral(id)}, ${rustLiteral(digest)}),`);
  }
  lines.push("];", "");
  return lines.join("\n");
};

const build = () => {
  const docs = {};
  const files = {};
  for (const surface of SURFACES) {
    const path = join(SURFACES_DIR, surface.file);
    const doc = surface.authored ? readJson(path) : buildCatalogLines();
    docs[surface.id] = doc;
    files[surface.file] = canonical(doc);
  }

  const digests = SURFACES.map((surface) => [surface.id, digestOf(files[surface.file])]);

  const index = {
    schema: INDEX_SCHEMA,
    surfaces: Object.fromEntries(
      SURFACES.map((surface, at) => [
        surface.id,
        {
          file: surface.file,
          schema: docs[surface.id].schema,
          keys: Object.keys(docs[surface.id].text).length,
          digest: digests[at][1],
        },
      ]),
    ),
  };

  return {
    files,
    index: canonical(index),
    rust: formatRust(rustModule(docs, digests)),
  };
};

const failures = [];
const complain = (what, detail) => failures.push(`${what}: ${detail}`);

const compare = (path, expected, label) => {
  let found;
  try {
    found = readFileSync(path, "utf8");
  } catch {
    complain(label, `${path} does not exist. Run \`pnpm run build:coder-surfaces\`.`);
    return;
  }
  if (found !== expected) {
    complain(
      label,
      `${path} is stale — it does not match what the staged artifacts build to. A surface was edited without the rebuild, so what ships is not what the artifact says. Run \`pnpm run build:coder-surfaces\`.`,
    );
  }
};

const built = build();
const write = process.argv.includes("--write");

if (write) {
  for (const surface of SURFACES) {
    writeFileSync(join(SURFACES_DIR, surface.file), built.files[surface.file]);
  }
  writeFileSync(INDEX_PATH, built.index);
  writeFileSync(RUST_MODULE, built.rust);
  console.log(
    `coder surfaces rebuilt: ${SURFACES.map((surface) => surface.id).join(", ")} + index and Rust module`,
  );
} else {
  for (const surface of SURFACES) {
    compare(join(SURFACES_DIR, surface.file), built.files[surface.file], `surface ${surface.id}`);
  }
  compare(INDEX_PATH, built.index, "surface digest index");
  compare(RUST_MODULE, built.rust, "embedded Rust module");

  if (failures.length > 0) {
    console.error("coder surfaces are out of date:\n");
    for (const failure of failures) console.error(`  - ${failure}`);
    console.error("");
    process.exit(1);
  }
  console.log(`coder surfaces are current (${String(SURFACES.length)} artifacts, digests pinned)`);
}
