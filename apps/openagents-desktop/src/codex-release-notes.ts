const CODEX_RELEASE_API = "https://api.github.com/repos/openai/codex/releases/tags"
const RELEASE_NOTES_TIMEOUT_MS = 8_000
const RELEASE_NOTES_MAX_CHARS = 12_000

export type CodexReleaseNotes = Readonly<{
  version: string
  title: string
  body: string
  url: string
  publishedAt: string | null
}>

type FetchLike = typeof fetch

/**
 * Best-effort official Codex release notes. The registry remains version
 * authority; GitHub is presentation-only and a missing note never blocks an
 * update check or update run.
 */
export const fetchCodexReleaseNotes = async (
  version: string | null,
  options: Readonly<{ fetch?: FetchLike; timeoutMs?: number }> = {},
): Promise<CodexReleaseNotes | null> => {
  if (version === null || !/^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/.test(version)) return null
  const controller = new AbortController()
  const timer = setTimeout(() => controller.abort(), options.timeoutMs ?? RELEASE_NOTES_TIMEOUT_MS)
  try {
    const response = await (options.fetch ?? fetch)(
      `${CODEX_RELEASE_API}/${encodeURIComponent(`rust-v${version}`)}`,
      {
        headers: {
          accept: "application/vnd.github+json",
          "x-github-api-version": "2022-11-28",
        },
        signal: controller.signal,
      },
    )
    if (!response.ok) return null
    const payload = await response.json() as Record<string, unknown>
    const url = typeof payload.html_url === "string" && payload.html_url.startsWith("https://github.com/openai/codex/releases/")
      ? payload.html_url
      : null
    if (url === null) return null
    const body = typeof payload.body === "string" ? payload.body.trim().slice(0, RELEASE_NOTES_MAX_CHARS) : ""
    return {
      version,
      title: typeof payload.name === "string" && payload.name.trim().length > 0
        ? payload.name.trim().slice(0, 160)
        : `Codex ${version}`,
      body: body.length > 0 ? body : "Release notes are not available for this Codex build.",
      url,
      publishedAt: typeof payload.published_at === "string" ? payload.published_at : null,
    }
  } catch {
    return null
  } finally {
    clearTimeout(timer)
  }
}
