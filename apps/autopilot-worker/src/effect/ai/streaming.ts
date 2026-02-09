import type { Response } from "@effect/ai"

export type WirePart = Response.StreamPartEncoded

export function encodeWirePart(part: WirePart): string {
  return JSON.stringify(part)
}

export function decodeWirePart(text: string): WirePart | null {
  try {
    const parsed: unknown = JSON.parse(text)
    if (!parsed || typeof parsed !== "object") return null
    const part = parsed as { type?: unknown };
    if (!("type" in part) || typeof part.type !== "string") return null;
    return parsed as WirePart
  } catch {
    return null
  }
}
