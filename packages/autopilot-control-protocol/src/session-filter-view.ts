import { filterSessions } from "./session-filter.js"
import { sortSessions, type SessionSortRow } from "./session-sort.js"

type SessionListFilter = "all" | "running" | "completed"

type SessionListViewInput = {
  rows: SessionSortRow[]
  filter: SessionListFilter
}

type SessionListView = {
  rows: any[]
  counts: {
    all: number
    running: number
    completed: number
  }
}

export function buildSessionListView(input: SessionListViewInput): SessionListView {
  const rows = Array.isArray(input.rows) ? input.rows : []
  const filteredRows = input.filter === "all"
    ? rows
    : filterSessions(rows, { state: input.filter })

  return {
    rows: sortSessions(filteredRows),
    counts: {
      all: rows.length,
      running: filterSessions(rows, { state: "running" }).length,
      completed: filterSessions(rows, { state: "completed" }).length,
    },
  }
}
