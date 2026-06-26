export interface KhalaChangelogEntry {
  readonly version: string
  readonly releasedAt: string
  readonly bullets: ReadonlyArray<string>
}

export const KHALA_CLI_VERSION = "0.1.3"

export const KHALA_CHANGELOG: ReadonlyArray<KhalaChangelogEntry> = [
  {
    version: "0.1.3",
    releasedAt: "2026-06-26T16:50:00.000Z",
    bullets: [
      "Added background npm auto-update checks for interactive sessions.",
      "Added khala tokens and /tokens backed by the public Khala tokens-served counter.",
    ],
  },
  {
    version: "0.1.2",
    releasedAt: "2026-06-26T16:38:47.676Z",
    bullets: [
      "Added /feedback and khala feedback for out-of-band product feedback.",
      "Added /changelog and khala changelog, plus clearer retrying and terminal errors for unavailable inference.",
    ],
  },
  {
    version: "0.1.1",
    releasedAt: "2026-06-26T16:12:03.491Z",
    bullets: [
      "Replaced the full-screen alternate prompt with a normal scrollback chat transcript.",
      "Removed runtime npm dependencies so global installs avoid unrelated engine warnings.",
    ],
  },
  {
    version: "0.1.0",
    releasedAt: "2026-06-26T16:02:59.786Z",
    bullets: [
      "Initial Khala command with interactive terminal chat and headless prompt/stdin modes.",
      "Published the OpenAI-compatible Khala client as @openagentsinc/khala.",
    ],
  },
]

export function formatKhalaChangelog(
  limit = 5,
  options: { readonly timeZone?: string | undefined } = {},
): string {
  const timeZone = options.timeZone ?? localTimeZone()
  return KHALA_CHANGELOG.slice(0, limit)
    .map(entry => [
      `v${entry.version} - ${formatReleaseTimestamp(entry.releasedAt, timeZone)}`,
      ...entry.bullets.map(bullet => `- ${bullet}`),
    ].join("\n"))
    .join("\n\n")
}

export function changelogTeaser(version: string, readme: string | undefined): string | undefined {
  if (readme === undefined) return undefined
  const escaped = version.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")
  const match = readme.match(new RegExp(`### v${escaped}[^\\n]*\\n+([\\s\\S]*?)(?:\\n### v|\\n## |$)`))
  const body = match?.[1]
  if (body === undefined) return undefined
  const firstBullet = body
    .split("\n")
    .map(line => line.trim())
    .find(line => line.startsWith("- "))
  return firstBullet?.slice(2).trim()
}

export function firstWords(input: string, count = 10): string {
  return input.replace(/\s+/g, " ").trim().split(" ").slice(0, count).join(" ")
}

export function formatReleaseTimestamp(iso: string, timeZone = localTimeZone()): string {
  const parts = new Intl.DateTimeFormat("en-US", {
    day: "2-digit",
    hour: "numeric",
    hour12: true,
    minute: "2-digit",
    month: "short",
    second: "2-digit",
    timeZone,
    timeZoneName: "short",
    year: "numeric",
  }).formatToParts(new Date(iso))
  const part = (type: Intl.DateTimeFormatPartTypes) => parts.find(item => item.type === type)?.value ?? ""
  return `${part("month")} ${part("day")}, ${part("year")}, ${part("hour")}:${part("minute")}:${part("second")} ${part("dayPeriod")} ${part("timeZoneName")}`
}

function localTimeZone(): string {
  return Intl.DateTimeFormat().resolvedOptions().timeZone || "America/Chicago"
}
