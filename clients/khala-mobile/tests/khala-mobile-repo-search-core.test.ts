import { describe, expect, test } from "bun:test"

import {
  dedupeKhalaMobileRepositoriesById,
  filterKhalaMobileRepositories,
  sortKhalaMobileRepositoriesForPicker,
} from "../src/sync/khala-mobile-repo-search-core"
import type { KhalaMobileRepository } from "../src/sync/khala-mobile-repos-api"

const repo = (overrides: Partial<KhalaMobileRepository> & Pick<KhalaMobileRepository, "id" | "name" | "owner">): KhalaMobileRepository => ({
  defaultBranch: "main",
  description: null,
  fullName: `${overrides.owner}/${overrides.name}`,
  htmlUrl: `https://github.com/${overrides.owner}/${overrides.name}`,
  private: false,
  provider: "github",
  ...overrides,
})

describe("filterKhalaMobileRepositories", () => {
  const repos = [
    repo({ id: "1", name: "openagents", owner: "OpenAgentsInc" }),
    repo({ description: "A game engine", id: "2", name: "godot", owner: "godotengine" }),
    repo({ id: "3", name: "khala-mobile-fork", owner: "someone" }),
  ]

  test("returns everything for a blank search term", () => {
    expect(filterKhalaMobileRepositories(repos, "  ")).toEqual(repos)
  })

  test("matches full name case-insensitively", () => {
    expect(filterKhalaMobileRepositories(repos, "OPENAGENTSINC/OPENAGENTS")).toEqual([repos[0]!])
  })

  test("matches by bare repo name", () => {
    expect(filterKhalaMobileRepositories(repos, "khala")).toEqual([repos[2]!])
  })

  test("matches by owner", () => {
    expect(filterKhalaMobileRepositories(repos, "godotengine")).toEqual([repos[1]!])
  })

  test("matches by description", () => {
    expect(filterKhalaMobileRepositories(repos, "game engine")).toEqual([repos[1]!])
  })

  test("returns empty for no match", () => {
    expect(filterKhalaMobileRepositories(repos, "nonexistent-xyz")).toEqual([])
  })
})

describe("sortKhalaMobileRepositoriesForPicker", () => {
  test("sorts alphabetically by fullName", () => {
    const repos = [
      repo({ id: "1", name: "zeta", owner: "z" }),
      repo({ id: "2", name: "alpha", owner: "a" }),
      repo({ id: "3", name: "middle", owner: "m" }),
    ]
    expect(sortKhalaMobileRepositoriesForPicker(repos).map(r => r.fullName)).toEqual([
      "a/alpha",
      "m/middle",
      "z/zeta",
    ])
  })
})

describe("dedupeKhalaMobileRepositoriesById", () => {
  test("keeps the first occurrence of each id", () => {
    const first = repo({ id: "1", name: "openagents", owner: "OpenAgentsInc" })
    const duplicate = repo({ id: "1", name: "openagents-renamed", owner: "OpenAgentsInc" })
    const other = repo({ id: "2", name: "godot", owner: "godotengine" })
    expect(dedupeKhalaMobileRepositoriesById([first, duplicate, other])).toEqual([first, other])
  })
})
