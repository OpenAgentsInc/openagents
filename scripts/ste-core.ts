import { readFileSync } from "node:fs";
import { extname } from "node:path";

export type SteMode = "descriptive" | "procedural" | "mixed" | "source-data";
export type SteStatus = "migration" | "checked" | "inspected" | "source-data" | "superseded";

export interface SteProfile {
  readonly path: string;
  readonly ste_issue: 9;
  readonly ste_mode: SteMode;
  readonly ste_glossary_revision: string;
  readonly ste_status: SteStatus;
  readonly ste_reviewer: string | null;
  readonly ste_reviewed_at: string | null;
  readonly owner: string;
  readonly risk: "control" | "high" | "public" | "active" | "legacy" | "source-data";
  readonly source: string | null;
  readonly replacement: string | null;
  readonly ste_accepted_screening_rules?: readonly (
    | "STE-2.4"
    | "STE-3.6"
    | "STE-5.1"
    | "STE-8.2"
  )[];
  readonly ste_audience?: "human" | "agent" | "dual";
  readonly ste_agent_compact_revision?: "openagents-agent-compact-v1";
}

export interface SteDiagnostic {
  readonly rule: string;
  readonly path: string;
  readonly line: number;
  readonly column: number;
  readonly action: string;
  readonly text: string;
}

export interface StructuralCounts {
  readonly [rule: string]: number;
}

export interface AgentCompactTerms {
  readonly revision: string;
  readonly baseGlossaryRevision: string;
  readonly terms: ReadonlyArray<{
    readonly term: string;
    readonly permittedForms: readonly string[];
    readonly meaning: string;
  }>;
}

export interface CheckerConfig {
  readonly policyRevision: string;
  readonly agentCompactRevision: "openagents-agent-compact-v1";
  readonly steIssue: 9;
  readonly glossaryRevision: string;
  readonly governedExtensions: readonly string[];
  readonly sourceDataPrefixes: readonly string[];
  readonly proceduralPathSignals: readonly string[];
  readonly controlPaths: readonly string[];
}

const contractionPattern =
  /\b(?:aren't|can't|couldn't|didn't|doesn't|don't|hadn't|hasn't|haven't|he's|isn't|it's|mustn't|shouldn't|that's|there's|they're|wasn't|we're|weren't|won't|wouldn't|you're)\b/i;
const passivePattern =
  /\b(?:am|are|be|been|being|is|was|were)\s+(?:\w+ed|built|done|found|given|kept|known|made|read|run|sent|set|shown|taken|written)\b/i;
const britishSpellingPattern =
  /\b(?:analyse|analysed|authorise|authorised|behaviour|colour|favour|labour|licence|optimise|organise|recognise)\b/i;
const markdownCodeFencePattern = /^\s*(```|~~~)/;

export const readCheckerConfig = (root: string): CheckerConfig =>
  JSON.parse(readFileSync(`${root}/docs/ste/checker-config.v1.json`, "utf8")) as CheckerConfig;

export const validateAgentCompactTerms = (value: AgentCompactTerms): readonly string[] => {
  const errors: string[] = [];
  const forms = new Set<string>();
  for (const entry of value.terms) {
    if (!entry.term || !entry.meaning || entry.permittedForms.length === 0)
      errors.push("each agent compact term needs a term, meaning, and permitted form");
    for (const form of entry.permittedForms) {
      const key = form.toLowerCase();
      if (forms.has(key)) errors.push(`duplicate agent compact form: ${form}`);
      forms.add(key);
    }
  }
  return errors;
};

export const isGovernedPath = (path: string, config: CheckerConfig): boolean =>
  config.governedExtensions.includes(extname(path).toLowerCase());

export const deriveProfile = (path: string, config: CheckerConfig): SteProfile => {
  const sourceData = config.sourceDataPrefixes.some((prefix) => path.startsWith(prefix));
  const lowerPath = path.toLowerCase();
  const procedural = config.proceduralPathSignals.some((signal) => lowerPath.includes(signal));
  const control =
    config.controlPaths.includes(path) ||
    path.endsWith("/AGENTS.md") ||
    path.endsWith("/INVARIANTS.md");
  const publicText =
    path.includes("/public/") || path.startsWith("docs/api/") || path.startsWith("docs/guides/");
  const dualChangelog = /^docs\/changelog\/\d{4}-\d{2}-\d{2}-desktop-.+\.md$/.test(path);

  return {
    path,
    ste_issue: 9,
    ste_mode: sourceData ? "source-data" : procedural ? "mixed" : "descriptive",
    ste_glossary_revision: config.glossaryRevision,
    ste_status: sourceData ? "source-data" : "migration",
    ste_reviewer: null,
    ste_reviewed_at: null,
    owner: control
      ? "OpenAgents control plane"
      : publicText
        ? "OpenAgents public documentation"
        : "OpenAgents documentation",
    risk: sourceData
      ? "source-data"
      : control
        ? "control"
        : procedural
          ? "high"
          : publicText
            ? "public"
            : "legacy",
    source: sourceData
      ? path.startsWith("docs/transcripts/")
        ? "Preserved transcript archive"
        : path.startsWith("apps/openagents.com/apps/start/public/docs/")
          ? "Generated from apps/openagents.com/apps/start/content/docs"
          : "Third-party reference material"
      : null,
    replacement: null,
    ...(dualChangelog
      ? {
          ste_audience: "dual" as const,
          ste_agent_compact_revision: config.agentCompactRevision,
        }
      : {}),
  };
};

interface ProseLine {
  readonly number: number;
  readonly text: string;
  readonly startsBlock: boolean;
}

export const extractProse = (input: string): readonly ProseLine[] => {
  const output: ProseLine[] = [];
  let inFence = false;
  const lines = input.split(/\r?\n/);
  let inFrontMatter = lines[0]?.trim() === "---";

  for (let index = 0; index < lines.length; index += 1) {
    const raw = lines[index] ?? "";
    const startsBlock = /^\s{0,3}(?:#{1,6}|>|[-*+]\s|\d+[.)]\s|\|)/.test(raw);
    if (index === 0 && inFrontMatter) continue;
    if (inFrontMatter) {
      if (raw.trim() === "---") inFrontMatter = false;
      continue;
    }
    if (markdownCodeFencePattern.test(raw)) {
      inFence = !inFence;
      continue;
    }
    if (inFence) continue;
    if (/^\s{4,}\S/.test(raw)) continue;

    const text = raw
      .replace(/<!--.*?-->/g, " ")
      .replace(/!\[[^\]]*\]\([^)]*\)/g, " ")
      .replace(/\[([^\]]+)\]\([^)]*\)/g, "$1")
      .replace(/`[^`]*`/g, " ")
      .replace(/https?:\/\/\S+/g, " ")
      .replace(/^\s{0,3}(?:#{1,6}|>|[-*+]\s|\d+[.)]\s)/, "")
      .replace(/^\s*\|?\s*:?-+:?\s*(?:\|\s*:?-+:?\s*)+\|?\s*$/, "")
      .trim();
    if (text) output.push({ number: index + 1, text, startsBlock });
  }
  return output;
};

export const agentCompactLineNumbers = (
  input: string,
  audience: SteProfile["ste_audience"],
): ReadonlySet<number> => {
  const lines = input.split(/\r?\n/);
  if (audience === "agent") return new Set(lines.map((_, index) => index + 1));
  if (audience !== "dual") return new Set();

  const selected = new Set<number>();
  let inAgentSection = false;
  for (let index = 0; index < lines.length; index += 1) {
    const line = lines[index] ?? "";
    if (/^##\s+Agent changelog\s*$/.test(line)) inAgentSection = true;
    else if (/^##\s+/.test(line)) inAgentSection = false;
    if (inAgentSection) selected.add(index + 1);
  }
  return selected;
};

const wordCount = (text: string): number =>
  text.match(/[A-Za-z0-9]+(?:[-'][A-Za-z0-9]+)*/g)?.length ?? 0;

const diagnostic = (
  rule: string,
  path: string,
  line: number,
  text: string,
  match: RegExpMatchArray | null,
  action: string,
): SteDiagnostic => ({
  rule,
  path,
  line,
  column: (match?.index ?? 0) + 1,
  action,
  text: text.trim().slice(0, 180),
});

export const inspectStructure = (
  path: string,
  input: string,
  mode: SteMode,
): readonly SteDiagnostic[] => {
  if (mode === "source-data") return [];
  const prose = extractProse(input);
  const diagnostics: SteDiagnostic[] = [];
  const sentenceLimit = mode === "procedural" ? 20 : 25;
  let paragraph: ProseLine[] = [];

  const inspectParagraph = (): void => {
    if (paragraph.length === 0) return;
    const paragraphText = paragraph.map((line) => line.text).join(" ");
    const sentences = paragraphText.match(/[^.!?]+[.!?]+|[^.!?]+$/g) ?? [];
    const sentenceCount = sentences.filter((sentence) => wordCount(sentence) > 1).length;
    for (const sentence of sentences) {
      if (wordCount(sentence) > sentenceLimit) {
        const first = paragraph[0]!;
        diagnostics.push(
          diagnostic(
            "STE-5.1",
            path,
            first.number,
            sentence,
            null,
            `Use not more than ${sentenceLimit} words in this sentence.`,
          ),
        );
      }
    }
    if (sentenceCount > 6) {
      const first = paragraph[0]!;
      diagnostics.push(
        diagnostic(
          "STE-8.2",
          path,
          first.number,
          first.text,
          null,
          "Divide the paragraph into units of not more than six sentences.",
        ),
      );
    }
    paragraph = [];
  };

  let previousLine = 0;
  for (const line of prose) {
    if (previousLine > 0 && (line.number > previousLine + 1 || line.startsBlock))
      inspectParagraph();
    paragraph.push(line);
    previousLine = line.number;

    const semicolon = line.text.match(/;/);
    if (semicolon)
      diagnostics.push(
        diagnostic(
          "STE-8.1",
          path,
          line.number,
          line.text,
          semicolon,
          "Replace the semicolon with approved punctuation and divide the sentence if necessary.",
        ),
      );
    const contraction = line.text.match(contractionPattern);
    if (contraction)
      diagnostics.push(
        diagnostic(
          "STE-9.1",
          path,
          line.number,
          line.text,
          contraction,
          "Write the full form of the contraction.",
        ),
      );
    const spelling = line.text.match(britishSpellingPattern);
    if (spelling)
      diagnostics.push(
        diagnostic(
          "STE-1.4",
          path,
          line.number,
          line.text,
          spelling,
          "Use the American English spelling.",
        ),
      );
    const ingForm = line.text.match(/\b[A-Za-z]{4,}ing\b/i);
    if (ingForm)
      diagnostics.push(
        diagnostic(
          "STE-2.4",
          path,
          line.number,
          line.text,
          ingForm,
          "Confirm that this -ing form is an approved noun or modifier. Otherwise, write the sentence again.",
        ),
      );
    if (mode === "procedural" || mode === "mixed") {
      const passive = line.text.match(passivePattern);
      if (passive)
        diagnostics.push(
          diagnostic(
            "STE-3.6",
            path,
            line.number,
            line.text,
            passive,
            "Use the active voice in the procedure.",
          ),
        );
    }
  }
  inspectParagraph();
  return diagnostics;
};

export const countDiagnostics = (diagnostics: readonly SteDiagnostic[]): StructuralCounts => {
  const counts: Record<string, number> = {};
  for (const item of diagnostics) counts[item.rule] = (counts[item.rule] ?? 0) + 1;
  return counts;
};

export const applyScreeningReview = (
  diagnostics: readonly SteDiagnostic[],
  profile: SteProfile,
): readonly SteDiagnostic[] => {
  const accepted = new Set(profile.ste_accepted_screening_rules ?? []);
  if (accepted.size === 0 || !profile.ste_reviewer || !profile.ste_reviewed_at) return diagnostics;
  return diagnostics.filter(
    (item) => !accepted.has(item.rule as "STE-2.4" | "STE-3.6" | "STE-5.1" | "STE-8.2"),
  );
};

export const validateGlossary = (value: unknown): readonly string[] => {
  if (!value || typeof value !== "object") return ["glossary must be an object"];
  const glossary = value as { revision?: unknown; steIssue?: unknown; terms?: unknown };
  const errors: string[] = [];
  if (typeof glossary.revision !== "string" || !glossary.revision)
    errors.push("glossary revision is required");
  if (glossary.steIssue !== 9) errors.push("glossary steIssue must be 9");
  if (!Array.isArray(glossary.terms)) return [...errors, "glossary terms must be an array"];
  const ids = new Set<string>();
  const forms = new Set<string>();
  for (const [index, raw] of glossary.terms.entries()) {
    if (!raw || typeof raw !== "object") {
      errors.push(`term ${index} must be an object`);
      continue;
    }
    const term = raw as Record<string, unknown>;
    const id = typeof term.id === "string" ? term.id : "";
    if (!/^OA-STE-[0-9]{4}$/.test(id)) errors.push(`term ${index} has an invalid id`);
    if (ids.has(id)) errors.push(`term ${index} has a duplicate id`);
    ids.add(id);
    if (!Array.isArray(term.permittedForms) || term.permittedForms.length === 0)
      errors.push(`term ${id || index} needs permittedForms`);
    for (const form of Array.isArray(term.permittedForms) ? term.permittedForms : []) {
      if (typeof form !== "string" || !form.trim())
        errors.push(`term ${id || index} has an invalid form`);
      const key = String(form).toLowerCase();
      if (forms.has(key))
        errors.push(`term ${id || index} has a duplicate permitted form: ${form}`);
      forms.add(key);
    }
    if (typeof term.term === "string" && term.term.trim().split(/\s+/).length > 3)
      errors.push(`term ${id || index} has more than three words`);
  }
  return errors;
};

export const dictionaryWords = (path: string): ReadonlySet<string> => {
  const value = JSON.parse(readFileSync(path, "utf8")) as { steIssue?: unknown; entries?: unknown };
  if (value.steIssue !== 9 || !Array.isArray(value.entries))
    throw new Error("authorized dictionary must contain steIssue 9 and an entries array");
  const words = new Set<string>();
  for (const entry of value.entries) {
    if (!entry || typeof entry !== "object") continue;
    const forms = (entry as { permittedForms?: unknown }).permittedForms;
    if (!Array.isArray(forms)) continue;
    for (const form of forms) if (typeof form === "string") words.add(form.toLowerCase());
  }
  return words;
};
