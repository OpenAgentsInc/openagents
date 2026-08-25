/**
 * Coder tiers: the only names a reader ever sees for what answers.
 *
 * The invariant (OpenAgentsInc/openagents#40, INVARIANTS.md "Coder Model
 * Naming"): vendor model names do not render in the coder. A session is
 * "OpenAgents Coder", and what varies is the tier — `Coder Auto` when the
 * server picks the lane, `Coder Flash` for the fast tier, `Coder Pro` for the
 * strong tier, `Coder Local` when a local model server answers. The vendor id
 * still exists — records and exports need a name a reader could run again —
 * but it travels as `modelId`, never as the label.
 *
 * The tier-to-model map lives here and nowhere else, so renaming a vendor
 * model is a one-line change and no display code ever holds a vendor string.
 */

export type CoderTierId = "auto" | "flash" | "pro" | "local";

/** The vendor model each pinned tier opens its thread on. */
export const TIER_MODELS: Readonly<Record<"flash" | "pro", string>> = {
  flash: "gemini-3.7-flash",
  pro: "gpt-5.6-luna",
};

const LABELS: Readonly<Record<CoderTierId, string>> = {
  auto: "Coder Auto",
  flash: "Coder Flash",
  pro: "Coder Pro",
  local: "Coder Local",
};

/** The label the status line shows for a tier. */
export const tierLabel = (tier: CoderTierId): string => LABELS[tier];

/**
 * The tier a vendor model id belongs to, or undefined for one no tier pins.
 *
 * `ollama:` names are the local lane wherever they appear, so a resumed or
 * delegated session is labelled by what it is rather than by what it typed.
 */
export const tierForModel = (modelId: string | undefined): CoderTierId | undefined => {
  if (modelId === undefined) return "auto";
  if (modelId.startsWith("ollama:")) return "local";
  if (modelId === TIER_MODELS.flash) return "flash";
  if (modelId === TIER_MODELS.pro) return "pro";
  return undefined;
};

/**
 * The display name for any model id a surface holds.
 *
 * A model outside the tier map is still not shown: it is "Coder", the bare
 * product name. That is the invariant's floor — an unknown id is exactly the
 * case where showing the id would leak a vendor name nobody chose to show.
 */
export const coderTierLabel = (modelId: string | undefined): string => {
  const tier = tierForModel(modelId);
  return tier === undefined ? "Coder" : LABELS[tier];
};

/** Shift+Tab's orbit: Auto → Flash → Pro → Local → Auto. */
export const nextTier = (tier: CoderTierId): CoderTierId => {
  switch (tier) {
    case "auto":
      return "flash";
    case "flash":
      return "pro";
    case "pro":
      return "local";
    case "local":
      return "auto";
  }
};
