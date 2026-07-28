import { readFileSync } from "node:fs"
import { resolve } from "node:path"

import { Schema as S } from "effect"
import { verifyEvent } from "nostr-effect/pure"
import { describe, expect, it } from "vitest"

import {
  PUBLIC_CHAT_BEHAVIOR_MATRIX,
  PUBLIC_CHAT_IMPLEMENTATION_SEAMS,
  PublicChatParityFixture,
  projectPublicChatTimeline,
  transitionPublicChatMedia,
  verifyPublicChatMedia,
} from "./parity.js"

const fixture = S.decodeUnknownSync(PublicChatParityFixture)(
  JSON.parse(
    readFileSync(
      resolve(import.meta.dirname, "../fixtures/agent-chat-parity.v1.json"),
      "utf8",
    ),
  ),
)

describe("Agent Chat parity contract", () => {
  it("records every web relay state and its desktop meaning", () => {
    expect(fixture.source).toEqual({
      groupId: "openagents-public",
      manifestPath: "/api/public/nostr-chat/manifest",
      messageKinds: [9, 1337],
      relayUrl: "wss://relay.openagents.com",
      route: "/agentchat",
      transport: "nip01-websocket-with-nip29-h-filter",
    })
    expect(fixture.behaviorMatrix).toEqual(PUBLIC_CHAT_BEHAVIOR_MATRIX)
    expect(fixture.behaviorMatrix.map(({ state }) => state)).toEqual([
      "disconnected",
      "connecting",
      "replaying",
      "current",
      "reconnecting",
      "stale",
    ])
  })

  it("records the exact TypeScript reuse, Rust, bridge, and risk seams", () => {
    expect(PUBLIC_CHAT_IMPLEMENTATION_SEAMS.sharedTypeScript).toContain(
      "projectPublicChatTimeline",
    )
    expect(PUBLIC_CHAT_IMPLEMENTATION_SEAMS.rustEquivalent).toHaveLength(3)
    expect(
      PUBLIC_CHAT_IMPLEMENTATION_SEAMS.bridge.requiredForReadOnlyOmega,
    ).toBe(false)
    expect(
      PUBLIC_CHAT_IMPLEMENTATION_SEAMS.currentWebRisks.map(
        ({ behavior }) => behavior,
      ),
    ).toEqual(
      expect.arrayContaining([
        expect.stringContaining("unions all retained kind 39001"),
        expect.stringContaining("starts once with a null manifest"),
        expect.stringContaining("includes every retained event kind"),
      ]),
    )
  })

  it("projects the shared signed corpus without arrival-order or duplicate drift", () => {
    expect(
      fixture.projection.events.every((event) =>
        verifyEvent({
          ...event,
          tags: event.tags.map((tag) => [...tag]),
        }),
      ),
    ).toBe(true)
    const projected = projectPublicChatTimeline(fixture.projection.events)
    expect(
      projected.map(
        ({
          attachments,
          contentParts,
          contentWarning,
          deletion,
          event,
          pinned,
          profile,
          reactions,
        }) => ({
          attachments,
          contentParts,
          contentWarning,
          deletion,
          eventId: event.id,
          kind: event.kind,
          pinned,
          profile,
          reactions,
        }),
      ),
    ).toEqual(fixture.projection.expectedTimeline)

    const unsafeParts = projected
      .flatMap(({ contentParts }) => contentParts)
      .filter(({ value }) => value.includes("javascript:"))
    expect(unsafeParts).toEqual([
      { type: "text", value: " javascript:alert(1)" },
    ])
    expect(projected.filter(({ event }) => event.kind === 9)).toHaveLength(2)
    expect(projected.filter(({ event }) => event.kind === 1337)).toHaveLength(1)
  })

  it("uses the selected group for author deletion checks", () => {
    const projected = projectPublicChatTimeline(
      fixture.projection.events,
      "another-group",
    )
    expect(projected[0]?.deletion).toBeNull()
  })

  it("covers every media state and keeps the signed message in the timeline", () => {
    const projected = projectPublicChatTimeline(fixture.projection.events)
    const mediaItem = projected.find(
      ({ attachments }) => attachments.length > 0,
    )
    expect(mediaItem).toBeDefined()

    const seen = new Set<string>()
    for (const mediaCase of fixture.media.cases) {
      let state = mediaCase.initialState
      seen.add(state)
      for (const step of mediaCase.steps) {
        state = transitionPublicChatMedia(state, step.action)
        expect(state).toBe(step.expectedState)
        seen.add(state)
      }
      expect(mediaCase.expectedMessageVisible).toBe(true)
      expect(
        projected.some(({ event }) => event.id === mediaItem?.event.id),
      ).toBe(true)
    }
    expect(seen).toEqual(
      new Set(["gated", "loading", "verified", "mismatch", "unavailable"]),
    )
  })

  it("matches the shared SHA-256 vector and rejects a different digest", async () => {
    const bytes = Uint8Array.from(
      Buffer.from(fixture.media.payloadBase64, "base64"),
    ).buffer
    const attachment = fixture.projection.expectedTimeline
      .flatMap(({ attachments }) => attachments)
      .at(0)
    expect(attachment).toBeDefined()
    if (attachment === undefined) return

    await expect(verifyPublicChatMedia(attachment, bytes)).resolves.toBe(
      "verified",
    )
    await expect(
      verifyPublicChatMedia({ ...attachment, digest: "0".repeat(64) }, bytes),
    ).resolves.toBe("mismatch")
  })
})
