import { compactGeneratedWGSL } from "./wgsl-source.ts";

type Token = { value: string; start: number; end: number };
type FunctionSpan = {
  name: string;
  start: number;
  end: number;
  body: string;
  entry: boolean;
};
const tokens = (source: string): Token[] =>
  Array.from(source.matchAll(/[A-Za-z_][A-Za-z_0-9]*|[^\s]/g), (match) => ({
    value: match[0],
    start: match.index!,
    end: match.index! + match[0].length,
  }));

/** Prune only explicitly listed helpers from owned generated WGSL, never user source. */
export function pruneGeneratedShaderFunctions(
  source: string,
  candidates: readonly string[],
): string {
  if (
    new Set(candidates).size !== candidates.length ||
    candidates.some((name) => !/^[A-Za-z_]\w*$/.test(name))
  )
    throw new Error("List distinct generated shader helper names.");
  const code = compactGeneratedWGSL(source);
  const lexed = tokens(code),
    functions = new Map<string, FunctionSpan>();
  let level = 0;
  for (let index = 0; index < lexed.length; index++) {
    const token = lexed[index];
    if (token.value === "{") level++;
    else if (token.value === "}") level--;
    if (level < 0)
      throw new Error("The generated shader has unmatched braces.");
    if (level || token.value !== "fn") continue;
    const name = lexed[index + 1]?.value;
    if (
      !name ||
      !/^[A-Za-z_]\w*$/.test(name) ||
      lexed[index + 2]?.value !== "("
    )
      throw new Error("The generated shader has an incomplete function.");
    if (functions.has(name))
      throw new Error("Generated shader function names must be unique.");
    let first = index,
      previous = index - 1;
    const attributes: string[] = [];
    while (previous >= 0) {
      if (lexed[previous].value === ")") {
        let parentheses = 1;
        previous--;
        while (previous >= 0 && parentheses) {
          if (lexed[previous].value === ")") parentheses++;
          if (lexed[previous].value === "(") parentheses--;
          previous--;
        }
        if (parentheses)
          throw new Error("The generated shader has an incomplete attribute.");
      }
      if (!lexed[previous] || lexed[previous - 1]?.value !== "@") break;
      attributes.push(lexed[previous].value);
      first = previous - 1;
      previous -= 2;
    }
    let open = index + 3;
    while (open < lexed.length && lexed[open].value !== "{") open++;
    if (open === lexed.length)
      throw new Error("The generated shader has an incomplete function.");
    let depth = 1,
      end = open + 1;
    while (end < lexed.length && depth) {
      if (lexed[end].value === "{") depth++;
      if (lexed[end].value === "}") depth--;
      end++;
    }
    if (depth) throw new Error("The generated shader has unmatched braces.");
    functions.set(name, {
      name,
      start: lexed[first].start,
      end: lexed[end - 1].end,
      body: code.slice(lexed[open].end, lexed[end - 1].start),
      entry: attributes.some((value) =>
        ["vertex", "fragment", "compute"].includes(value),
      ),
    });
    index = end - 1;
  }
  if (level) throw new Error("The generated shader has unmatched braces.");

  const called = (body: string) => {
    const values = tokens(body);
    return values
      .filter(
        (token, index) =>
          values[index + 1]?.value === "(" && functions.has(token.value),
      )
      .map((token) => token.value);
  };
  const removable = new Set(candidates),
    reachable = new Set<string>();
  const pending = [...functions.values()]
    .filter((fn) => fn.entry || !removable.has(fn.name))
    .map((fn) => fn.name);
  let outside = "",
    position = 0;
  for (const fn of functions.values()) {
    outside += code.slice(position, fn.start);
    position = fn.end;
  }
  outside += code.slice(position);
  pending.push(...called(outside));
  while (pending.length) {
    const name = pending.pop()!;
    if (reachable.has(name)) continue;
    reachable.add(name);
    pending.push(...called(functions.get(name)!.body));
  }
  let output = "";
  position = 0;
  for (const fn of functions.values())
    if (removable.has(fn.name) && !reachable.has(fn.name)) {
      output += code.slice(position, fn.start);
      position = fn.end;
    }
  output += code.slice(position);
  return compactGeneratedWGSL(output);
}
