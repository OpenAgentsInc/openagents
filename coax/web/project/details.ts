// WORK_DETAILS (project/<slug>/work.coax): one row an item, the text its
// detail panel shows, keyed by alias. The tables split rows on newlines and
// fields on '|', so a field never holds either raw. Inside a field:
//
//   \\  a backslash      \n  a newline      \p  a '|'
//   \.  the next item of a list             \:  the next part of an item
//
// Columns: alias | event id | claimed at (Chicago) | body | acceptance | notes
//   body        the issue's text, its "## Acceptance" section taken out
//   acceptance  that section's checks, a list
//   notes       a list; each note is time (Chicago) \: by \: text
//
// Decoding reads left to right: a backslash and the byte after it are one
// unit, so "\\." is a backslash then a dot, never a list break.
// project.coax's undetail is the same decoder in Coax.

export const ITEM = "\\.", PART = "\\:"

/** One text as a WORK_DETAILS field. */
export const escapeDetail = (s: string) => s.replace(/\r\n?/g, "\n").replace(/\\/g, "\\\\").replace(/\|/g, "\\p").replace(/\n/g, "\\n")

/** A list (of lists of parts) as one field. */
export const escapeList = (items: string[][]) => items.map((parts) => parts.map(escapeDetail).join(PART)).join(ITEM)

/** The inverse: a field back to its items and their parts. */
export function unescapeDetail(field: string): string[][] {
  const items: string[][] = [[""]]
  for (let i = 0; i < field.length; i++) {
    const c = field[i]
    const item = items[items.length - 1]
    if (c !== "\\" || i + 1 >= field.length) { item[item.length - 1] += c; continue }
    const e = field[++i]
    if (e === ".") items.push([""])
    else if (e === ":") item.push("")
    else item[item.length - 1] += e === "n" ? "\n" : e === "p" ? "|" : e
  }
  return items
}

/** A body split into its text and its acceptance checks ("## Acceptance", then "- " lines). */
export function splitAcceptance(body: string): { body: string; acceptance: string[] } {
  const lines = body.replace(/\r\n?/g, "\n").split("\n")
  const at = lines.findIndex((l) => /^##\s+Acceptance\s*$/i.test(l.trim()))
  if (at < 0) return { body: body.trim(), acceptance: [] }
  let end = lines.findIndex((l, i) => i > at && /^##\s/.test(l.trim()))
  if (end < 0) end = lines.length
  const acceptance: string[] = []
  for (const l of lines.slice(at + 1, end)) {
    const m = /^\s*[-*]\s+(.*)$/.exec(l)
    if (m) acceptance.push(m[1].trim())
    else if (l.trim() && acceptance.length) acceptance[acceptance.length - 1] += " " + l.trim()
  }
  const rest = [...lines.slice(0, at), ...lines.slice(end)].join("\n").replace(/\n{3,}/g, "\n\n").trim()
  return { body: rest, acceptance }
}
