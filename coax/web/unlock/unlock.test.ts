import { afterEach, beforeAll, describe, expect, test } from "bun:test"
import { finalizeEvent, generateSecretKey, getPublicKey, verifyEvent, type Event } from "nostr-effect/pure"
import { getConversationKey, decrypt, encrypt } from "nostr-effect/nip44"
import { sha256 } from "@noble/hashes/sha256"
import { mkdtempSync, readFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { DECK_SCHEMA, coaxNostr, currentNostr, deckLines, effectNostr, jcs, localOpener, openDeck, parseDeck, sealDeck, useNostr,
  type Nostr } from "./lib.ts"
import { makeEntry, match, normalize } from "./gate.ts"
import { parseUnlocks, signerChoice } from "./inject.ts"

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
  test("the opener's signer switch: coax needs its module; unset or nostr-effect ships none", () => {
    expect(signerChoice(undefined, "/x.wasm")).toBeNull()
    expect(signerChoice("", undefined)).toBeNull()
    expect(signerChoice("nostr-effect", "/x.wasm")).toBeNull()
    expect(signerChoice("coax", "/x.wasm")).toBe("/x.wasm")
    expect(() => signerChoice("coax", undefined)).toThrow()
    expect(() => signerChoice("libsecp", "/x.wasm")).toThrow()
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

// The opener on Coax (unlock-nostr.coax as Wasm): UNLOCK_NOSTR_WASM names a
// built module, else this builds one with coax/web/wasm.sh (about a minute
// cold, seconds from its cache). Test keys only: 31 zero bytes and k.
const testKey = (k: number) => { const s = new Uint8Array(32); s[31] = k; return s }
describe("the opener on Coax", () => {
  let coax: Nostr
  beforeAll(async () => {
    let wasm = process.env.UNLOCK_NOSTR_WASM
    if (!wasm) {
      wasm = join(mkdtempSync(join(tmpdir(), "unlock-nostr-")), "unlock-nostr.wasm")
      const built = Bun.spawnSync([join(import.meta.dir, "../wasm.sh"), join(import.meta.dir, "unlock-nostr.coax"),
        join(import.meta.dir, "unlock-nostr-face.c"), wasm], { stderr: "inherit" })
      if (built.exitCode !== 0) throw new Error("wasm.sh failed")
    }
    coax = await coaxNostr(await Bun.file(wasm).arrayBuffer())
  }, 600_000)
  afterEach(() => { useNostr(null) })
  const md = "# Hello\n- one\n---\n# Two"

  test("the switch: nostr-effect unless Coax is installed, and null rolls back", () => {
    expect(currentNostr()).toBe(effectNostr)
    expect(useNostr(coax).name).toBe("coax")
    expect(useNostr(null)).toBe(effectNostr)
  })
  test("the same public keys as nostr-effect", () => {
    for (const k of [1, 2, 3, 200]) expect(coax.getPublicKey(testKey(k))).toBe(getPublicKey(testKey(k)))
    expect(() => coax.getPublicKey(new Uint8Array(32))).toThrow()
    expect(() => coax.getPublicKey(new Uint8Array(31))).toThrow()
  })
  test("an AUTH signed in Coax verifies with nostr-effect, and each verifies the other's", () => {
    const auth = { kind: 22242, created_at: 1790000000, content: "", tags: [["relay", "wss://relay.example"], ["challenge", "c-1"]] }
    const signed = coax.finalizeEvent(auth, testKey(3))
    expect(signed.pubkey).toBe(getPublicKey(testKey(3)))
    expect(verifyEvent({ ...signed })).toBe(true)
    expect(coax.verifyEvent(signed)).toBe(true)
    expect(coax.verifyEvent(finalizeEvent({ ...auth, content: "ünïcode \"q\" \\ \n" }, testKey(4)))).toBe(true)
    expect(coax.idMatches(signed)).toBe(true)
  })
  test("a tampered event is refused: content, id, signature, another key's signature", () => {
    const e = coax.finalizeEvent({ kind: 13, created_at: 1790000001, content: "sealed", tags: [] }, testKey(5))
    const other = coax.finalizeEvent({ kind: 13, created_at: 1790000001, content: "sealed", tags: [] }, testKey(6))
    const flip = (s: string) => (s[0] === "0" ? "1" : "0") + s.slice(1)
    expect(coax.verifyEvent({ ...e, content: "sealeD" })).toBe(false)
    expect(coax.verifyEvent({ ...e, id: flip(e.id) })).toBe(false)
    expect(coax.verifyEvent({ ...e, sig: flip(e.sig) })).toBe(false)
    expect(coax.verifyEvent({ ...e, sig: other.sig })).toBe(false)
    expect(coax.verifyEvent({ ...e, pubkey: other.pubkey })).toBe(false)
    expect(coax.idMatches({ ...e, content: "sealeD" })).toBe(false)
    expect(coax.idMatches({ ...e, sig: flip(e.sig) })).toBe(true)   // a rumor's id doesn't cover a signature
  })
  test("a deck opens with the Coax opener, and every verdict on the way is Coax's", async () => {
    const author = testKey(7), me = testKey(8)
    const deck = parseDeck(md)
    const wrap = sealDeck(deck, author, getPublicKey(me))
    const calls: string[] = []
    useNostr({ ...coax, verifyEvent: (e) => (calls.push(`verify ${e.kind}`), coax.verifyEvent(e)),
      idMatches: (e) => (calls.push(`id ${e.kind}`), coax.idMatches(e)) })
    const opener = localOpener(me)
    expect(opener.pubkey).toBe(getPublicKey(me))
    expect(await openDeck(wrap, opener)).toEqual({ deck, author: getPublicKey(author) })
    expect(calls).toEqual(["verify 1059", "verify 13", "id 3188"])
    expect((await opener.sign({ kind: 22242, created_at: 1, content: "", tags: [] })).pubkey).toBe(getPublicKey(me))
  })
  test("a tampered wrap, seal or artifact is refused by the Coax opener", async () => {
    const author = testKey(9), me = testKey(10), once = testKey(11)
    useNostr(coax)
    const opener = localOpener(me)
    const wrap = sealDeck(parseDeck(md), author, getPublicKey(me))
    // the wrap: a changed content byte (its id no longer matches)
    await expect(openDeck({ ...wrap, content: wrap.content.slice(0, -4) + "AAAA" }, opener)).rejects.toThrow("not a valid gift wrap")
    // the seal: rewrapped (a valid wrap) around a seal with one signature digit changed
    const seal = JSON.parse(decrypt(wrap.content, getConversationKey(me, wrap.pubkey))) as Event
    const rewrap = (inner: object) => finalizeEvent({ kind: 1059, created_at: wrap.created_at, tags: [["p", getPublicKey(me)]],
      content: encrypt(JSON.stringify(inner), getConversationKey(once, getPublicKey(me))) }, once)
    await expect(openDeck(rewrap({ ...seal, sig: (seal.sig[0] === "0" ? "1" : "0") + seal.sig.slice(1) }), opener)).rejects.toThrow("not a valid seal")
    // the artifact: resealed (a valid seal) around a rumor whose content changed under its id
    const rumor = JSON.parse(decrypt(seal.content, getConversationKey(me, seal.pubkey)))
    const reseal = finalizeEvent({ kind: 13, created_at: seal.created_at, tags: [],
      content: encrypt(JSON.stringify({ ...rumor, content: rumor.content.slice(0, -4) + "AAAA" }), getConversationKey(author, getPublicKey(me))) }, author)
    await expect(openDeck(rewrap(reseal), opener)).rejects.toThrow("the artifact's id doesn't match it")
    // untouched, the same layers open
    expect((await openDeck(rewrap(seal), opener)).author).toBe(getPublicKey(author))
  })
})

// NIP-44 v2 on Coax: the upstream vectors (paulmillr/nip44, pinned in
// coax/tests/vectors/upstream/nip44), round trips with nostr-effect both ways,
// and the gift wrap and seal made and opened through the Coax module.
// Test keys only.
const vectors = JSON.parse(readFileSync(join(import.meta.dir, "../../tests/vectors/upstream/nip44/nip44.vectors.json"), "utf8")).v2
const bytes = (h: string) => Uint8Array.from(Buffer.from(h, "hex"))
const sha = (s: string) => Buffer.from(sha256(new TextEncoder().encode(s))).toString("hex")
describe("NIP-44 on Coax", () => {
  let coax: Nostr
  beforeAll(async () => {
    let wasm = process.env.UNLOCK_NOSTR_WASM
    if (!wasm) {
      wasm = join(mkdtempSync(join(tmpdir(), "unlock-nostr-")), "unlock-nostr.wasm")
      const built = Bun.spawnSync([join(import.meta.dir, "../wasm.sh"), join(import.meta.dir, "unlock-nostr.coax"),
        join(import.meta.dir, "unlock-nostr-face.c"), wasm], { stderr: "inherit" })
      if (built.exitCode !== 0) throw new Error("wasm.sh failed")
    }
    coax = await coaxNostr(await Bun.file(wasm).arrayBuffer())
  }, 600_000)
  afterEach(() => { useNostr(null) })
  const md = "# Hello\n- one\n---\n# Two"
  const flip = (s: string, at: number) => s.slice(0, at) + (s[at] === "A" ? "B" : "A") + s.slice(at + 1)

  test("the conversation keys: 35 valid rows equal; 8 invalid keys refused, as nostr-effect refuses them", () => {
    expect(vectors.valid.get_conversation_key.length).toBe(35)
    for (const r of vectors.valid.get_conversation_key)
      expect(Buffer.from(coax.conversationKey(bytes(r.sec1), r.pub2)).toString("hex")).toBe(r.conversation_key)
    expect(vectors.invalid.get_conversation_key.length).toBe(8)
    for (const r of vectors.invalid.get_conversation_key) {
      expect(() => coax.conversationKey(bytes(r.sec1), r.pub2)).toThrow("NIP-44 refused the keys")
      expect(() => getConversationKey(bytes(r.sec1), r.pub2)).toThrow()
    }
  })
  test("the payloads: 10 rows and 3 long ones, byte for byte; nostr-effect opens each", () => {
    expect(vectors.valid.encrypt_decrypt.length).toBe(10)
    for (const r of vectors.valid.encrypt_decrypt) {
      const ck = bytes(r.conversation_key)
      expect(Buffer.from(coax.conversationKey(bytes(r.sec1), getPublicKey(bytes(r.sec2)))).toString("hex")).toBe(r.conversation_key)
      expect(coax.encrypt(r.plaintext, ck, bytes(r.nonce))).toBe(r.payload)
      expect(coax.decrypt(r.payload, ck)).toBe(r.plaintext)
      expect(decrypt(coax.encrypt(r.plaintext, ck), ck)).toBe(r.plaintext)
    }
    expect(vectors.valid.encrypt_decrypt_long_msg.length).toBe(3)
    for (const r of vectors.valid.encrypt_decrypt_long_msg) {
      const plaintext = r.pattern.repeat(r.repeat), ck = bytes(r.conversation_key)
      expect(sha(plaintext)).toBe(r.plaintext_sha256)
      const payload = coax.encrypt(plaintext, ck, bytes(r.nonce))
      expect(sha(payload)).toBe(r.payload_sha256)
      expect(coax.decrypt(payload, ck)).toBe(plaintext)
    }
  })
  test("the refusals: 12 invalid payloads and 4 invalid lengths, refused by both", () => {
    const why: string[] = []
    for (const r of vectors.invalid.decrypt) {
      const ck = bytes(r.conversation_key)
      expect(() => decrypt(r.payload, ck)).toThrow()
      try { coax.decrypt(r.payload, ck); why.push("opened") } catch (e) { why.push((e as Error).message.replace("NIP-44 refused the payload: ", "")) }
    }
    expect(why).toEqual(["version", "version", "base64", "MAC", "MAC", "padding", "padding", "padding", "payload", "payload", "payload", "payload"])
    const ck = bytes(vectors.valid.encrypt_decrypt[0].conversation_key)
    expect(vectors.invalid.encrypt_msg_lengths).toEqual([0, 65536, 100000, 10000000])
    for (const n of vectors.invalid.encrypt_msg_lengths) {
      expect(() => coax.encrypt("x".repeat(n), ck)).toThrow()
      expect(() => encrypt("x".repeat(n), ck)).toThrow()
    }
    expect(coax.encrypt("x".repeat(65535), ck).length).toBe(87472)
  })
  test("round trips with test keys, both ways, and a changed byte or another key refused", () => {
    const a = testKey(21), b = testKey(22), c = testKey(23)
    const ab = coax.conversationKey(a, getPublicKey(b))
    expect(Buffer.from(ab).equals(Buffer.from(coax.conversationKey(b, getPublicKey(a))))).toBe(true)
    expect(Buffer.from(ab).equals(Buffer.from(getConversationKey(a, getPublicKey(b))))).toBe(true)
    for (const text of ["a", "x".repeat(32), "x".repeat(33), "ünïcode \"q\" \\ \n 🦄", "y".repeat(65535)]) {
      expect(decrypt(coax.encrypt(text, ab), ab)).toBe(text)
      expect(coax.decrypt(encrypt(text, ab), ab)).toBe(text)
    }
    const p = coax.encrypt("the deck", ab)
    expect(coax.encrypt("the deck", ab)).not.toBe(p)   // a fresh nonce each time
    expect(() => coax.decrypt(flip(p, 80), ab)).toThrow("NIP-44 refused the payload: MAC")
    expect(() => decrypt(flip(p, 80), ab)).toThrow()
    expect(() => coax.decrypt(p, coax.conversationKey(c, getPublicKey(b)))).toThrow("NIP-44 refused the payload: MAC")
    expect(() => coax.conversationKey(a, "zz".repeat(32))).toThrow("not a public key")
    expect(() => coax.decrypt(p, new Uint8Array(31))).toThrow()
  })
  test("a deck sealed through Coax opens with nostr-effect, and one sealed by nostr-effect opens through Coax", async () => {
    const author = testKey(24), me = testKey(25), deck = parseDeck(md)
    useNostr(coax)
    const byCoax = sealDeck(deck, author, getPublicKey(me))
    expect(verifyEvent({ ...byCoax })).toBe(true)
    useNostr(null)
    expect(await openDeck(byCoax, localOpener(me))).toEqual({ deck, author: getPublicKey(author) })
    const byEffect = sealDeck(deck, author, getPublicKey(me))
    const calls: string[] = []
    useNostr({ ...coax, decrypt: (payload, key) => (calls.push("decrypt"), coax.decrypt(payload, key)),
      conversationKey: (s, peer) => (calls.push("key"), coax.conversationKey(s, peer)) })
    expect(await openDeck(byEffect, localOpener(me))).toEqual({ deck, author: getPublicKey(author) })
    expect(calls).toEqual(["key", "decrypt", "key", "decrypt", "key", "decrypt"])   // the wrap, the seal, the artifact
  })
  test("a wrap or seal whose ciphertext changed under a valid signature, or sealed to another key, is refused", async () => {
    const author = testKey(26), me = testKey(27), once = testKey(28), other = testKey(29)
    useNostr(coax)
    const opener = localOpener(me)
    const wrap = sealDeck(parseDeck(md), author, getPublicKey(me))
    const toMe = (inner: object) => finalizeEvent({ kind: 1059, created_at: wrap.created_at, tags: [["p", getPublicKey(me)]],
      content: coax.encrypt(JSON.stringify(inner), coax.conversationKey(once, getPublicKey(me))) }, once)
    // the wrap: its content changed by one character and signed again by its throwaway key
    const resigned = finalizeEvent({ kind: 1059, created_at: wrap.created_at, tags: wrap.tags, content: flip(wrap.content, 100) }, once)
    await expect(openDeck(resigned, opener)).rejects.toThrow("NIP-44 refused the payload: MAC")
    // the seal: its content changed and signed again by the author, inside a valid wrap
    const seal = JSON.parse(coax.decrypt(wrap.content, coax.conversationKey(me, wrap.pubkey))) as Event
    const badSeal = finalizeEvent({ kind: 13, created_at: seal.created_at, tags: [], content: flip(seal.content, 100) }, author)
    await expect(openDeck(toMe(badSeal), opener)).rejects.toThrow("NIP-44 refused the payload: MAC")
    // the seal's rumor encrypted to another key: a valid seal and wrap, but not for this conversation
    const rumor = coax.decrypt(seal.content, coax.conversationKey(me, getPublicKey(author)))
    const elsewhere = finalizeEvent({ kind: 13, created_at: seal.created_at, tags: [],
      content: coax.encrypt(rumor, coax.conversationKey(author, getPublicKey(other))) }, author)
    await expect(openDeck(toMe(elsewhere), opener)).rejects.toThrow("NIP-44 refused the payload: MAC")
    // untouched, every layer opens
    expect((await openDeck(toMe(seal), opener)).author).toBe(getPublicKey(author))
  })
})
