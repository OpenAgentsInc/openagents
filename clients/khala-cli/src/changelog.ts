export interface KhalaChangelogEntry {
  readonly version: string
  readonly date: string
  readonly bullets: ReadonlyArray<string>
}

export const KHALA_CLI_VERSION = "0.1.2"

export const KHALA_CHANGELOG: ReadonlyArray<KhalaChangelogEntry> = [
  {
    version: "0.1.2",
    date: "2026-06-26",
    bullets: [
      "Added /feedback and khala feedback for out-of-band product feedback.",
      "Added /changelog and khala changelog, plus clearer retrying and terminal errors for unavailable inference.",
    ],
  },
  {
    version: "0.1.1",
    date: "2026-06-26",
    bullets: [
      "Replaced the full-screen alternate prompt with a normal scrollback chat transcript.",
      "Removed runtime npm dependencies so global installs avoid unrelated engine warnings.",
    ],
  },
  {
    version: "0.1.0",
    date: "2026-06-26",
    bullets: [
      "Initial Khala command with interactive terminal chat and headless prompt/stdin modes.",
      "Published the OpenAI-compatible Khala client as @openagentsinc/khala.",
    ],
  },
]

export function formatKhalaChangelog(limit = 5): string {
  return KHALA_CHANGELOG.slice(0, limit)
    .map(entry => [
      `v${entry.version} - ${entry.date}`,
      ...entry.bullets.map(bullet => `- ${bullet}`),
    ].join("\n"))
    .join("\n\n")
}

