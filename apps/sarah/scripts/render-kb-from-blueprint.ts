/**
 * KHS-5 (#8604): regenerate docs/sarah/SARAH_KNOWLEDGE_BASE.md FROM Sarah's
 * Blueprint (the exit criterion — the doc is generated, never hand-edited).
 *
 * Renders the current blueprint (Postgres when SARAH_DATABASE_URL /
 * KHALA_SYNC_DATABASE_URL is configured; the checked-in seed otherwise) into
 * the single-paste KB document with a generation header.
 *
 *   bun apps/sarah/scripts/render-kb-from-blueprint.ts
 */

import { writeFile } from "node:fs/promises"
import path from "node:path"
import { fileURLToPath } from "node:url"

import {
  compileSarahKnowledgeBaseMarkdown,
  loadSarahBlueprint,
  sarahBlueprintStoreMode,
} from "../src/services/sarah-blueprint.ts"

const appRoot = fileURLToPath(new URL("..", import.meta.url))
const KB_PATH = path.join(appRoot, "../..", "docs/sarah/SARAH_KNOWLEDGE_BASE.md")

const markdown = await compileSarahKnowledgeBaseMarkdown()
if (!markdown) {
  console.error(
    "blueprint empty or unavailable — refusing to overwrite the KB doc",
  )
  process.exit(1)
}
await writeFile(KB_PATH, markdown)
const blueprint = await loadSarahBlueprint()
console.log(
  JSON.stringify(
    {
      wrote: "docs/sarah/SARAH_KNOWLEDGE_BASE.md",
      storeMode: sarahBlueprintStoreMode(),
      revision: blueprint.currentRevision,
      activeFacts: blueprint.facts.filter((fact) => fact.status === "active")
        .length,
      retiredFacts: blueprint.facts.filter(
        (fact) => fact.status === "retired",
      ).length,
    },
    null,
    2,
  ),
)
process.exit(0)
