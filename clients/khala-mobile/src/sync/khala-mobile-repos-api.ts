/**
 * MM-B2 (#8472): client for the MM-B1 (#8471, merged 58a07f0657) mobile repo
 * list/detail API. Route shape pinned from the #8471 handoff comment on
 * #8472 (also documented server-side in `openagents-openapi.ts`,
 * operationIds `listMobileRepositories` / `getMobileRepository`):
 *
 * `GET /api/mobile/repos?page=&perPage=` — paginated repo list
 *   200 `{ repositories, page, perPage, hasNextPage }`
 *   400 `{ error: "bad_request", reason }`
 *   401 `{ error: "unauthorized" }`
 *   409 `{ error: "github_token_missing" }`
 *   401 `{ error: "github_token_expired" }`
 *
 * `GET /api/mobile/repos/{owner}/{name}` — single-repo fetch
 *   200 `{ repository }`
 *   404 `{ error: "repository_not_found", repositoryId }`
 *   409 / 401 token errors same shape as above
 */

export type KhalaMobileRepository = Readonly<{
  defaultBranch: string
  description: string | null
  fullName: string
  htmlUrl: string
  id: string
  name: string
  owner: string
  private: boolean
  provider: "github"
}>

export type KhalaMobileReposFetchLike = (
  url: string,
  init: { headers: Record<string, string> },
) => Promise<{ json: () => Promise<unknown>; ok: boolean; status?: number }>

export type KhalaMobileReposErrorKind =
  | "unauthorized"
  | "github_token_missing"
  | "github_token_expired"
  | "not_found"
  | "bad_request"
  | "unknown"

export type KhalaMobileReposResult<T> =
  | Readonly<{ ok: true; value: T }>
  | Readonly<{ kind: KhalaMobileReposErrorKind; messageSafe: string; ok: false }>

export type KhalaMobileReposListPage = Readonly<{
  hasNextPage: boolean
  page: number
  perPage: number
  repositories: ReadonlyArray<KhalaMobileRepository>
}>

const errorKindFromBody = (status: number | undefined, body: unknown): KhalaMobileReposErrorKind => {
  const errorCode =
    body !== null && typeof body === "object" && "error" in body && typeof (body as { error: unknown }).error === "string"
      ? (body as { error: string }).error
      : null
  if (errorCode === "github_token_missing") return "github_token_missing"
  if (errorCode === "github_token_expired") return "github_token_expired"
  if (errorCode === "repository_not_found") return "not_found"
  if (errorCode === "bad_request") return "bad_request"
  if (status === 401) return "unauthorized"
  if (status === 404) return "not_found"
  if (status === 400) return "bad_request"
  return "unknown"
}

const MESSAGES: Record<KhalaMobileReposErrorKind, string> = {
  bad_request: "That page request was invalid.",
  github_token_expired: "Your GitHub connection has expired. Sign in with GitHub again to reconnect.",
  github_token_missing: "Connect your GitHub account to see your repositories.",
  not_found: "That repository could not be found.",
  unauthorized: "Sign in again to continue.",
  unknown: "Could not load repositories right now.",
}

const requestRepos = async <T>(
  url: string,
  token: string,
  fetchImpl: KhalaMobileReposFetchLike,
  parse: (body: unknown) => T | null,
): Promise<KhalaMobileReposResult<T>> => {
  try {
    const response = await fetchImpl(url, { headers: { authorization: `Bearer ${token}` } })
    const body = await response.json()
    if (!response.ok) {
      const kind = errorKindFromBody(response.status, body)
      return { kind, messageSafe: MESSAGES[kind], ok: false }
    }
    const parsed = parse(body)
    if (parsed === null) {
      return { kind: "unknown", messageSafe: MESSAGES.unknown, ok: false }
    }
    return { ok: true, value: parsed }
  } catch {
    return { kind: "unknown", messageSafe: MESSAGES.unknown, ok: false }
  }
}

const parseRepository = (value: unknown): KhalaMobileRepository | null => {
  if (value === null || typeof value !== "object") return null
  const record = value as Record<string, unknown>
  if (
    typeof record.id !== "string" ||
    record.provider !== "github" ||
    typeof record.owner !== "string" ||
    typeof record.name !== "string" ||
    typeof record.fullName !== "string" ||
    typeof record.private !== "boolean" ||
    typeof record.defaultBranch !== "string" ||
    typeof record.htmlUrl !== "string"
  ) {
    return null
  }
  return {
    defaultBranch: record.defaultBranch,
    description: typeof record.description === "string" ? record.description : null,
    fullName: record.fullName,
    htmlUrl: record.htmlUrl,
    id: record.id,
    name: record.name,
    owner: record.owner,
    private: record.private,
    provider: "github",
  }
}

const parseRepositoryListPage = (body: unknown): KhalaMobileReposListPage | null => {
  if (body === null || typeof body !== "object") return null
  const record = body as Record<string, unknown>
  if (
    !Array.isArray(record.repositories) ||
    typeof record.page !== "number" ||
    typeof record.perPage !== "number" ||
    typeof record.hasNextPage !== "boolean"
  ) {
    return null
  }
  const repositories: Array<KhalaMobileRepository> = []
  for (const entry of record.repositories) {
    const parsed = parseRepository(entry)
    if (parsed === null) return null
    repositories.push(parsed)
  }
  return { hasNextPage: record.hasNextPage, page: record.page, perPage: record.perPage, repositories }
}

export const fetchKhalaMobileRepositories = async (
  apiBaseUrl: string,
  token: string,
  input: Readonly<{ page?: number; perPage?: number }>,
  fetchImpl: KhalaMobileReposFetchLike = fetch,
): Promise<KhalaMobileReposResult<KhalaMobileReposListPage>> => {
  const url = new URL("/api/mobile/repos", apiBaseUrl)
  if (input.page !== undefined) url.searchParams.set("page", String(input.page))
  if (input.perPage !== undefined) url.searchParams.set("perPage", String(input.perPage))
  return requestRepos(url.toString(), token, fetchImpl, parseRepositoryListPage)
}

export const fetchKhalaMobileRepository = async (
  apiBaseUrl: string,
  token: string,
  owner: string,
  name: string,
  fetchImpl: KhalaMobileReposFetchLike = fetch,
): Promise<KhalaMobileReposResult<KhalaMobileRepository>> => {
  const url = new URL(
    `/api/mobile/repos/${encodeURIComponent(owner)}/${encodeURIComponent(name)}`,
    apiBaseUrl,
  )
  return requestRepos(url.toString(), token, fetchImpl, body => {
    if (body === null || typeof body !== "object") return null
    return parseRepository((body as Record<string, unknown>).repository)
  })
}
