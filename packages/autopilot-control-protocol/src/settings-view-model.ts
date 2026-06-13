type SettingsViewInput = {
  baseUrl: string | null
  owner: string | null
  nodeName: string | null
  connected: boolean
  version: string | null
}

type SettingsView = {
  rows: {
    label: string
    value: string
  }[]
}

export function buildSettingsView(input: SettingsViewInput): SettingsView {
  return {
    rows: [
      { label: "Connection", value: readString(input.baseUrl, "Not configured") },
      { label: "Owner", value: readString(input.owner, "Unknown") },
      { label: "Node", value: readString(input.nodeName, "Unknown") },
      { label: "Version", value: readString(input.version, "Unknown") },
      { label: "Status", value: input.connected ? "Connected" : "Disconnected" },
    ],
  }
}

function readString(value: string | null, fallback: string): string {
  if (value === null) return fallback

  const trimmed = value.trim()
  return trimmed === "" ? fallback : trimmed
}
