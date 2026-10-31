// Generates coax/tests/nostr/host_grant_vectors.coax for
// coax/lib/nostr/host/grant.coax: NIP-HOST grants sealed as 3188 private
// artifacts by an independent implementation, wrapped in
// openagents.host.v1.Access protobuf wires encoded here by hand.
// Firm reason: 3 (runs the reference implementation we test against:
// nostr-effect's NIP-44 v2 and signing, and the unlock sealer's JCS).
// Tooling only; never shipped. Keys are the scalars 1 (host), 2 (owner),
// 3 (device) and 7 (another device): TEST ONLY. Nonces are random, so each
// run writes new envelope bytes.
//
//   bun coax/tests/nostr/host_grant_fixtures.ts > coax/tests/nostr/host_grant_vectors.coax
import { getConversationKey, encrypt } from "../../web/project/node_modules/nostr-effect/src/wrappers/nip44.ts"
import { finalizeEvent, getEventHash, getPublicKey } from "../../web/project/node_modules/nostr-effect/src/wrappers/pure.ts"
import { sha256 } from "../../web/project/node_modules/@noble/hashes/sha256.js"
import { bytesToHex } from "../../web/project/node_modules/@noble/hashes/utils.js"
import { jcs } from "../../web/unlock/lib.ts"

const key = (n: number) => { const k = new Uint8Array(32); k[31] = n; return k }
const HOST = key(1), DEVICE = key(3), OTHER = key(7)
const HOST_HEX = getPublicKey(HOST), OWNER_HEX = getPublicKey(key(2)), DEVICE_HEX = getPublicKey(DEVICE), OTHER_HEX = getPublicKey(OTHER)
const MAILBOX = "1".repeat(64)
type Grant = Record<string, any>
// NIP-HOST's fixture grant (crates/coder-access/fixtures/nip-host.json before the reset).
const FIXTURE: Grant = { v: "openagents.host-grant.v1", requires: [], grant: MAILBOX, host: HOST_HEX, owner: OWNER_HEX, device: DEVICE_HEX,
  relay: "wss://relay.example.com/", rights: ["observe", "operate", "terminal", "review"], epoch: 0,
  origin: { kind: "invitation", id: "2".repeat(64), issuer: HOST_HEX }, issued_at: 1790000000, expires_at: 1790086400 }
const variant = (change: Grant) => { const g = structuredClone(FIXTURE); for (const [k, v] of Object.entries(change)) v === undefined ? delete g[k] : (g[k] = v); return g }

// ---- protobuf by hand (proto3: absent zero scalars and empty strings)
const varint = (n: bigint | number) => { let v = BigInt(n); const out: number[] = []; do { let b = Number(v & 0x7fn); v >>= 7n; if (v) b |= 0x80; out.push(b) } while (v); return out }
const field = (no: number, wire: number) => varint((no << 3) | wire)
const bytesField = (no: number, b: Uint8Array | number[]) => [...field(no, 2), ...varint(b.length), ...b]
const str = (no: number, s?: string) => (s ? bytesField(no, [...new TextEncoder().encode(s)]) : [])
const u64 = (no: number, n?: number) => (n ? [...field(no, 0), ...varint(n)] : [])
function grantWire(g: Grant): number[] {
  const o = g.origin ? [...str(1, g.origin.kind), ...str(2, g.origin.id), ...str(3, g.origin.issuer)] : null
  return [...str(1, g.grant), ...str(2, g.host), ...str(3, g.owner), ...str(4, g.device), ...str(5, g.relay),
    ...(g.rights ?? []).flatMap((r: string) => str(6, r)), ...u64(7, g.epoch), ...(o ? bytesField(8, o) : []), ...u64(9, g.issued_at), ...u64(10, g.expires_at)]
}
const accessWire = (g: number[] | null, json: string | null) =>
  [...(g ? bytesField(1, g) : []), ...(json !== null ? bytesField(2, str(1, json)) : [])]

// ---- the envelope: a 3188 from `signer` to `to`, inline grant body
function seal(body: Grant, signer = HOST, to = DEVICE_HEX, mailbox = MAILBOX, edit?: (envelope: any) => void, tags?: string[][]) {
  const bytes = new TextEncoder().encode(jcs(body))
  const envelope: any = { v: "openagents.artifact-envelope.v1", requires: [],
    artifact: { digest: "sha256:" + bytesToHex(sha256(bytes)), size: bytes.length, media_type: "application/json", schema: "openagents.host-grant.v1" },
    inline: body, issued_at: body.issued_at ?? 1790000000, retain_until: (body.expires_at ?? 1790086400) + 60 }
  edit?.(envelope)
  return JSON.stringify(finalizeEvent({ kind: 3188, created_at: 1790000000,
    tags: tags ?? [["p", to], ["h", mailbox], ["t", "oa:artifact:v1"]],
    content: encrypt(JSON.stringify(envelope), getConversationKey(signer, to)) }, signer))
}
const flipSig = (json: string) => { const e = JSON.parse(json); e.sig = (e.sig[0] === "0" ? "1" : "0") + e.sig.slice(1); return JSON.stringify(e) }

const lit = (b: number[]) => `[${b.length}]u8.[${b.join(",")}]`
const q = (s: string) => JSON.stringify(s)

// Body-level refusals hg_body must name without any crypto.
const BAD: [Grant | string, string][] = [
  [variant({ v: "openagents.host-grant.v2" }), "HG_GRANT"], [variant({ extra: "x" }), "HG_GRANT"],
  [variant({ relay: undefined }), "HG_GRANT"], [variant({ requires: ["x"] }), "HG_GRANT"],
  [variant({ requires: ["openagents.workspace-scope.v1"] }), "HG_SCOPED"], [variant({ scope: { account: "a" } }), "HG_SCOPED"],
  [variant({ rights: ["operate", "observe"] }), "HG_GRANT"], [variant({ rights: ["observe", "observe"] }), "HG_GRANT"],
  [variant({ rights: [] }), "HG_GRANT"], [variant({ rights: ["observe", "admin"] }), "HG_GRANT"],
  [variant({ owner: HOST_HEX }), "HG_GRANT"], [variant({ expires_at: 1790000000 + 2592001 }), "HG_GRANT"],
  [variant({ expires_at: 1790000000 }), "HG_GRANT"], [variant({ epoch: -1 }), "HG_GRANT"], [variant({ epoch: "0" }), "HG_GRANT"],
  [variant({ origin: { kind: "gift", id: "2".repeat(64), issuer: HOST_HEX } }), "HG_GRANT"],
  [variant({ origin: { kind: "invitation", id: "2".repeat(64) } }), "HG_GRANT"],
  [variant({ host: HOST_HEX.toUpperCase() }), "HG_GRANT"], [variant({ grant: "3".repeat(64) }), "HG_ISSUER"],
  [variant({ device: OTHER_HEX }), "HG_ISSUER"], ["[]", "HG_GRANT"],
  [variant({ v: undefined }), "HG_GRANT"], [variant({ zz: "openagents.host-grant.v1" }), "HG_GRANT"],
  [variant({ origin: { id: "2".repeat(64), issuer: HOST_HEX } }), "HG_GRANT"],
  [variant({ origin: { ...FIXTURE.origin, x: "invitation" } }), "HG_GRANT"],
  [variant({ owner: DEVICE_HEX }), "HG_GRANT"], [variant({ relay: "wss://" + "r".repeat(2000) }), "HG_GRANT"],
  [variant({ relay: "" }), "HG_GRANT"], [variant({ epoch: 9007199254740992 }), "HG_GRANT"],
  // The longest lifetime is admitted, so this one fails only on its changed expiry.
  [variant({ expires_at: 1790000000 + 2592000 }), "HG_PROJECTION"],
]
// Projections that differ from the fixture in one field each.
const PROJECTIONS: Grant[] = [
  variant({ grant: "3".repeat(64) }), variant({ host: OWNER_HEX }), variant({ owner: DEVICE_HEX }), variant({ device: HOST_HEX }),
  variant({ relay: "wss://relay.example.com" }), variant({ rights: ["observe", "terminal", "operate", "review"] }),
  variant({ rights: ["observe", "operate", "terminal"] }), variant({ rights: ["observe", "operate", "terminal", "review", "access_read"] }),
  variant({ epoch: 1 }), variant({ origin: { ...FIXTURE.origin, kind: "approval" } }), variant({ origin: { ...FIXTURE.origin, id: "4".repeat(64) } }),
  variant({ origin: { ...FIXTURE.origin, issuer: OWNER_HEX } }), variant({ issued_at: 1790000001 }), variant({ expires_at: 1790086401 }),
  variant({ origin: undefined }), variant({ relay: undefined }),
]

if (import.meta.main) {
  const good = seal(FIXTURE)
  const other = variant({ relay: "wss://other.example.com/" })
  const badS = (() => { const e = JSON.parse(good); e.sig = e.sig.slice(0, 64) + "f".repeat(64); return JSON.stringify(e) })()
  // Unsigned shapes: the reader checks ids, not signatures, so these reach hg_open.
  const unsigned = (pubkey: string, kind: number) => { const e: any = { pubkey, created_at: 1790000000, kind,
    tags: [["p", DEVICE_HEX], ["h", MAILBOX], ["t", "oa:artifact:v1"]], content: "x" }; e.id = getEventHash(e); e.sig = "0".repeat(128); return JSON.stringify(e) }
  const ORIGINALS: Record<string, string> = {
    OTHER_ENVELOPE: seal(other), FOR_OTHER: seal(variant({ device: OTHER_HEX }), HOST, OTHER_HEX),
    WRONG_KEY_P: seal(FIXTURE, HOST, OTHER_HEX, MAILBOX, undefined, [["p", DEVICE_HEX], ["h", MAILBOX], ["t", "oa:artifact:v1"]]),
    NOT_HOST: seal(FIXTURE, key(2)), SCOPED: seal(variant({ requires: ["openagents.workspace-scope.v1"], scope: { account: "a" } })),
    DIGEST: seal(FIXTURE, HOST, DEVICE_HEX, MAILBOX, (e) => { e.artifact.digest = "sha256:" + "0".repeat(64) }),
    SCHEMA: seal(FIXTURE, HOST, DEVICE_HEX, MAILBOX, (e) => { e.artifact.schema = "openagents.host-access.v1" }),
    MAILBOX: seal(FIXTURE, HOST, DEVICE_HEX, "5".repeat(64)),
    KIND1: unsigned(HOST_HEX, 1), BAD_PEER: unsigned("f".repeat(64), 3188),
  }
  const lines = [
    "// Generated by host_grant_fixtures.ts (nostr-effect 0.0.13 and coax/web/unlock/lib.ts's JCS). Do not edit.",
    "// Keys: host k=1, owner k=2, device k=3, another device k=7. TEST ONLY.",
    `HG_DEVICE_SECRET :: [32]u8.[${[...DEVICE].join(",")}];`,
    `HG_DEVICE_X :: [32]u8.[${[...Buffer.from(DEVICE_HEX, "hex")].join(",")}];`,
    `HG_OTHER_SECRET :: [32]u8.[${[...OTHER].join(",")}];`,
    `HG_OTHER_X :: [32]u8.[${[...Buffer.from(OTHER_HEX, "hex")].join(",")}];`,
    `HG_DEVICE_HEX :: ${q(DEVICE_HEX)};`, `HG_HOST_HEX :: ${q(HOST_HEX)};`, `HG_MAILBOX_HEX :: ${q(MAILBOX)};`,
    "HG_EMPTY_WIRE :: [1]u8.[0];",
    `HG_FIXTURE_GRANT :: ${q(jcs(FIXTURE))};`,
    `HG_FIXTURE_PROJECTION :: ${lit(grantWire(FIXTURE))};`,
    `HG_BAD_GRANTS :: [${BAD.length}]string.[${BAD.map(([g]) => q(typeof g === "string" ? g : JSON.stringify(g))).join(",")}];`,
    `HG_BAD_GRANT_CODES :: [${BAD.length}]u32.[${BAD.map(([, c]) => c).join(",")}];`,
    `HG_PROJECTION_COUNT :: ${PROJECTIONS.length};`,
    `HG_PROJECTIONS :: ${lit(PROJECTIONS.flatMap(grantWire))};`,
    `HG_PROJECTION_ENDS :: [${PROJECTIONS.length}]int.[${PROJECTIONS.map((_, i) => PROJECTIONS.slice(0, i + 1).flatMap(grantWire).length).join(",")}];`,
    // End to end: Access wires for host_grant_checks.coax.
    `HG_ACCESS_OK :: ${lit(accessWire(grantWire(FIXTURE), good))};`,
    `HG_ORIGINAL_OK :: ${q(good)};`,
    `HG_ACCESS_FORGED :: ${lit(accessWire(grantWire(FIXTURE), flipSig(good)))};`,
    `HG_ACCESS_SUBSTITUTED :: ${lit(accessWire(grantWire(other), good))};`,
    ...Object.entries(ORIGINALS).flatMap(([name, json]) => [`HG_ORIGINAL_${name} :: ${q(json)};`,
      `HG_ACCESS_${name} :: ${lit(accessWire(grantWire(name === "FOR_OTHER" ? variant({ device: OTHER_HEX }) : FIXTURE), json))};`]),
    `HG_CK_HOST :: [32]u8.[${[...getConversationKey(DEVICE, HOST_HEX)].join(",")}];`,
    `HG_CK_OWNER :: [32]u8.[${[...getConversationKey(DEVICE, OWNER_HEX)].join(",")}];`,
    `HG_GRANT_EPOCH1 :: ${q(jcs(variant({ epoch: 1 })))};`,
    `HG_ACCESS_NO_GRANT :: ${lit(accessWire(null, good))};`,
    `HG_ACCESS_NO_AUTH :: ${lit(accessWire(grantWire(FIXTURE), null))};`,
    `HG_ACCESS_EMPTY_JSON :: ${lit([...bytesField(1, grantWire(FIXTURE)), ...bytesField(2, [])])};`,
    `HG_ACCESS_BAD_EVENT :: ${lit(accessWire(grantWire(FIXTURE), "{}"))};`,
    `HG_ACCESS_BAD_S :: ${lit(accessWire(grantWire(FIXTURE), badS))};`,
  ]
  console.log(lines.join("\n"))
}
