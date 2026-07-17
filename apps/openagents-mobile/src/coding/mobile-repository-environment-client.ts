import type { MobileComposerPathSearchPort } from "./mobile-composer-path-context"
import type { MobileRepositoryFilesPort } from "./mobile-repository-files"
import type { MobileRepositoryGitPort } from "./mobile-repository-git"
import type { MobileRepositoryReviewPort } from "./mobile-repository-review"
import type { MobileRepositoryTerminalPort } from "./mobile-repository-terminal"

export const MOBILE_REPOSITORY_TREE_ENDPOINT = "/api/mobile/coding/repository/tree"
export const MOBILE_REPOSITORY_READ_ENDPOINT = "/api/mobile/coding/repository/read"
export const MOBILE_REPOSITORY_SEARCH_ENDPOINT = "/api/mobile/coding/repository/search"
export const MOBILE_REPOSITORY_STATUS_ENDPOINT = "/api/mobile/coding/repository/status"
export const MOBILE_REPOSITORY_DIFF_ENDPOINT = "/api/mobile/coding/repository/diff"
export const MOBILE_REPOSITORY_REVIEW_ENDPOINT = "/api/mobile/coding/repository/reviews"
export const MOBILE_REPOSITORY_GIT_STATUS_ENDPOINT = "/api/mobile/coding/repository/git/status"
export const MOBILE_REPOSITORY_GIT_MUTATE_ENDPOINT = "/api/mobile/coding/repository/git/mutate"
export const MOBILE_REPOSITORY_TERMINAL_SNAPSHOT_ENDPOINT = "/api/mobile/coding/repository/terminal/snapshot"
export const MOBILE_REPOSITORY_TERMINAL_CREATE_ENDPOINT = "/api/mobile/coding/repository/terminal/create"
export const MOBILE_REPOSITORY_TERMINAL_REPLAY_ENDPOINT = "/api/mobile/coding/repository/terminal/replay"
export const MOBILE_REPOSITORY_TERMINAL_COMMAND_ENDPOINT = "/api/mobile/coding/repository/terminal/command"

const MAX_METADATA_RESPONSE_BYTES = 256 * 1024
const MAX_CONTENT_RESPONSE_BYTES = 14 * 1024 * 1024

export type MobileRepositoryEnvironmentPort = MobileRepositoryFilesPort & MobileRepositoryReviewPort & MobileRepositoryGitPort & MobileRepositoryTerminalPort & Readonly<{
  search: MobileComposerPathSearchPort["search"]
}>

const exactBaseUrl = (value: string): URL => {
  const url = new URL(value)
  if (url.protocol !== "https:" || url.username !== "" || url.password !== "" ||
    url.search !== "" || url.hash !== "" || (url.pathname !== "" && url.pathname !== "/")) {
    throw new Error("mobile repository environment base URL is invalid")
  }
  return url
}

const exactAccessToken = (value: string): string => {
  if (value.length < 16 || value.length > 8_192 || value.trim() !== value || /[\u0000-\u001f\u007f]/u.test(value)) {
    throw new Error("mobile repository environment access token is invalid")
  }
  return value
}

const readBoundedJson = async (response: Response, maxBytes: number): Promise<unknown> => {
  if (!response.ok || response.status !== 200) throw new Error("mobile repository environment request failed")
  const mediaType = response.headers.get("content-type")?.split(";", 1)[0]?.trim().toLowerCase()
  if (mediaType !== "application/json") throw new Error("mobile repository environment response is not JSON")
  const declared = response.headers.get("content-length")
  if (declared !== null && (!/^\d+$/u.test(declared) || Number(declared) > maxBytes)) {
    throw new Error("mobile repository environment response is oversized")
  }
  const text = await response.text()
  if (new TextEncoder().encode(text).byteLength > maxBytes) {
    throw new Error("mobile repository environment response is oversized")
  }
  return JSON.parse(text) as unknown
}

export const createAuthenticatedMobileRepositoryEnvironment = (input: Readonly<{
  baseUrl: string
  accessToken: string
  fetch?: typeof globalThis.fetch
}>): MobileRepositoryEnvironmentPort => {
  const baseUrl = exactBaseUrl(input.baseUrl)
  const accessToken = exactAccessToken(input.accessToken)
  const fetchImpl = input.fetch ?? globalThis.fetch
  const post = async (path: string, body: unknown, maxBytes: number): Promise<unknown> => {
    const response = await fetchImpl(new URL(path, baseUrl), {
      method: "POST",
      credentials: "omit",
      redirect: "error",
      cache: "no-store",
      headers: {
        accept: "application/json",
        authorization: `Bearer ${accessToken}`,
        "content-type": "application/json",
      },
      body: JSON.stringify(body),
    })
    return readBoundedJson(response, maxBytes)
  }
  return {
    tree: request => post(MOBILE_REPOSITORY_TREE_ENDPOINT, request, MAX_METADATA_RESPONSE_BYTES),
    read: request => post(MOBILE_REPOSITORY_READ_ENDPOINT, request, MAX_CONTENT_RESPONSE_BYTES),
    search: request => post(MOBILE_REPOSITORY_SEARCH_ENDPOINT, request, MAX_METADATA_RESPONSE_BYTES),
    status: request => post(MOBILE_REPOSITORY_STATUS_ENDPOINT, request, MAX_METADATA_RESPONSE_BYTES),
    diff: request => post(MOBILE_REPOSITORY_DIFF_ENDPOINT, request, MAX_CONTENT_RESPONSE_BYTES),
    submitReview: request => post(MOBILE_REPOSITORY_REVIEW_ENDPOINT, request, MAX_METADATA_RESPONSE_BYTES),
    gitStatus: request => post(MOBILE_REPOSITORY_GIT_STATUS_ENDPOINT, request, MAX_METADATA_RESPONSE_BYTES),
    gitMutate: request => post(MOBILE_REPOSITORY_GIT_MUTATE_ENDPOINT, request, MAX_METADATA_RESPONSE_BYTES),
    terminalSnapshot: request => post(MOBILE_REPOSITORY_TERMINAL_SNAPSHOT_ENDPOINT, request, MAX_CONTENT_RESPONSE_BYTES),
    terminalCreate: request => post(MOBILE_REPOSITORY_TERMINAL_CREATE_ENDPOINT, request, MAX_CONTENT_RESPONSE_BYTES),
    terminalReplay: request => post(MOBILE_REPOSITORY_TERMINAL_REPLAY_ENDPOINT, request, MAX_CONTENT_RESPONSE_BYTES),
    terminalCommand: request => post(MOBILE_REPOSITORY_TERMINAL_COMMAND_ENDPOINT, request, MAX_METADATA_RESPONSE_BYTES),
  }
}
