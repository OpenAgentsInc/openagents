export type ShipStatusRoundtripInput = {
  originClientRef: string
  mode: "ota" | "rebuild"
  state: "queued" | "building" | "published" | "failed"
  version: string | null
  url: string | null
}

export type ShipStatusMessage = {
  clientRef: string
  kind: "ship_status"
  text: string
  terminal: boolean
}

export function buildShipStatusMessage(input: ShipStatusRoundtripInput): ShipStatusMessage {
  const terminal = input.state === "published" || input.state === "failed"

  return {
    clientRef: input.originClientRef,
    kind: "ship_status",
    text: formatShipStatusText(input),
    terminal,
  }
}

function formatShipStatusText(input: ShipStatusRoundtripInput): string {
  const mode = input.mode === "ota" ? "OTA" : "Rebuild"
  const version = input.version === null ? "" : ` ${input.version}`

  if (input.state === "queued") {
    return `${mode}${version} ship queued.`
  }

  if (input.state === "building") {
    return `${mode}${version} ship building.`
  }

  if (input.state === "published") {
    const url = input.url === null ? "" : ` ${input.url}`
    return `${mode}${version} ship published.${url}`
  }

  return `${mode}${version} ship failed.`
}
