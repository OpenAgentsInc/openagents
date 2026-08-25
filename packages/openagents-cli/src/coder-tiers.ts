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

/** One lane of the served catalog, as much of it as a tier decision needs. */
interface ServedLane {
  readonly id: string;
  readonly available: boolean;
}

/**
 * Why this tier cannot open a thread on this deployment, or `undefined` if it
 * can.
 *
 * A tier is a friendlier name than a vendor id, which makes it a better place
 * to hide a dead lane — the reader who used to see `gemini-3.7-flash` fail
 * could at least search for it, while a reader who sees `Coder Flash` fail has
 * been told nothing. So the tier a reader picks is checked against what the
 * server says it can answer, the same catalog and the same `available` reading
 * that `chooseBackend` already applies to the model a session opens with. The
 * refusal names tiers only, because the invariant does not lapse when the news
 * is bad.
 *
 * `undefined` for `served` means the catalog could not be read — an older
 * server, an unreachable one, a resumed session that never asked. That is not
 * the same as "serves nothing", so the tier is allowed and the turn reports
 * the truth. `Coder Local` is never refused here: it answers from this machine
 * rather than this deployment, and its own discovery says so when no local
 * server is running.
 *
 * Availability is the only claim made. No tier is described as cheaper or
 * dearer than another: the catalog declares a rate for some lanes and none for
 * others, and a tier label that implied a price the server never quoted would
 * be a worse lie than the vendor name it replaced.
 */
export const tierUnavailable = (
  served: ReadonlyArray<ServedLane> | undefined,
  tier: CoderTierId,
): string | undefined => {
  if (served === undefined || tier === "local") return undefined;

  const answering = (id: string): boolean =>
    served.some((lane) => lane.id === id && lane.available);

  // Auto is openable whenever anything is, because it names no model and lets
  // the server pick the lane per call.
  const openable: CoderTierId[] = [];
  if (served.some((lane) => lane.available)) openable.push("auto");
  if (answering(TIER_MODELS.flash)) openable.push("flash");
  if (answering(TIER_MODELS.pro)) openable.push("pro");

  if (openable.includes(tier)) return undefined;

  // A reason clause, not a sentence: the caller that reports this already
  // names the tier that could not be had, and saying it twice reads as a
  // stutter. It still names the tiers that *can* answer, because a refusal
  // that leaves the reader guessing which way to press shift+tab has only
  // half done the job.
  return openable.length === 0
    ? `no lane on this deployment has a configured provider credential. ` +
        `${tierLabel("local")} still runs on this machine.`
    : `this deployment is not serving that lane right now. ` +
        `Available: ${openable.map(tierLabel).join(", ")}, and ` +
        `${tierLabel("local")} on this machine.`;
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
