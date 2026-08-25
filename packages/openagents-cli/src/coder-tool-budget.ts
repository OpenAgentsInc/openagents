/**
 * What one tool result may cost, per model family.
 *
 * A tool result is not paid for once. Every later round of the turn re-sends
 * the whole transcript, so the result a command printed on round two is bought
 * again on rounds three through fifteen. The measured bill for that is in the
 * first graded Gym runs: a session that read the issue boards accumulated 82 KB
 * of tool output and re-sent it 25 times, and 91% of everything it sent each
 * round was output the model had already read
 * (`openagents.com` docs/terminalbench/2026-08-24-fix-git-run-analysis.md).
 *
 * The bound on that used to be one number — 4,000 characters — written twice,
 * once in the thread lane and once in the local lane, and applied to a 32k
 * local model and to a million-token hosted one alike. A single number cannot
 * be right for both: it is either most of a small model's window or a rounding
 * error in a large one, and in neither case does it reflect what the family's
 * tokenizer charges for those characters. So the budget is per family, it is
 * stated in tokens, and it is converted to characters through that family's own
 * density.
 *
 * ## The density figures are approximations, and say so
 *
 * Nothing here tokenizes. A real tokenizer per family is a dependency and a
 * download for a decision that is a ceiling, not an accounting entry — the
 * result is cut at a character count either way. What each family's figure is
 * for is that tokenizers differ enough to matter: byte-pair vocabularies
 * trained mostly on English prose land near four characters a token on prose
 * and lower on the shell output, paths, and diffs a coding session actually
 * reads. The numbers below are documented approximations, held low rather than
 * high, so an error spends less of the window than the budget says rather than
 * more. Replace one when a family is measured; do not read it as a measurement.
 *
 * ## Cutting is reported, never silent
 *
 * The caller of a tool is a model, and a model handed a quietly shortened
 * `git log` will summarize it as though it were the whole log. So a cut result
 * says it was cut, by how much, out of what, and against which family's budget
 * — the fail-closed limit discipline in `INVARIANTS.md`: a cap may drop
 * coverage, but it may never let an incomplete result read as complete.
 */

import type { ToolFamily } from "./coder-tool-families.js";

/** The figures a family's budget is derived from. */
interface FamilyBudget {
  /** The family's context window, in tokens, as the lane advertises it. */
  readonly contextWindowTokens: number;
  /** What one tool result may take of that window, in tokens. */
  readonly resultTokens: number;
  /** Approximate characters per token for this family's tokenizer. */
  readonly charactersPerToken: number;
  /** Why this family's result allowance is what it is. */
  readonly because: string;
}

/**
 * The budgets, as data.
 *
 * Exhaustive over `ToolFamily` on purpose: adding a family to that union
 * without deciding what it may spend does not compile. Each row carries the
 * reason it holds the figure it does, because a number nobody can argue with
 * is a number nobody will ever correct.
 */
const BUDGETS: Record<ToolFamily, FamilyBudget> = {
  default: {
    contextWindowTokens: 200_000,
    resultTokens: 1_100,
    // Byte-pair vocabularies of the o200k shape run near four characters a
    // token on prose and lower on the command output a session reads. Held at
    // 3.6 so the estimate errs toward spending less.
    charactersPerToken: 3.6,
    because:
      "the hosted general lanes carry a large window, and 1,100 tokens is the " +
      "measured allowance the shipped 4,000-character bound already amounted to",
  },
  gemini: {
    contextWindowTokens: 1_000_000,
    resultTokens: 700,
    // Google documents roughly four characters a token for English on the
    // SentencePiece vocabulary these lanes use.
    charactersPerToken: 4,
    because:
      "the window is the largest of any lane and the round count is what costs: " +
      "this family issued fifteen tool rounds where another issued six and spent " +
      "three times the input tokens replaying whole-file dumps, so its results " +
      "are cut sooner to make a narrower second read the cheaper move",
  },
  local: {
    contextWindowTokens: 32_768,
    resultTokens: 500,
    // Qwen-shaped byte-pair vocabularies, again held low for code and paths.
    charactersPerToken: 3.5,
    because:
      "the window is a fraction of a hosted one and generation is slow on one " +
      "machine, so every re-sent character is paid in wall clock rather than in " +
      "money",
  },
};

/** The smallest budget on the table: what an unrecognized family is given. */
const mostConservative = (): ToolFamily => {
  const families = Object.keys(BUDGETS) as ReadonlyArray<ToolFamily>;
  let smallest: ToolFamily = "local";
  for (const family of families) {
    if (charactersOf(BUDGETS[family]) < charactersOf(BUDGETS[smallest])) smallest = family;
  }
  return smallest;
};

const charactersOf = (budget: FamilyBudget): number =>
  Math.floor(budget.resultTokens * budget.charactersPerToken);

/** What one tool result may spend, resolved for one family. */
export interface ToolResultBudget {
  /** The family the budget was asked for. */
  readonly family: ToolFamily;
  /** The ceiling the result is cut to. */
  readonly characters: number;
  /** What those characters are believed to cost. */
  readonly tokens: number;
  /** The approximation the two are related by. */
  readonly charactersPerToken: number;
  readonly contextWindowTokens: number;
  /**
   * True when the family had no row and the smallest budget was substituted.
   *
   * Carried rather than hidden: the notice on a cut result says so, because a
   * budget that is a guess and a budget that is a decision are not the same
   * claim.
   */
  readonly substituted: boolean;
}

/**
 * The budget for a family.
 *
 * A family with no row falls back to the smallest budget on the table rather
 * than to a generous default. A family name reaches this from data — a lane
 * name derived from a server catalog, a resumed session's record — so the case
 * is reachable at runtime even though the union is exhaustive at compile time,
 * and guessing high is the failure that is expensive.
 */
export const toolResultBudget = (family: ToolFamily): ToolResultBudget => {
  const held = BUDGETS[family] as FamilyBudget | undefined;
  const substituted = held === undefined;
  const budget = held ?? BUDGETS[mostConservative()];
  return {
    family,
    characters: charactersOf(budget),
    tokens: budget.resultTokens,
    charactersPerToken: budget.charactersPerToken,
    contextWindowTokens: budget.contextWindowTokens,
    substituted,
  };
};

/** One line naming a family's allowance, for a reader asking what it is. */
export const describeBudget = (budget: ToolResultBudget): string =>
  `Tool results are cut to ${String(budget.characters)} characters for the ` +
  `${budget.family} model family: about ${String(budget.tokens)} tokens at an ` +
  `approximate ${String(budget.charactersPerToken)} characters per token, against a ` +
  `${String(budget.contextWindowTokens)}-token window` +
  (budget.substituted
    ? " — the smallest budget on the table, substituted because that family has none of its own."
    : ".");

/**
 * One tool result as the model transcript carries it.
 *
 * Kept at both ends, which is what a long output is read for: a command's
 * first lines say what it did and its last lines say how it ended, and the
 * middle is the part a second, narrower run can recover. The notice in place of
 * the middle is written for the model that has to decide what to do next, so it
 * carries the arithmetic rather than the word "truncated".
 */
export const budgetedResult = (output: string, family: ToolFamily): string => {
  const budget = toolResultBudget(family);
  if (output.length <= budget.characters) return output;

  const half = Math.floor(budget.characters / 2);
  const omitted = output.length - budget.characters;
  const notice =
    `[${String(omitted)} of ${String(output.length)} characters omitted from the middle. ` +
    `${describeBudget(budget)} ` +
    "Every later round of this turn re-sends what you are reading, which is why the " +
    "budget exists. What you have is incomplete and must not be summarized as if it " +
    "were the whole answer: run it again more narrowly — a range, a filter, a count — " +
    "if you need what is missing.]";
  return `${output.slice(0, half)}\n\n${notice}\n\n${output.slice(-half)}`;
};
