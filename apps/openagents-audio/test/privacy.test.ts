import { expect, test } from "bun:test"

test("SQL schema stores refs and metadata, never media blobs or credentials", async () => {
  const sql = await Bun.file(new URL("../../../packages/khala-sync-server/migrations/0064_audio_retention.sql", import.meta.url)).text()
  expect(sql).not.toMatch(/\b(bytea|blob|credential|signed_url|public_url)\b/i)
  expect(sql).toContain("object_ref text NOT NULL UNIQUE")
  expect(sql).toContain("audio_access_receipts")
})

test("retention source has no Sync, gateway, renderer, analytics, or crash-bundle boundary", async () => {
  const files = ["model.ts", "crypto.ts", "storage.ts", "service.ts"]
  const source = (await Promise.all(files.map((file) => Bun.file(new URL(`../src/${file}`, import.meta.url)).text()))).join("\n")
  expect(source).not.toMatch(/khala.?sync|runtime.?gateway|renderer|analytics|crash|support.?bundle/i)
  expect(source).not.toMatch(/signed.?url|makePublic|allUsers/i)
})

test("bucket policy refuses indefinite media backups", async () => {
  const policy = await Bun.file(new URL("../deploy/gcs-bucket-policy.json", import.meta.url)).json() as { versioning: { enabled: boolean }; lifecycle: { rule: unknown[] } }
  expect(policy.versioning.enabled).toBe(false)
  expect(policy.lifecycle.rule.length).toBeGreaterThanOrEqual(2)
})
