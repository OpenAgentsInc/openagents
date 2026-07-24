#!/usr/bin/env node
/**
 * Pack @openagentsinc/omega-effectd and print the immutable tarball digest.
 * Omega pins this digest. It must not need a relative monorepo path.
 */

import { createHash } from "node:crypto"
import { readFileSync } from "node:fs"
import { execFileSync } from "node:child_process"
import path from "node:path"
import { fileURLToPath } from "node:url"

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..")
const out = execFileSync("pnpm", ["pack"], { cwd: root, encoding: "utf8" })
const line = out
  .trim()
  .split("\n")
  .map((entry) => entry.trim())
  .filter(Boolean)
  .at(-1)
if (!line) {
  console.error("pnpm pack produced no tarball path")
  process.exit(1)
}
const tarball = path.isAbsolute(line) ? line : path.join(root, line)
const digest = createHash("sha256").update(readFileSync(tarball)).digest("hex")
console.log(
  JSON.stringify(
    {
      package: "@openagentsinc/omega-effectd",
      version: "0.1.0",
      tarball,
      sha256: digest,
    },
    null,
    2,
  ),
)
