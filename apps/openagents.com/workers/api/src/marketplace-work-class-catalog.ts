// Marketplace work-class catalog — the typed registry of the work classes the
// Autopilot control-center fanout marketplace can list, and (critically) which
// ones are actually LIVE versus registered-but-INERT scaffolds (promise
// autopilot.control_center_fanout_marketplace.v1, yellow).
//
// THE GAP THIS ADDRESSES: the self-serve fanout
// (`self-serve-fanout.ts`) HARD-CODES a single work class, `code_task`, and the
// promise's standing blocker
// `blocker.product_promises.plugin_marketplace_beyond_code_task_missing` says a
// plugin marketplace BEYOND that one class is not live. Before this module there
// was no typed place to even ENUMERATE the additional plugin work classes the
// marketplace intends to support, nor a single source of truth for which class
// is live. A reviewer could not tell "is `data_labeling` a real class or a
// claim?" from the code.
//
// WHAT THIS IS: a typed catalog. Each entry declares the contract a plugin work
// class must satisfy to be listable on the market — its required capability
// refs, the verification command a validator re-runs, and the settlement stream
// it settles on — plus an explicit `status`:
//   - `live`           => actually executable+settleable on the market today;
//   - `inert_scaffold` => the contract is registered but NOTHING is wired; no
//                         provider executes it, no escrow opens, no money moves.
//
// HONESTY / SCOPE: this module is contract authority only. `code_task` and the
// first plugin class, `data_labeling`, are live fanout work classes; other plugin
// classes remain `inert_scaffold`. This does not flip the promise green because
// green still requires an armed, settled, owner-signed receipt.

import { Schema as S } from 'effect'

import {
  type PublicProjectionStalenessContract,
  liveAtReadStaleness,
} from './public-projection-staleness'
import { currentIsoTimestamp } from './runtime-primitives'

export const MARKETPLACE_WORK_CLASS_CATALOG_SCHEMA =
  'openagents.marketplace_work_class_catalog.v1' as const

// The yellow promise this catalog sits under. It STAYS yellow.
export const MARKETPLACE_WORK_CLASS_CATALOG_PROMISE =
  'autopilot.control_center_fanout_marketplace.v1' as const

// The original live work class — the same class the self-serve fanout defaults
// to and #4783 settled under.
export const MARKETPLACE_LIVE_WORK_CLASS = 'code_task' as const

export const MARKETPLACE_DATA_LABELING_WORK_CLASS = 'data_labeling' as const

export const MarketplaceWorkClassId = S.Literals([
  MARKETPLACE_LIVE_WORK_CLASS,
  MARKETPLACE_DATA_LABELING_WORK_CLASS,
  'content_writing',
  'research_brief',
])
export type MarketplaceWorkClassId = typeof MarketplaceWorkClassId.Type

// Historical blocker ref retained for old receipts and registry history.
export const MARKETPLACE_PLUGIN_BEYOND_CODE_TASK_BLOCKER_REF =
  'blocker.product_promises.plugin_marketplace_beyond_code_task_missing' as const

/**
 * Whether a catalog entry is actually executable+settleable on the market
 * (`live`) or merely a registered contract with nothing wired (`inert_scaffold`).
 * The plugin marketplace is "live beyond code_task" only when at least one
 * non-`code_task` class is `live`.
 */
export const MarketplaceWorkClassStatus = S.Literals(['live', 'inert_scaffold'])
export type MarketplaceWorkClassStatus =
  typeof MarketplaceWorkClassStatus.Type

/**
 * The NIP-90 market stream a work class settles on. `labor` for agentic labor
 * products, `data` for data-shaped deliverables, `compute` for raw compute jobs.
 */
export const MarketplaceSettlementStream = S.Literals([
  'labor',
  'data',
  'compute',
])
export type MarketplaceSettlementStream =
  typeof MarketplaceSettlementStream.Type

/**
 * One marketplace work class: the typed contract a provider must satisfy to be
 * matched to a job of this class, and whether the class is live. Neutral refs
 * only — no host, wallet, or payment material.
 */
export const MarketplaceWorkClassDefinition = S.Struct({
  /** Stable work class id (the value a market work-request carries). */
  workClass: MarketplaceWorkClassId,
  /** Short public-safe title. */
  title: S.String,
  /** Capabilities a provider must advertise to be matched to this class. */
  requiredCapabilityRefs: S.NonEmptyArray(S.String),
  /** Public-safe verification command a validator re-runs on the deliverable. */
  verificationCommandRef: S.String,
  /** The market stream a job of this class settles on. */
  settlementStream: MarketplaceSettlementStream,
  /** Whether this class is actually live or a registered inert scaffold. */
  status: MarketplaceWorkClassStatus,
})
export type MarketplaceWorkClassDefinition =
  typeof MarketplaceWorkClassDefinition.Type

/**
 * The catalog. `code_task` is the class #4783 settled under. `data_labeling` is
 * the first non-code plugin work class wired into the fanout planner; it still
 * requires the same opt-in, budget, and validator gates before a fanout can be
 * authorized.
 */
export const MARKETPLACE_WORK_CLASS_CATALOG: ReadonlyArray<MarketplaceWorkClassDefinition> =
  [
    {
      workClass: MARKETPLACE_LIVE_WORK_CLASS,
      title: 'Code task',
      requiredCapabilityRefs: ['capability.pylon.local_claude_agent'],
      verificationCommandRef: 'command.public.pylon.labor.bun_test',
      settlementStream: 'labor',
      status: 'live',
    },
    {
      workClass: 'data_labeling',
      title: 'Data labeling / annotation',
      requiredCapabilityRefs: ['capability.market.data_labeling'],
      verificationCommandRef: 'command.public.market.data_labeling.audit',
      settlementStream: 'data',
      status: 'live',
    },
    {
      workClass: 'content_writing',
      title: 'Content writing / drafting',
      requiredCapabilityRefs: ['capability.market.content_writing'],
      verificationCommandRef: 'command.public.market.content_writing.review',
      settlementStream: 'labor',
      status: 'inert_scaffold',
    },
    {
      workClass: 'research_brief',
      title: 'Research brief / synthesis',
      requiredCapabilityRefs: ['capability.market.research_brief'],
      verificationCommandRef: 'command.public.market.research_brief.citation_check',
      settlementStream: 'labor',
      status: 'inert_scaffold',
    },
  ] as const

export class MarketplaceWorkClassCatalogError extends S.TaggedErrorClass<MarketplaceWorkClassCatalogError>()(
  'MarketplaceWorkClassCatalogError',
  {
    reason: S.String,
  },
) {}

/**
 * Enforce the catalog's honesty invariants, throwing
 * `MarketplaceWorkClassCatalogError` if violated:
 *   - work class ids are unique and non-empty;
 *   - `code_task` is present and `live`;
 *   - at least one non-code plugin work class is now `live`, proving the typed
 *     marketplace extends beyond code-task fanout;
 *   - every live class carries a verification command and required capability.
 *
 * Called by the projection so a misedit surfaces as an error, never as a false
 * "live" claim.
 */
export const assertCatalogInvariants = (
  catalog: ReadonlyArray<MarketplaceWorkClassDefinition> = MARKETPLACE_WORK_CLASS_CATALOG,
): void => {
  const seen = new Set<string>()
  for (const entry of catalog) {
    if (entry.workClass.trim().length === 0) {
      throw new MarketplaceWorkClassCatalogError({
        reason: 'work class id must be non-empty',
      })
    }
    if (seen.has(entry.workClass)) {
      throw new MarketplaceWorkClassCatalogError({
        reason: `duplicate work class id: ${entry.workClass}`,
      })
    }
    seen.add(entry.workClass)
  }

  const codeTask = catalog.find(
    entry => entry.workClass === MARKETPLACE_LIVE_WORK_CLASS,
  )
  if (codeTask === undefined || codeTask.status !== 'live') {
    throw new MarketplaceWorkClassCatalogError({
      reason: `${MARKETPLACE_LIVE_WORK_CLASS} must be present and live`,
    })
  }

  const liveBeyondCodeTask = catalog.filter(
    entry =>
      entry.status === 'live' &&
      entry.workClass !== MARKETPLACE_LIVE_WORK_CLASS,
  )
  if (liveBeyondCodeTask.length === 0) {
    throw new MarketplaceWorkClassCatalogError({
      reason:
        'at least one work class beyond code_task must be live for plugin-marketplace fanout',
    })
  }

  for (const entry of catalog.filter(item => item.status === 'live')) {
    if (entry.requiredCapabilityRefs.length === 0) {
      throw new MarketplaceWorkClassCatalogError({
        reason: `live work class ${entry.workClass} must require a capability`,
      })
    }
    if (entry.verificationCommandRef.trim().length === 0) {
      throw new MarketplaceWorkClassCatalogError({
        reason: `live work class ${entry.workClass} must carry a verification command`,
      })
    }
  }
}

/** Look up one work class definition by id, or null when absent. */
export const getMarketplaceWorkClass = (
  workClass: string,
  catalog: ReadonlyArray<MarketplaceWorkClassDefinition> = MARKETPLACE_WORK_CLASS_CATALOG,
): MarketplaceWorkClassDefinition | null =>
  catalog.find(entry => entry.workClass === workClass) ?? null

export const isMarketplaceWorkClassId = (
  workClass: string,
  catalog: ReadonlyArray<MarketplaceWorkClassDefinition> = MARKETPLACE_WORK_CLASS_CATALOG,
): workClass is MarketplaceWorkClassId =>
  getMarketplaceWorkClass(workClass, catalog) !== null

/** True when a work class is present AND `live` (executable on the market). */
export const isMarketplaceWorkClassLive = (
  workClass: string,
  catalog: ReadonlyArray<MarketplaceWorkClassDefinition> = MARKETPLACE_WORK_CLASS_CATALOG,
): boolean => getMarketplaceWorkClass(workClass, catalog)?.status === 'live'

/** The live work classes. */
export const liveMarketplaceWorkClasses = (
  catalog: ReadonlyArray<MarketplaceWorkClassDefinition> = MARKETPLACE_WORK_CLASS_CATALOG,
): ReadonlyArray<MarketplaceWorkClassDefinition> =>
  catalog.filter(entry => entry.status === 'live')

/** The registered-but-inert plugin work classes beyond `code_task`. */
export const inertMarketplaceWorkClasses = (
  catalog: ReadonlyArray<MarketplaceWorkClassDefinition> = MARKETPLACE_WORK_CLASS_CATALOG,
): ReadonlyArray<MarketplaceWorkClassDefinition> =>
  catalog.filter(entry => entry.status === 'inert_scaffold')

/**
 * True once at least one work class BEYOND `code_task` is live — i.e. the moment
 * the plugin-marketplace-beyond-code_task blocker is addressed at the fanout
 * planner level.
 */
export const isPluginMarketplaceBeyondCodeTaskLive = (
  catalog: ReadonlyArray<MarketplaceWorkClassDefinition> = MARKETPLACE_WORK_CLASS_CATALOG,
): boolean =>
  liveMarketplaceWorkClasses(catalog).some(
    entry => entry.workClass !== MARKETPLACE_LIVE_WORK_CLASS,
  )

/**
 * Staleness contract for the catalog projection. Built fresh on every request
 * from the in-module catalog, so it is `live_at_read` (maxStaleness 0).
 */
export const MarketplaceWorkClassCatalogStaleness: PublicProjectionStalenessContract =
  liveAtReadStaleness(['marketplace_work_class_catalog_changed'])

/**
 * Public-safe catalog projection. Honest: it lists every registered work class
 * with its `status`, names the live classes, and reports whether plugin fanout
 * extends beyond `code_task`.
 * Calls `assertCatalogInvariants` first so a misedit fails loudly rather than
 * shipping a false claim.
 */
export const projectMarketplaceWorkClassCatalog = (
  catalog: ReadonlyArray<MarketplaceWorkClassDefinition> = MARKETPLACE_WORK_CLASS_CATALOG,
): {
  schema: typeof MARKETPLACE_WORK_CLASS_CATALOG_SCHEMA
  promiseIds: readonly [typeof MARKETPLACE_WORK_CLASS_CATALOG_PROMISE]
  promiseState: 'yellow'
  inert: false
  liveWorkClass: typeof MARKETPLACE_LIVE_WORK_CLASS
  liveWorkClasses: ReadonlyArray<string>
  pluginMarketplaceBeyondCodeTaskLive: boolean
  generatedAt: string
  maxStalenessSeconds: number
  staleness: PublicProjectionStalenessContract
  unclearedBlockerRefs: ReadonlyArray<string>
  workClasses: ReadonlyArray<MarketplaceWorkClassDefinition>
} => {
  assertCatalogInvariants(catalog)
  return {
    schema: MARKETPLACE_WORK_CLASS_CATALOG_SCHEMA,
    promiseIds: [MARKETPLACE_WORK_CLASS_CATALOG_PROMISE],
    promiseState: 'yellow',
    inert: false,
    liveWorkClass: MARKETPLACE_LIVE_WORK_CLASS,
    liveWorkClasses: liveMarketplaceWorkClasses(catalog).map(
      entry => entry.workClass,
    ),
    pluginMarketplaceBeyondCodeTaskLive:
      isPluginMarketplaceBeyondCodeTaskLive(catalog),
    generatedAt: currentIsoTimestamp(),
    maxStalenessSeconds:
      MarketplaceWorkClassCatalogStaleness.maxStalenessSeconds,
    staleness: MarketplaceWorkClassCatalogStaleness,
    unclearedBlockerRefs: [],
    workClasses: catalog,
  }
}
