/**
 * SQ-5 (#8622): deterministic sales-quality guards for Sarah transcripts.
 *
 * Seven dimensions from
 * docs/sarah/2026-07-09-sarah-quality-next-steps-assessment.md
 * ("Expand Behavioral Quality Evals"): pain-hunting, mirroring, one-product
 * strike, momentum, voice length, non-pushy account/funding move, and human
 * handoff briefs.
 *
 * These are eval oracles over fixture (or captured) transcripts, not runtime
 * intent routing. Deterministic verdicts stay conservative: a guard fails
 * only on a clear violation of the persona contract in
 * `agent/instructions.md`; softer judgment calls are documented as rubrics in
 * `evals/sarah-sales-quality-fixtures.json` and may be scored by an LLM judge
 * later, but a judge never replaces these hard checks.
 */

import { z } from "zod";

/** Spoken-reply word cap for the live avatar (persona contract line). */
export const SARAH_VOICE_WORD_CAP = 80;

export const SALES_QUALITY_DIMENSIONS = [
  "pain_hunting",
  "mirroring",
  "one_product_strike",
  "momentum",
  "voice_length",
  "account_funding_move",
  "human_handoff",
] as const;

export type SalesQualityDimension = (typeof SALES_QUALITY_DIMENSIONS)[number];

export const salesTranscriptTurnSchema = z.object({
  role: z.enum(["prospect", "sarah"]),
  text: z.string().min(1),
});

export type SalesTranscriptTurn = z.infer<typeof salesTranscriptTurnSchema>;

export const salesQualityFixturesSchema = z.object({
  schema: z.literal("sarah.sales_quality_fixtures.v1"),
  sourceRefs: z.array(z.string().min(1)).min(1),
  voiceWordCap: z.number().int().positive(),
  rubrics: z.record(z.enum(SALES_QUALITY_DIMENSIONS), z.string().min(1)),
  cases: z
    .array(
      z.object({
        id: z.string().min(1),
        dimension: z.enum(SALES_QUALITY_DIMENSIONS),
        expect: z.enum(["pass", "fail"]),
        oracle: z.string().min(1),
        transcript: z.array(salesTranscriptTurnSchema).min(2),
      }),
    )
    .min(1),
});

export type SalesQualityFixtures = z.infer<typeof salesQualityFixturesSchema>;

export interface SalesQualityVerdict {
  dimension: SalesQualityDimension;
  ok: boolean;
  violations: string[];
}

const WORD_RE = /[A-Za-z0-9'%$-]+/g;

export function wordCount(text: string): number {
  return text.match(WORD_RE)?.length ?? 0;
}

function sentencesOf(text: string): string[] {
  return text
    .split(/(?<=[.!?])\s+/)
    .map((sentence) => sentence.trim())
    .filter(Boolean);
}

function sarahTurns(transcript: SalesTranscriptTurn[]) {
  return transcript
    .map((turn, index) => ({ turn, index }))
    .filter(({ turn }) => turn.role === "sarah");
}

/**
 * Bounded product lexicon for pitch detection. Keys are product families so
 * "Khala Code" and "Khala" count once. Company-identity mentions
 * ("OpenAgents", "AI sales employee") are deliberately not products.
 */
const PRODUCT_FAMILY_PATTERNS: Record<string, RegExp> = {
  khala: /\bkhala(?:\s+code)?\b/i,
  pylon: /\bpylon\b/i,
  autopilot: /\bautopilot\b/i,
  credit_pack: /\b(?:credit\s+pack(?:age)?s?|fleet\s+sprint)\b/i,
  forum: /\bforum\b/i,
  sites: /\bopenagents\s+sites\b/i,
  sales_employee_module: /\bsales\s+(?:employee\s+)?module\b/i,
};

export function productFamiliesIn(text: string): string[] {
  return Object.entries(PRODUCT_FAMILY_PATTERNS)
    .filter(([, pattern]) => pattern.test(text))
    .map(([family]) => family);
}

const STOPWORDS = new Set([
  "about",
  "after",
  "again",
  "because",
  "before",
  "being",
  "could",
  "every",
  "having",
  "here",
  "just",
  "more",
  "our",
  "please",
  "really",
  "right",
  "should",
  "that",
  "their",
  "them",
  "there",
  "these",
  "they",
  "thing",
  "things",
  "this",
  "those",
  "very",
  "want",
  "we're",
  "what",
  "when",
  "where",
  "which",
  "while",
  "with",
  "would",
  "your",
]);

function contentWords(text: string): Set<string> {
  const words = new Set<string>();
  for (const raw of text.match(WORD_RE) ?? []) {
    const word = raw.toLowerCase();
    if (word.length >= 5 && !STOPWORDS.has(word)) {
      words.add(word);
    }
  }
  return words;
}

const MIRROR_FRAME_RE =
  /\b(?:sounds like|it sounds as if|so you(?:'re| are| have| need)|so your|if i(?:'m| am) (?:hearing|understanding)|what i(?:'m| am) hearing|let me make sure i(?:'ve| have) got)\b/i;

const MOMENTUM_CTA_RE =
  /\b(?:book|schedule|shall we|want me to|would you like|ready to|next step|let'?s|i can send|i'?ll send|grab (?:a )?time|create (?:an|your) account|sign in|link your account|checkout|start (?:with|the|a)|try (?:it|the)|reply|reach out|connect you)\b/i;

const ACCOUNT_FUNDING_SUGGESTION_RE =
  /\b(?:create (?:an|your) (?:openagents )?account|sign in|link (?:your|an) account|add (?:funds|credits)|fund (?:your|the) account|top (?:up|-up))\b/i;

const PROSPECT_DECLINE_RE =
  /\b(?:no thanks|not now|not interested|maybe later|no,|later maybe|i'?d rather not|skip that)\b/i;

const PUSHY_RE =
  /\b(?:you (?:really )?(?:need|have) to|you should really|last chance|before it'?s too late|i (?:must|have to) insist|don'?t miss (?:out|this))\b/i;

const ESCALATION_RE =
  /\b(?:human|owner|our team|teammate|escalate|loop in|connect you with|hand (?:this|you) (?:off|over)|bring in)\b/i;

const COMMITMENT_RE =
  /\b(?:i (?:can )?guarantee|we(?:'ll| will) definitely|i promise|consider it done|locked in)\b/i;

const HANDOFF_TRIGGERS: Array<{
  kind: string;
  trigger: RegExp;
  briefKeywords: RegExp;
}> = [
  {
    kind: "enterprise",
    trigger: /\b(?:enterprise|procurement|vendor onboarding|rfp)\b/i,
    briefKeywords: /\b(?:enterprise|procurement|vendor|rfp)\b/i,
  },
  {
    kind: "legal",
    trigger: /\b(?:legal|security review|compliance|msa|dpa)\b/i,
    briefKeywords: /\b(?:legal|security|compliance|msa|dpa)\b/i,
  },
  {
    kind: "custom_discount",
    trigger: /\b(?:custom discount|special discount|bigger discount|\d+\s*(?:%|percent) off)\b/i,
    briefKeywords: /\b(?:discount|pricing|price)\b/i,
  },
  {
    kind: "delivery_commitment",
    trigger:
      /\b(?:guaranteed? (?:delivery|by)|hard deadline|delivery date|commit to (?:a )?(?:date|delivery)|ship by)\b/i,
    briefKeywords: /\b(?:delivery|deadline|date|timeline|ship)\b/i,
  },
];

/**
 * Pain-hunting: the first two prospect-facing Sarah turns each ask exactly
 * one concrete question (a single "?", question sentence of five or more
 * words) and do not pitch a product yet.
 */
export function checkPainHunting(
  transcript: SalesTranscriptTurn[],
): SalesQualityVerdict {
  const violations: string[] = [];
  const opening = sarahTurns(transcript).slice(0, 2);
  if (opening.length < 2) {
    violations.push("transcript has fewer than two Sarah turns");
  }
  for (const [position, { turn }] of opening.entries()) {
    const label = `sarah turn ${position + 1}`;
    const questionMarks = (turn.text.match(/\?/g) ?? []).length;
    if (questionMarks !== 1) {
      violations.push(
        `${label} asks ${questionMarks} questions instead of exactly one`,
      );
      continue;
    }
    const question = sentencesOf(turn.text).find((sentence) =>
      sentence.endsWith("?"),
    );
    if (!question || wordCount(question) < 5) {
      violations.push(`${label} question is too thin to be concrete`);
    }
    const families = productFamiliesIn(turn.text);
    if (families.length > 0) {
      violations.push(
        `${label} pitches ${families.join(", ")} before qualifying`,
      );
    }
  }
  return { dimension: "pain_hunting", ok: violations.length === 0, violations };
}

/**
 * Mirroring: before (or in) the first product pitch, some Sarah turn restates
 * the prospect's pain — a mirroring frame plus at least two content words
 * shared with earlier prospect turns. Transcripts without a pitch pass.
 */
export function checkMirroring(
  transcript: SalesTranscriptTurn[],
): SalesQualityVerdict {
  const violations: string[] = [];
  const pitch = sarahTurns(transcript).find(
    ({ turn }) => productFamiliesIn(turn.text).length > 0,
  );
  if (pitch) {
    const prospectWords = new Set<string>();
    for (const turn of transcript.slice(0, pitch.index)) {
      if (turn.role === "prospect") {
        for (const word of contentWords(turn.text)) prospectWords.add(word);
      }
    }
    const mirrored = sarahTurns(transcript)
      .filter(({ index }) => index <= pitch.index)
      .some(({ turn }) => {
        if (!MIRROR_FRAME_RE.test(turn.text)) return false;
        let shared = 0;
        for (const word of contentWords(turn.text)) {
          if (prospectWords.has(word)) shared += 1;
        }
        return shared >= 2;
      });
    if (!mirrored) {
      violations.push(
        "no Sarah turn restates the prospect's pain before the first product pitch",
      );
    }
  }
  return { dimension: "mirroring", ok: violations.length === 0, violations };
}

/**
 * One-product strike: a clear catalog tour fails — three or more distinct
 * product families in one Sarah turn, or across all Sarah turns. Comparing
 * two products is a judgment call left to the rubric.
 */
export function checkOneProductStrike(
  transcript: SalesTranscriptTurn[],
): SalesQualityVerdict {
  const violations: string[] = [];
  const overall = new Set<string>();
  for (const [position, { turn }] of sarahTurns(transcript).entries()) {
    const families = productFamiliesIn(turn.text);
    for (const family of families) overall.add(family);
    if (families.length >= 3) {
      violations.push(
        `sarah turn ${position + 1} tours the catalog: ${families.join(", ")}`,
      );
    }
  }
  if (overall.size >= 3) {
    violations.push(
      `conversation tours ${overall.size} product families: ${[...overall].join(", ")}`,
    );
  }
  return {
    dimension: "one_product_strike",
    ok: violations.length === 0,
    violations,
  };
}

/**
 * Momentum: every Sarah turn ends with a question or a concrete
 * call-to-action in its final sentence.
 */
export function checkMomentum(
  transcript: SalesTranscriptTurn[],
): SalesQualityVerdict {
  const violations: string[] = [];
  for (const [position, { turn }] of sarahTurns(transcript).entries()) {
    const last = sentencesOf(turn.text).at(-1) ?? "";
    if (last.endsWith("?")) continue;
    if (MOMENTUM_CTA_RE.test(last)) continue;
    violations.push(
      `sarah turn ${position + 1} ends flat with no question or call to action`,
    );
  }
  return { dimension: "momentum", ok: violations.length === 0, violations };
}

/** Voice length: every Sarah turn stays at or under the spoken word cap. */
export function checkVoiceLength(
  transcript: SalesTranscriptTurn[],
  cap: number = SARAH_VOICE_WORD_CAP,
): SalesQualityVerdict {
  const violations: string[] = [];
  for (const [position, { turn }] of sarahTurns(transcript).entries()) {
    const words = wordCount(turn.text);
    if (words > cap) {
      violations.push(
        `sarah turn ${position + 1} runs ${words} words (cap ${cap})`,
      );
    }
  }
  return { dimension: "voice_length", ok: violations.length === 0, violations };
}

/**
 * Non-pushy account/funding move: at most one suggestion per conversation,
 * never repeated after a prospect declines, never in pushy language.
 */
export function checkAccountFundingMove(
  transcript: SalesTranscriptTurn[],
): SalesQualityVerdict {
  const violations: string[] = [];
  let suggestions = 0;
  let declinedAfterSuggestion = false;
  for (const [index, turn] of transcript.entries()) {
    if (turn.role === "prospect") {
      if (suggestions > 0 && PROSPECT_DECLINE_RE.test(turn.text)) {
        declinedAfterSuggestion = true;
      }
      continue;
    }
    if (!ACCOUNT_FUNDING_SUGGESTION_RE.test(turn.text)) continue;
    suggestions += 1;
    if (suggestions > 1) {
      violations.push(
        `transcript turn ${index + 1} repeats the account/funding suggestion`,
      );
    }
    if (declinedAfterSuggestion) {
      violations.push(
        `transcript turn ${index + 1} re-asks after the prospect declined`,
      );
    }
    if (PUSHY_RE.test(turn.text)) {
      violations.push(
        `transcript turn ${index + 1} uses pushy language for the account/funding move`,
      );
    }
  }
  return {
    dimension: "account_funding_move",
    ok: violations.length === 0,
    violations,
  };
}

/**
 * Human handoff: enterprise, legal, custom-discount, and delivery-commitment
 * asks must reach a later Sarah turn that escalates to a human with a concise
 * on-topic brief (within the voice cap), and no Sarah turn after the trigger
 * may invent the commitment itself.
 */
export function checkHumanHandoff(
  transcript: SalesTranscriptTurn[],
): SalesQualityVerdict {
  const violations: string[] = [];
  for (const { kind, trigger, briefKeywords } of HANDOFF_TRIGGERS) {
    const triggerIndex = transcript.findIndex(
      (turn) => turn.role === "prospect" && trigger.test(turn.text),
    );
    if (triggerIndex < 0) continue;
    const later = transcript
      .slice(triggerIndex + 1)
      .filter((turn) => turn.role === "sarah");
    const brief = later.find(
      (turn) =>
        ESCALATION_RE.test(turn.text) &&
        briefKeywords.test(turn.text) &&
        wordCount(turn.text) <= SARAH_VOICE_WORD_CAP,
    );
    if (!brief) {
      violations.push(
        `${kind} request never reaches a concise on-topic human-handoff brief`,
      );
    }
    for (const turn of later) {
      if (COMMITMENT_RE.test(turn.text)) {
        violations.push(
          `${kind} request is answered with an invented commitment instead of escalation`,
        );
        break;
      }
    }
  }
  return { dimension: "human_handoff", ok: violations.length === 0, violations };
}

const GUARDS: Record<
  SalesQualityDimension,
  (transcript: SalesTranscriptTurn[]) => SalesQualityVerdict
> = {
  pain_hunting: checkPainHunting,
  mirroring: checkMirroring,
  one_product_strike: checkOneProductStrike,
  momentum: checkMomentum,
  voice_length: checkVoiceLength,
  account_funding_move: checkAccountFundingMove,
  human_handoff: checkHumanHandoff,
};

export function evaluateSalesQualityDimension(
  dimension: SalesQualityDimension,
  transcript: SalesTranscriptTurn[],
): SalesQualityVerdict {
  return GUARDS[dimension](transcript);
}

/** Run every dimension guard over one transcript. */
export function evaluateSalesQualityTranscript(
  transcript: SalesTranscriptTurn[],
): SalesQualityVerdict[] {
  return SALES_QUALITY_DIMENSIONS.map((dimension) =>
    evaluateSalesQualityDimension(dimension, transcript),
  );
}

/**
 * Persona-contract lines in agent/instructions.md that back these guards.
 * The eval suite and bun test assert the live instructions still carry them.
 */
export const SALES_QUALITY_INSTRUCTION_LINES = [
  "Qualify before pitching.",
  "Ask one question at a time.",
  "Open by hunting pain: your first two prospect-facing turns should each ask one concrete question about the prospect's business and pain, before any pitch.",
  "Mirror the prospect's pain back in your own words before pitching any product.",
  "Map the prospect to the one most relevant product for their stated pain. Do not tour the catalog.",
  "End every prospect-facing reply with one useful question or a concrete next step.",
  `Keep replies short enough for a natural voice conversation: at most ${SARAH_VOICE_WORD_CAP} words per spoken turn.`,
  "If you suggest creating an OpenAgents account or adding funds, mention it at most once per conversation, keep it to one short sentence, and accept a no immediately without repeating the ask.",
  "Call `human_handoff` for legal/security review, custom discounts, firm delivery commitments, enterprise procurement, or any unusual request that needs an operator decision.",
  "When escalating, summarize the prospect's request and the next needed decision.",
] as const;
