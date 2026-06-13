export type ConnectionStatus = "discovering" | "connecting" | "connected" | "error" | "offline"

export type ConnectionStatusTone = "ok" | "warn" | "error" | "pending"

export type ConnectionStatusViewInput = {
  status: ConnectionStatus
  nodeName: string | null
  baseUrl: string | null
}

export type ConnectionStatusView = {
  label: string
  tone: ConnectionStatusTone
  detail: string
}

export function connectionStatusView(input: ConnectionStatusViewInput): ConnectionStatusView {
  const target = connectionTarget(input)

  switch (input.status) {
    case "discovering":
      return {
        label: "Discovering connection",
        tone: "pending",
        detail: target === null ? "Looking for an Autopilot node" : `Looking for ${target}`,
      }

    case "connecting":
      return {
        label: "Connecting",
        tone: "pending",
        detail: target === null ? "Opening Autopilot connection" : `Opening ${target}`,
      }

    case "connected":
      return {
        label: "Connected",
        tone: "ok",
        detail: target === null ? "Autopilot node connected" : `Connected to ${target}`,
      }

    case "error":
      return {
        label: "Connection error",
        tone: "error",
        detail: target === null ? "Autopilot connection failed" : `Could not reach ${target}`,
      }

    case "offline":
      return {
        label: "Offline",
        tone: "warn",
        detail: target === null ? "Autopilot node unavailable" : `${target} unavailable`,
      }
  }
}

function connectionTarget(input: ConnectionStatusViewInput): string | null {
  const nodeName = cleanText(input.nodeName)
  if (nodeName !== null) return nodeName

  return cleanBaseUrl(input.baseUrl)
}

function cleanText(value: string | null): string | null {
  if (value === null) return null

  const trimmed = value.trim()
  return trimmed.length === 0 ? null : trimmed
}

function cleanBaseUrl(value: string | null): string | null {
  const trimmed = cleanText(value)
  if (trimmed === null) return null

  try {
    const url = new URL(trimmed)
    url.username = ""
    url.password = ""
    url.search = ""
    url.hash = ""
    return url.toString().replace(/\/$/, "")
  } catch {
    return trimmed.split(/[?#]/, 1)[0] || null
  }
}
