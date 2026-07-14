export type KhalaCodeDeepLinkView =
  | "chat"
  | "editor"
  | "fleet"
  | "forum"
  | "home"
  | "inbox"
  | "review"
  | "settings"

export type KhalaCodeDeepLinkTarget =
  | Readonly<{ kind: "view"; view: KhalaCodeDeepLinkView }>
  | Readonly<{ kind: "thread"; threadId: string }>
  | Readonly<{ kind: "project"; projectId: string }>
  | Readonly<{ kind: "server"; serverId: string }>

export type KhalaCodeDeepLinkResult =
  | Readonly<{ ok: true; target: KhalaCodeDeepLinkTarget; url: string }>
  | Readonly<{ error: string; ok: false; url: string }>

const allowedViews: ReadonlySet<string> = new Set([
  "chat",
  "editor",
  "fleet",
  "forum",
  "home",
  "inbox",
  "review",
  "settings",
])

const safeIdPattern = /^[A-Za-z0-9][A-Za-z0-9._:-]{0,127}$/u

const safeId = (value: string | null, label: string): string | { readonly error: string } => {
  if (value === null || value.trim().length === 0) return { error: `Missing ${label}.` }
  const decoded = decodeURIComponent(value).trim()
  if (!safeIdPattern.test(decoded)) return { error: `Invalid ${label}.` }
  return decoded
}

const routeParts = (url: URL): readonly string[] =>
  [url.hostname, ...url.pathname.split("/")].filter(Boolean)

export const parseKhalaCodeDeepLink = (rawUrl: string): KhalaCodeDeepLinkResult => {
  let url: URL
  try {
    url = new URL(rawUrl)
  } catch {
    return { error: "Invalid URL.", ok: false, url: rawUrl }
  }
  if (url.protocol !== "khala-code:") {
    return { error: "Unsupported deep-link scheme.", ok: false, url: rawUrl }
  }

  const [kind, rawValue] = routeParts(url)
  const view = url.searchParams.get("view")
  const thread = url.searchParams.get("thread") ?? url.searchParams.get("threadId")
  const project = url.searchParams.get("project") ?? url.searchParams.get("projectId")
  const server = url.searchParams.get("server") ?? url.searchParams.get("serverId")

  if (kind === "view" || (view !== null && thread === null && project === null && server === null)) {
    const candidate = rawValue ?? view
    if (candidate !== null && allowedViews.has(candidate)) {
      return { ok: true, target: { kind: "view", view: candidate as KhalaCodeDeepLinkView }, url: rawUrl }
    }
    return { error: "Unsupported view target.", ok: false, url: rawUrl }
  }

  if (kind === "thread" || kind === "session" || thread !== null) {
    const value = safeId(rawValue ?? thread, "thread id")
    if (typeof value !== "string") return { error: value.error, ok: false, url: rawUrl }
    return { ok: true, target: { kind: "thread", threadId: value }, url: rawUrl }
  }

  if (kind === "project" || project !== null) {
    const value = safeId(rawValue ?? project, "project id")
    if (typeof value !== "string") return { error: value.error, ok: false, url: rawUrl }
    return { ok: true, target: { kind: "project", projectId: value }, url: rawUrl }
  }

  if (kind === "server" || server !== null) {
    const value = safeId(rawValue ?? server, "server id")
    if (typeof value !== "string") return { error: value.error, ok: false, url: rawUrl }
    return { ok: true, target: { kind: "server", serverId: value }, url: rawUrl }
  }

  if (kind !== undefined && allowedViews.has(kind)) {
    return { ok: true, target: { kind: "view", view: kind as KhalaCodeDeepLinkView }, url: rawUrl }
  }

  return { error: "Unsupported deep-link target.", ok: false, url: rawUrl }
}

export const khalaCodeDeepLinkFromLocation = (
  location: Pick<Location, "hash" | "href" | "protocol" | "search">,
): KhalaCodeDeepLinkResult | null => {
  if (location.protocol === "khala-code:") return parseKhalaCodeDeepLink(location.href)
  const searchParams = new URLSearchParams(location.search)
  const hashParams = new URLSearchParams(location.hash.replace(/^#/, ""))
  const rawUrl = searchParams.get("khala-code-url") ?? hashParams.get("khala-code-url")
  return rawUrl === null ? null : parseKhalaCodeDeepLink(rawUrl)
}

export const viewForKhalaCodeDeepLinkTarget = (
  target: KhalaCodeDeepLinkTarget,
): KhalaCodeDeepLinkView => {
  switch (target.kind) {
    case "view":
      return target.view
    case "project":
      return "home"
    case "server":
      return "settings"
    case "thread":
      return "chat"
  }
}
