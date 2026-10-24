import type { EditorCommand, SelectionSummary } from "./protocol.ts";

const aliases: Record<string, string[]> = {
  cottage: ["cottage", "cottages", "house", "houses"],
  townhouse: [
    "townhouse",
    "townhouses",
    "town house",
    "town houses",
    "manor",
    "manors",
  ],
  barrel: ["barrel", "barrels", "cask", "casks"],
  bench: ["bench", "benches"],
  chair: ["chair", "chairs"],
  "market-stall": [
    "market stall",
    "market stalls",
    "market stand",
    "market stands",
    "stall",
    "stalls",
  ],
  cart: ["cart", "carts", "wagon", "wagons"],
  fountain: ["fountain", "fountains"],
  "quaternius:fantasy-standard:cauldron": ["cauldron", "cauldrons"],
  "quaternius:fantasy-standard:bookstand": [
    "book stand",
    "book stands",
    "lectern",
    "lecterns",
  ],
  "quaternius:fantasy-standard:book_stack_2": [
    "book stack",
    "book stacks",
    "books",
  ],
  "quaternius:fantasy-standard:bookcase_2": [
    "bookcase",
    "bookcases",
    "bookshelf",
    "bookshelves",
  ],
  "quaternius:fantasy-standard:candlestick_stand": [
    "candle stand",
    "candle stands",
    "candlestick",
    "candlesticks",
  ],
  "quaternius:fantasy-standard:cage_small": [
    "cage",
    "cages",
    "small cage",
    "small cages",
  ],
  "quaternius:fantasy-standard:chain_coil": [
    "chain",
    "chains",
    "chain coil",
    "chain coils",
    "coiled chain",
  ],
  "quaternius:fantasy-standard:chest_wood": [
    "chest",
    "chests",
    "wooden chest",
    "wooden chests",
  ],
};
const names = new Map(
  Object.entries(aliases).flatMap(([id, words]) =>
    words.map((word) => [word, id] as const),
  ),
);
const nounPattern = Array.from(names.keys())
  .sort((a, b) => b.length - a.length)
  .join("|");
const counts = [
  "zero",
  "one",
  "two",
  "three",
  "four",
  "five",
  "six",
  "seven",
  "eight",
  "nine",
  "ten",
  "eleven",
  "twelve",
  "thirteen",
  "fourteen",
  "fifteen",
  "sixteen",
  "seventeen",
  "eighteen",
  "nineteen",
  "twenty",
];
const countPattern = `a|an|${counts.join("|")}|\\d+`;
const addPattern = new RegExp(
  `^(?:add|place|spawn|put|give me) (?:(${countPattern}) )?(${nounPattern})(?: (.+))?$`,
);
const placements = new Map<string, "ahead" | "left" | "right">([
  ["", "ahead"],
  ["ahead", "ahead"],
  ["in front", "ahead"],
  ["in front of me", "ahead"],
  ["in front of you", "ahead"],
  ["left", "left"],
  ["to my left", "left"],
  ["on my left", "left"],
  ["to the left", "left"],
  ["on the left", "left"],
  ["left of me", "left"],
  ["right", "right"],
  ["to my right", "right"],
  ["on my right", "right"],
  ["to the right", "right"],
  ["on the right", "right"],
  ["right of me", "right"],
]);
const sizes: Record<string, number> = {
  small: 4,
  normal: 6,
  medium: 6,
  large: 8,
  big: 8,
  huge: 10,
};
type Selection = Pick<SelectionSummary, "id" | "scale">;

function placement(text = ""): "ahead" | "left" | "right" {
  const value = placements.get(text);
  if (!value)
    throw new Error("Choose in front of you, to your left, or to your right.");
  return value;
}
function selected(selection?: Selection | null): Selection {
  if (!selection || !selection.id || !Number.isFinite(selection.scale))
    throw new Error("Select an object first.");
  return selection;
}
function resize(
  selection: Selection | null | undefined,
  scale: number,
): EditorCommand {
  const target = selected(selection);
  if (!Number.isFinite(scale) || scale < 0.25 || scale > 4)
    throw new Error("Choose an object scale from 0.25 to 4.");
  return { kind: "entity.transform", id: target.id, scale };
}

/** Closed local commands: exact values and aliases resolve in code, with no model or source execution. */
export function parseCommand(
  input: string,
  selection?: Selection | null,
): EditorCommand {
  if (typeof input !== "string" || Array.from(input).length > 1200)
    throw new Error("Keep your request to 1,200 characters or fewer.");
  const text = input
    .trim()
    .toLowerCase()
    .replace(/\s+/g, " ")
    .replace(/^(?:please )?(?:(?:can|could|would) you )?(?:please )?/, "")
    .replace(/(?: please)?[.!?]*$/, "")
    .trim();
  if (!text) throw new Error("Describe a change first.");
  if (/^(?:undo|undo (?:that|this|last (?:edit|change))|go back)$/.test(text))
    return { kind: "history.undo" };
  if (/^(?:redo|redo (?:that|this|last (?:edit|change)))$/.test(text))
    return { kind: "history.redo" };
  if (
    /^(?:knock down|remove|delete|demolish) (?:(?:this|that|the|aimed) )?wall$/.test(
      text,
    )
  )
    return { kind: "wall.remove" };

  const room =
    /^(?:add|build|create) (?:a |an |one )?(?:(small|normal|medium|large|big|huge|\d+(?:\.\d+)?(?: (?:meter|metre|meters|metres|m))?) )?room(?: (.+))?$/.exec(
      text,
    );
  if (room) {
    const sizeText = room[1];
    const size = !sizeText
      ? 6
      : (sizes[sizeText] ?? Number.parseFloat(sizeText));
    if (!Number.isFinite(size) || size < 3 || size > 10)
      throw new Error("Choose a room size from 3 to 10 meters.");
    return { kind: "room.add", side: placement(room[2]), size };
  }
  const asset = addPattern.exec(text);
  if (asset) {
    const countWord = asset[1];
    const count =
      !countWord || countWord === "a" || countWord === "an"
        ? 1
        : /^\d+$/.test(countWord)
          ? Number(countWord)
          : counts.indexOf(countWord);
    if (!Number.isSafeInteger(count) || count < 1 || count > 8)
      throw new Error("Add between 1 and 8 objects at a time.");
    return {
      kind: "asset.add",
      assetId: names.get(asset[2])!,
      count,
      placement: placement(asset[3]),
    };
  }
  if (
    /^(?:remove|delete) (?:it|this|that|(?:the |this |that )?(?:selected )?object)$/.test(
      text,
    )
  )
    return { kind: "entity.remove", id: selected(selection).id };
  const relative =
    /^(?:make (?:it|this|that|the selected object) |)(bigger|larger|smaller)$/.exec(
      text,
    );
  if (relative)
    return resize(
      selection,
      selected(selection).scale * (relative[1] === "smaller" ? 0.8 : 1.25),
    );
  const exact =
    /^(?:resize|scale) (?:it|this|that|the selected object) to (\d+(?:\.\d+)?)(?:x| times)?$/.exec(
      text,
    );
  if (exact) return resize(selection, Number(exact[1]));
  throw new Error(
    "Try “add two barrels to my left”, “add a room”, “knock down this wall”, or “undo”.",
  );
}
