/** Compact owned generated WGSL without changing token boundaries or user edits. */
export function compactGeneratedWGSL(source: string): string {
  let output = "",
    block = 0,
    line = false;
  for (let i = 0; i < source.length; i++) {
    const c = source[i],
      next = source[i + 1];
    if (line) {
      if (c === "\n") {
        line = false;
        output += c;
      }
    } else if (block) {
      if (c === "/" && next === "*") {
        block++;
        i++;
      } else if (c === "*" && next === "/") {
        block--;
        i++;
      } else if (c === "\n") output += c;
    } else if (c === "/" && next === "/") {
      output += " ";
      line = true;
      i++;
    } else if (c === "/" && next === "*") {
      output += " ";
      block = 1;
      i++;
    } else output += c;
  }
  if (block)
    throw new Error("The generated shader contains an unfinished comment.");
  return output
    .split("\n")
    .map((line) => line.replace(/[\t ]+/g, " ").trim())
    .filter(Boolean)
    .join("\n")
    .replace(/\s*([{}()[\],;])\s*/g, "$1");
}
