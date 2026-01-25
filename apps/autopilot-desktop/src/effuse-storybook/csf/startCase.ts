/**
 * Utility to convert strings to Start Case
 * e.g. "someName1234" -> "Some Name 1234"
 *
 * Implements minimal lodash.startCase behavior without deps
 */

export function startCase(str: string): string {
  if (!str) return ""

  // 1. Split into words based on camelCase, snake_case, kebab-case, or numbers
  const words = str
    // Inject space before capital letters (camelCase)
    .replace(/([a-z])([A-Z])/g, "$1 $2")
    // Inject space around numbers
    .replace(/([a-zA-Z])([0-9])/g, "$1 $2")
    .replace(/([0-9])([a-zA-Z])/g, "$1 $2")
    // Replace separators with spaces
    .replace(/[-_.]/g, " ")
    // Split by whitespace
    .split(/\s+/)

  // 2. Capitalize first letter of each word and join
  return words
    .filter((w) => w.length > 0)
    .map((w) => w.charAt(0).toUpperCase() + w.slice(1))
    .join(" ")
}
