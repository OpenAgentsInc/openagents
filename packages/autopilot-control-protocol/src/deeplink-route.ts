import { parseAutopilotDeepLink } from "./deeplink-parse.js"

export type DeepLinkRouteScreen = "Nodes" | "SessionDetail" | "Settings" | null

export type DeepLinkRoute = {
  screen: DeepLinkRouteScreen
  params: Record<string, string>
}

export function resolveDeepLinkRoute(url: unknown): DeepLinkRoute {
  const parsed = parseAutopilotDeepLink(url)

  if (parsed.kind === "session" && parsed.sessionRef !== null) {
    return {
      screen: "SessionDetail",
      params: { sessionRef: parsed.sessionRef },
    }
  }

  if (parsed.kind === "node" && parsed.nodeRef !== null) {
    return {
      screen: "Nodes",
      params: { nodeRef: parsed.nodeRef },
    }
  }

  if (parsed.kind === "ship" && parsed.sessionRef !== null) {
    return {
      screen: "Nodes",
      params: { sessionRef: parsed.sessionRef },
    }
  }

  return {
    screen: null,
    params: {},
  }
}
