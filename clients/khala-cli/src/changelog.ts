import { readFileSync } from "node:fs"

export interface KhalaChangelogEntry {
  readonly version: string
  readonly releasedAt: string
  readonly bullets: ReadonlyArray<string>
}

// The CLI version is the single source of truth read from package.json — the
// actually deployed/installed package — never a hardcoded constant that drifts.
// This keeps the startup banner, `khala --version`, and the auto-updater's
// version comparison honest about what is really installed. Resolves relative to
// this module: both the `src/` dev entry and the bundled `dist/` build sit one
// level under the package root, so `../package.json` is the package root in both.
const packageJson = JSON.parse(
  readFileSync(new URL("../package.json", import.meta.url), "utf8"),
) as { readonly version: string }

export const KHALA_CLI_VERSION = packageJson.version

export const KHALA_CHANGELOG: ReadonlyArray<KhalaChangelogEntry> = [
  {
    version: "0.1.21",
    releasedAt: "2026-06-27T16:52:00.000Z",
    bullets: [
      "Adds `khala fleet run` — a turnkey Pylon/Codex backlog supervisor for your own repo and issues, with auto-resolved Pylon targeting, account-scaled slots, `--dry-run`, and one-round `--once` mode.",
    ],
  },
  {
    version: "0.1.20",
    releasedAt: "2026-06-27T15:43:41.000Z",
    bullets: [
      "Adds `khala fleet connect` — connect your own Codex account to Khala with one short command and a paste-free device login (browser + short code, no long auth strings).",
      "Adds `khala fleet status` — see your connected Codex fleet and readiness; run `khala fleet connect` again to add more accounts (auto-assigned codex, codex-2, …) for more throughput.",
      "Adds `khala fleet status --live` — poll the owner-only operator fleet status endpoint and render Pace, Fleet, Watchdog, GLM, and Brain/Artanis blocks as a terminal dashboard.",
      "Accounts use isolated per-account homes and are registered into your Pylon config; the flow never touches `~/.codex` and never prints tokens.",
    ],
  },
  {
    version: "0.1.19",
    releasedAt: "2026-06-27T13:20:06.000Z",
    bullets: [
      "Runs on plain Node now (not just Bun), so `npm install -g @openagentsinc/khala` and `khala` just work for anyone.",
      "Adds bring-your-own-key: `khala key add <provider> <api-key>` (and /key) stores a provider key locally and sends it with your chats; friendlier first-run welcome, /help, and network/auth/quota error messages.",
    ],
  },
  {
    version: "0.1.18",
    releasedAt: "2026-06-27T13:05:24.000Z",
    bullets: [
      "`khala info` no longer prints raw agent tokens or token-bearing trace URLs; it reports configured trace access with the token redacted.",
      "`khala info` no longer mints a new trace token just to show diagnostics.",
      "`khala --api` and `khala spawn --strategy pylon` now honor the stored `khala login` token when no `--token` flag or `OPENAGENTS_AGENT_TOKEN` is provided.",
    ],
  },
  {
    version: "0.1.17",
    releasedAt: "2026-06-27T12:42:24.000Z",
    bullets: [
      "Adds supervised Khala spawn workers for local Codex-backed subagent fanout, with status polling and join support.",
      "Routes natural-language spawn requests to the typed `spawn_khala` tool and bridges `--strategy pylon` to caller-owned Pylon coding capacity.",
    ],
  },
  {
    version: "0.1.16",
    releasedAt: "2026-06-27T04:43:28.000Z",
    bullets: [
      "Login now identifies you by your account email and no longer prints a display name as your identity, so it never conflates you (the signed-in user) with Artanis (the operator agent you talk to).",
      "Clarifies after sign-in that Artanis is the operator agent you reach with /artanis.",
    ],
  },
  {
    version: "0.1.15",
    releasedAt: "2026-06-27T04:00:14.000Z",
    bullets: [
      "Adds `khala login` (and the interactive /login command) using the standard OpenAgents device-auth flow, so you can sign in as the owner and then talk to Artanis.",
      "Adds `khala logout` / /logout to clear the stored Khala token, and points the Artanis owner-only and --api messages at `khala login`.",
    ],
  },
  {
    version: "0.1.14",
    releasedAt: "2026-06-27T03:26:46.000Z",
    bullets: [
      "Adds the /artanis owner-only operator channel (talk to the real Artanis operator agent, powered by Khala).",
      "Reads the CLI version from package.json instead of a hardcoded constant, so the banner and auto-updater always match what is actually installed.",
    ],
  },
  {
    version: "0.1.12",
    releasedAt: "2026-06-26T19:45:49.000Z",
    bullets: [
      "Adds one-dot-per-second waiting feedback before the first Khala stream output.",
      "Restores Ctrl-L screen clearing and shows first-byte, first-token, stream, and total latency in /msginfo.",
    ],
  },
  {
    version: "0.1.11",
    releasedAt: "2026-06-26T19:01:29.000Z",
    bullets: [
      "Adds Blueprint-selected local Codex delegation for workspace, filesystem, shell, git, and code tasks.",
      "Adds khala auth codex, khala codex, and /codex commands with Pylon Codex account reuse.",
    ],
  },
  {
    version: "0.1.10",
    releasedAt: "2026-06-26T18:37:50.000Z",
    bullets: [
      "Fixes streamed Markdown rendering when bold spans are split across SSE chunks.",
      "Records served tokens from the default public Khala chat path so /tokens moves after successful turns.",
    ],
  },
  {
    version: "0.1.9",
    releasedAt: "2026-06-26T18:30:01.000Z",
    bullets: [
      "Adds /info and khala info with a CLI thread id plus owner-token trace viewing link.",
      "Rewords /msginfo around Khala as the orchestrator and backend models/adapters as routing details.",
    ],
  },
  {
    version: "0.1.8",
    releasedAt: "2026-06-26T18:04:35.000Z",
    bullets: [
      "Adds Up/Down prompt history, switches the interactive prompt to >, and keeps provider reasoning in a separate dim stream.",
      "Adds the Blueprint response-discipline contract so Khala answers land as one coherent final answer instead of visible revision loops.",
    ],
  },
  {
    version: "0.1.7",
    releasedAt: "2026-06-26T17:36:47.000Z",
    bullets: [
      "Shows the installed Khala CLI version in the interactive startup banner.",
      "Makes /tokens and khala tokens read the live ledger total without a stale isolate cache.",
    ],
  },
  {
    version: "0.1.6",
    releasedAt: "2026-06-26T17:24:07.000Z",
    bullets: [
      "Restored live streaming in interactive chat while keeping Markdown color rendering.",
      "Removed the timestamp from khala tokens and aligned Khala fallback docs with GLM -> OpenRouter -> Gemini -> Fireworks.",
    ],
  },
  {
    version: "0.1.5",
    releasedAt: "2026-06-26T17:06:14.000Z",
    bullets: [
      "Corrected the bundled v0.1.4 release timestamp after npm publish verification.",
      "Kept the v0.1.4 diagnostics, Markdown, help, version, and msginfo changes as the active CLI.",
    ],
  },
  {
    version: "0.1.4",
    releasedAt: "2026-06-26T17:05:31.009Z",
    bullets: [
      "Added /help, /version, /msginfo, Markdown rendering, colors, and faded metadata.",
      "Added backend trace reporting, public stream metadata, exact feedback lookup, and longer exponential retries.",
    ],
  },
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
