// Card commands for Kiki's house editor (`.kiki` pages): "show inventory",
// "close chat", "open the tools window". This is an explicitly modeled
// grammar, not keyword spotting: a whole message parses as
//
//   [please] VERB [the|my] CARD [card|window|panel|pane]
//
// with VERB and CARD taken from the closed tables below, or it is not a
// command at all (and goes to the editing agent as an ordinary request).
export const CARDS = [
  "tools",
  "voice",
  "chat",
  "inventory",
  "payouts",
  "actions",
] as const;
export type Card = (typeof CARDS)[number];

export const CARD_NAMES: Readonly<Record<Card, string>> = {
  tools: "Tools",
  voice: "Voice",
  chat: "Chat",
  inventory: "World inventory",
  payouts: "Edits and payouts",
  actions: "World actions",
};

export type Verb = "open" | "close";

const VERBS: Readonly<Record<string, Verb>> = {
  open: "open",
  show: "open",
  close: "close",
  hide: "close",
};

/** Every accepted way to name a card, as whole token sequences. */
const ALIASES: ReadonlyArray<readonly [string, Card]> = [
  ["tools", "tools"],
  ["tool", "tools"],
  ["designer tools", "tools"],
  ["editor", "tools"],
  ["jarvis", "tools"],
  ["voice", "voice"],
  ["voice bot", "voice"],
  ["voice controls", "voice"],
  ["chat", "chat"],
  ["inventory", "inventory"],
  ["world inventory", "inventory"],
  ["assets", "inventory"],
  ["props", "inventory"],
  ["payouts", "payouts"],
  ["edits", "payouts"],
  ["edits and payouts", "payouts"],
  ["edit log", "payouts"],
  ["log", "payouts"],
  ["contributions", "payouts"],
  ["actions", "actions"],
  ["world actions", "actions"],
];
const ALIAS = new Map(ALIASES);
const ARTICLES = new Set(["the", "my"]);
const NOUNS = new Set(["card", "window", "panel", "pane"]);

export interface Command {
  readonly verb: Verb;
  readonly card: Card;
}

/** Lowercase words, without the punctuation a person or speech engine adds. */
const words = (text: string) =>
  text
    .toLowerCase()
    .replace(/[^\p{L}\p{N}\s]/gu, " ")
    .split(/\s+/)
    .filter(Boolean);

export function parseCommand(text: string): Command | undefined {
  const tokens = words(text);
  if (tokens.length > 8) return undefined;
  let at = 0;
  if (tokens[at] === "please") at++;
  const verb = VERBS[tokens[at] ?? ""];
  if (!verb) return undefined;
  at++;
  if (ARTICLES.has(tokens[at] ?? "")) at++;
  let end = tokens.length;
  if (end > at + 1 && NOUNS.has(tokens[end - 1]!)) end--;
  const card = ALIAS.get(tokens.slice(at, end).join(" "));
  return card ? { verb, card } : undefined;
}
