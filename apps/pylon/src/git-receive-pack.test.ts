import { createHash } from "node:crypto"
import { describe, expect, test } from "bun:test"

import {
  GitReceivePackParseError,
  parseGitReceivePackRequest,
} from "./git-receive-pack.js"

const encoder = new TextEncoder()
const ZERO_40 = "0".repeat(40)
const ZERO_64 = "0".repeat(64)
const A = "a".repeat(40)
const B = "b".repeat(40)
const C = "c".repeat(40)
const SHA256_A = "a".repeat(64)
const SHA256_B = "b".repeat(64)

function concat(parts: readonly Uint8Array[]): Uint8Array {
  const length = parts.reduce((sum, part) => sum + part.length, 0)
  const out = new Uint8Array(length)
  let offset = 0
  for (const part of parts) {
    out.set(part, offset)
    offset += part.length
  }
  return out
}

function text(value: string): Uint8Array {
  return encoder.encode(value)
}

function pktLine(payload: string): Uint8Array {
  const payloadBytes = text(payload)
  const length = (payloadBytes.length + 4).toString(16).padStart(4, "0")
  return concat([text(length), payloadBytes])
}

function receivePackBody(input: {
  commands: readonly string[]
  packfile?: Uint8Array
}): Uint8Array {
  return concat([
    ...input.commands.map(pktLine),
    text("0000"),
    input.packfile ?? new Uint8Array(),
  ])
}

describe("git receive-pack parser", () => {
  test("parses command pkt-lines, first-line capabilities, and raw packfile bytes", () => {
    const packfile = concat([text("PACK"), new Uint8Array([0, 0, 0, 2, 1, 2, 3, 4])])
    const parsed = parseGitReceivePackRequest(
      receivePackBody({
        commands: [
          `${A} ${B} refs/heads/main\0report-status side-band-64k object-format=sha1 agent=openagents-pylon\n`,
          `${ZERO_40} ${C} refs/heads/forge/receive-pack\n`,
        ],
        packfile,
      }),
    )

    expect(parsed.schema).toBe("openagents.pylon.git_receive_pack.v0.1")
    expect(parsed.capabilities).toEqual([
      "report-status",
      "side-band-64k",
      "object-format=sha1",
      "agent=openagents-pylon",
    ])
    expect(parsed.commands).toEqual([
      {
        oldObjectId: A,
        newObjectId: B,
        refName: "refs/heads/main",
        action: "update",
      },
      {
        oldObjectId: ZERO_40,
        newObjectId: C,
        refName: "refs/heads/forge/receive-pack",
        action: "create",
      },
    ])
    expect(parsed.packfileBytes).toBe(packfile.length)
    expect(parsed.packfile).toEqual(packfile)
    expect(parsed.packfileSha256).toBe(
      createHash("sha256").update(packfile).digest("hex"),
    )
    expect(parsed.sourceRefs).toContain(
      "issue.public.github.OpenAgentsInc.openagents.6747",
    )
  })

  test("accepts delete-only pushes without a packfile", () => {
    const parsed = parseGitReceivePackRequest(
      receivePackBody({
        commands: [`${A} ${ZERO_40} refs/heads/stale-agent-branch\n`],
      }),
    )

    expect(parsed.commands).toEqual([
      {
        oldObjectId: A,
        newObjectId: ZERO_40,
        refName: "refs/heads/stale-agent-branch",
        action: "delete",
      },
    ])
    expect(parsed.packfileBytes).toBe(0)
  })

  test("accepts SHA-256 object-format receive-pack commands", () => {
    const packfile = text("PACK0000sha256-fixture")
    const parsed = parseGitReceivePackRequest(
      receivePackBody({
        commands: [
          `${ZERO_64} ${SHA256_A} refs/heads/sha256\0report-status object-format=sha256\n`,
          `${SHA256_A} ${SHA256_B} refs/tags/build-proof\n`,
        ],
        packfile,
      }),
    )

    expect(parsed.capabilities).toEqual(["report-status", "object-format=sha256"])
    expect(parsed.commands.map((command) => command.action)).toEqual([
      "create",
      "update",
    ])
    expect(parsed.commands[0]?.oldObjectId).toHaveLength(64)
    expect(parsed.commands[1]?.refName).toBe("refs/tags/build-proof")
  })

  test("rejects malformed command streams before the supervisor accepts bytes", () => {
    expect(() =>
      parseGitReceivePackRequest(text("0005a")),
    ).toThrow(GitReceivePackParseError)

    expect(() =>
      parseGitReceivePackRequest(
        receivePackBody({
          commands: [`${A} ${B} refs/heads/../escape\n`],
          packfile: text("PACK0000"),
        }),
      ),
    ).toThrow("forbidden sequence")

    expect(() =>
      parseGitReceivePackRequest(
        receivePackBody({
          commands: [`${A} ${B} refs/heads/main\n`],
          packfile: text("not-a-pack"),
        }),
      ),
    ).toThrow("not a PACK")
  })
})
