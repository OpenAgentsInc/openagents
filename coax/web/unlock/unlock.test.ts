import { describe, expect, test } from "bun:test"
import { generateSecretKey, getPublicKey } from "nostr-effect/pure"
import { getConversationKey, decrypt } from "nostr-effect/nip44"
import { DECK_SCHEMA, deckLines, jcs, localOpener, openDeck, parseDeck, sealDeck } from "./lib.ts"
import { makeEntry, match, normalize } from "./gate.ts"
import { parseUnlocks } from "./inject.ts"

const pointer = { relay: "wss://relay.example", id: "a".repeat(64), recipient: "b".repeat(64) }
const ROUNDS = 1000 // the real 210,000 is slow in a test; the format is the same

describe("the phrase gate", () => {
  test("a typed phrase at the end of anything opens its pointer", async () => {
    const e = await makeEntry("Open Sesame!", pointer, ROUNDS)
    expect(JSON.stringify(e)).not.toContain("sesame")
    expect(await match("hello there opensesame", [e], ROUNDS)).toEqual(pointer)
    expect(await match("OPEN-sesame", [e], ROUNDS)).toEqual(pointer)
  })
  test("anything else opens nothing", async () => {
    const e = await makeEntry("opensesame", pointer, ROUNDS)
    expect(await match("opensesam", [e], ROUNDS)).toBeNull()
    expect(await match("opensesamex", [e], ROUNDS)).toBeNull()
    expect(await match("", [e], ROUNDS)).toBeNull()
  })
  test("phrases are letters and digits, 4 to 40", async () => {
    expect(normalize("A b-C 1!")).toBe("abc1")
    await expect(makeEntry("abc", pointer, ROUNDS)).rejects.toThrow()
  })
  test("deploy entries parse, and bad ones refuse", () => {
    expect(parseUnlocks(`# comment\nopensesame=wss://r.example,${"c".repeat(64)},${"d".repeat(64)}`)).toEqual(
      [{ phrase: "opensesame", pointer: { relay: "wss://r.example", id: "c".repeat(64), recipient: "d".repeat(64) } }])
    expect(() => parseUnlocks("opensesame=https://x,1,2")).toThrow()
    expect(parseUnlocks("")).toEqual([])
  })
})

describe("sealed presentations", () => {
  const md = "# Hello\n## Sub\n- one **two**\n- three\n> a note\n---\n# Second\nSome body"
  test("the deck format", () => {
    const d = parseDeck(md)
    expect(d.slides).toEqual([
      { title: "Hello", subtitle: "Sub", bullets: ["one **two**", "three"], notes: "a note" },
      { title: "Second", body: "Some body" },
    ])
    expect(() => parseDeck("no title here")).toThrow()
  })
  test("the lines the page draws a deck from", () => {
    expect(deckLines(parseDeck(md))).toBe("DHello\n=\nTHello\nSSub\nBone **two**\nBthree\nNa note\n=\nTSecond\nPSome body\n")
  })
  test("JCS sorts keys and refuses floats", () => {
    expect(jcs({ b: 1, a: [true, null, "x"] })).toBe('{"a":[true,null,"x"],"b":1}')
    expect(() => jcs({ a: 1.5 })).toThrow()
  })
  test("only the recipient opens it, and it round-trips", async () => {
    const author = generateSecretKey(), me = generateSecretKey(), other = generateSecretKey()
    const deck = parseDeck(md)
    const wrap = sealDeck(deck, author, getPublicKey(me))
    expect(wrap.kind).toBe(1059)
    expect(wrap.tags).toEqual([["p", getPublicKey(me)]])
    expect(wrap.pubkey).not.toBe(getPublicKey(author))
    expect(JSON.stringify(wrap)).not.toContain("Hello")
    const opened = await openDeck(wrap, localOpener(me))
    expect(opened).toEqual({ deck, author: getPublicKey(author) })
    await expect(openDeck(wrap, localOpener(other))).rejects.toThrow()
  })
  test("the layers are spec NIP-44 v2, so a NIP-07 signer opens them too", () => {
    const author = generateSecretKey(), me = generateSecretKey()
    const wrap = sealDeck(parseDeck(md), author, getPublicKey(me))
    const seal = JSON.parse(decrypt(wrap.content, getConversationKey(me, wrap.pubkey)))
    expect(seal.kind).toBe(13)
    const rumor = JSON.parse(decrypt(seal.content, getConversationKey(me, seal.pubkey)))
    expect(rumor.kind).toBe(3188)
    expect(rumor.sig).toBeUndefined()
    const envelope = JSON.parse(decrypt(rumor.content, getConversationKey(me, rumor.pubkey)))
    expect(envelope.v).toBe("openagents.artifact-envelope.v1")
    expect(envelope.artifact.schema).toBe(DECK_SCHEMA)
  })
  test("a tampered wrap refuses", async () => {
    const author = generateSecretKey(), me = generateSecretKey()
    const wrap = sealDeck(parseDeck(md), author, getPublicKey(me))
    await expect(openDeck({ ...wrap, content: wrap.content.slice(0, -4) + "AAAA" }, localOpener(me))).rejects.toThrow()
  })
})
