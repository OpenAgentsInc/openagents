// NIP-98 authorization headers signed by an independent implementation, for
// nip98.coax's checks (nip98_vectors.coax) and the C-vs-Coax differential
// (nip98_corpus.tsv, read by nip98_diff.c).
// Firm reason: 3 (runs nostr-effect, the reference implementation we test against).
// Test keys only (scalar 1 and 2). Deterministic: BIP-340 aux is zero.
//   bun coax/web/api/nip98_fixtures.ts
import { getEventHash, getPublicKey } from "../project/node_modules/nostr-effect/src/wrappers/pure.ts"
import { schnorr } from "../project/node_modules/@noble/curves/secp256k1.js"
import { sha256 } from "../project/node_modules/@noble/hashes/sha256.js"
import { bytesToHex } from "../project/node_modules/@noble/hashes/utils.js"
import { writeFileSync } from "node:fs"

const key = (n: number) => { const k = new Uint8Array(32); k[31] = n; return k }
const SECRET = key(1), PUB = getPublicKey(SECRET)
const URL_ = "https://openagents.com/api/playtest/feedback", NOW = 1790000000
const BODY = '{"a":1}', BODY_HASH = bytesToHex(sha256(new TextEncoder().encode(BODY)))
type Ev = { pubkey: string; created_at: number; kind: number; tags: string[][]; content: string; id?: string; sig?: string }
const sign = (e: Ev, secret = SECRET): Ev => {
  const out = { ...e, pubkey: e.pubkey ?? getPublicKey(secret) }
  out.id = getEventHash(out as any)
  out.sig = bytesToHex(schnorr.sign(out.id, secret, new Uint8Array(32)))
  return out
}
const ev = (change: Partial<Ev> = {}): Ev => ({ pubkey: PUB, created_at: NOW, kind: 27235,
  tags: [["u", URL_], ["method", "POST"], ["payload", BODY_HASH]], content: "", ...change })
const text = (e: Ev) => JSON.stringify({ id: e.id, pubkey: e.pubkey, created_at: e.created_at, kind: e.kind, tags: e.tags, content: e.content, sig: e.sig })
const b64 = (s: string, url = false) => { const b = Buffer.from(s, "utf8").toString("base64"); return url ? b.replaceAll("+", "-").replaceAll("/", "_").replace(/=+$/, "") : b }
const auth = (json: string, url = false) => "Nostr " + b64(json, url)

type Case = { name: string; auth: string; method?: string; url?: string; body?: string; now?: number; want: string }
const good = sign(ev())
const cases: Case[] = [
  { name: "OK", auth: auth(text(good)), want: "N98_OK" },
  { name: "OK_LOWER_METHOD", auth: auth(text(good)), method: "post", want: "N98_OK" },
  { name: "OK_URL_ALPHABET", auth: auth(text(good), true), want: "N98_OK" },
  { name: "OK_NO_PAYLOAD", auth: auth(text(sign(ev({ tags: [["u", URL_], ["method", "POST"]] })))), want: "N98_OK" },
  { name: "OK_EDGE_TIME", auth: auth(text(sign(ev({ created_at: NOW - 60 })))), want: "N98_OK" },
  { name: "OK_PRETTY", auth: auth(JSON.stringify(JSON.parse(text(good)), null, 2)), want: "N98_OK" },
  { name: "OK_PLUS", auth: auth(text(good).replace(`"created_at":${NOW}`, `"created_at": +${NOW}`)), want: "N98_OK" },
  { name: "OK_LAST_U", auth: auth(text(sign(ev({ tags: [["u", "x"], ["u", URL_], ["method", "POST"]] })))), want: "N98_OK" },
  { name: "OK_SIXTEEN_TAGS", auth: auth(text(sign(ev({ tags: [["u", URL_], ["method", "POST"], ...Array.from({ length: 14 }, (_, i) => ["x", `${i}`])] })))), want: "N98_OK" },
  { name: "OK_ESCAPES", auth: auth(text(sign(ev({ content: "q\"b\\s\nn\tt é 😀 /" })))), want: "N98_OK" },
  { name: "OK_LONG_FIELD", auth: auth(text(sign(ev({ tags: [["u", URL_], ["method", "POST"], ["x", "a".repeat(1023)]] })))), want: "N98_OK" },
  { name: "OK_LONG_CONTENT", auth: auth(text(sign(ev({ content: "c".repeat(8191) })))), want: "N98_OK" },
  { name: "WRONG_URL", auth: auth(text(good)), url: URL_ + "/", want: "N98_URL" },
  { name: "WRONG_METHOD", auth: auth(text(good)), method: "GET", want: "N98_METHOD" },
  { name: "WRONG_BODY", auth: auth(text(good)), body: '{"a":2}', want: "N98_PAYLOAD" },
  { name: "LATE", auth: auth(text(sign(ev({ created_at: NOW - 61 })))), want: "N98_TIME" },
  { name: "EARLY", auth: auth(text(sign(ev({ created_at: NOW + 61 })))), want: "N98_TIME" },
  { name: "KIND1", auth: auth(text(sign(ev({ kind: 1 })))), want: "N98_KIND" },
  { name: "LONG_METHOD", auth: auth(text(sign(ev({ tags: [["u", URL_], ["method", "POSTPOSTPOSTPOSTPOST"]] })))), want: "N98_METHOD" },
  { name: "SEVENTEEN_TAGS", auth: auth(text(sign(ev({ tags: [["u", URL_], ["method", "POST"], ...Array.from({ length: 15 }, (_, i) => ["x", `${i}`])] })))), want: "N98_BAD_TAG" },
  { name: "FIVE_FIELDS", auth: auth(text(sign(ev({ tags: [["u", URL_, "a", "b", "c"]] })))), want: "N98_BAD_TAG_FIELD" },
  { name: "FIELD_1024", auth: auth(text(sign(ev({ tags: [["x", "a".repeat(1024)]] })))), want: "N98_BAD_TAG_FIELD" },
  { name: "CONTENT_8192", auth: auth(text(sign(ev({ content: "c".repeat(8192) })))), want: "N98_BAD_CONTENT" },
  { name: "FORGED", auth: auth(text({ ...good, sig: (good.sig![0] === "0" ? "1" : "0") + good.sig!.slice(1) })), want: "N98_SIGNATURE" },
  { name: "BAD_S", auth: auth(text({ ...good, sig: good.sig!.slice(0, 64) + "f".repeat(64) })), want: "N98_SIGNATURE" },
  { name: "NOT_POINT", auth: auth(text((() => { const e = { ...ev(), pubkey: "f".repeat(64) }; e.id = getEventHash(e as any); e.sig = good.sig; return e })())), want: "N98_POINT" },
  { name: "ID", auth: auth(text({ ...good, id: "0".repeat(64) })), want: "N98_ID" },
  { name: "UPPER_ID", auth: auth(text({ ...good, id: good.id!.toUpperCase() })), want: "N98_BAD_ID" },
  { name: "UNKNOWN", auth: auth(text(good).replace("{", '{"extra":1,')), want: "N98_UNKNOWN" },
  { name: "UNKNOWN_STRING", auth: auth(text(good).replace("{", '{"extra":"x",')), want: "N98_UNKNOWN" },
  { name: "MISSING", auth: auth(text(good).replace(',"content":""', "")), want: "N98_MISSING" },
  { name: "TRAILING", auth: auth(text(good) + " x"), want: "N98_TRAILING" },
  { name: "DECIMAL", auth: auth(text(good).replace(`"created_at":${NOW}`, `"created_at":${NOW}.0`)), want: "N98_BAD_CREATED" },
  { name: "BIG_KIND", auth: auth(text(good).replace('"kind":27235', '"kind":65536')), want: "N98_BAD_KIND" },
  { name: "SATURATED", auth: auth(text(good).replace(`"created_at":${NOW}`, '"created_at":99999999999999999999')), want: "N98_ID" },
  { name: "NUL_CONTENT", auth: auth(text(good).replace('"content":""', '"content":"a\\u0000b"')), want: "N98_ID" },
  { name: "DUP_ID", auth: auth(text(good).replace("{", `{"id":"${"0".repeat(64)}",`)), want: "N98_OK" },
  { name: "LONE_SURROGATE", auth: auth(text(good).replace('"content":""', '"content":"\\udc00"')), want: "N98_BAD_CONTENT" },
  { name: "NOT_NOSTR", auth: "Bearer x", want: "N98_NO_AUTH" },
  { name: "EMPTY", auth: "Nostr ", want: "N98_BASE64" },
  { name: "BAD_BASE64", auth: "Nostr ab*c", want: "N98_BASE64" },
  { name: "NOT_OBJECT", auth: auth("[]"), want: "N98_NOT_OBJECT" },
  { name: "EMPTY_OBJECT", auth: auth("{}"), want: "N98_MISSING" },
]
const lit = (s: string) => JSON.stringify(s)
const coax = [
  "// Generated by nip98_fixtures.ts (nostr-effect). Do not edit. Test keys only.",
  `N98_T_URL :: ${lit(URL_)}; N98_T_BODY :: ${lit(BODY)}; N98_T_NOW :: ${NOW};`,
  `N98_T_COUNT :: ${cases.length};`,
  `N98_T_NAME :: [${cases.length}]string.[${cases.map((c) => lit(c.name)).join(",")}];`,
  `N98_T_AUTH :: [${cases.length}]string.[${cases.map((c) => lit(c.auth)).join(",")}];`,
  `N98_T_METHOD :: [${cases.length}]string.[${cases.map((c) => lit(c.method ?? "POST")).join(",")}];`,
  `N98_T_URLS :: [${cases.length}]string.[${cases.map((c) => lit(c.url ?? URL_)).join(",")}];`,
  `N98_T_BODIES :: [${cases.length}]string.[${cases.map((c) => lit(c.body ?? BODY)).join(",")}];`,
  `N98_T_WANT :: [${cases.length}]int.[${cases.map((c) => c.want).join(",")}];`,
  `N98_T_PUB :: ${lit(PUB)}; N98_T_ID :: ${lit(good.id!)};`,
]
const dir = new URL(".", import.meta.url).pathname
writeFileSync(dir + "nip98_vectors.coax", coax.join("\n") + "\n")
const hex = (s: string) => Buffer.from(s, "utf8").toString("hex")
writeFileSync(dir + "nip98_corpus.tsv", cases.map((c) => [c.name, c.method ?? "POST", c.url ?? URL_, c.now ?? NOW, hex(c.body ?? BODY), c.auth].join("\t")).join("\n") + "\n")
console.log(`${cases.length} cases`)
