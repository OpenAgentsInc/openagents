// Pylon earning-mode FAMILY canonicalizer — the integrity layer that decides
// whether two earning-mode labels are genuinely DIFFERENT earning modes or just
// two spellings/versions of the SAME one
// (EPIC #5523 / DE-4 #5527; promise pylon.v0_3_multi_earning_node.v1, red).
//
// THE GAP THIS ADVANCES: blocker `multi_earning_mode_receipts_missing`. The
// multi-earning projection's headline bar for green is ">=2 SETTLED modes in one
// install" — the structural meaning of "earns Bitcoin in MULTIPLE ways". But
// `settledModeCount` counts distinct free-form mode LABELS. The existing
// cross-mode auditors (verifyWorkReceiptSettlementCoverage /
// verifyWorkReceiptWorkUnitCoverage) stop ONE work unit or ONE settlement being
// reused across two labels, but nothing stops two labels of the SAME earning
// mode (e.g. "training" and "training_v2", or "forum_tips" and "forum_tips_2")
// from being counted as two modes — a label-splitting over-claim that would fake
// multi-earning with a single earning mode. This module closes that hole: it maps
// a label to a canonical FAMILY by dropping version/variant suffixes, so the
// >=2-modes bar can be measured against distinct FAMILIES, not labels.
//
// HONESTY / SCOPE: PURE and INERT. It moves no money, reads no wallet, admits no
// settlement. It is a deterministic string canonicalizer. It does NOT invent a
// fixed taxonomy that could wrongly COLLAPSE two genuinely-distinct custom modes
// into one — two labels with different stems stay distinct families. It only
// collapses obvious version/variant spellings of one stem. The promise
// pylon.v0_3_multi_earning_node.v1 STAYS red.

// Trailing path/label segments that mark a variant of a stem rather than a
// distinct earning mode. Dropped only as TRAILING segments and only while at
// least one non-variant segment remains, so a label like "test_harness" keeps
// its meaningful stem.
const VARIANT_SEGMENTS: ReadonlySet<string> = new Set([
  'again',
  'alpha',
  'alt',
  'beta',
  'copy',
  'dup',
  'final',
  'new',
  'old',
  'rc',
  'retry',
  'temp',
  'test',
  'tmp',
  'v',
])

// A version-like segment: an optional leading "v" then digits (v1, v02, 3).
const isVersionSegment = (segment: string): boolean => /^v?\d+$/.test(segment)

const isVariantSegment = (segment: string): boolean =>
  isVersionSegment(segment) || VARIANT_SEGMENTS.has(segment)

// Strip a trailing version-like suffix from WITHIN a single segment, e.g.
// "training2" -> "training", "trainingv2" -> "training". Only strips when the
// remaining stem is at least two characters, so short tokens (e.g. "v2", "h2")
// are left intact rather than reduced to noise.
const stripTrailingVersionFromSegment = (segment: string): string => {
  const match = segment.match(/^(.*?)v?\d+$/)
  if (match !== null && match[1] !== undefined && match[1].length >= 2) {
    return match[1]
  }
  return segment
}

/**
 * Canonicalize an earning-mode LABEL to its FAMILY. PURE / deterministic. The
 * family is the label's meaningful stem with version/variant spellings removed,
 * so two labels that name the same earning mode collapse to one family while two
 * labels with different stems stay distinct.
 *
 * Examples (-> family):
 *   "training"          -> "training"
 *   "training_v2"       -> "training"
 *   "training2"         -> "training"
 *   "forum_tips"        -> "forum_tips"
 *   "forum_tips_2"      -> "forum_tips"
 *   "compute"           -> "compute"   (stays distinct from "training")
 *
 * Inputs are assumed already validated as public-safe upstream (the projection
 * and receipt builders reject unsafe tokens); this function only normalizes.
 */
export const canonicalizeEarningModeFamily = (label: string): string => {
  const lower = label.trim().toLowerCase()
  const segments = lower.split(/[_.:/-]+/).filter(segment => segment.length > 0)
  if (segments.length === 0) {
    return lower
  }
  // Drop trailing variant/version segments while a meaningful segment remains.
  let end = segments.length
  while (end > 1 && isVariantSegment(segments[end - 1] as string)) {
    end -= 1
  }
  const kept = segments.slice(0, end)
  const lastIndex = kept.length - 1
  kept[lastIndex] = stripTrailingVersionFromSegment(kept[lastIndex] as string)
  return kept.join('_')
}

/**
 * Whether two earning-mode labels name the SAME earning mode (same family).
 */
export const isSameEarningModeFamily = (
  labelA: string,
  labelB: string,
): boolean =>
  canonicalizeEarningModeFamily(labelA) === canonicalizeEarningModeFamily(labelB)

/**
 * The distinct earning-mode FAMILIES present in a set of labels, in first-seen
 * order. This is the honest count of "how many different ways this set earns",
 * immune to label-splitting (versioned/duplicated spellings of one mode).
 */
export const distinctEarningModeFamilies = (
  labels: ReadonlyArray<string>,
): ReadonlyArray<string> => {
  const seen = new Set<string>()
  const order: string[] = []
  for (const label of labels) {
    const family = canonicalizeEarningModeFamily(label)
    if (!seen.has(family)) {
      seen.add(family)
      order.push(family)
    }
  }
  return order
}
