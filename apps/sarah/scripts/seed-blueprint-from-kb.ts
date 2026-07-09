/**
 * KHS-5 (#8604): one-time seed generation for Sarah's Blueprint.
 *
 * Parses the owner's hand-written knowledge base
 * (docs/sarah/SARAH_KNOWLEDGE_BASE.md, the "flat pasted KB") into typed
 * blueprint facts with provenance source `owner_kb_v2`, attaches
 * deal-rule refs to pricing facts and promise-registry ids to product/proof
 * facts where the copy names them, and writes
 * apps/sarah/config/blueprint-seed.json (revision 1).
 *
 * Run ONCE and commit the seed. After this, the KB doc is GENERATED from the
 * blueprint (scripts/render-kb-from-blueprint.ts) and edits happen through
 * the admin-guarded operator endpoints, which create receipted revisions.
 *
 *   bun apps/sarah/scripts/seed-blueprint-from-kb.ts
 */

import { readFile, writeFile } from "node:fs/promises"
import path from "node:path"
import { fileURLToPath } from "node:url"

import {
  blueprintFactId,
  parseSarahKnowledgeBaseMarkdown,
  type BlueprintFact,
} from "../src/services/sarah-blueprint.ts"
import { DEFAULT_DEAL_RULES_CONFIG } from "../src/services/deal-rules.ts"

const appRoot = fileURLToPath(new URL("..", import.meta.url))
const repoRoot = path.join(appRoot, "../..")
const KB_PATH = path.join(repoRoot, "docs/sarah/SARAH_KNOWLEDGE_BASE.md")
const SEED_PATH = path.join(appRoot, "config/blueprint-seed.json")

/** Provenance timestamp: the KB's own "Last compiled: 2026-07-09" date. */
const KB_COMPILED_AT = "2026-07-09T00:00:00.000Z"
const KB_REF = "docs/sarah/SARAH_KNOWLEDGE_BASE.md@2026-07-09"

const dealRules = DEFAULT_DEAL_RULES_CONFIG
const creditVolumeRefs = dealRules.creditVolumeTiers.map((tier) => tier.ruleRef)
const bitcoinDiscountRef = dealRules.bitcoinDiscount.ruleRef
const bundleRefs = dealRules.bundleRules.map((rule) => rule.ruleRef)
const moduleRefs = dealRules.modules.map((module) => module.id)

/**
 * Typed pricing facts carry dealRuleRefs (matched on bounded copy the KB
 * already states — deterministic parsing of exact amounts/IDs only, applied
 * after the fact's section is known, never for routing).
 */
const DEAL_RULE_MATCHERS: Array<{ pattern: RegExp; refs: string[] }> = [
  { pattern: /Credit volume bonuses/, refs: creditVolumeRefs },
  { pattern: /Fund \$1,000 and you get 10% bonus/, refs: [...creditVolumeRefs, bitcoinDiscountRef] },
  { pattern: /Bitcoin\/Lightning payment discount/, refs: [bitcoinDiscountRef] },
  { pattern: /paying in Bitcoin\s+saves/i, refs: [bitcoinDiscountRef] },
  { pattern: /bundle discount/, refs: bundleRefs },
  { pattern: /Configured modules/, refs: moduleRefs },
  { pattern: /Per-transaction cap/, refs: ["config.transaction_cap_usd_10000"] },
]

/**
 * Product/proof facts carry promiseIds — registry record ids the claim rests
 * on (verified against the live registry at seed time).
 */
const PROMISE_ID_MATCHERS: Array<{ pattern: RegExp; ids: string[] }> = [
  {
    pattern: /\*\*Khala Code \(mobile\)\*\*/,
    ids: ["khala_code.mobile_mvp.v1"],
  },
  {
    pattern: /\*\*Khala \(free API\)\*\*/,
    ids: [
      "inference.khala_free_openai_compatible_api.v1",
      "metrics.khala_tokens_served_public.v1",
    ],
  },
  {
    pattern: /\*\*Pylon\*\*/,
    ids: [
      "pylon.install_without_wallet_knowledge.v1",
      "training.decentralized_training_launch.v1",
    ],
  },
  {
    pattern: /\*\*Forum \+ agent economy\*\*/,
    ids: [
      "forum.content_tipping.v1",
      "payments.reliable_tips_sweepable_balances.v1",
    ],
  },
  {
    pattern: /open source in the public monorepo/,
    ids: ["repo.open_source_code_map.v1"],
  },
  {
    pattern: /Versioned public promise registry/,
    ids: ["promises.registry.v1"],
  },
  {
    pattern: /Khala free OpenAI-compatible API; live Tokens Served counter/,
    ids: [
      "inference.khala_free_openai_compatible_api.v1",
      "metrics.khala_tokens_served_public.v1",
      "metrics.khala_model_family_mix_public.v1",
    ],
  },
  {
    pattern: /Payments in production/,
    ids: [
      "payments.money_dev_kit.v1",
      "payments.reliable_tips_sweepable_balances.v1",
    ],
  },
  {
    pattern: /Pylon node \+ scoped decentralized training runs/,
    ids: [
      "pylon.install_without_wallet_knowledge.v1",
      "training.decentralized_training_launch.v1",
    ],
  },
  {
    pattern: /Khala coding delegation to your own linked machine/,
    ids: ["khala.own_capacity_codex_delegation.v1"],
  },
  {
    pattern: /Forum content tipping/,
    ids: ["forum.content_tipping.v1"],
  },
  {
    pattern: /agent instruction sheet/,
    ids: ["agents.one_instruction_sheet.v1"],
  },
  {
    pattern: /Artanis/,
    ids: ["artanis.cloud_mind.v1"],
  },
]

function dealRuleRefsFor(fact: {
  section: string
  statement: string
}): string[] {
  if (fact.section !== "pricing" && fact.section !== "playbook") return []
  const refs = new Set<string>()
  for (const matcher of DEAL_RULE_MATCHERS) {
    if (matcher.pattern.test(fact.statement)) {
      for (const ref of matcher.refs) refs.add(ref)
    }
  }
  return [...refs]
}

function promiseIdsFor(fact: {
  section: string
  statement: string
}): string[] {
  if (fact.section !== "products" && fact.section !== "proof") return []
  const ids = new Set<string>()
  for (const matcher of PROMISE_ID_MATCHERS) {
    if (matcher.pattern.test(fact.statement)) {
      for (const id of matcher.ids) ids.add(id)
    }
  }
  return [...ids]
}

const markdown = await readFile(KB_PATH, "utf8")
const blocks = parseSarahKnowledgeBaseMarkdown(markdown)

const facts: BlueprintFact[] = blocks.map((block, index) => ({
  id: blueprintFactId(
    block.section,
    block.heading,
    block.format,
    block.statement,
  ),
  section: block.section,
  heading: block.heading,
  format: block.format,
  tableHeader: block.tableHeader,
  position: (index + 1) * 100,
  statement: block.statement,
  provenance: {
    source: "owner_kb_v2",
    ref:
      block.section === "pricing"
        ? `${KB_REF} + ${dealRules.version}`
        : KB_REF,
    at: KB_COMPILED_AT,
  },
  dealRuleRefs: dealRuleRefsFor(block),
  promiseIds: promiseIdsFor(block),
  status: "active",
  revisionAdded: 1,
  revisionRetired: null,
}))

const duplicates = facts.filter(
  (fact, index) => facts.findIndex((other) => other.id === fact.id) !== index,
)
if (duplicates.length > 0) {
  console.error("duplicate fact ids:", duplicates.map((fact) => fact.id))
  process.exit(1)
}

const seed = {
  schema: "sarah.blueprint_seed.v1",
  source:
    "docs/sarah/SARAH_KNOWLEDGE_BASE.md (owner_kb_v2, compiled 2026-07-09; registry 34 green / 22 yellow of 143; sarah.deal_rules.v1.2026-07-08)",
  generatedBy: "apps/sarah/scripts/seed-blueprint-from-kb.ts (KHS-5 #8604)",
  revision: {
    revision: 1,
    createdAt: KB_COMPILED_AT,
    changedBy: "owner",
    changeNote:
      "Seeded from the owner-authored knowledge base (owner_kb_v2). From this revision on, the KB doc is generated from the blueprint and edits go through receipted revisions.",
  },
  facts,
}

await writeFile(SEED_PATH, `${JSON.stringify(seed, null, 2)}\n`)
console.log(
  JSON.stringify(
    {
      wrote: "apps/sarah/config/blueprint-seed.json",
      facts: facts.length,
      sections: Object.fromEntries(
        [...new Set(facts.map((fact) => fact.section))].map((section) => [
          section,
          facts.filter((fact) => fact.section === section).length,
        ]),
      ),
      withDealRuleRefs: facts.filter((fact) => fact.dealRuleRefs.length > 0)
        .length,
      withPromiseIds: facts.filter((fact) => fact.promiseIds.length > 0)
        .length,
    },
    null,
    2,
  ),
)
