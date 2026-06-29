export type SessionSearchRankRow = {
  sessionRef: string
  latestActivity: string
  state: string
}

export type SessionSearchRankResult = {
  sessionRef: string
  score: number
}

const RUNNING_STATE_BOOST = 1

function countMatches(haystack: string, needle: string): number {
  let count = 0
  let fromIndex = 0

  while (fromIndex < haystack.length) {
    const matchIndex = haystack.indexOf(needle, fromIndex)
    if (matchIndex === -1) break

    count += 1
    fromIndex = matchIndex + needle.length
  }

  return count
}

export function rankSessionSearch(
  query: string,
  rows: SessionSearchRankRow[],
): SessionSearchRankResult[] {
  if (query === "") return []

  const needle = query.toLowerCase()

  return rows
    .map((row, index) => {
      const haystack = `${row.latestActivity} ${row.sessionRef}`.toLowerCase()
      const matchCount = countMatches(haystack, needle)

      if (matchCount === 0) {
        return {
          index,
          result: null,
        }
      }

      return {
        index,
        result: {
          sessionRef: row.sessionRef,
          score: matchCount + (row.state === "running" ? RUNNING_STATE_BOOST : 0),
        },
      }
    })
    .filter((row): row is { index: number; result: SessionSearchRankResult } => row.result !== null)
    .sort((left, right) => right.result.score - left.result.score || left.index - right.index)
    .map((row) => row.result)
}
