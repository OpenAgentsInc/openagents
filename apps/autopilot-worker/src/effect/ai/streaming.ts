import type { Response } from "@effect/ai"

export type WirePart = Response.StreamPartEncoded

export function encodeWirePart(part: WirePart): string {
  return JSON.stringify(part)
}

export function decodeWirePart(text: string): WirePart | null {
  try {
    const parsed: unknown = JSON.parse(text)
    if (!parsed || typeof parsed !== "object") return null
    if (!("type" in (parsed as any)) || typeof (parsed as any).type !== "string") return null
    return parsed as WirePart
  } catch {
    return null
  }
}

